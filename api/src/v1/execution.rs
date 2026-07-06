//! Rule execution API endpoints

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use engine::engine::{EngineRegistry, EngineSelector};
use engine::environment::EngineBuilder;
use engine::rules::{Rule, RuleSet};

use crate::metrics::{ExecutionCounters, MetricsSnapshot, MetricsTx};
use crate::v1::sessions::SessionManager;

/// Request for rule execution
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct ExecuteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub facts: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_filter: Option<RuleFilter>,
    /// Engine name override (ENGINE-07/08). When omitted, resolves to
    /// `RRULES_DEFAULT_ENGINE` (default `"rete"`). Unknown names return
    /// HTTP 400. Validation uses `EngineRegistry::available_engines()` —
    /// NOT `EngineSelector::select_engine` (which takes `&mut self`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Phase 28 (D-02 / EXEC-04): opt-in inference trace. Off by default
    /// for performance (SC-3 latency neutrality). When absent/false the
    /// evaluator skips all trace collection and `inference_trace` is
    /// `None`. `#[serde(default)]` keeps existing clients/fixtures parsing
    /// (absent deserializes to `None`).
    #[serde(default)]
    pub trace: Option<bool>,
}

/// Filter for selecting which rules to execute
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct RuleFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from rule execution
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ExecuteResponse {
    pub execution_id: String,
    pub rules_fired: usize,
    #[serde(default)]
    pub rules_not_fired: Vec<String>,
    #[serde(default)]
    pub facts_after: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_trace: Option<InferenceTrace>,
    /// Engine generation counter (Phase 24 / Open Q6). Always `0` in
    /// Phase 24 — Phase 25 wires the real per-engine generation counter.
    /// `#[serde(default)]` keeps existing clients (and the Phase 23 test
    /// fixtures) parsing correctly: the field serializes as `0` when not
    /// explicitly set (Pitfall 8). The `response_includes_engine_generation`
    /// Wave 0 test asserts the field is a u64 — it goes GREEN once this
    /// field exists, which is expected and documented in Plan 24-02.
    #[serde(default)]
    pub engine_generation: u64,
    /// Phase 27 (D-04): server-generated UUID v4 primary key for the
    /// stateful session. Absent (`None`) on the stateless one-shot path
    /// (Phase 23 — unchanged, D-05); present on the sessionful path so the
    /// caller learns the canonical id on first creation. The client
    /// `session_id` is an idempotency alias; this field is the server truth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Atomic ruleset + generation container (Phase 25 / D-02 / L-01).
///
/// Wraps the shared `RuleSet` with a `generation: u64` counter so the two are
/// read together under a single `RwLock` read guard (coherent snapshot — a
/// separate `AtomicU64` would race, see CONTEXT D-02). Reload (Plan 25-02's
/// `reload_and_swap`) replaces `ruleset` and increments `generation` under the
/// write lock; `execute_single` reads both atomically under the read lock and
/// stamps `generation` into `ExecuteResponse.engine_generation` (L-04).
///
/// Lives inside the EXISTING `Arc<RwLock<SharedRuleSet>>` built once at
/// startup (`main.rs`) — no `ArcSwap` (L-01). `Clone` is cheap (`RuleSet`'s
/// `rules: Vec<Rule>` is the only non-Copy field; cloning the shared payload
/// is never done on the hot read path — `execute_single` clones only the
/// ruleset via `load_ruleset` and copies the `u64` generation out, then
/// releases the guard before `web::block` per L-05).
#[derive(Debug, Clone)]
pub struct SharedRuleSet {
    pub ruleset: RuleSet,
    pub generation: u64,
}

impl SharedRuleSet {
    /// Construct a new shared payload with generation 0 (pre-reload baseline).
    pub fn new(ruleset: RuleSet) -> Self {
        Self {
            ruleset,
            generation: 0,
        }
    }
}

/// Build a shared read-only `RuleSet` from a rules directory (R1 / D-05 / D-03a).
///
/// Reads each `*.rules` and `*.drl` file in `rules_dir`, parses the JSON
/// content via `serde_json::from_str` (all rule AST types — `Rule`,
/// `Condition`, `Action`, `Expression`, `ComparisonOp` — derive
/// `Serialize, Deserialize`), and collects the rules into a `RuleSet`.
/// Each file may contain either a single `Rule` JSON object or a JSON array
/// of `Rule` objects.
///
/// This is the single source of truth for both startup (`main.rs`) and
/// reload (`reload_and_swap` in this module) — D-05: the swapped `RuleSet`
/// matches what a restart would load.
///
/// **D-03a (warn + continue, never crash):** on a missing directory, failed
/// `read_dir`, unreadable file, unparseable content, or a rule that fails
/// `RuleSet::add_rule` validation, this function logs a `tracing::warn!` and
/// skips the offending file/rule. One bad file does not crash startup or
/// reload. If the resulting ruleset is empty, a warning is logged.
pub fn build_ruleset_from_dir(rules_dir: &str) -> RuleSet {
    let path = std::path::Path::new(rules_dir);
    if !path.exists() {
        tracing::warn!(
            rules_dir = %rules_dir,
            "rules directory not found, starting with no rules"
        );
        return RuleSet::new();
    }
    let mut ruleset = RuleSet::new();
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                rules_dir = %rules_dir,
                error = %e,
                "failed to read rules dir, starting with no rules"
            );
            return RuleSet::new();
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.ends_with(".rules") || name.ends_with(".drl")) {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(file = %name, error = %e, "skipping unreadable rule file");
                continue;
            }
        };
        // Try single Rule first, then Vec<Rule> (supports both file shapes).
        let rules: Vec<Rule> = match serde_json::from_str::<Rule>(&content) {
            Ok(r) => vec![r],
            Err(_) => match serde_json::from_str::<Vec<Rule>>(&content) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        file = %name,
                        error = %e,
                        "skipping unparseable rule file"
                    );
                    continue;
                }
            },
        };
        for rule in rules {
            if let Err(e) = ruleset.add_rule(rule) {
                tracing::warn!(error = %e, "skipping rule that failed validation");
            }
        }
    }
    if ruleset.rules.is_empty() {
        tracing::warn!(
            rules_dir = %rules_dir,
            "no parseable rule files found, starting with no rules"
        );
    }
    ruleset
}

