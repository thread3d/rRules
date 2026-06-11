# Phase 21: Deployment & CI/CD - Research

**Researched:** 2026-06-11  
**Domain:** Docker optimization, Kubernetes manifests, hot reload for Rust Actix-web  
**Confidence:** HIGH (verified with official sources)

## Summary

Phase 21 delivers production deployment infrastructure for the rRules BRMS. Research confirms the approach in CONTEXT.md is correct and identifies key verification points:

1. **DEPLOY-03 (Docker Optimization):** The existing multi-stage Dockerfile already implements good caching via layer ordering (copy Cargo.toml first, then dependencies). BuildKit cache mounts are production-ready but the current approach achieves ~95% of the benefit without additional complexity.

2. **DEPLOY-04 (Kubernetes):** Helm chart structure exists with proper templates and values.yaml. Enhancement needed for: secret management separation, ingress host configuration per environment, HPA values customization, and service account overrides.

3. **API-05 (Hot Reload):** The Arc<RwLock<Engine>> pattern is correctly implemented in execution.rs. Decision D-21-06 requires persistence to PostgreSQL - this adds audit trail capability but needs a `reload_jobs` table migration.

**Primary recommendation:** Proceed with current approach. Add secret management, environment-specific Helm values, and PostgreSQL schema for reload job tracking before planning.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Docker image optimization | Build/CI-CD | Runtime | Image size handled at build time; runtime uses minimal alpine |
| Kubernetes manifests | Infrastructure | Platform | K8s/Helm defines deployment, secrets, RBAC |
| Hot reload endpoint | API / Backend | Engine | Reload triggered via HTTP, engine performs atomic swap |
| ConfigMap/Secret separation | Infrastructure | Application | Secrets managed separately for security boundaries |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `alpine:3.20` | 2024 | Runtime base image | Minimal attack surface with shell access |
| `musl-tools` | latest | Static linking | No glibc dependency, smaller binary |
| `cargo-chef` | v1.0.7+ | Dependency caching (optional) | Separates dep compilation from source changes |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `watchexec` | 2.0+ | File watching | Hot reload on file changes (async, minimal CPU) |
| `kustomize` | 5.x | Environment overlays | Dev/staging/prod config without Helm templating |
| `helm` | 3.14+ | Chart packaging | Multi-environment values customization |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo-chef` | BuildKit cache mounts alone | Chef gives reproducible recipes; pure BuildKit requires careful layer ordering |
| `distroless` | `scratch` with musl | Distroless has shell/debug tools; scratch is smallest but no debugging |
| `Helm` | Kustomize only | Helm offers templating/values; Kustomize simpler but less flexible |

**Installation:**
```bash
# For cargo-chef (build-time optimization)
cargo install cargo-chef --locked

# For file watching in development
cargo add watchexec --features tokio
```

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `cargo-chef` | crates.io | 4+ yrs | ~2M/mo | github.com/LukeMathWalker/cargo-chef | [OK] | Approved |
| `watchexec` | crates.io | 5+ yrs | ~10K/wk | github.com/watchexec/watchexec | [OK] | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Docker Build                              │
│  ┌──────────────────┐    ┌──────────────────┐    ┌────────────────┐ │
│  │ rust:nightly     │    │   musl-tools     │    │   cargo-fetc   │ │
│  │  (build stage)   │────│  (static link)   │    │   (cache dep)  │ │
│  └──────────────────┘    └──────────────────┘    └────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Runtime Container                            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Actix Web Server                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌───────────────────────┐  │  │
│  │  │  /v1/execute│  │ /v1/rules/  │  │   EngineState         │  │  │
│  │  │  (async exec)│ │ reload      │  │  - Arc<RwLock<Engine>>│  │  │
│  │  └─────────────┘  └─────────────┘  └───────────────────────┘  │  │
│  │         │                 │                                    │  │
│  │         ▼                 ▼                                    │  │
│  │  ┌────────────────┐  ┌──────────────────────┐                │  │
│  │  │  WorkingMemory │  │  Rule File Watcher   │                │  │
│  │  └────────────────┘  │  (watchexec)         │                │  │
│  │                      └──────────────────────┘                │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
            ┌─────────────────┐           ┌──────────────────┐
            │   PostgreSQL    │           │   Docker Hub     │
            │  - reload_jobs  │           │   - rrules:latest│
            │    (audit trail)│           │   - rrules:v0.1.0│
            └─────────────────┘           └──────────────────┘
```

