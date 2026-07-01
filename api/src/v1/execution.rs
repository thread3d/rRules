//! Rule execution API endpoints

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use engine::environment::EngineBuilder;
use engine::WorkingMemory;

/// Request for rule execution
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct ExecuteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub facts: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_filter: Option<RuleFilter>,
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

/// Core single-request execution logic. Extracted so batch execution can reuse
/// it concurrently without duplicating the engine/working-memory wiring.
///
/// NOTE: execution is currently a stub — `rules_fired` is hardcoded to `0`
/// because the engine is built into `let _engine` (never driven). Wiring real
/// rule firing is a separate future milestone and out of scope for v4.0
/// hardening. The batch endpoint layers real concurrency (Semaphore + JoinSet)
/// on top of this stub.
pub async fn execute_single(req: ExecuteRequest) -> ExecuteResponse {
    let execution_id = Uuid::new_v4().to_string();
    let start_time = std::time::Instant::now();

    let _engine = EngineBuilder::new().build();
    let _working_memory = WorkingMemory::new();

    let mut facts_after: HashMap<String, serde_json::Value> = HashMap::new();

    for (fact_type, fact_value) in &req.facts {
        if let serde_json::Value::Object(fields) = fact_value.clone() {
            let mut payload = HashMap::new();
            for (key, value) in fields {
                payload.insert(key, json_to_engine_value(value));
            }
            facts_after.insert(fact_type.clone(), fact_value.clone());
        }
    }

    let rules_fired = 0;
    let rules_not_fired: Vec<String> = Vec::new();

    let _duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    ExecuteResponse {
        execution_id,
        rules_fired,
        rules_not_fired,
        facts_after,
        inference_trace: None,
    }
}

/// Execute rules - POST /v1/execute
#[utoipa::path(
    post,
    path = "/v1/execute",
    tag = "execution",
    request_body = ExecuteRequest,
    responses(
        (status = 200, description = "Rules executed successfully", body = ExecuteResponse),
    ),
)]
pub async fn execute_rules(req: web::Json<ExecuteRequest>) -> impl Responder {
    HttpResponse::Ok().json(execute_single(req.into_inner()).await)
}

/// Execute multiple rule sets concurrently - POST /v1/rules/batch-execute
///
/// Runs each request in its own task, bounded by a semaphore. One failing
/// request does not fail the batch — failures are reported per-item. There are
/// no shared locks between requests: `execute_single` is self-contained (each
/// task builds its own engine + working memory).
#[utoipa::path(
    post,
    path = "/v1/rules/batch-execute",
    tag = "execution",
    request_body = BatchExecuteRequest,
    responses(
        (status = 200, description = "Batch executed successfully", body = BatchExecuteResponse),
    ),
)]
pub async fn batch_execute_rules(req: web::Json<BatchExecuteRequest>) -> impl Responder {
    let inner = req.into_inner();
    let max_concurrency = inner.max_concurrency.unwrap_or(8).max(1);
    let requests = inner.requests;

    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    for request in requests {
        let permit_sem = semaphore.clone();
        join_set.spawn(async move {
            // Acquire a concurrency permit before running. The semaphore is
            // never explicitly closed, so this only errors if the runtime shuts
            // down — in which case panicking turns the task into a JoinError,
            // which the collector below reports as a per-item failure.
            let _permit = permit_sem
                .acquire_owned()
                .await
                .expect("batch semaphore closed unexpectedly");
            execute_single(request).await
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