/// Complete inference trace with full detail
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct InferenceTrace {
    #[serde(default)]
    pub entries: Vec<InferenceTraceEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TraceSummary>,
}

/// Single trace entry for debugging
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct InferenceTraceEntry {
    pub timestamp: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Summary statistics for inference trace
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct TraceSummary {
    pub total_duration_ms: f64,
    pub facts_matched: usize,
}

/// Phase 28 (D-03 / FRONTEND-04): response body for `GET /v1/engines`. The
/// `engines` array is sorted ascending so the frontend dropdown is stable
/// across requests (28-RESEARCH Open Q5 — `available_engines()` iterates a
/// HashMap and is not order-guaranteed). `default` mirrors the exact
/// default-resolution expression from `execute_single`:
/// `RRULES_DEFAULT_ENGINE` env > `selector.current_engine()`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EnginesResponse {
    /// Sorted ascending list of registered engine names (from
    /// `EngineRegistry::available_engines()`).
    pub engines: Vec<String>,
    /// The resolved default engine name.
    pub default: String,
}

/// Shared engine state for hot reload functionality
pub struct EngineState {
    pub engine: Arc<std::sync::RwLock<engine::environment::Engine>>,
    pub rule_files: Arc<std::sync::RwLock<HashMap<String, String>>>,
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineState {
    pub fn new() -> Self {
        let engine = EngineBuilder::new().build();
        EngineState {
            engine: Arc::new(std::sync::RwLock::new(engine)),
            rule_files: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Reload rules from a directory (async-friendly)
    pub async fn reload_from_directory(&self, directory: &str) -> Result<usize, String> {
        let path = Path::new(directory);

        if !path.exists() {
            return Err(format!("Directory not found: {}", directory));
        }

        let mut rule_count = 0;
        let mut files_updated = HashMap::new();

        for entry in fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if file_name_str.ends_with(".rules") || file_name_str.ends_with(".drl") {
                let file_path = entry.path();
                let content = fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

                files_updated.insert(file_path.to_string_lossy().to_string(), content);
                rule_count += 1;
            }
        }

        {
            let mut rule_files = self.rule_files.write()
                .map_err(|e| format!("Failed to acquire lock: {}", e))?;
            *rule_files = files_updated;
        }

        Ok(rule_count)
    }

    /// Reload engine with new rules (full recompilation)
    pub async fn reload_engine(&self, directory: &str) -> Result<ReloadResult, String> {
        let start_time = Utc::now();

        let path = Path::new(directory);
        if !path.exists() {
            return Err(format!("Directory not found: {}", directory));
        }

        let mut rules_content = Vec::new();
        let mut rule_files = HashMap::new();

        for entry in fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if file_name_str.ends_with(".rules") || file_name_str.ends_with(".drl") {
                let file_path = entry.path();
                let content = fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

                rules_content.push(content.clone());
                rule_files.insert(file_path.to_string_lossy().to_string(), content);
            }
        }

        {
            let mut files = self.rule_files.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            *files = rule_files;
        }

        {
            let _engine = self.engine.write()
                .map_err(|e| format!("Failed to acquire engine lock: {}", e))?;
        }

        Ok(ReloadResult {
            reload_id: Uuid::new_v4().to_string(),
            rule_count: rules_content.len(),
            start_time,
            end_time: Utc::now(),
            success: true,
        })
    }
}

/// Result of a reload operation
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ReloadResult {
    pub reload_id: String,
    pub rule_count: usize,
    #[serde(rename = "started_at")]
    pub start_time: chrono::DateTime<Utc>,
    #[serde(rename = "completed_at")]
    pub end_time: chrono::DateTime<Utc>,
    pub success: bool,
}

/// Atomically swap the shared `SharedRuleSet` with a freshly-built one and
/// bump the generation counter (D-04 / D-02 / L-01 / Pitfall 4).
///
/// This is the ONE shared reload helper called by BOTH the sync and async
/// reload paths in Plan 25-03, so they cannot diverge. The new `RuleSet` is
/// built **off-to-side** via `build_ruleset_from_dir` (R1 — real JSON parser)
/// BEFORE the write lock is acquired, so no file I/O or parsing happens
/// under the lock (Pitfall 4: lock scope). Under the write lock, the whole
/// `SharedRuleSet { ruleset, generation }` is replaced in a SINGLE
/// assignment (`*guard = ...`) with `generation.wrapping_add(1)` — never
/// in-place mutation of `ruleset` followed by a separate generation bump
/// (D-02: two writes would be non-atomic; swap the whole struct). The guard
/// is dropped immediately after the assignment.
///
/// Returns `Ok(ReloadResult)` on success or `Err(String)` if the write lock
/// is poisoned. Not yet wired to any handler in this plan (Plan 25-03 Task 1
/// rewrites `reload_rules` to call this).
pub async fn reload_and_swap(
    shared: &Arc<std::sync::RwLock<SharedRuleSet>>,
    rules_dir: &str,
) -> Result<ReloadResult, String> {
    let start_time = Utc::now();

    // Build the new RuleSet OFF-SIDE — no lock held during I/O + parse
    // (Pitfall 4: lock scope). This is R1/D-05: the same parser used at
    // startup, so the swapped ruleset matches what a restart would load.
    let new_ruleset = build_ruleset_from_dir(rules_dir);
    let rule_count = new_ruleset.rules.len();

    // Acquire the write lock ONLY for the single whole-value swap.
    let mut guard = shared
        .write()
        .map_err(|e| format!("shared ruleset lock poisoned: {}", e))?;
    let next_gen = guard.generation.wrapping_add(1);
    let reload_id = Uuid::new_v4().to_string();
    // Pitfall 4 / D-02: swap the WHOLE SharedRuleSet in one assignment —
    // never `guard.ruleset.rules.push(...)` in-place mutation.
    *guard = SharedRuleSet {
        ruleset: new_ruleset,
        generation: next_gen,
    };
    // Guard dropped here (implicit on scope exit) — lock released.

    Ok(ReloadResult {
        reload_id,
        rule_count,
        start_time,
        end_time: Utc::now(),
        success: true,
    })
}

/// Async job tracking for long-running operations
#[derive(Debug, Clone, utoipa::ToSchema)]
pub struct ReloadJob {
    pub job_id: String,
    pub result: Option<ReloadResult>,
    pub status: ReloadStatus,
    pub error: Option<String>,
}

impl serde::Serialize for ReloadJob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(5))?;
        map.serialize_entry("job_id", &self.job_id)?;
        map.serialize_entry("status", self.status.as_str())?;
        if let Some(ref result) = self.result {
            map.serialize_entry("result", result)?;
        }
        if let Some(ref error) = self.error {
            map.serialize_entry("error", error)?;
        }
        map.end()
    }
}

