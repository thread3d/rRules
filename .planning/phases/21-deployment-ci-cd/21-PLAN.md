# Phase 21: Deployment & CI/CD

**Phase Number:** 21  
**Milestone:** v4.0 - Production Platform  
**Target Completion:** 2026-06-15  

---

## Goal

Complete the production deployment stack with optimized Docker images, Kubernetes manifests, and hot reload mechanism for rule updates without service restart.

---

## Requirements Addressed

| Requirement | Priority |
|-------------|----------|
| DEPLOY-03: Optimized Docker Image | P1 |
| DEPLOY-04: Kubernetes Manifests | P1 |
| API-05: Hot Reload Mechanism | P2 |

---

## Success Criteria (what must be TRUE)

1. Docker image uses multi-stage builds with layer caching optimization
2. Runtime image size < 100MB as verified by `docker images --format "{{.Size}}"`
3. Kubernetes manifests include ConfigMap, Deployment, Service, HPA, and Ingress
4. Hot reload endpoint triggers atomic rule set swap without container restart

---

## Plans

### Phase 21-01: Docker Image Optimization (P1)

**Goal:** Optimize Docker image with multi-stage builds for faster builds and smaller runtime image.

**Wave:** 1

**Files modified:**
- `Dockerfile` - Multi-stage build with Alpine + musl static linking

**Tasks:**

#### 21-01-01: Add cargo cache layer with BuildKit mounts
**Priority:** P1  
**Read first:** None  
**Action:** Modify Dockerfile to use BuildKit cache mounts for cargo registry and target directories

```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target/ \
    cargo fetch --manifest-path api/Cargo.toml
```

**Acceptance criteria:**
- `cargo fetch` caches dependencies in /usr/local/cargo/registry
- `docker history rrules:latest` shows separate layer for dependencies

#### 21-01-02: Use Alpine Linux base with musl-tools
**Priority:** P1  
**Read first:** None  
**Action:** Change build stage to `rust:1.79-alpine` and install musl-dev, musl-utils for static linking

```dockerfile
FROM rust:1.79-alpine AS build
RUN apk add --no-cache musl-dev musl-utils
```

**Acceptance criteria:**
- Build stage uses `rust:1.79-alpine`
- Runtime image uses `alpine:3.19` per D-21-01 decision

#### 21-01-03b: Verify runtime size < 100MB
**Priority:** P1  
**Read first:** None  
**Action:** Add automated test to verify Docker image meets size requirement

```rust
// tests/docker_test.rs
use assert_cmd::Command;

#[test]
fn docker_image_size_under_100mb() {
    let cmd = Command::new("docker")
        .args(["images", "rrules:latest", "--format", "{{.Size}}"])
        .assert()
        .success();
    
    // Parse size and verify < 100MB
    // (implementation would parse output string like "85MiB")
}
```

**Acceptance criteria:**
- `docker images rrules:latest --format "{{.Size}}"` returns <100MB

#### 21-01-03: Implement multi-stage build with builder and runtime stages
**Priority:** P1  
**Read first:** None  
**Action:** Structure Dockerfile with two stages:
1. Build stage: Compile with musl-static linking
2. Runtime stage: Copy binary to distroless image

```dockerfile
# Runtime stage
FROM gcr.io/distroless/cc-debian12
COPY --from=build /build/target/release/rRules-server /app/rules-server
USER 65534:65534
EXPOSE 8080
ENTRYPOINT ["/app/rules-server"]
```

**Acceptance criteria:**
- `docker build .` completes successfully
- `docker images rrules:latest --format "{{.Size}}"` returns <100MB

---

### Phase 21-02: Kubernetes Manifests (P1)

**Goal:** Create complete K8s manifests for deployment and management.

**Wave:** 1

**Files created:**
- `k8s/configmap.yaml` - Configuration values
- `k8s/service-account.yaml` - ServiceAccount with RBAC
- `k8s/hpa.yaml` - HorizontalPodAutoscaler (enhanced)
- `k8s/ingress.yaml` - Ingress controller config

**Files modified:**
- `k8s/deployment.yaml` - Add liveness/readiness probes
- `k8s/service.yaml` - Service definition with proper ports

**Tasks:**

#### 21-02-01: Create ConfigMap for configuration values
**Priority:** P1  
**Read first:** None  
**Action:** Create k8s/configmap.yaml with non-sensitive environment variables:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: rrules-config
data:
  RUST_LOG: "info"
  APP_ENV: "production"
  RULE_STORAGE_PATH: "/app/rules"
```

**Acceptance criteria:**
- ConfigMap includes all non-sensitive config values
- Values match those in existing Dockerfile ENV directives

#### 21-02-02: Configure ServiceAccount with RBAC roles
**Priority:** P1  
**Read first:** None  
**Action:** Create k8s/service-account.yaml with minimal permissions:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: rrules-service-account
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: rrules-role
rules:
- apiGroups: [""]
  resources: ["configmaps", "pods"]
  verbs: ["get", "list", "watch"]
```

**Acceptance criteria:**
- ServiceAccount created with minimal RBAC permissions
- No admin or cluster-admin roles used

#### 21-02-03: Add HorizontalPodAutoscaler for scaling
**Priority:** P1  
**Read first:** None  
**Action:** Create k8s/hpa.yaml with CPU/memory metrics:

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: rrules-hpa
spec:
  minReplicas: 2
  maxReplicas: 5
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

**Acceptance criteria:**
- HPA scales between 2-5 replicas
- Uses CPU utilization as scaling metric

#### 21-02-04: Verify manifests with kubectl dry-run
**Priority:** P1  
**Read first:** None  
**Action:** Test all manifests with `kubectl apply --dry-run=client`:

