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
use engine::rules::RuleSet;

use crate::metrics::{ExecutionCounters, MetricsSnapshot, MetricsTx};

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
type JobStore = Arc<std::sync::RwLock<HashMap<String, ReloadJob>>>;

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
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
) -> Result<ExecuteResponse, HttpResponse> {
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
        return Err(HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("unknown engine: {}", engine_name)
        })));
    }

    // 3. Build a fresh per-request engine (Pitfall 1 / Pitfall 9: ReteEngine
    //    is not Clone; construct a new Box<dyn ExecutionEngine> per request).
    let mut engine = match registry.create_engine_by_name(&engine_name) {
        Some(Ok(engine)) => engine,
        Some(Err(e)) => {
            return Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("engine creation failed: {}", e)
            })))
        }
        None => {
            return Err(HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("unknown engine: {}", engine_name)
            })))
        }
    };

    // 4. Load the shared ruleset (read-only guard) and collect rule ids for
    //    rules_not_fired computation. The guard is dropped before execution
    //    so no lock spans the CPU-bound engine.execute() call.
    let all_rule_ids: Vec<String> = {
        let guard = ruleset.read().map_err(|e| {
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("ruleset lock poisoned: {}", e)
            }))
        })?;
        engine
            .load_ruleset(&guard)
            .map_err(|e| {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("load_ruleset failed: {}", e)
                }))
            })?;
        guard.rules.iter().map(|r| r.id.clone()).collect()
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
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("insert_fact failed: {}", e)
                }))
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
            return Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("engine execution panicked: {}", e)
            })))
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
        // Open Q3: active_sessions and memory_bytes are 0 in Phase 24;
        // Phase 27 populates active_sessions.
        active_sessions: 0,
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

    Ok(ExecuteResponse {
        execution_id,
        rules_fired: engine_result.rules_fired,
        rules_not_fired,
        facts_after,
        inference_trace: None,
        engine_generation: 0u64,
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
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
) -> impl Responder {
    match execute_single(req.into_inner(), registry, selector, ruleset, counters, metrics_tx).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(err) => err,
    }
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
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
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
        join_set.spawn(async move {
            // Acquire a concurrency permit before running. The semaphore is
            // never explicitly closed, so this only errors if the runtime shuts
            // down — in which case panicking turns the task into a JoinError,
            // which the collector below reports as a per-item failure.
            let _permit = permit_sem
                .acquire_owned()
                .await
                .expect("batch semaphore closed unexpectedly");
            // Each task builds its own engine via execute_single — no shared
            // &mut engine across requests (Pitfall 1). On error, report a
            // zero-fired placeholder so the per-item failure is surfaced.
            match execute_single(request, registry, selector, ruleset, counters, metrics_tx).await {
                Ok(resp) => resp,
                Err(_) => ExecuteResponse {
                    execution_id: Uuid::new_v4().to_string(),
                    rules_fired: 0,
                    rules_not_fired: Vec::new(),
                    facts_after: HashMap::new(),
                    inference_trace: None,
                    engine_generation: 0u64,
                },
            }
        });
    }

    let mut results = Vec::new();
    let mut total_fired = 0usize;
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(resp) => {
                total_fired += resp.rules_fired;
                succeeded += 1;
                results.push(BatchItemResult {
                    success: true,
                    response: Some(resp),
                    error: None,
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
    engine_state: web::Data<EngineState>,
) -> impl Responder {
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());

    if req.async_reload {
        let job_id = Uuid::new_v4().to_string();
        let engine_state_clone = engine_state.clone();

        tokio::spawn(async move {
            let _result = engine_state_clone.reload_engine(&rules_dir).await;
        });

        HttpResponse::Accepted().json(JobResponse {
            job_id,
            status: "running".to_string(),
        })
    } else {
        match engine_state.reload_from_directory(&rules_dir).await {
            Ok(rule_count) => {
                HttpResponse::Ok().json(ReloadResponse {
                    reload_id: Uuid::new_v4().to_string(),
                    rules_reloaded: rule_count,
                    status: format!("Successfully reloaded {} rules", rule_count),
                })
            }
            Err(e) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "reload_id": Uuid::new_v4().to_string(),
                    "rules_reloaded": 0,
                    "status": format!("Reload failed: {}", e)
                }))
            }
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

/// Execute with session - POST /v1/execute/session
pub async fn execute_with_session(
    session_id: web::Path<String>,
    _req: web::Json<ExecuteRequest>,
) -> impl Responder {
    let execution_id = Uuid::new_v4().to_string();

    HttpResponse::Ok().json(serde_json::json!({
        "execution_id": execution_id,
        "session_id": session_id.into_inner(),
        "status": "executed",
        "rules_fired": 0
    }))
}