/// Status of a reload job
#[derive(Debug, Clone, PartialEq, utoipa::ToSchema)]
pub enum ReloadStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl ReloadStatus {
    fn as_str(&self) -> &'static str {
        match self {
            ReloadStatus::Pending => "pending",
            ReloadStatus::Running => "running",
            ReloadStatus::Completed => "completed",
            ReloadStatus::Failed => "failed",
        }
    }
}

/// Shared state for tracking reload jobs
pub type JobStore = Arc<std::sync::RwLock<HashMap<String, ReloadJob>>>;

/// Initialize job store
pub fn init_job_store() -> JobStore {
    Arc::new(std::sync::RwLock::new(HashMap::new()))
}

/// Request to reload rules
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct ReloadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_file: Option<String>,
    #[serde(default)]
    pub async_reload: bool,
}

/// Response from rule reload (sync)
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ReloadResponse {
    pub reload_id: String,
    pub rules_reloaded: usize,
    pub status: String,
}

/// Response from async job submission
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct JobResponse {
    pub job_id: String,
    pub status: String,
}

/// Request to execute multiple rule sets concurrently in one call
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct BatchExecuteRequest {
    /// Individual execution requests to run as a batch
    #[serde(default)]
    pub requests: Vec<ExecuteRequest>,
    /// Maximum number of requests to run concurrently (default 8, minimum 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrency: Option<usize>,
}

/// Result of a single item within a batch execution
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct BatchItemResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ExecuteResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response from batch rule execution
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct BatchExecuteResponse {
    /// Per-request results, in submission order
    pub results: Vec<BatchItemResult>,
    /// Total rules fired across all successful requests
    pub total_fired: usize,
    /// Number of requests that completed successfully
    pub succeeded: usize,
    /// Number of requests that failed (task panic / join error)
    pub failed: usize,
}

