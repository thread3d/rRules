//! OpenAPI specification via utoipa.
//!
//! Generates the OpenAPI 3.0 spec served at `/api-docs/openapi.json` and
//! rendered by Swagger UI at `/swagger-ui/`.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "rRules API",
        version = "9.0.49",
        description = "Production-grade Business Rules Management System REST API",
    ),
    paths(
        // Execution
        crate::v1::execution::execute_rules,
        crate::v1::execution::list_engines,
        crate::v1::execution::batch_execute_rules,
        crate::v1::execution::reload_rules,
        crate::v1::execution::get_job_status,
        crate::v1::execution::list_jobs,
        // Rules CRUD
        crate::v1::rules::create_rule,
        crate::v1::rules::get_rules,
        crate::v1::rules::get_rule,
        crate::v1::rules::update_rule,
        crate::v1::rules::delete_rule,
        crate::v1::rules::undelete_rule,
        // Decisions
        crate::v1::decisions::evaluate_decision,
        crate::v1::decisions::list_decisions,
        crate::v1::decisions::get_decision,
        crate::v1::decisions::create_decision,
        crate::v1::decisions::update_decision,
        crate::v1::decisions::delete_decision,
        crate::v1::decisions::import_dmn,
        crate::v1::decisions::export_dmn,
        crate::v1::decisions::get_decision_ir,
        crate::v1::decisions::reload_dmn,
        // Authentication
        crate::auth::login,
        crate::auth::me,
        crate::auth::refresh,
        // Sessions
        crate::v1::sessions::dispose_session,
        crate::v1::sessions::reset_session,
        // DMN metrics + health aliases
        crate::v1::dmn_metrics::dmn_metrics_endpoint,
        crate::health::live,
        crate::health::ready,
        // Categories
        crate::v1::categories::create_category,
        crate::v1::categories::list_categories,
        crate::v1::categories::get_category,
        crate::v1::categories::update_category,
        crate::v1::categories::delete_category,
        // Tags
        crate::v1::tags::create_tag,
        crate::v1::tags::list_tags,
        crate::v1::tags::get_tag,
        crate::v1::tags::update_tag,
        crate::v1::tags::delete_tag,
        crate::v1::tags::assign_tag_to_rule,
        crate::v1::tags::remove_tag_from_rule,
        // Tenants
        crate::v1::tenants::create_tenant,
        crate::v1::tenants::list_tenants,
        crate::v1::tenants::get_tenant,
        crate::v1::tenants::delete_tenant,
        // Approvals
        crate::v1::approvals::submit_for_approval,
        crate::v1::approvals::list_pending_approvals,
        crate::v1::approvals::approve_request,
        crate::v1::approvals::reject_request,
        // Audit
        crate::v1::audit::query_audit_events,
        crate::v1::audit::export_audit_log,
        // Branches
        crate::v1::branches::create_branch,
        crate::v1::branches::list_branches,
        crate::v1::branches::get_branch,
        crate::v1::branches::delete_branch,
        crate::v1::branches::merge_branch,
        crate::v1::branches::get_git_log,
        // Monitoring
        crate::v1::routes::monitoring::get_engine_state,
        crate::v1::routes::monitoring::monitoring_health,
        // Simulation
        crate::v1::simulation::run_simulation,
        // Health (root-level)
        crate::health::live,
        crate::health::ready,
        // Metrics (root-level)
        crate::metrics::metrics_endpoint,
        crate::metrics::dead_rules_endpoint,
        crate::metrics::coverage_endpoint,
    ),
    components(schemas(
        // Tenant types
        engine::governance::tenant::CreateTenantRequest,
        engine::governance::tenant::TenantResponse,
        engine::governance::tenant::TenantListResponse,
        // Execution types
        crate::v1::execution::ExecuteRequest,
        crate::v1::execution::RuleFilter,
        crate::v1::execution::ExecuteResponse,
        crate::v1::execution::InferenceTrace,
        crate::v1::execution::InferenceTraceEntry,
        crate::v1::execution::TraceSummary,
        crate::v1::execution::EnginesResponse,
        crate::v1::execution::ReloadResult,
        crate::v1::execution::ReloadJob,
        crate::v1::execution::ReloadStatus,
        crate::v1::execution::ReloadRequest,
        crate::v1::execution::ReloadResponse,
        crate::v1::execution::JobResponse,
        crate::v1::execution::BatchExecuteRequest,
        crate::v1::execution::BatchItemResult,
        crate::v1::execution::BatchExecuteResponse,
        // Rules types
        crate::v1::rules::StoredRule,
        crate::v1::rules::RuleStatus,
        crate::v1::rules::CreateRuleRequest,
        crate::v1::rules::RuleResponse,
        crate::v1::rules::RuleListResponse,
        crate::v1::rules::RuleListRequest,
        crate::v1::rules::UpdateRuleRequest,
        // Categories types
        crate::v1::categories::CreateCategoryRequest,
        crate::v1::categories::CategoryResponse,
        crate::v1::categories::CategoryListResponse,
        // Tags types
        crate::v1::tags::CreateTagRequest,
        crate::v1::tags::TagResponse,
        crate::v1::tags::RuleTagActionRequest,
        // Approvals types
        crate::v1::approvals::ApprovalState,
        crate::v1::approvals::SubmitApprovalRequest,
        crate::v1::approvals::SubmitApprovalResponse,
        crate::v1::approvals::ApprovalActionRequest,
        // Audit types
        crate::v1::audit::AuditAction,
        crate::v1::audit::ResourceType,
        crate::v1::audit::AuditEvent,
        crate::v1::audit::AuditListResponse,
        crate::v1::audit::AuditQuery,
        // Branches types
        crate::v1::branches::CreateBranchRequest,
        crate::v1::branches::BranchResponse,
        crate::v1::branches::BranchListResponse,
        crate::v1::branches::GitLogRequest,
        crate::v1::branches::GitLogResponse,
        crate::v1::branches::GitCommitEntry,
        // Monitoring types
        crate::v1::routes::monitoring::EngineStateResponse,
        // Simulation types (API-side)
        crate::v1::simulation::SimulationRunRequest,
        crate::v1::simulation::SimulationRunResponse,
        crate::v1::simulation::SimulationTestCaseResponse,
        crate::v1::simulation::SimulationVariantResponse,
        crate::v1::simulation::SimulationSummary,
        // Engine simulation types (referenced by SimulationRunRequest)
        engine::simulation::types::WorkingMemorySnapshot,
        engine::simulation::types::SnapshotFact,
        engine::simulation::types::RuleChange,
        engine::simulation::types::TestFact,
    )),
    tags(
        (name = "rules", description = "Rule CRUD and management"),
        (name = "execution", description = "Rule execution and batch execution"),
        (name = "jobs", description = "Reload job tracking and status"),
        (name = "categories", description = "Category CRUD and hierarchy"),
        (name = "tags", description = "Tag CRUD and rule-tag assignment"),
        (name = "approvals", description = "Approval workflow and maker-checker"),
        (name = "audit", description = "Audit trail queries and export"),
        (name = "branches", description = "Git-backed branch management"),
        (name = "monitoring", description = "Engine monitoring and health"),
        (name = "simulation", description = "What-If simulation and scenario testing"),
        (name = "health", description = "Health check endpoints"),
        (name = "metrics", description = "Prometheus metrics and dashboards"),
    ),
)]
pub struct ApiDoc;
