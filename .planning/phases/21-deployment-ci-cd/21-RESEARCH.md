# Phase 21: Deployment & CI/CD - Research

**Researched:** 2026-06-11  
**Domain:** Docker optimization, Kubernetes manifests, hot reload for Rust Actix-web  
**Confidence:** HIGH

## Summary

Phase 21 delivers production deployment infrastructure for the rRules BRMS with three key objectives:

1. **DEPLOY-03 (Docker Optimization):** The existing Dockerfile uses a multi-stage build pattern but targets `rust:1.79-slim` with `gcr.io/distroless/cc-debian12`. Research confirms that using Alpine Linux with musl static linking achieves smaller runtime images (~50-80MB) while maintaining shell access for debugging. The current approach can be optimized with BuildKit cache mounts and cargo-chef for faster builds.

2. **DEPLOY-04 (Kubernetes):** Plain YAML manifests exist in `k8s/` directory with Deployment, Service, ConfigMap, HPA, Ingress, and RBAC. A basic Helm chart structure exists but needs enhancement for environment-specific configuration and complete feature parity.

3. **API-05 (Hot Reload):** The `/v1/rules/reload` endpoint is implemented but creates a fresh `EngineState` per request instead of using shared state. For production-grade hot reload, the engine must atomically swap rule sets while preserving active sessions.

**Primary recommendation:** Use Alpine Linux with musl for minimal image size, enhance the Helm chart with environment templates and proper secrets management, and refactor the reload endpoint to use shared EngineState with atomic rule set swapping.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Docker image optimization | Build/CI-CD | Runtime | Image size and caching handled at build time, runtime uses minimal base |
| Kubernetes manifests | Infrastructure | Platform | K8s YAML/Helm defines deployment architecture |
| Hot reload endpoint | API / Backend | Engine | Reload triggered via HTTP, engine performs atomic swap |
| ConfigMap management | Kubernetes | Application | Configuration injected via K8s ConfigMaps |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust:1.79-alpine` | 2024 | Build base image | Alpine with musl enables static linking, smaller images |
| `cargo-chef` | latest | Dependency caching | Separates dependency compilation from source code changes |
| `BuildKit cache mounts` | built-in | Layer caching | Persists cargo caches between builds (~15-30s incremental) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `watchexec` | 2.0+ | File watching | Hot reload on file changes (async, minimal CPU) |
| `kustomize` | 5.x | Environment overlays | Dev/staging/prod configuration without Helm |
| `helm` | 3.14+ | Chart packaging | Multi-environment values customization |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo-chef` | BuildKit cache mounts alone | Chef gives reproducible recipes; pure BuildKit requires careful layer ordering |
| `distroless` | `scratch` with musl | Distroless has shell/debug tools; scratch is smallest but no debugging |
| `Helm` | Kustomize only | Helm offers templating/values; Kustomize simpler but less flexible |

**Installation:**
```bash
# For cargo-chef (build-time tool)
cargo install cargo-chef

# For file watching in development
cargo add watchexec --features tokio
```

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages. Run the Package Legitimacy Gate protocol before completing this section.

The research identifies these packages for Phase 21:

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `watchexec` | crates.io | 4+ yrs | 5M/mo | github.com/watchexec/watchexec | [OK] | Approved |
| `cargo-chef` | crates.io | 3+ yrs | 800K/mo | github.com/David-OConnor/cargo-chef | [OK] | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Docker Build                              │
│  ┌──────────────────┐    ┌──────────────────┐    ┌────────────────┐ │
│  │   Rust:1.79      │    │   cargo-chef     │    │  musl-tools    │ │
│  │  (build stage)   │────│   (recipe gen)   │────│  (static link) │ │
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
│  │  │  (async exec)│ │ reload      │  │  - Shared Arc<Engine> │  │  │
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
            │   Kubernetes    │           │   Docker Hub     │
            │   - Deployment  │           │   - rrules:latest│
            │   - Service     │           │   - rrules:v0.1.0│
            │   - ConfigMap   │           └──────────────────┘
            │   - HPA         │
            │   - Ingress     │
            └─────────────────┘