/// Core single-request execution logic (Plan 23-05 rewrite). Extracted so
/// batch execution can reuse it concurrently without duplicating the
/// engine/working-memory wiring.
///
/// Per-request isolation (Pitfall 1 / Pitfall 9): a fresh `Box<dyn
/// ExecutionEngine>` is constructed from the shared `EngineRegistry` for EACH
/// call and moved into `web::block`. No `&mut` engine crosses a request
/// boundary — the shared `EngineState.engine` write lock is NEVER taken in
/// this path. The shared `Arc<RwLock<RuleSet>>` is acquired read-only to load
/// the ruleset snapshot, then released before execution.
///
/// `facts_after` is serialized from post-execution working memory via
/// `engine.get_facts()` + the reverse `engine_value_to_json` mapping (D-08 /
/// Pitfall 11) — NEVER by echoing the request input JSON. `rules_not_fired`
/// is the shared ruleset's rule ids minus `EngineResult.fired_rule_ids`.
/// `inference_trace` remains `None` (L-04 — Phase 28 populates it).
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,
) -> Result<ExecuteResponse, (actix_web::http::StatusCode, String)> {
    // Phase 27 (D-05): branch on session_id — sessionful path is additive,
    // stateless path unchanged. The sessionful path delegates to
    // `execute_sessionful`; the stateless path continues below.
    if req.session_id.is_some() {
        return execute_sessionful(
            req,
            registry,
            ruleset,
            counters,
            metrics_tx,
            session_manager,
        )
        .await;
    }
    let execution_id = Uuid::new_v4().to_string();

    // 1. Resolve engine name: request override > RRULES_DEFAULT_ENGINE env >
    //    selector.current_engine() (set once at startup). The env var is the
    //    source of truth for per-request default resolution when the request
    //    omits the engine field (ENGINE-08).
    let engine_name = req.engine.clone().unwrap_or_else(|| {
        std::env::var("RRULES_DEFAULT_ENGINE")
            .unwrap_or_else(|_| selector.current_engine().to_string())
    });

    // 2. Validate the engine param against registered factories (ENGINE-07).
    //    Do NOT call selector.select_engine (it takes &mut self).
    let available = registry.available_engines();
    if !available.iter().any(|name| *name == engine_name.as_str()) {
        return Err((
            actix_web::http::StatusCode::BAD_REQUEST,
            format!("unknown engine: {}", engine_name),
        ));
    }

    // 3. Build a fresh per-request engine (Pitfall 1 / Pitfall 9: ReteEngine
    //    is not Clone; construct a new Box<dyn ExecutionEngine> per request).
    let mut engine = match registry.create_engine_by_name(&engine_name) {
        Some(Ok(engine)) => engine,
        Some(Err(e)) => {
            return Err((
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("engine creation failed: {}", e),
            ))
        }
        None => {
            return Err((
                actix_web::http::StatusCode::BAD_REQUEST,
                format!("unknown engine: {}", engine_name),
            ))
        }
    };

    // Phase 28 (D-02 / EXEC-04): resolve the trace opt-in BEFORE web::block
    // and enable it on the engine so the evaluator knows whether to emit
    // ordered TraceEvents. `trace` is `#[serde(default)] Option<bool>` —
    // absent deserializes to `None`, which `unwrap_or(false)` maps to the
    // SC-3 latency-neutral off path (no trace allocation).
    let collect_trace = req.trace.unwrap_or(false);
    engine.set_collect_trace(collect_trace);

    // 4. Load the shared ruleset (read-only guard) and collect rule ids for
    //    rules_not_fired computation. The guard is dropped before execution
    //    so no lock spans the CPU-bound engine.execute() call. Phase 25
    //    (D-02 / L-04 / L-05): the shared payload is `SharedRuleSet`, so we
    //    read the inner ruleset for `load_ruleset` + rule ids AND copy the
    //    generation counter (a `u64` — free) atomically under the same read
    //    lock, then release before `web::block` (per-request isolation
    //    preserved — the generation reflects the snapshot the engine loaded).
    let (all_rule_ids, generation): (Vec<String>, u64) = {
        let guard = ruleset.read().map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("ruleset lock poisoned: {}", e),
            )
        })?;
        engine.load_ruleset(&guard.ruleset).map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("load_ruleset failed: {}", e),
            )
        })?;
        let ids = guard.ruleset.rules.iter().map(|r| r.id.clone()).collect();
        let gen = guard.generation;
        (ids, gen)
    };

    // 5. Insert request facts (convert JSON → engine::Value → Fact). Only
    //    JSON object facts carry a field payload (matches the keystone
    //    `{scorecard: {score: 0}}` shape); non-object values are skipped.
    for (fact_type, fact_value) in &req.facts {
        if let serde_json::Value::Object(fields) = fact_value.clone() {
            let mut payload = HashMap::new();
            for (key, value) in fields {
                payload.insert(key, json_to_engine_value(value));
            }
            let fact = engine::fact::Fact::new(fact_type.clone(), payload);
            engine.insert_fact(fact).map_err(|e| {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("insert_fact failed: {}", e),
                )
            })?;
        }
    }

    // 6. Execute inside web::block (CPU-bound — Pitfall 1 / per-request
    //    isolation). The engine is moved into the closure so no shared &mut
    //    exists across requests. Return both the EngineResult and the
    //    post-execution facts so facts_after is serialized from real working
    //    memory (Pitfall 11) — NOT from the echoed request input.
    //
    //    Phase 24 (Plan 24-03): capture `Instant::now()` BEFORE web::block so
    //    the histogram observes the full CPU-bound execute duration (Don't
    //    Hand-Roll — Instant, not SystemTime). The counter increment + AtomicU64
    //    fetch_add + MetricsSnapshot broadcast happen AFTER web::block returns,
    //    in the async context (Pitfall 24-B / Open Q5 — NOT inside the closure).
    let start = Instant::now();
    let (engine_result, post_facts) = match web::block(move || {
        let result = engine.execute();
        let facts = engine.get_facts();
        (result, facts)
    })
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            return Err((
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("engine execution panicked: {}", e),
            ))
        }
    };

    // Phase 24 (Plan 24-03 / Deliverable 3): instrument the execute path.
    // Increment the three labeled Prometheus counters and the three AtomicU64
    // snapshot mirrors AFTER web::block returns (Pitfall 24-B / Open Q5),
    // then broadcast a populated MetricsSnapshot so the WS monitoring channel
    // goes live (SC-2). The broadcast send error is discarded (Pitfall 24-C)
    // — `broadcast::send` returns `Err(SendError)` when there are no
    // subscribers, which is the normal case when no WS client is connected.
    let elapsed = start.elapsed().as_secs_f64();
    counters
        .requests_total
        .with_label_values(&[&engine_name])
        .inc();
    counters
        .rules_fired_total
        .with_label_values(&[&engine_name])
        .inc_by(engine_result.rules_fired as u64);
    counters
        .duration_seconds
        .with_label_values(&[&engine_name])
        .observe(elapsed);
    counters.snapshot_requests.fetch_add(1, Ordering::Relaxed);
    counters
        .snapshot_rules_fired
        .fetch_add(engine_result.rules_fired as u64, Ordering::Relaxed);
    counters
        .snapshot_facts_inserted
        .fetch_add(req.facts.len() as u64, Ordering::Relaxed);
    let snapshot = MetricsSnapshot {
        engine_name: engine_name.clone(),
        rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
        facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
        // Phase 27 (D-08): populate active_sessions from SessionManager::len()
        // so the WS monitoring channel reflects the true live session count.
        active_sessions: session_manager.len(),
        memory_bytes: 0,
        last_updated: chrono::Utc::now().to_rfc3339(),
    };
    // Pitfall 24-C: discard the SendError — no panic when no WS subscribers.
    let _ = metrics_tx.send(snapshot);

    // 7. Serialize facts_after from post-execution working memory via the
    //    reverse engine_value_to_json mapping (D-08 / Pitfall 11). Group by
    //    fact_type_id; if multiple facts share a type, merge payloads into
    //    one object (matches the existing { fact_type: json_value } shape).
    let mut facts_after: HashMap<String, serde_json::Value> = HashMap::new();
    for fact in post_facts {
        let obj: serde_json::Map<String, serde_json::Value> = fact
            .payload
            .into_iter()
            .map(|(k, v)| (k, engine_value_to_json(v)))
            .collect();
        let fact_json = serde_json::Value::Object(obj);
        facts_after
            .entry(fact.fact_type_id)
            .and_modify(|existing| {
                if let serde_json::Value::Object(existing_map) = existing {
                    if let serde_json::Value::Object(new_map) = &fact_json {
                        for (k, v) in new_map {
                            existing_map.insert(k.clone(), v.clone());
                        }
                    }
                }
            })
            .or_insert(fact_json);
    }

    // 8. Compute rules_not_fired = shared ruleset rule ids - fired_rule_ids.
    let fired_set: std::collections::HashSet<String> = engine_result
        .fired_rule_ids
        .iter()
        .cloned()
        .collect();
    let rules_not_fired: Vec<String> = all_rule_ids
        .iter()
        .filter(|id| !fired_set.contains(*id))
        .cloned()
        .collect();

    // Phase 28 (D-01 / D-02): map the engine-side TraceData → api
    // InferenceTrace when `collect_trace` is true. The engine emits its own
    // TraceEvent type (engine cannot import api types — Critical Finding 1);
    // this is the cross-crate mapping boundary. `data: None` per D-01
    // discretion (lean payload — SC-3). When `collect_trace` is false,
    // `engine_result.trace` is `None` and `inference_trace` is `None` (the
    // SC-3 latency-neutral path).
    let inference_trace = if collect_trace {
        engine_result.trace.as_ref().map(|td| InferenceTrace {
            entries: td
                .events
                .iter()
                .map(|e| InferenceTraceEntry {
                    timestamp: e.timestamp.clone(),
                    event_type: e.event_type.to_string(),
                    rule_name: Some(e.rule_id.clone()),
                    data: None,
                })
                .collect(),
            summary: Some(TraceSummary {
                total_duration_ms: td.total_duration_ms,
                facts_matched: td.facts_matched,
            }),
        })
    } else {
        None
    };

    Ok(ExecuteResponse {
        execution_id,
        rules_fired: engine_result.rules_fired,
        rules_not_fired,
        facts_after,
        inference_trace,
        engine_generation: generation,
        // Phase 27 (D-05): stateless path — no server session id.
        session_id: None,
    })
}