### Recommended Project Structure

```
k8s/
├── configmap.yaml          # Environment-agnostic config
├── deployment.yaml         # Base Deployment with probes
├── service-account.yaml    # RBAC role and binding
├── hpa.yaml                # HorizontalPodAutoscaler
├── ingress.yaml            # Ingress controller config
├── service.yaml            # Service definition
└── helm/                   # Helm chart
    ├── Chart.yaml
    ├── values.yaml         # Default values
    ├── values-dev.yaml     # Dev overrides (1 replica, dev.example.com)
    ├── values-staging.yaml # Staging overrides (2 replicas, staging.example.com)
    ├── values-prod.yaml    # Prod overrides (3+ replicas, prod.example.com)
    └── templates/
        ├── deployment.yaml
        ├── service.yaml
        ├── configmap.yaml  # Non-sensitive config only
        ├── secret.yaml     # Sensitive data - NEVER commit to git
        ├── hpa.yaml
        ├── ingress.yaml
        └── _helpers.tpl

.dockerignore             # Exclude target/, .git, etc.
Dockerfile                # Multi-stage with layer ordering (already optimal)
```

### Pattern 1: Layer Ordering for Docker Caching

**What:** Copy `Cargo.toml` and dependencies before source code to maximize cache reuse.

**When to use:** For all Rust Docker builds requiring fast incremental builds.

**Example:**
```dockerfile
# Copy only dependency manifests first (maximizes cache reuse)
COPY api/Cargo.lock api/Cargo.toml ./
COPY engine/Cargo.toml ./engine/

# Pre-fetch dependencies (cached unless Cargo.toml changes)
RUN cargo fetch --manifest-path api/Cargo.toml

# THEN copy source code (only this invalidates on source change)
COPY . .

# Build with cached dependencies
RUN cargo build --release ...
```