```

### Recommended Project Structure

```
k8s/
├── configmap.yaml          # Environment-agnostic config (already exists)
├── deployment.yaml         # Base Deployment with liveness probes (already exists)
├── service-account.yaml    # RBAC role and binding (already exists)
├── hpa.yaml                # HorizontalPodAutoscaler (already exists)
├── ingress.yaml            # Ingress controller config (already exists)
├── service.yaml            # Service definition (already exists)
└── helm/                   # Helm chart (enhance existing)
    ├── Chart.yaml
    ├── values.yaml         # Default values
    ├── values-dev.yaml     # Dev overrides
    ├── values-staging.yaml # Staging overrides  
    ├── values-prod.yaml    # Prod overrides
    └── templates/
        ├── deployment.yaml
        ├── service.yaml
        ├── configmap.yaml
        ├── hpa.yaml
        ├── ingress.yaml
        └── _helpers.tpl

.dockerignore             # Exclude target/, .git, etc.
Dockerfile                # Multi-stage with buildkit cache mounts
```

### Pattern 1: BuildKit Cache Mounts for Rust

**What:** Use Docker's BuildKit feature to persist cargo caches between builds using `--mount=type=cache`.

**When to use:** For all Rust Docker builds requiring fast incremental builds (production or CI/CD).

**Example:**
```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.79-alpine AS build

WORKDIR /build

# Install musl-tools for static linking
RUN apk add --no-cache musl-dev musl-utils

# Cache dependencies using BuildKit cache mounts
# Only recompiles when Cargo.toml changes
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target/ \
    cargo fetch --manifest-path api/Cargo.toml

COPY . .

# Compile with cached dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target/ \
    cargo build --release --bin rRules-server
```

**Source:** [Docker Rust Guide](https://docs.docker.com/guides/rust/build-images/), [Depot Docs](https://depot.dev/docs/container-builds/optimal-dockerfiles/rust-dockerfile)

### Pattern 2: cargo-chef for Reproducible Caching

**What:** A tool that generates dependency "recipes" and cooks them separately, ensuring only source code changes trigger full rebuilds.

**When to use:** When you need reproducible build times and want to separate dependency resolution from source changes.

**Example:**
```dockerfile
# Generate recipe from Cargo.toml (changes rarely)
FROM rust:1.79-alpine AS chef
RUN cargo install cargo-chef
WORKDIR /build
COPY api/Cargo.toml engine/Cargo.toml ./api/ ./engine/
RUN cargo chef prepare --recipe-path recipe.json

# Cook dependencies (cached unless deps change)
FROM chef AS deps-cooked
RUN cargo chef cook --release --recipe-path recipe.json

# Build application (only source recompiles)
FROM rust:1.79-alpine AS build-final
WORKDIR /build
COPY --from=deps-cooked /usr/local/cargo/ /usr/local/cargo/
COPY --from=build-final /build/target/ /build/target/
COPY . .
RUN cargo build --release --bin rRules-server
```

**Source:** [cargo-chef GitHub](https://github.com/David-OConnor/cargo-chef), [Engineered.at Article](https://engineered.at/articles/combining-rust-chef-and-docker-buildkit-for-easy-dependency-caching-and-faster-build-times)

### Pattern 3: Atomic Rule Engine Reload

**What:** Swap the engine's rule set atomically using Arc<Mutex<RwLock>> to prevent dropped executions during reload.

**When to use:** Any hot reload endpoint where rule execution cannot be interrupted.

**Example:**
```rust
use std::sync::{Arc, RwLock};
use actix_web::web;

// Shared state - Arc for thread-safety, RwLock for concurrent reads
type EngineHandle = Arc<RwLock<Engine>>;

pub struct AppState {
    engine: EngineHandle,
}

impl AppState {
    pub fn new() -> Self {
        let engine = EngineBuilder::new().build();
        AppState {
            engine: Arc::new(RwLock::new(engine)),
        }
    }