/// Phase 27 (D-05): sessionful execution path — called by `execute_single`
/// when `req.session_id` is `Some(alias)`. The session's `Box<dyn
/// ExecutionEngine>` holds accumulated working memory across calls (SC-1's
/// keystone honesty anchor). Engine is fixed at session creation (D-06);
/// `engine_generation` reflects the creation generation (D-07).
///
/// Pitfall 7 (web::block closure shape): `std::sync::MutexGuard` is `!Send`,
/// so the closure cannot capture the guard. The engine is moved OUT of the
/// session under the lock (via `std::mem::replace` with a placeholder), the
/// guard is dropped, and `web::block` owns the engine by value. After
/// `web::block` returns, the entry is re-locked to write the engine back +
/// update `last_used`. No DashMap shard guard is held across `web::block`
/// (Pitfall 1 — `lookup_or_create` clones the `Arc<Mutex<Entry>>` out).
async fn execute_sessionful(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,
) -> Result<ExecuteResponse, (actix_web::http::StatusCode, String)> {
    let client_alias = req.session_id.clone().unwrap_or_default();

    // 1. Lookup or create the session (D-04/D-06). Conflicting engine on
    //    resume → 400 surfaces here from lookup_or_create's Err.
    let (server_uuid, entry_arc) = session_manager.lookup_or_create(
        &client_alias,
        req.engine.as_deref(),
        &registry,
        &ruleset,
    )?;

    // 2. Insert request facts into the session's engine. Lock the per-session
    //    Mutex (the Arc was cloned out by lookup_or_create — no DashMap shard
    //    guard is held).
    {
        let mut guard = entry_arc.lock().map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("session lock poisoned: {}", e),
            )
        })?;
        for (fact_type, fact_value) in &req.facts {
            if let serde_json::Value::Object(fields) = fact_value.clone() {
                let mut payload = HashMap::new();
                for (key, value) in fields {
                    payload.insert(key, json_to_engine_value(value));
                }
                let fact = engine::fact::Fact::new(fact_type.clone(), payload);
                guard.engine.insert_fact(fact).map_err(|e| {
                    (
                        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("insert_fact failed: {}", e),
                    )
                })?;
            }
        }
    }

    // 3. Pitfall 7: move engine out, drop guard, web::block owns the engine,
    //    re-lock to write back. The placeholder is a throwaway fresh engine
    //    created from the same factory — it's discarded after the real engine
    //    is written back.
    //
    // The guard is scoped to its own block so the compiler proves it's
    // dropped BEFORE the `web::block(...).await` — `std::sync::MutexGuard`
    // is `!Send` and cannot be held across an `.await` point.
    let engine_name_str;
    let engine_generation;
    let start = Instant::now();

    // Extract the engine + metadata from the session, dropping the guard
    // before web::block (Pitfall 7).
    let mut engine = {
        let mut guard = entry_arc.lock().map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("session lock poisoned: {}", e),
            )
        })?;
        engine_name_str = guard.engine_name.clone();
        engine_generation = guard.engine_generation;

        // Create a placeholder engine to swap with (Pitfall 7).
        let placeholder = match registry.create_engine_by_name(&guard.engine_name) {
            Some(Ok(e)) => e,
            _ => {
                return Err((
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to create placeholder engine for web::block".to_string(),
                ))
            }
        };

        // Move the real engine out, put placeholder in. The guard is dropped
        // at the end of this block.
        std::mem::replace(&mut guard.engine, placeholder)
    };

    // Phase 28 (D-02 / EXEC-04): resolve the trace opt-in and enable it on
    // the session engine before web::block owns it. Same pattern as the
    // stateless path — `unwrap_or(false)` maps absent/None to the SC-3
    // latency-neutral off path.
    let collect_trace = req.trace.unwrap_or(false);
    engine.set_collect_trace(collect_trace);

    // web::block owns the engine by value — no MutexGuard captured across
    // the .await (Pitfall 7 / Open Q#3).
    let (returned_engine, engine_result, post_facts) = match web::block(move || {
        let result = engine.execute();
        let facts = engine.get_facts();
        (engine, result, facts)
    })
    .await
    {
        Ok(tuple) => tuple,
        Err(e) => {
            return Err((
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("engine execution panicked: {}", e),
            ))
        }
    };

    // Re-lock to write the engine back + update last_used.
    {
        let mut guard = entry_arc.lock().map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("session lock poisoned: {}", e),
            )
        })?;
        guard.engine = returned_engine;
        guard.last_used = Instant::now();
    }

    // 4. Prometheus counters + MetricsSnapshot (mirror stateless path).
    let elapsed = start.elapsed().as_secs_f64();
    counters
        .requests_total
        .with_label_values(&[&engine_name_str])
        .inc();
    counters
        .rules_fired_total
        .with_label_values(&[&engine_name_str])
        .inc_by(engine_result.rules_fired as u64);
    counters
        .duration_seconds
        .with_label_values(&[&engine_name_str])
        .observe(elapsed);
    counters.snapshot_requests.fetch_add(1, Ordering::Relaxed);
    counters
        .snapshot_rules_fired
        .fetch_add(engine_result.rules_fired as u64, Ordering::Relaxed);
    counters
        .snapshot_facts_inserted
        .fetch_add(req.facts.len() as u64, Ordering::Relaxed);

    // D-08: update the gauge + populate active_sessions in the snapshot.
    session_manager
        .sessions_active
        .set(session_manager.len() as i64);
    let snapshot = MetricsSnapshot {
        engine_name: engine_name_str.clone(),
        rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
        facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
        active_sessions: session_manager.len(),
        memory_bytes: 0,
        last_updated: chrono::Utc::now().to_rfc3339(),
    };
    let _ = metrics_tx.send(snapshot);

    // 5. Serialize facts_after from post-execution working memory (Pitfall 11).
    let mut facts_after: HashMap<String, serde_json::Value> = HashMap::new();
    for fact in post_facts {
        let obj: serde_json::Map<String, serde_json::Value> = fact
            .payload
            .into_iter()
            .map(|(k, v)| (k, engine_value_to_json(v)))
            .collect();
        let fact_json = serde_json::Value::Object(obj);
        facts_after
            .entry(fact.fact_type_id)
            .and_modify(|existing| {
                if let serde_json::Value::Object(existing_map) = existing {
                    if let serde_json::Value::Object(new_map) = &fact_json {
                        for (k, v) in new_map {
                            existing_map.insert(k.clone(), v.clone());
                        }
                    }
                }
            })
            .or_insert(fact_json);
    }

    // 6. Compute rules_not_fired from the shared ruleset.
    let all_rule_ids: Vec<String> = {
        let guard = ruleset.read().map_err(|e| {
            (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("ruleset lock poisoned: {}", e),
            )
        })?;
        guard.ruleset.rules.iter().map(|r| r.id.clone()).collect()
    };
    let fired_set: std::collections::HashSet<String> =
        engine_result.fired_rule_ids.iter().cloned().collect();
    let rules_not_fired: Vec<String> = all_rule_ids
        .iter()
        .filter(|id| !fired_set.contains(*id))
        .cloned()
        .collect();

    // 7. Build ExecuteResponse with the server UUID (D-04) + creation gen (D-07).
    let execution_id = Uuid::new_v4().to_string();
    // Phase 28 (D-01 / D-02): map engine TraceData → api InferenceTrace on
    // the sessionful path too (do NOT leave this as None — the sessionful
    // path also benefits from trace). Same mapping as the stateless path.
    let inference_trace = if collect_trace {
        engine_result.trace.as_ref().map(|td| InferenceTrace {
            entries: td
                .events
                .iter()
                .map(|e| InferenceTraceEntry {
                    timestamp: e.timestamp.clone(),
                    event_type: e.event_type.to_string(),
                    rule_name: Some(e.rule_id.clone()),
                    data: None,
                })
                .collect(),
            summary: Some(TraceSummary {
                total_duration_ms: td.total_duration_ms,
                facts_matched: td.facts_matched,
            }),
        })
    } else {
        None
    };
    Ok(ExecuteResponse {
        execution_id,
        rules_fired: engine_result.rules_fired,
        rules_not_fired,
        facts_after,
        inference_trace,
        engine_generation,
        // Phase 27 (D-04): server-generated UUID v4 primary key.
        session_id: Some(server_uuid.to_string()),
    })
}