```bash
kubectl apply --dry-run=client -f k8s/configmap.yaml
kubectl apply --dry-run=client -f k8s/service-account.yaml
kubectl apply --dry-run=client -f k8s/hpa.yaml
```

**Acceptance criteria:**
- All manifests pass dry-run without errors
- Service discovery works within cluster

---

### Phase 21-03: Hot Reload Mechanism (P2)

**Goal:** Implement rule hot reload without container restart.

**Wave:** 2

**Files modified:**
- `api/src/v1/rules.rs` - Add hot reload endpoints
- `engine/src/environment.rs` - Create EngineHandle type alias

**Tasks:**

#### 21-03-01: Create EngineHandle with Arc<RwLock<Engine>> pattern
**Priority:** P1  
**Read first:** None  
**Action:** Modify engine/src/environment.rs to add shared state wrapper:

```rust
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use std::collections::HashMap;

pub type EngineHandle = Arc<RwLock<engine::environment::Engine>>;

pub struct AppState {
    pub engine: EngineHandle,
    pub reload_status: Arc<RwLock<HashMap<String, ReloadStatus>>>,
}

pub enum ReloadStatus {
    Pending,
    Running,
    Completed { rules_reloaded: usize },
    Failed { error: String },
}
```

**Acceptance criteria:**
- `EngineHandle` type alias defined as `Arc<RwLock<Engine>>`
- `AppState` includes engine field with proper synchronization

#### 21-03-02: Implement async reload endpoint with job tracking
**Priority:** P1  
**Read first:** None  
**Action:** Add POST /v1/rules/reload endpoint that spawns background reload:

```rust
use uuid::Uuid;

pub struct ReloadRequest {
    pub rule_file: Option<String>,
}

pub struct ReloadResponse {
    pub reload_id: String,
    pub status: String,
}

pub async fn reload_rules(
    req: web::Json<ReloadRequest>,
    state: web::Data<AppState>,
) -> impl Responder {
    let reload_id = Uuid::new_v4().to_string();
    
    // Get rules directory from request or environment
    let rules_dir = req.rule_file
        .as_deref()
        .unwrap_or_else(|| std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string()));

    // Spawn reload in background (returns immediately)
    let state_clone = web::Data::clone(&state);
    tokio::spawn(async move {
        match state_clone.reload_from_directory(&rules_dir).await {
            Ok(count) => log::info!("Reloaded {} rules", count),
            Err(e) => log::error!("Reload failed: {}", e),
        }
    });

    HttpResponse::Accepted().json(ReloadResponse {
        reload_id,
        status: "reload initiated".to_string(),
    })
}
```

**Acceptance criteria:**
- POST /v1/rules/reload returns job ID immediately
- Reload spawns async background task

#### 21-03-03: Add GET /v1/rules/reload/{job_id} status endpoint
**Priority:** P1  
**Read first:** None  
**Action:** Add status tracking with in-memory HashMap for job IDs:

```rust
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub enum ReloadStatus {
    Pending,
    Running,
    Completed { rules_reloaded: usize },
    Failed { error: String },
}

pub struct AppState {
    pub engine: EngineHandle,
    pub reload_status: Arc<RwLock<HashMap<String, ReloadStatus>>>,
}
```

**Acceptance criteria:**
- GET /v1/rules/reload/{job_id} returns status
- Status transitions: pending -> running -> completed/failed

#### 21-03-04: Implement atomic swap in reload_from_directory method
**Priority:** P1  
**Read first:** None  
**Action:** Add method to AppState that performs atomic engine swap:

```rust
impl AppState {
    pub async fn reload_from_directory(&self, dir: &str) -> Result<usize, String> {
        // 1. Load new rules into temporary engine
        let mut temp_engine = EngineBuilder::new().build();
        let rule_count = load_rules_from_directory(&mut temp_engine, dir).await?;

        // 2. Atomic swap - write lock blocks until all readers are done
        let mut existing_engine = self.engine.write()
            .map_err(|e| format!("Failed to acquire engine write lock: {}", e))?;
        
        *existing_engine = temp_engine;

        Ok(rule_count)
    }
}
```

**Acceptance criteria:**
- Reload completes without dropped executions
- Active sessions continue without interruption

---

## Dependencies

| Phase | Status |
|-------|--------|
| Phase 20 | Must be complete (API endpoints depend on existing structure) |

---

## Success Metrics

- **Docker build time:** < 60 seconds from cache
- **Runtime image size:** < 100MB (verified with docker images --format "{{.Size}}")
- **Hot reload latency:** < 5 seconds for rule set recompilation

---

## Implementation Checklist

### Phase 21-01: Docker Image
- [ ] Multi-stage build with cargo cache layer
- [ ] Use rust:1.79-alpine with musl-tools for build stage
- [ ] Use distroless/cc-debian12 as runtime base
- [ ] Verify size < 100MB with docker images --format "{{.Size}}"

### Phase 21-02: Kubernetes
- [ ] ConfigMap with all non-sensitive configuration values
- [ ] ServiceAccount with minimal RBAC role (no admin privileges)
- [ ] HPA with CPU utilization target (70%)
- [ ] All manifests pass kubectl apply --dry-run

### Phase 21-03: Hot Reload
- [ ] EngineHandle = Arc<RwLock<Engine>> in AppState
- [ ] POST /v1/rules/reload returns job ID immediately
- [ ] GET /v1/rules/reload/{job_id} returns status
- [ ] Atomic swap via RwLock write lock during reload

---

*Phase planned: 2026-06-11*
*Verified by gsd-plan-checker on 2026-06-11*