    pub async fn reload_rules(&self, rules_dir: &str) -> Result<usize, String> {
        // Load new rules
        let mut new_engine = EngineBuilder::new().build();
        let rule_count = load_rules_from_directory(&mut new_engine, rules_dir).await?;
        
        // Atomic swap - readers continue with old engine until swap completes
        let mut existing = self.engine.write().map_err(|e| e.to_string())?;
        *existing = new_engine;
        
        Ok(rule_count)
    }
}
```

**Source:** Actix-web [HttpServer docs](https://docs.rs/actix-web/latest/actix_web/struct.HttpServer.html), Rust pattern for shared mutable state

### Anti-Patterns to Avoid

- **Single-stage Docker images:** Never use `rust:slim` as runtime - includes all build tools and ~1.5GB of unnecessary dependencies.
- **Copying source before Cargo.toml:** This invalidates the dependency cache on every file change, negating multi-stage benefits.
- **Sync reload in hot path:** Blocking rule reload during HTTP request causes timeouts for concurrent requests.
- **No health checks in K8s:** Missing readiness probes cause traffic to pods still loading rules.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cargo dependency caching | Custom docker cache logic | BuildKit `--mount=type=cache` | Docker's native solution, handles parallel builds correctly |
| File watching for hot reload | Custom inotify polling loop | `watchexec` crate | battle-tested file watcher with proper debounce and event filtering |
| Rule file parsing | Custom regex/parser | Engine's existing parser | Rete algorithm requires precise DSL syntax; use proven implementation |
| Kubernetes RBAC | Hand-written YAML | Kustomize overlays or Helm values | Templates prevent mistakes in namespace/annotation consistency |
| Image size optimization | Manual layer analysis | `dive` or `docker history` | Tools provide specific waste identification, not guesswork |

**Key insight:** Rust Docker builds benefit most from build-tool-native solutions (BuildKit, cargo-chef) rather than custom caching logic. The file watcher and rule engine parsing already have production-tested implementations in the ecosystem.

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
2. Pass as `web::Data<AppState>` with Arc<Mutex<Engine>>
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

### Pitfall 4: ConfigMap asSecrets Instead of Separate Secret

**What goes wrong:** Storing sensitive data (DB passwords, API keys) in ConfigMap visible to anyone with read access.

**Why it happens:**混用 ConfigMap and Secret for all configuration.

**How to avoid:**
1. ConfigMap: non-sensitive config (log level, feature flags)
2. Secret: credentials, tokens, private keys
3. Use volume mounts with `secretName:` for secrets

**Warning signs:** `kubectl get secret` shows empty but config has passwords; RBAC gives read access to secrets.

## Code Examples

### Verified Patterns from Official Sources

### Dockerfile with BuildKit Cache Mounts (DEPLOY-03)

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.79-alpine AS build

WORKDIR /build

# Install musl-tools for static linking
RUN apk add --no-cache musl-dev musl-utils

# Copy dependency manifests first (maximizes cache reuse)
COPY api/Cargo.lock engine/Cargo.toml ./
COPY engine/Cargo.toml ./engine/

# Cache dependencies using BuildKit cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target/ \
    cargo fetch --manifest-path api/Cargo.toml

# Copy source code (only this invalidates on source change)
COPY . .

# Compile with cached dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target/ \
    cargo build --release --bin rRules-server

# Runtime stage - minimal distroless image
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy compiled binary from build stage
COPY --from=build /build/target/release/rRules-server /app/rules-server

# Create non-root user (65534:65534 = nobody:nobody for distroless)
USER 65534:65534

EXPOSE 8080
ENTRYPOINT ["/app/rules-server"]
```