/// Execute rules - POST /v1/execute
#[utoipa::path(
    post,
    path = "/v1/execute",
    tag = "execution",
    request_body = ExecuteRequest,
    responses(
        (status = 200, description = "Rules executed successfully", body = ExecuteResponse),
        (status = 400, description = "Unknown engine name"),
    ),
)]
pub async fn execute_rules(
    req: web::Json<ExecuteRequest>,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,
) -> impl Responder {
    match execute_single(req.into_inner(), registry, selector, ruleset, counters, metrics_tx, session_manager).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err((code, msg)) => HttpResponse::build(code).json(serde_json::json!({ "error": msg })),
    }
}

/// List available engines - GET /v1/engines
///
/// Phase 28 (D-03 / FRONTEND-04): returns the registered engine names + the
/// resolved default so the frontend engine-selector dropdown is honest about
/// feature-gated engines (never hardcoded). `engines` is sorted ascending
/// for a stable dropdown (28-RESEARCH Open Q5 — `available_engines()`
/// iterates a HashMap and is not order-guaranteed). `default` mirrors the
/// exact default-resolution expression from `execute_single`:
/// `RRULES_DEFAULT_ENGINE` env > `selector.current_engine()`. Reuses the
/// already-shared `web::Data<EngineRegistry>` + `web::Data<EngineSelector>`
/// — no new app_data. Behind the existing `requires_auth` middleware on
/// the `/v1` scope (T-28-03-engine-enum mitigation).
#[utoipa::path(
    get,
    path = "/v1/engines",
    tag = "execution",
    responses(
        (status = 200, description = "Available engines", body = EnginesResponse),
    ),
)]
pub async fn list_engines(
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
) -> impl Responder {
    let mut engines: Vec<String> = registry
        .available_engines()
        .iter()
        .map(|s| s.to_string())
        .collect();
    engines.sort(); // stable dropdown order (Open Q5)
    let default = std::env::var("RRULES_DEFAULT_ENGINE")
        .unwrap_or_else(|_| selector.current_engine().to_string());
    HttpResponse::Ok().json(EnginesResponse { engines, default })
}

