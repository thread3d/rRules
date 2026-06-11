# Phase 21: Deployment & CI/CD - Implementation Plan

**Phase Number:** 21  
**Plan Number:** 01  
**Type:** execute  
**Wave:** 1 (Kubernetes Setup) / Wave 2 (Hot Reload Route Registration)  
**Files Modified:**
- `k8s/helm/secret.yaml.template` — Helm secret template
- `k8s/helm/values-dev.yaml` — Dev environment values
- `k8s/helm/values-staging.yaml` — Staging environment values
- `k8s/helm/values-prod.yaml` — Prod environment values
- `api/src/main.rs` — Register missing /v1/jobs routes

**Autonomous:** true (no checkpoints)

---

## Objective

Complete Phase 21 deployment infrastructure with:
1. **Wave 1 (P1):** Helm secret management and multi-environment values configuration
2. **Wave 2 (P2):** Register existing hot reload endpoints in main.rs routing

**Purpose:** Enable production deployments across dev/staging/prod environments with secure secret handling and rule hot reload without container restarts.

**Output:**
- `k8s/helm/secret.yaml.template` — Secret template for external generation
- Three environment-specific Helm values files (dev, staging, prod)
- Registered routes in main.rs for reload job status tracking

---

## Execution Context

@$HOME/.claude/get-shit-done/workflows/execute-plan.md  
@$HOME/.claude/get-shit-done/templates/summary.md  

**Reference:** `21-CONTEXT.md`, `21-RESEARCH.md`, `STATE.md`

---

## Context

@.planning/PROJECT.md  
@.planning/ROADMAP.md  
@.planning/REQUIREMENTS.md  
@.planning/phases/21-deployment-ci-cd/21-CONTEXT.md  
@.planning/phases/21-deployment-ci-cd/21-RESEARCH.md  

**Existing codebase interfaces:**

From `engine/src/environment.rs`:
```rust
pub type EngineHandle = Arc<RwLock<Engine>>;
```

From `api/src/v1/execution.rs` (existing implementation):
```rust
// Job tracking for async operations
pub struct JobStore(Arc<RwLock<HashMap<String, ReloadJob>>>);
pub fn init_job_store() -> JobStore;

// Hot reload with atomic swap
impl EngineState {
    pub async fn reload_from_directory(&self, directory: &str) -> Result<usize, String>;
}

// Reload endpoints (defined but not all registered in main.rs):
pub async fn reload_rules(...) -> impl Responder  // POST /v1/rules/reload - ALREADY REGISTERED
pub async fn get_job_status(...) -> impl Responder // GET /v1/jobs/{job_id} - NOT YET REGISTERED
pub async fn list_jobs(...) -> impl Responder      // GET /v1/jobs - NOT YET REGISTERED
```

**Missing Route Registration:**
- `GET /v1/jobs/{job_id}` — Get reload job status (defined in execution.rs, not registered)
- `GET /v1/jobs` — List all jobs (defined in execution.rs, not registered)

---

## Tasks

<task type="auto">
  <name>Task 1: Create Helm secret template</name>
  <files>/home/threaded/projects/rRules/k8s/helm/secret.yaml.template</files>
  <action>Create k8s/helm/secret.yaml.template with database credentials and API secrets. Use Helm placeholder syntax for values provided at install time. Per D-21-07, separate secrets from ConfigMaps for security.</action>
  <verify>File exists with proper YAML structure and Helm templating placeholders</verify>
  <done>Template file created at k8s/helm/secret.yaml.template with database_url, api_secret_key fields</done>
</task>

<task type="auto">
  <name>Task 2: Create environment-specific Helm values files</name>
  <files>/home/threaded/projects/rRules/k8s/helm/values-dev.yaml,/home/threaded/projects/rRules/k8s/helm/values-staging.yaml,/home/threaded/projects/rRules/k8s/helm/values-prod.yaml</files>
  <action>Create three environment-specific values files per D-21-07:
  - values-dev.yaml: 1 replica, minimal resources (128Mi/100m), ingress host dev.example.com
  - values-staging.yaml: 2 replicas, moderate resources (256Mi/200m limits), ingress host staging.example.com  
  - values-prod.yaml: 3 replicas, configured resources (512Mi requests/1Gi limits), ingress host prod.example.com

  All files must reference secret via rrules-secrets secretRef.</action>
  <verify>Three files exist with correct replica counts and resource configurations</verify>
  <done>All three values files created with environment-appropriate configurations per D-21-07</done>
</task>

<task type="auto">
  <name>Task 3: Register missing reload job routes in main.rs</name>
  <files>/home/threaded/projects/rRules/api/src/main.rs</files>
  <action>Add route registrations for existing endpoints from execution.rs:
  - Import get_job_status and list_jobs functions
  - Add routes for GET /v1/jobs/{job_id} and GET /v1/jobs

  Code to add in the .service() block (after /rules/reload route):
  ```rust
  // Reload job status endpoints
  .route("/jobs/{job_id}", web::get().to(v1::execution::get_job_status))
  .route("/jobs", web::get().to(v1::execution::list_jobs))
  ```

  Per D-21-03, this enables job tracking for async reload operations.</action>
  <verify>Build compiles successfully with cargo check --bin rRules-server</verify>
  <done>Routes registered in main.rs per D-21-03</done>
</task>

---

## Threat Model

### Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Client → `/v1/rules/reload` | Untrusted job_id input, must validate UUID format |
| Kubernetes Secret → Pod env | Secrets mounted via volume, not environment variables |

### STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-21-01 | Spoofing | POST /v1/rules/reload | mitigate | Validate job_id is valid UUID, reject malformed requests |
| T-21-02 | Tampering | Secret values in Helm templates | accept | Template uses placeholders only; secrets generated externally with kubectl create secret |

---

## Verification

### Phase Completion Checks
```bash
# Verify secret template exists
ls -la k8s/helm/secret.yaml.template

# Verify environment values files exist
cat k8s/helm/values-dev.yaml | grep replicaCount: 1
cat k8s/helm/values-staging.yaml | grep replicaCount: 2  
cat k8s/helm/values-prod.yaml | grep replicaCount: 3

# Build and verify API compiles with new routes
cargo check --bin rRules-server
```

---

## Success Criteria

Phase 21 plan complete when:
- [ ] `k8s/helm/secret.yaml.template` created with Helm templating placeholders
- [ ] Three environment values files (dev/staging/prod) exist with correct replica counts
- [ ] Reload job routes registered in main.rs (GET /v1/jobs/{job_id}, GET /v1/jobs)
- [ ] Code compiles successfully per verification checks
- [ ] All artifacts committed to git

---

## Output

Create `.planning/phases/21-deployment-ci-cd/21-01-SUMMARY.md` when done