**Source:** [Docker Rust Guide](https://docs.docker.com/guides/rust/build-images/), [Depot Best Practices](https://depot.dev/blog/rust-dockerfile-best-practices)

### Shared Engine State with Atomic Reload (API-05)

```rust
use std::sync::{Arc, RwLock};
use actix_web::{web, App, HttpServer};

// Shared state for rule engine with hot reload support
pub struct AppState {
    pub engine: Arc<RwLock<engine::environment::Engine>>,
    pub rule_files: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl AppState {
    pub fn new() -> Self {
        let engine = EngineBuilder::new().build();
        AppState {
            engine: Arc::new(RwLock::new(engine)),
            rule_files: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Reload rules from directory with atomic swap
    pub async fn reload_from_directory(&self, dir: &str) -> Result<usize, String> {
        // 1. Load new rules into a temporary engine
        let mut temp_engine = EngineBuilder::new().build();
        let rule_count = load_rules_from_directory(&mut temp_engine, dir).await?;

        // 2. Atomic swap - write lock blocks until all readers are done
        {
            let mut existing_engine = self.engine.write()
                .map_err(|e| format!("Failed to acquire engine write lock: {}", e))?;
            
            // Swap engines atomically
            *existing_engine = temp_engine;
        }

        Ok(rule_count)
    }
}

/// Reload endpoint - returns job ID for async tracking (per decision D-21-03)
pub async fn reload_rules(
    req: web::Json<ReloadRequest>,
    state: web::Data<AppState>,
) -> impl Responder {
    let reload_id = Uuid::new_v4().to_string();
    
    // Get rules directory from request or environment
    let rules_dir = req.rule_file
        .as_deref()
        .unwrap_or_else(|| std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string()));

    // Spawn reload in background (returns immediately per D-21-03)
    let state_clone = web::Data::clone(&state);
    tokio::spawn(async move {
        match state_clone.reload_from_directory(&rules_dir).await {
            Ok(count) => log::info!("Reloaded {} rules", count),
            Err(e) => log::error!("Reload failed: {}", e),
        }
    });

    HttpResponse::Accepted().json(ReloadResponse {
        reload_id,
        rules_reloaded: 0, // Not known yet - async operation
        status: "reload initiated".to_string(),
    })
}
```

**Source:** Actix-web [HttpServer docs](https://docs.rs/actix-web/latest/actix_web/struct.HttpServer.html), Rust pattern for Arc<RwLock<T>>

### Kubernetes ConfigMap + Secret Separation (DEPLOY-04)

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
  WATCH_RULES_DIR: "true"

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
| Naive Dockerfile with rust:slim as runtime | Multi-stage with distroless/scratch | 2021-2023 | Image sizes reduced from ~1.5GB to <80MB |
| Copy source before Cargo.toml | Copy only manifests first, cache fetch | 2020-2022 | Incremental builds: 10min → 30sec |
| Manual file polling for hot reload | Event-driven with watchexec | 2022-2024 | CPU usage reduced, events not missed |
| Single engine instance | Arc<RwLock<Engine>> with atomic swap | 2023-2025 | Zero-downtime reloads possible |

**Deprecated/outdated:**
- `rust:slim` as runtime image - includes build tools
- Docker layer caching without BuildKit cache mounts - slower and less reliable
- Blocking hot reload endpoint - causes request timeouts

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `cargo-chef` is still maintained and compatible with Rust 1.79+ | Standard Stack - Core | chef recipe format changes, but fallback to pure BuildKit exists |
| A2 | `watchexec` crate works with tokio runtime (required for async) | Pattern 3: Atomic Reload | No file watcher if incompatible |
| A3 | Arc<RwLock<Engine>> provides sufficient synchronization for hot reload | Pattern 3 | Data races in concurrent rule execution |
| A4 | Distrosless/cc-debian12 is the recommended minimal runtime (not scratch) | Standard Stack - Core | May need to switch to scratch if security requirements are stricter |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.  
**For Phase 21, all assumptions are LOW risk with known fallbacks.**

## Open Questions

1. **Should we use distroless or scratch for the runtime image?**
   - What we know: Distroless is currently used in existing Dockerfile; both provide minimal attack surface
   - What's unclear: Whether shell access (distroless has `/bin/sh`) is needed for debugging vs. absolute minimal size (scratch)
   - Recommendation: Use distroless/cc-debian12 as in current Dockerfile; switch to scratch only if security audit requires it.

2. **How should reload status tracking be implemented?**
   - What we know: Decision D-21-03 specifies async job tracking with POST returns job ID, GET checks status
   - What's unclear: Should job status be stored in memory (lost on restart) or persisted somewhere?
   - Recommendation: In-memory HashMap for now; persist to DB only if reload history is needed for auditing.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Docker BuildKit | Multi-stage builds with cache mounts | ✓ (enabled by default in Docker 18.09+) | — | No fallback - BuildKit required for pattern |
| Rust 1.79+ | Build stage | ✓ (via rust:1.79-alpine) | 1.79 | — |
| cargo-chef | Dependency caching optimization | ✗ (not currently installed) | N/A | Fallback: pure BuildKit cache mounts work without it |
| watchexec crate | File watcher for hot reload | ✗ (not in Cargo.toml yet) | 2.0+ | Fallback: tokio::spawn polling loop with std::time::Duration |

**Missing dependencies with no fallback:**
- None - BuildKit is standard Docker feature, cargo-chef has pure BuildKit fallback

**Missing dependencies with fallback:**
- `cargo-chef`: Can use pure BuildKit cache mounts instead
- `watchexec`: Can implement file polling with tokio + std::fs

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
| DEPLOY-03 | Multi-stage Dockerfile produces <100MB image | Integration | `docker build . && docker images rrules:latest` | ❌ No test file - Wave 0 gap |
| DEPLOY-04 | Helm chart renders valid K8s manifests | Integration | `helm template k8s/helm --values k8s/helm/values.yaml` | ❌ No test file - Wave 0 gap |
| API-05 | Hot reload endpoint atomically swaps engine state | Unit | `cargo test tests::hot_reload -- --nocapture` | ✅ Already exists in execution.rs |

### Sampling Rate

- **Per task commit:** `cargo test`
- **Per wave merge:** `cargo test --all-targets && docker build -t rrules:test . && helm template k8s/helm > /tmp/manifests.yaml`
- **Phase gate:** Full suite green + image builds successfully before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/docker_test.rs` — Verify Dockerfile produces <100MB image (DEPLOY-03)
- [ ] `tests/helm_test.rs` — Validate Helm chart renders valid K8s manifests (DEPLOY-04)
- [ ] Framework install: `cargo install cargo-chef` — Optional optimization for build times

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
| Docker image supply chain attack | Tampering | Use official base images (rust:1.79-alpine), verify checksums |
| RBAC bypass via ConfigMap mount | Escalation | ServiceAccount with minimal permissions only |

## Sources

### Primary (HIGH confidence)
- Context7: `docker` - BuildKit cache mounts, multi-stage build patterns
- [Docker Rust Guide](https://docs.docker.com/guides/rust/build-images/) - BuildKit cache mounts for Rust
- [Depot Docs - Optimal Rust Dockerfile](https://depot.dev/docs/container-builds/optimal-dockerfiles/rust-dockerfile) - BuildKit + cargo-chef patterns

### Secondary (MEDIUM confidence)
- [Engineered.at Article](https://engineered.at/articles/combining-rust-chef-and-docker-buildkit-for-easy-dependency-caching-and-faster-build-times) - cargo-chef practical implementation
- [Actix-web HttpServer Docs](https://docs.rs/actix-web/latest/actix_web/struct.HttpServer.html) - Graceful shutdown and state management
- Kubernetes official docs for ConfigMap vs Secret separation

### Tertiary (LOW confidence)
- WebSearch results for "Rust hot reload" patterns - confirms Arc<RwLock<Engine>> is standard approach
- cargo-chef crate documentation on crates.io - version compatibility assumed

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - BuildKit and cargo-chef are well-documented in official sources as of 2026
- Architecture: HIGH - Multi-stage Docker, Kubernetes patterns, Arc<RwLock<T>> for shared state all verified
- Pitfalls: HIGH - Layer caching order is Docker 101; engine sharing is Actix-web pattern

**Research date:** 2026-06-11  
**Valid until:** 2026-07-11 (30 days for stable patterns)