/// Execute multiple rule sets concurrently - POST /v1/rules/batch-execute
///
/// Runs each request in its own task, bounded by a semaphore. One failing
/// request does not fail the batch — failures are reported per-item. There are
/// no shared locks between requests: `execute_single` builds its own engine +
/// working memory per request (Pitfall 1). The shared `web::Data` handles are
/// cloned into each task (Arc-under-the-hood, cheap) so every task constructs
/// an independent engine from the shared registry and reads the shared
/// ruleset snapshot.
#[utoipa::path(
    post,
    path = "/v1/rules/batch-execute",
    tag = "execution",
    request_body = BatchExecuteRequest,
    responses(
        (status = 200, description = "Batch executed successfully", body = BatchExecuteResponse),
    ),
)]
pub async fn batch_execute_rules(
    req: web::Json<BatchExecuteRequest>,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,
) -> impl Responder {
    let inner = req.into_inner();
    let max_concurrency = inner.max_concurrency.unwrap_or(8).max(1);
    let requests = inner.requests;

    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    for request in requests {
        let permit_sem = semaphore.clone();
        let registry = registry.clone();
        let selector = selector.clone();
        let ruleset = ruleset.clone();
        // Phase 24 (Plan 24-03 / PATTERNS.md batch pattern): clone the
        // counters + metrics_tx web::Data handles into each JoinSet task
        // alongside the existing registry/selector/ruleset clones so
        // per-item execution increments counters and broadcasts — N items
        // = N increments + N broadcasts.
        let counters = counters.clone();
        let metrics_tx = metrics_tx.clone();
        // Phase 27 (D-12): clone the SessionManager into each JoinSet task
        // so sessionful batch items can call execute_single with session_id.
        let session_manager = session_manager.clone();
        join_set.spawn(async move {
            // Acquire a concurrency permit before running. The semaphore is
            // never explicitly closed, so this only errors if the runtime shuts
            // down — in which case panicking turns the task into a JoinError,
            // which the collector below reports as a per-item failure.
            let _permit = permit_sem
                .acquire_owned()
                .await
                .expect("batch semaphore closed unexpectedly");
            // D-03 / R4: each task builds its own engine via execute_single
            // (Pitfall 1 — no shared &mut engine across requests). The task
            // returns `Result<ExecuteResponse, String>`: `Ok(resp)` stays
            // `Ok(resp)`; `Err((_, msg))` is mapped to `Err(msg)` so the
            // three-arm collector below can surface the error string in the
            // per-item `BatchItemResult.error` field without consuming an
            // `HttpResponse` body. The Err-mask placeholder (hardcoded
            // zero-fired `ExecuteResponse`) is GONE — an `execute_single` Err
            // is now a per-item FAILURE, not a masked success (SC-2).
            execute_single(request, registry, selector, ruleset, counters, metrics_tx, session_manager)
                .await
                .map_err(|(_code, msg)| msg)
        });
    }

    let mut results = Vec::new();
    let mut total_fired = 0usize;
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    // D-03 three-arm collector. `join_next()` yields `Result<Result<ExecuteResponse,
    // String>, JoinError>`:
    //   - `Ok(Ok(resp))`  → succeeded += 1, success:true, total_fired += resp.rules_fired
    //   - `Ok(Err(msg))`  → failed += 1, success:false, error:Some(msg), response:None
    //     (the D-03 fix — execute_single Err is a per-item FAILURE, not a
    //     zero-fired success; e.g. unknown engine → 400 is now counted as
    //     failed, not succeeded with rules_fired:0)
    //   - `Err(JoinError)` → failed += 1, success:false, error:Some(join_err string)
    //     (L-06 — panic → JoinError → failed; already correct, preserved)
    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(Ok(resp)) => {
                total_fired += resp.rules_fired;
                succeeded += 1;
                results.push(BatchItemResult {
                    success: true,
                    response: Some(resp),
                    error: None,
                });
            }
            Ok(Err(msg)) => {
                failed += 1;
                results.push(BatchItemResult {
                    success: false,
                    response: None,
                    error: Some(msg),
                });
            }
            Err(join_err) => {
                failed += 1;
                results.push(BatchItemResult {
                    success: false,
                    response: None,
                    error: Some(join_err.to_string()),
                });
            }
        }
    }

    HttpResponse::Ok().json(BatchExecuteResponse {
        results,
        total_fired,
        succeeded,
        failed,
    })
}