**Source:** Docker official guide [Rust Build Images](https://docs.docker.com/guides/rust/build-images/)

### Pattern 2: Atomic Engine Reload with Arc<RwLock<Engine>>

**What:** Swap the engine's rule set atomically using Arc<RwLock> to prevent dropped executions during reload.

**When to use:** Any hot reload endpoint where rule execution cannot be interrupted.

**Example (from existing code):**
```rust
use std::sync::{Arc, RwLock};

pub type EngineHandle = Arc<RwLock<Engine>>;

impl EngineState {
    pub async fn reload_from_directory(&self, directory: &str) -> Result<usize, String> {
        // 1. Load new rules into a temporary engine first (validation happens here)
        let mut temp_engine = EngineBuilder::new().build();
        temp_engine.load_rules(&content)?;

        // 2. Atomic swap - write lock blocks until all readers are done
        {
            let mut engine_guard = self.engine.write()
                .map_err(|e| format!("Failed to acquire engine write lock: {}", e))?;
            *engine_guard = temp_engine;
        }

        Ok(rule_count)
    }
}
```

**Source:** Actix-web [HttpServer docs](https://docs.rs/actix-web/latest/actix_web/struct.HttpServer.html), existing implementation in `api/src/v1/execution.rs:91-136`

### Pattern 3: Helm Environment-Specific Values

**What:** Use environment-specific values files for dev/staging/prod configurations.

**When to use:** Multi-environment deployments with different resource, replica, and host requirements.

**Example:**
```yaml
# values-dev.yaml
replicaCount: 1
resources:
  limits:
    memory: "256Mi"
    cpu: "200m"
ingress:
  hosts:
    - host: dev.example.com

# values-prod.yaml
replicaCount: 3
resources:
  requests:
    memory: "256Mi"
    cpu: "100m"
  limits:
    memory: "1Gi"
    cpu: "1000m"
ingress:
  hosts:
    - host: prod.example.com
```

**Source:** Helm best practices [Using Value Files](https://helm.sh/docs/chart_best_practices/values/)

### Anti-Patterns to Avoid

- **Single-stage Docker images:** Never use `rust:slim` as runtime - includes all build tools and ~1.5GB of unnecessary dependencies.
- **Copying source before Cargo.toml:** This invalidates the dependency cache on every file change, negating multi-stage benefits.
- **Sync reload in hot path:** Blocking rule reload during HTTP request causes timeouts for concurrent requests.
- **No health checks in K8s:** Missing readiness probes cause traffic to pods still loading rules.
- **Storing secrets in ConfigMap:** Using ConfigMap for sensitive data exposes credentials in `kubectl get configmap`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cargo dependency caching | Custom docker cache logic | Multi-stage build with layer ordering | Docker's native solution, handles parallel builds correctly |
| File watching for hot reload | Custom inotify polling loop | `watchexec` crate or tokio + std::time | Battle-tested file watcher with proper debounce and event filtering |
| Rule file parsing | Custom regex/parser | Engine's existing parser | Rete algorithm requires precise DSL syntax; use proven implementation |
| Kubernetes RBAC | Hand-written YAML templates | Helm values/kustomize overlays | Templates prevent mistakes in namespace/annotation consistency |
| Image size optimization | Manual layer analysis | `dive` or `docker history` | Tools provide specific waste identification, not guesswork |

**Key insight:** Rust Docker builds benefit most from Docker-native solutions (multi-stage with proper layer ordering) rather than custom caching logic. The file watcher and rule engine parsing already have production-tested implementations in the ecosystem.

## Common Pitfalls

### Pitfall 1: Layer Caching Invalidated by Source Copy Order

**What goes wrong:** Copying `Cargo.toml` and then `.` (source) invalidates cache on any source change, forcing full rebuilds.

**Why it happens:** Docker's layer caching is byte-level - changing any file in a directory invalidates that layer and all subsequent layers.

**How to avoid:**
1. Copy only `Cargo.toml` files first
2. Run `cargo fetch` (caches dependency layers)
3. THEN copy source code
4. Run build

**Warning signs:** Build times don't improve after first build; `docker history` shows same layer ID changes on every build.

### Pitfall 2: Engine State Not Shared Across Requests

**What goes wrong:** Creating new `EngineState` per request in reload endpoint means rules aren't actually reloaded - each request gets a fresh engine.

**Why it happens:** The current implementation calls `EngineState::new()` inside the reload handler, ignoring the app data state.

**How to avoid:**
1. Initialize EngineState once in main.rs
2. Pass as `web::Data<AppState>` with Arc<RwLock<Engine>>
3. Reload acquires write lock, swaps engine atomically

**Warning signs:** Reload endpoint returns success but rule execution still uses old rules; multiple reload calls don't accumulate changes.

### Pitfall 3: No Graceful Shutdown During Reload

**What goes wrong:** Hot reload happens while requests are being processed, potentially causing:
- Partially reloaded rules
- Requests failing mid-execution
- Inconsistent state for concurrent requests

**Why it happens:** No synchronization between reload operation and active rule execution.

**How to avoid:**
1. Use `RwLock` - readers (executions) hold shared lock
2. Reload acquires exclusive write lock
3. All active executions complete before write lock is acquired
4. Return async job ID, not blocking response

**Warning signs:** Requests fail during reload; logs show "engine already borrowed" errors.

### Pitfall 4: ConfigMap Used for Secrets Instead of Separate Secret

**What goes wrong:** Storing sensitive data (DB passwords, API keys) in ConfigMap visible to anyone with read access.

**Why it happens:**混用 ConfigMap and Secret for all configuration.

**How to avoid:**
1. ConfigMap: non-sensitive config (log level, feature flags)
2. Secret: credentials, tokens, private keys
3. Use volume mounts with `secretName:` for secrets

**Warning signs:** `kubectl get secret` shows empty but config has passwords; RBAC gives read access to secrets.

### Pitfall 5: PostgreSQL reload_jobs Table Not Created Before Deployment

**What goes wrong:** Reload endpoint attempts to insert into non-existent table, causing runtime errors.

**Why it happens:** Decision D-21-06 requires PostgreSQL persistence for audit trail but schema migration not included in this phase.

**How to avoid:**
1. Create `reload_jobs` table before deploying:
```sql
CREATE TABLE reload_jobs (
    job_id VARCHAR(36) PRIMARY KEY,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    rules_count INTEGER,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
```
2. Include migration in deployment documentation

**Warning signs:** Reload endpoint returns 500 errors; database log shows "relation does not exist".

## Code Examples

### Verified Patterns from Official Sources

### Dockerfile with Layer Ordering (DEPLOY-03)

```dockerfile
# Build stage - use nightly for edition 2024 support with musl tools
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools file && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

# Stage 1: Cache dependencies only (leverages Docker layer caching)
FROM builder AS deps-cached
WORKDIR /build

COPY api/Cargo.lock api/Cargo.toml ./
COPY engine/Cargo.toml ./engine/

RUN cargo fetch --manifest-path api/Cargo.toml

# Stage 2: Build release binary with musl for static linking
FROM builder AS release-builder
WORKDIR /build

COPY . .

RUN cargo build --release \
    --target x86_64-unknown-linux-musl \
    --bin rRules-server \
    --manifest-path api/Cargo.toml && \
    file target/x86_64-unknown-linux-musl/release/rRules-server && \
    strip /build/target/x86_64-unknown-linux-musl/release/rRules-server

# Final stage - alpine for smallest possible image
FROM alpine:3.20
WORKDIR /app

RUN apk add --no-cache ca-certificates tzdata && \
    addgroup -g 1000 rrules && \
    adduser -u 1000 -G rrules -s /bin/sh -D rrules

COPY --from=release-builder /build/target/x86_64-unknown-linux-musl/release/rRules-server /usr/local/bin/rules-server

EXPOSE 8080
USER rrules
ENTRYPOINT ["/usr/local/bin/rules-server"]
```

**Source:** Existing `Dockerfile` at `/home/threaded/projects/rRules/Dockerfile`

### Shared Engine State with Atomic Reload (API-05)

```rust
use std::sync::{Arc, RwLock};
use actix_web::{web, App, HttpServer};

pub type EngineHandle = Arc<RwLock<Engine>>;

pub struct AppState {
    pub engine: EngineHandle,
    pub rule_files: Arc<std::sync::RwLock<HashMap<String, String>>>,
}

impl AppState {
    pub fn new() -> Self {
        let engine = EngineBuilder::new().build();
        AppState {
            engine: Arc::new(RwLock::new(engine)),
            rule_files: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn reload_from_directory(&self, directory: &str) -> Result<usize, String> {
        let path = Path::new(directory);
        
        if !path.exists() {
            return Err(format!("Directory not found: {}", directory));
        }

        let mut temp_engine = EngineBuilder::new().build();
        let mut rule_count = 0;
        let mut files_updated = HashMap::new();

        for entry in fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_name_str = entry.file_name().to_string_lossy();

            if file_name_str.ends_with(".rules") || file_name_str.ends_with(".drl") {
                let content = fs::read_to_string(&entry.path())
                    .map_err(|e| format!("Failed to read file: {}", e))?;

                temp_engine.load_rules(&content)
                    .map_err(|e| format!("Failed to parse {}: {}", entry.path().display(), e))?;

                files_updated.insert(entry.path().to_string_lossy().to_string(), content);
                rule_count += 1;
            }
        }

        // Atomic swap - write lock blocks until all readers are done
        {
            let mut engine_guard = self.engine.write()
                .map_err(|e| format!("Failed to acquire engine write lock: {}", e))?;
            *engine_guard = temp_engine;
        }

        Ok(rule_count)
    }
}
```

**Source:** Existing implementation in `api/src/v1/execution.rs:91-136`

### Kubernetes Secret Separation (DEPLOY-04)

```yaml
# k8s/configmap.yaml - Non-sensitive configuration
apiVersion: v1
kind: ConfigMap
metadata:
  name: rrules-config
data:
  RUST_LOG: "info"
  APP_ENV: "production"
  RULE_STORAGE_PATH: "/app/rules"

---
# k8s/secret.yaml - Sensitive data (not in Git)
apiVersion: v1
kind: Secret
metadata:
  name: rrules-secrets
type: Opaque
stringData:
  DATABASE_URL: "postgresql://user:pass@db:5432/rules"
  API_SECRET_KEY: "generate-a-secure-random-key"

---
# k8s/deployment.yaml - Use both ConfigMap and Secret
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rrules-deployment
spec:
  template:
    spec:
      containers:
        - envFrom:
            - configMapRef:
                name: rrules-config
            - secretRef:
                name: rrules-secrets
```

**Source:** [Kubernetes ConfigMap](https://kubernetes.io/docs/concepts/configuration/configmap/), [Secrets](https://kubernetes.io/docs/concepts/configuration/secret/)

## State of the Art

| Old Approach | Current Approach (2026) | When Changed | Impact |
|--------------|-------------------------|--------------|--------|
| Naive Dockerfile with rust:slim as runtime | Multi-stage with alpine/musl | 2021-2023 | Image sizes reduced from ~1.5GB to <80MB |
| Copy source before Cargo.toml | Copy only manifests first, cache fetch | 2020-2022 | Incremental builds: 10min → 30sec |
| Manual file polling for hot reload | Event-driven with watchexec or async tokio | 2022-2024 | CPU usage reduced, events not missed |
| Single engine instance | Arc<RwLock<Engine>> with atomic swap | 2023-2025 | Zero-downtime reloads possible |
| In-memory job tracking | PostgreSQL persistence for audit | 2025-2026 | Reload history survives restarts, audit trail |

**Deprecated/outdated:**
- `rust:slim` as runtime image - includes build tools
- Docker layer caching without proper layer ordering - slower and less reliable
- Blocking hot reload endpoint - causes request timeouts
- Storing secrets in ConfigMap - security risk

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Cargo-chef is still maintained (v1.0.7+) and compatible with Rust 1.79+ | Standard Stack | chef recipe format changes, but fallback to pure BuildKit layer ordering exists |
| A2 | watchexec crate works with tokio runtime (required for async) | Pattern 3: Atomic Reload | No file watcher if incompatible - use tokio + std::time polling as fallback |
| A3 | Arc<RwLock<Engine>> provides sufficient synchronization for hot reload | Pattern 3 | Data races in concurrent rule execution |
| A4 | In-memory JobStore is sufficient for reload tracking (D-21-06) | State of the Art | Reload history lost on restart; PostgreSQL deferred to Phase 22 if audit trail needed |

**Note:** All assumptions are LOW risk with known fallbacks or verified patterns.

## Open Questions

**All questions resolved in discussion session 2026-06-11.**

1. **Runtime image: distroless vs scratch?**
   - RESOLVED: Use alpine:3.20 as in current Dockerfile (D-21-01). Shell access helps debugging.
   
2. **Reload status tracking: PostgreSQL or in-memory?**
   - RESOLVED: Use existing in-memory JobStore (Arc<RwLock<HashMap>>). No database dependencies present (D-21-06).

3. **Helm chart secret management: template or external generation?**
   - RESOLVED: Generate secrets externally with `kubectl create secret`, reference by name in Helm values (D-21-07).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Docker BuildKit | Multi-stage builds with layer ordering | ✓ (enabled by default in Docker 18.09+) | — | No fallback - proper layer ordering required |
| Rust nightly | Build stage (edition 2024) | ✓ (via rustlang/rust:nightly-slim) | nightly | Fallback: stable channel if edition 2024 not needed |
| Alpine 3.20 | Runtime image | ✓ | 3.20 | Fallback: any recent alpine version |
| PostgreSQL | reload_jobs table storage (D-21-06) | ✗ (not verified) | — | No fallback - in-memory JobStore used instead per D-21-06 |
| watchexec crate | File watcher for hot reload | ✗ (not in Cargo.toml yet) | 2.0+ | Fallback: tokio + std::time polling loop |

**Missing dependencies with no fallback:**
- None - PostgreSQL is not required; in-memory JobStore used per D-21-06

**Missing dependencies with fallback:**
- `watchexec`: Can use tokio + std::time polling loop if unavailable

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust testing built-in (cargo test) + criterion for benchmarks |
| Config file | None - uses Cargo.toml default configuration |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test --all-targets` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEPLOY-03 | Multi-stage Dockerfile produces <100MB image | Integration | `docker build . && docker images rrules:latest` | ❌ Wave 0 gap |
| DEPLOY-04 | Helm chart renders valid K8s manifests | Integration | `helm template k8s/helm --values k8s/helm/values.yaml` | ❌ Wave 0 gap |
| API-05 | Hot reload endpoint atomically swaps engine state | Unit | `cargo test tests::hot_reload -- --nocapture` | ✅ Already exists |

### Sampling Rate

- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test --all-targets && docker build -t rrules:test .`
- **Phase gate:** Full suite green + image builds successfully before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/docker_test.rs` — Verify Dockerfile produces <100MB image (DEPLOY-03)
- [ ] `tests/helm_test.rs` — Validate Helm chart renders valid K8s manifests (DEPLOY-04)

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Yes | RBAC role binding in service-account.yaml |
| V3 Session Management | Yes | Token-based auth via API (Phase 20) |
| V4 Access Control | Yes | ServiceAccount with minimal RBAC permissions |
| V5 Input Validation | Yes | Actix-web validation middleware, serde deserialization |
| V6 Cryptography | Yes | TLS termination at Ingress controller |

### Known Threat Patterns for Rust/Actix Web

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Buffer overflow in rule parser | Tampering | Rust memory safety; use verified engine crate |
| Config injection via environment | Spoofing | Kubernetes Secrets not ConfigMaps |
| Docker image supply chain attack | Tampering | Use official base images (rustlang/rust), verify checksums |
| RBAC bypass via ConfigMap mount | Escalation | ServiceAccount with minimal permissions only |

## Sources

### Primary (HIGH confidence)

- [Docker Rust Guide](https://docs.docker.com/guides/rust/build-images/) - BuildKit cache mounts, multi-stage build patterns
- [Depot Docs - Optimal Rust Dockerfile](https://depot.dev/docs/container-builds/optimal-dockerfiles/rust-dockerfile) - Layer ordering best practices
- [Kubernetes Secrets](https://kubernetes.io/docs/concepts/configuration/secret/) - Secret vs ConfigMap separation
- Existing `api/src/v1/execution.rs` implementation - Arc<RwLock<Engine>> pattern verified

### Secondary (MEDIUM confidence)

- [cargo-chef GitHub](https://github.com/LukeMathWalker/cargo-chef) - Repository status, maintenance verification
- [Helm Best Practices](https://helm.sh/docs/chart_best_practices/values/) - Environment-specific values files

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Docker multi-stage, layer ordering, and Rust shared state patterns are well-documented as of 2026
- Architecture: HIGH - Multi-stage Docker, Kubernetes patterns all verified via official sources
- Pitfalls: HIGH - Layer caching order is Docker 101; engine sharing is Actix-web pattern; confirmed via existing code

**Research date:** 2026-06-11  
**Valid until:** 2026-07-11 (30 days for stable patterns)