/// Reload rules - POST /v1/rules/reload
#[utoipa::path(
    post,
    path = "/v1/rules/reload",
    tag = "execution",
    request_body = ReloadRequest,
    responses(
        (status = 200, description = "Rules reloaded synchronously", body = ReloadResponse),
        (status = 202, description = "Async reload job accepted", body = JobResponse),
        (status = 500, description = "Reload failed"),
    ),
)]
pub async fn reload_rules(
    req: web::Json<ReloadRequest>,
    shared: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    job_store: web::Data<JobStore>,
) -> impl Responder {
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());

    if req.async_reload {
        // L-03 / Pitfall 14: insert the Pending ReloadJob into the JobStore
        // BEFORE spawning the reload task. If the spawn happened first, a
        // rapid poll of `GET /v1/jobs/{job_id}` could see `404 not_found`
        // before the task had a chance to insert the row. By inserting here
        // (synchronously, under the handler's call frame) the 202 response
        // and the Pending row are observable together.
        let job_id = Uuid::new_v4().to_string();
        {
            let mut jobs = job_store
                .write()
                .expect("job_store lock poisoned during Pending insert");
            jobs.insert(
                job_id.clone(),
                ReloadJob {
                    job_id: job_id.clone(),
                    result: None,
                    status: ReloadStatus::Pending,
                    error: None,
                },
            );
        }

        // Clone the Arc handles + rules_dir into the spawned task so it can
        // run independently of the handler's borrowed `web::Data` lifetime.
        // `web::Data<T>` is `Arc<T>` underneath, so `get_ref().clone()` gives
        // another `Arc` (cheap, refcount-only).
        let shared_clone: Arc<std::sync::RwLock<SharedRuleSet>> = shared.get_ref().clone();
        let job_store_clone: JobStore = job_store.get_ref().clone();
        let dir_clone = rules_dir.clone();
        let job_id_clone = job_id.clone();

        tokio::spawn(async move {
            // Transition Pending → Running (L-03).
            {
                let mut jobs = job_store_clone
                    .write()
                    .expect("job_store lock poisoned during Running transition");
                if let Some(job) = jobs.get_mut(&job_id_clone) {
                    job.status = ReloadStatus::Running;
                }
            }

            // D-04: call the ONE shared reload helper (same impl as the sync
            // branch below — they cannot diverge). reload_and_swap builds the
            // new RuleSet off-to-side, then swaps the whole SharedRuleSet +
            // bumps generation under the write lock (Pitfall 4: no I/O under
            // the lock).
            match reload_and_swap(&shared_clone, &dir_clone).await {
                Ok(result) => {
                    // Running → Completed (with the ReloadResult payload).
                    let mut jobs = job_store_clone
                        .write()
                        .expect("job_store lock poisoned during Completed transition");
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = ReloadStatus::Completed;
                        job.result = Some(result);
                    }
                }
                Err(e) => {
                    // Running → Failed (with the error string).
                    let mut jobs = job_store_clone
                        .write()
                        .expect("job_store lock poisoned during Failed transition");
                    if let Some(job) = jobs.get_mut(&job_id_clone) {
                        job.status = ReloadStatus::Failed;
                        job.error = Some(e);
                    }
                }
            }
        });

        // 202 + JobResponse { job_id, status: "pending" } — the status
        // matches the row we just inserted (D discretion; SC-1 polls and
        // observes pending/running → completed).
        HttpResponse::Accepted().json(JobResponse {
            job_id,
            status: "pending".to_string(),
        })
    } else {
        // D-04: sync reload ALSO calls reload_and_swap inline — one shared
        // swap implementation for both paths. A sync reload that didn't swap
        // would be a lie (on-disk rules would differ from the executing
        // ruleset). reload_and_swap builds the new RuleSet off-to-side and
        // swaps under the write lock, bumping generation (L-02, L-04).
        match reload_and_swap(shared.get_ref(), &rules_dir).await {
            Ok(result) => HttpResponse::Ok().json(ReloadResponse {
                reload_id: result.reload_id,
                rules_reloaded: result.rule_count,
                status: format!("Successfully reloaded {} rules", result.rule_count),
            }),
            Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                "reload_id": Uuid::new_v4().to_string(),
                "rules_reloaded": 0,
                "status": format!("Reload failed: {}", e)
            })),
        }
    }
}

/// Get reload job status - GET /v1/jobs/{job_id}
#[utoipa::path(
    get,
    path = "/v1/jobs/{job_id}",
    tag = "jobs",
    responses(
        (status = 200, description = "Job found", body = ReloadJob),
        (status = 404, description = "Job not found"),
    ),
    params(
        ("job_id" = String, Path, description = "Reload job ID"),
    ),
)]
pub async fn get_job_status(
    path: web::Path<String>,
    store: web::Data<JobStore>,
) -> impl Responder {
    let job_id = path.into_inner();

    let jobs = store.read().unwrap();

    match jobs.get(&job_id) {
        Some(job) => HttpResponse::Ok().json(job),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Job not found"
        })),
    }
}

/// List all jobs - GET /v1/jobs
#[utoipa::path(
    get,
    path = "/v1/jobs",
    tag = "jobs",
    responses(
        (status = 200, description = "List of reload jobs", body = Vec<ReloadJob>),
    ),
)]
pub async fn list_jobs(store: web::Data<JobStore>) -> impl Responder {
    let jobs = store.read().unwrap();

    let job_list: Vec<&ReloadJob> = jobs.values().collect();

    HttpResponse::Ok().json(job_list)
}

/// Start file watcher for automatic hot reload on rule changes
pub async fn start_file_watcher(rules_dir: String) {
    log::info!("File watcher started for: {}", rules_dir);
}

fn json_to_engine_value(value: serde_json::Value) -> engine::Value {
    match value {
        serde_json::Value::Null => engine::Value::Null,
        serde_json::Value::Bool(b) => engine::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                engine::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                engine::Value::Float(f)
            } else {
                engine::Value::Null
            }
        }
        serde_json::Value::String(s) => engine::Value::String(s),
        serde_json::Value::Array(arr) => {
            engine::Value::Array(arr.into_iter().map(json_to_engine_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_engine_value(v)))
                .collect();
            engine::Value::Object(map)
        }
    }
}

/// Reverse of `json_to_engine_value` (D-08 / Pitfall 11). Serializes
/// post-execution `engine::Value`s (read from `engine.get_facts()`) back into
/// `serde_json::Value` so `facts_after` reflects real working memory rather
/// than the echoed request input. Inverts arm-by-arm; `DateTime` serializes
/// to an RFC3339-ish string (no native JSON datetime).
fn engine_value_to_json(value: engine::Value) -> serde_json::Value {
    match value {
        engine::Value::Null => serde_json::Value::Null,
        engine::Value::Bool(b) => serde_json::Value::Bool(b),
        engine::Value::Integer(i) => serde_json::json!(i),
        engine::Value::Float(f) => serde_json::json!(f),
        engine::Value::String(s) => serde_json::Value::String(s),
        engine::Value::DateTime(dt) => serde_json::Value::String(dt.to_string()),
        engine::Value::Array(arr) => serde_json::Value::Array(
            arr.into_iter().map(engine_value_to_json).collect(),
        ),
        engine::Value::Object(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .into_iter()
                .map(|(k, v)| (k, engine_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}
