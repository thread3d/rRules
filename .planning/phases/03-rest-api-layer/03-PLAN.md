---
phase: 03-rest-api-layer
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/api/Cargo.toml
  - src/api/lib.rs
  - src/api/models/mod.rs
  - src/api/models/rules.rs
  - src/api/models/execution.rs
  - src/api/v1/mod.rs
autonomous: true
requirements:
  - API-01
  - API-02
user_setup: []
must_haves:
  truths:
    - External systems can execute rules via POST /v1/rules/execute endpoint with flexible JSON payload
    - Users can create, read, update, and delete rules through RESTful API endpoints at /v1/rules
    - Rule execution auto-detects between facts object (stateless) and session_id (stateful)
    - Full trace output includes execution_id, fired_rules, facts_after, inference_trace for debugging
  artifacts:
    - path: "src/api/Cargo.toml"
      provides: "API crate with actix-web dependency"
      contains: ["actix-web", "serde"]
    - path: "src/api/lib.rs"
      provides: "API crate root with versioned modules"
      exports: ["create_http_server", "App"]
    - path: "src/api/models/rules.rs"
      provides: "Rule CRUD DTOs for request/response"
      contains: ["CreateRuleRequest", "UpdateRuleRequest", "RuleResponse"]
    - path: "src/api/models/execution.rs"
      provides: "Execution request/response DTOs with flexible payload support"
      contains: ["ExecuteRequest", "ExecuteResponse", "InferenceTraceEntry"]
    - path: "src/api/v1/mod.rs"
      provides: "Versioned API router with all endpoints"
      exports: ["configure_v1_routes"]
  key_links:
    - from: "src/api/v1/rules.rs"
      to: "src/engine/core.rs"
      via: "execute_forward/execute_backward calls"
      pattern: "engine::execute_\\w+"
    - from: "src/api/models/execution.rs"
      to: "src/engine/session.rs"
      via: "Session::from_id() call for stateful mode"
      pattern: "session_store\\.get\\("
---

## Phase Goal

**As a** developer or external system, **I want to** execute rules and manage rule lifecycles through RESTful API endpoints, **so that** I can integrate rRules into my applications without modifying their code.

### Purpose

Phase 3 exposes the core rule engine capabilities via HTTP, enabling integration with external systems. This plan establishes the foundational API structure including versioned routing (v1/), request/response models for rules CRUD and execution, and the hybrid REST+GraphQL approach where REST handles CRUD operations while GraphQL will handle rule execution exploration.

### Output

- `/src/api/Cargo.toml` - API crate definition with actix-web dependency
- `/src/api/lib.rs` - API crate root module
- `/src/api/models/` - Request/response DTOs for rules and execution
- `/src/api/v1/` - Versioned HTTP endpoints router

## Context

### Source Files

@.planning/phases/03-rest-api-layer/03-CONTEXT.md (locked decisions D-04, D-05, D-06)
@.planning/ROADMAP.md (Phase 3 requirements API-01, API-02)

### Key Interfaces from Phase 1 Engine

The API must integrate with the rule engine created in Phase 1:

```rust
// From src/engine/core.rs - Rule execution functions
pub fn execute_forward(
    kie_base: &KieBase,
    facts: HashMap<String, Value>,
) -> ExecutionResult;

pub fn execute_backward(
    kie_base: &KieBase,
    query: &str,
    initial_facts: HashMap<String, Value>,
) -> ExecutionResult;
```

### Storage Layer

- Git for rule file versioning (stored in rules repository)
- PostgreSQL for rule metadata and session state
- API layer abstracts storage details behind service functions

## Tasks

<tasks>

<task type="auto">
  <name>Task 1: Create API crate structure with actix-web dependency</name>
  <files>
    src/api/Cargo.toml
    src/api/lib.rs
  </files>
  <action>
Create the API crate structure for Phase 3:

1. Create `src/api/Cargo.toml` with:
   - Package name: "rules-api"
   - Dependencies: actix-web = "4", serde = { version = "1.0", features = ["derive"] }, serde_json = "1.0", thiserror = "2.0"
   - Target type: lib

2. Create `src/api/lib.rs` with:
   - Module declarations for models, v1, middleware
   - Public export of `create_http_server` function that takes config and returns actix_web::HttpServer
   - Error type re-exports from thiserror

This is the crate root that will be built by the main Cargo.toml as a workspace member.
  </action>
  <verify>
File exists at src/api/Cargo.toml with actix-web dependency, src/api/lib.rs exports create_http_server
  </verify>
  <done>Crate compiles successfully with `cargo check`</done>
</task>

<task type="auto">
  <name>Task 2: Define rule CRUD request/response DTOs</name>
  <files>
    src/api/models/rules.rs
  </files>
  <action>
Create the rules DTO module for CRUD operations:

1. Create `src/api/models/rules.rs` with:
   - `CreateRuleRequest`: name, description, language, content, tags (Vec<String>)
   - `UpdateRuleRequest`: same as Create but all fields optional
   - `RuleResponse`: id (UUID), created_at, updated_at, and all request fields
   - `ListRulesQuery`: filter support with optional name, tag, status, page, limit
   - All structs derive Serialize/Deserialize from serde

2. Define custom error types:
   - RuleNotFoundError (404)
   - RuleValidationError (400)
   - StorageError (500)

Use thiserror for clean error definitions matching the Phase 1 engine errors.
  </action>
  <verify>
File exists at src/api/models/rules.rs with all DTOs, compiles without warnings
  </verify>
  <done>All rule CRUD operations have properly typed request/response models</done>
</task>

<task type="auto">
  <name>Task 3: Define execution request/response DTOs with flexible payload support</name>
  <files>
    src/api/models/execution.rs
  </files>
  <action>
Create the execution DTO module supporting both stateful and stateless modes:

1. Create `src/api/models/execution.rs` with:
   - `ExecuteRequest`: Flexible payload that auto-detects mode
     - Either `session_id: String` OR `facts: HashMap<String, Value>` (use serde with custom deserializer)
     - Optional `actions: Vec<String>` for execution control
     - Optional `webhook_url: String` for async callbacks
   - `ExecuteResponse`: Full trace output
     - execution_id: String (UUID prefix "exec_")
     - fired_rules: Vec<RuleSummary>
     - facts_after: HashMap<String, Value>
     - inference_trace: Vec<InferenceTraceEntry>
   - `InferenceTraceEntry`: step_number, rule_name, conditions_met, actions_triggered
   - `ExecuteResponseAsync`: For webhook callbacks - just execution_id and status

2. Implement custom deserializer for ExecuteRequest to handle both session_id and facts modes.
  </action>
  <verify>
File exists at src/api/models/execution.rs with all DTOs including flexible ExecuteRequest that accepts either session_id or facts
  </verify>
  <done>Execute endpoint can accept both stateful (session_id) and Stateless (facts) payloads</done>
</task>

<task type="auto">
  <name>Task 4: Create versioned API router with CRUD endpoints</name>
  <files>
    src/api/v1/rules.rs
  </files>
  <action>
Create the versioned rules API module:

1. Create `src/api/v1/rules.rs` implementing RESTful CRUD:
   - POST /v1/rules - create rule (calls RuleService::create)
   - GET /v1/rules - list rules with filtering (RuleService::list)
   - GET /v1/rules/{id} - get single rule (RuleService::get)
   - PUT /v1/rules/{id} - update rule (RuleService::update)
   - DELETE /v1/rules/{id} - delete rule (RuleService::delete)

2. Each handler:
   - Uses actix-web web::Path and web::Json extractors
   - Returns appropriate HTTP status codes (200, 201, 400, 404)
   - Maps domain errors to HTTP responses using ErrorResponse DTO

3. Import models from crate root: `use crate::models::{CreateRuleRequest, RuleResponse};`
  </action>
  <verify>
File exists at src/api/v1/rules.rs with all 5 CRUD endpoints
  </verify>
  <done>All CRUD operations return correct status codes and properly formatted responses</done>
</task>

<task type="auto">
  <name>Task 5: Create versioned execution endpoint with flexible payload</name>
  <files>
    src/api/v1/execution.rs
  </files>
  <action>
Create the execution API module:

1. Create `src/api/v1/execution.rs` implementing:
   - POST /v1/rules/execute - execute rules with flexible payload

2. Handler logic:
   - Deserialize ExecuteRequest with custom deserializer that checks for session_id presence
   - If session_id present: load session from Redis/db, use existing working memory
   - If facts present: create fresh working memory from provided facts
   - Call Phase 1 engine's execute_forward/execute_backward based on actions parameter
   - Format response as ExecuteResponse with full trace

3. Async handling:
   - If webhook_url provided: spawn background task, return immediately with async response
   - Otherwise: synchronous execution, return full response

4. Error handling for session not found, engine errors
  </action>
  <verify>
File exists at src/api/v1/execution.rs with POST /v1/rules/execute endpoint, handles both facts and session_id modes
  </verify>
  <done>Execute endpoint auto-detects mode and returns full trace output</done>
</task>

<task type="auto">
  <name>Task 6: Wire versioned router into API crate root</name>
  <files>
    src/api/v1/mod.rs
  </files>
  <action>
Create the versioned router module:

1. Create `src/api/v1/mod.rs`:
   - Module declarations for rules, execution
   - Public function `configure_v1_routes(app: &mut web::ServiceConfig)` that registers all v1 endpoints

2. Register routes:
   - POST /v1/rules -> execute rule creation
   - GET /v1/rules -> list rules with filter query params
   - GET /v1/rules/{id} -> get single rule
   - PUT /v1/rules/{id} -> update rule
   - DELETE /v1/rules/{id} -> delete rule
   - POST /v1/rules/execute -> execute rules

3. Export from lib.rs: `pub use v1::configure_v1_routes;`

This is the configuration module that ties everything together.
  </action>
  <verify>
File exists at src/api/v1/mod.rs with route configuration, exports configure_v1_routes
  </verify>
  <done>All v1 routes registered and accessible via http_server</done>
</task>

</tasks>

## Threat Model

### Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Client→API (HTTP) | Untrusted input - request bodies, headers, query params |
| API→Engine | Trusted internal call - validated request data |
| Engine→Storage | Trusted internal - session lookup, rule persistence |

### STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-03-01 | I | ExecuteRequest deserialization | mitigate | Custom deserializer validates only one of session_id or facts present, rejects unknown fields |
| T-03-02 | T | Rule execution endpoint | mitigate | Rate limiting middleware (to be added in Phase 4) |
| T-03-03 | E | Error responses | accept | No sensitive info exposed - generic error messages only |

## Verification

- All API tests pass: `cargo test --package rules-api`
- Server starts without errors: `cargo run --bin rrules`
- Health endpoint responds: `curl http://localhost:8080/health/live`

## Success Criteria

1. API crate compiles and links with engine crate
2. HTTP server starts on configured port (default 8080)
3. All CRUD endpoints respond correctly to valid/invalid requests
4. Execute endpoint accepts both session_id and facts payloads
5. Response includes full trace output (execution_id, fired_rules, facts_after, inference_trace)

---

phase: 03-rest-api-layer
plan: 02
type: execute
wave: 1
depends_on:
  - 03-01
files_modified:
  - src/api/middleware/auth.rs
  - src/api/middleware/logging.rs
autonomous: true
requirements:
  - API-04
user_setup: []
must_haves:
  truths:
    - Session-based authentication validates cookies against active user sessions
    - Authenticated requests have user context available to downstream handlers
    - Unauthenticated requests return 401 Unauthorized
    - Request/response logging records method, path, status code for debugging
  artifacts:
    - path: "src/api/middleware/auth.rs"
      provides: "Session-based authentication middleware"
      exports: ["auth_middleware", "extract_user_from_session"]
    - path: "src/api/middleware/logging.rs"
      provides: "Request/response logging middleware"
      exports: ["logging_middleware"]
  key_links:
    - from: "src/api/v1/rules.rs"
      to: "src/api/middleware/auth.rs"
      via: "@actix_web::middleware" annotation
      pattern: "wrap\\(.*auth_middleware"
---

## Objective

Implement cross-cutting concerns: session-based authentication middleware and request/response logging for the Phase 3 API layer.

### Purpose

Cross-cutting concerns like authentication and logging are essential for production readiness. Session-based auth integrates with Phase 4 RBAC (where user sessions are validated), while logging provides visibility into API usage and debugging.

### Output

- `/src/api/middleware/auth.rs` - Authentication middleware with session validation
- `/src/api/middleware/logging.rs` - Request/response logging middleware

## Tasks

<tasks>

<task type="auto">
  <name>Task 1: Implement session-based authentication middleware</name>
  <files>
    src/api/middleware/auth.rs
  </files>
  <action>
Create the authentication middleware module:

1. Create `src/api/middleware/auth.rs` with:
   - `auth_middleware` - Actix-web middleware that intercepts requests
   - `extract_user_from_session` - Function to validate session cookie and retrieve user

2. Handler logic:
   - Extract "session" cookie from request headers
   - Look up session in Redis (Phase 1 session store) or PostgreSQL session table
   - If valid: attach User context to request extensions for downstream handlers
   - If invalid/missing: return 401 Unauthorized with JSON body

3. Session validation:
   - Check session expiry timestamp
   - Verify session belongs to active user account
   - Support both Redis (in-memory) and PostgreSQL (persistent) backends

4. Error responses:
   - `{"error": "unauthorized", "message": "Invalid or expired session"}` for auth failures
   - No session cookie returned on 401 (clears stale cookies)

This middleware will be applied globally in Phase 3 and extended with role-based checks in Phase 4.
  </action>
  <verify>
File exists at src/api/middleware/auth.rs, compiles without errors
  </verify>
  <done>Middleware correctly validates sessions and returns 401 for invalid/missing sessions</done>
</task>

<task type="auto">
  <name>Task 2: Implement request/response logging middleware</name>
  <files>
    src/api/middleware/logging.rs
  </files>
  <action>
Create the logging middleware module:

1. Create `src/api/middleware/logging.rs` with:
   - `logging_middleware` - Actix-web middleware that logs requests and responses

2. Log format (JSON for structured logging):
```json
{
  "timestamp": "2026-05-24T...",
  "method": "GET",
  "path": "/v1/rules",
  "query": "filter[name]=test&limit=10",
  "user_id": "usr_123",
  "status_code": 200,
  "duration_ms": 15.2
}
```

3. Log levels:
   - INFO: Successful requests (2xx, 3xx)
   - WARN: Client errors (4xx)
   - ERROR: Server errors (5xx)

4. Include correlation ID in logs for request tracing

This provides audit trail and debugging visibility.
  </action>
  <verify>
File exists at src/api/middleware/logging.rs, compiles without errors
  </verify>
  <done>Middleware logs all requests with required fields including duration</done>
</task>

<task type="auto">
  <name>Task 3: Wire middleware into HTTP server configuration</name>
  <files>
    src/api/lib.rs
  </files>
  <action>
Update the API crate root to include middleware:

1. Import middleware modules in `src/api/lib.rs`:
   ```rust
   pub mod middleware;
   ```

2. Create middleware chain function:
   ```rust
   pub fn build_middleware_stack(app: App) -> App {
       app.wrap(middleware::logging::logging_middleware)
          .wrap(middleware::auth::auth_middleware)
   }
   ```

3. In `create_http_server` function, apply middleware to app configuration

4. Export middleware types from lib.rs for testing
  </action>
  <verify>
File at src/api/lib.rs updated with middleware imports and stack builder
  </verify>
  <done>Server starts with both logging and auth middleware applied</done>
</task>

</tasks>

## Threat Model

### Trust Boundaries

| Boundary | Description |
|----------|-------------|
| HTTP headers→Auth | Untrusted cookies must be validated |

### STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-03-04 | T | Session cookie tampering | mitigate | Session validation includes signature verification and expiry check |

## Verification

- Auth middleware rejects requests without valid session: `curl -H "Cookie: session=invalid" http://localhost:8080/v1/rules` returns 401
- Logging middleware outputs JSON to stdout/stderr in development mode

## Success Criteria

1. Middleware chain executes on every request
2. Auth middleware allows requests with valid sessions
3. Auth middleware blocks requests with invalid/missing sessions (401)
4. Logging produces structured JSON output with all required fields

---

phase: 03-rest-api-layer
plan: 03
type: execute
wave: 2
depends_on:
  - 03-01
files_modified:
  - openapi/v1/openapi.yaml
autonomous: true
requirements:
  - API-03
user_setup: []
must_haves:
  truths:
    - OpenAPI/Swagger spec documents all /v1 endpoints
    - Each endpoint specifies request body schema and response schemas
    - Authentication requirement is documented (session cookie)
    - Response examples included for common scenarios
  artifacts:
    - path: "openapi/v1/openapi.yaml"
      provides: "OpenAPI 3.0 specification for v1 API"
      contains: ["openapi: 3.0.0", "/v1/rules:", "/v1/rules/execute:"]
  key_links:
    - from: "src/api/models/rules.rs"
      to: "openapi/v1/openapi.yaml"
      via: "DTOs match schema definitions"
      pattern: "#/components/schemas/.*Response"
---

## Objective

Generate comprehensive OpenAPI/Swagger specification documenting all Phase 3 API endpoints for developer documentation and tooling.

### Purpose

OpenAPI spec enables automatic client generation, API testing tools (Swagger UI), and documentation generation. This plan creates the v1 specification that will be versioned alongside the API implementation.

### Output

- `/openapi/v1/openapi.yaml` - OpenAPI 3.0 specification document

## Tasks

<tasks>

<task type="auto">
  <name>Task 1: Create complete OpenAPI v1 specification</name>
  <files>
    openapi/v1/openapi.yaml
  </files>
  <action>
Create the OpenAPI specification file:

1. Set up base document structure:
   - openapi: 3.0.0
   - info.title: "rRules API"
   - info.version: "1.0.0"
   - servers with /v1 path

2. Components section with schemas:
   - CreateRuleRequest, UpdateRuleRequest, RuleResponse (from models/rules.rs)
   - ExecuteRequest, ExecuteResponse, InferenceTraceEntry (from models/execution.rs)
   - ErrorResponse for error handling
   - ListRulesQuery for filter parameters

3. Paths section:
   - POST /rules: create rule
   - GET /rules: list rules with query params
   - GET /rules/{id}: get single rule
   - PUT /rules/{id}: update rule
   - DELETE /rules/{id}: delete rule
   - POST /rules/execute: execute rules

4. For each endpoint:
   - summary and description
   - request body schema (or query params)
   - all response codes with example payloads
   - authentication requirement marked

5. Security scheme definition for session cookie auth

6. Include response examples:
```yaml
# Example successful rule creation response
201:
  content:
    application/json:
      example:
        id: "rule_abc123"
        name: "Fraud Detection"
        status: "active"
```

Use YAML format with clear documentation strings for each field.
  </action>
  <verify>
File exists at openapi/v1/openapi.yaml, contains all endpoints and schemas
  </verify>
  <done>OpenAPI spec validates against schema and includes examples</done>
</task>

<task type="auto">
  <name>Task 2: Add health endpoint specification</name>
  <files>
    openapi/v1/openapi.yaml
  </files>
  <action>
Extend the OpenAPI spec with health endpoints:

1. Add /health/live path:
   - GET returns 200 with {"status": "ok"}
   - No authentication required

2. Add /health/ready path:
   - GET returns 200 when ready to accept traffic
   - Returns 503 if database/Redis unavailable
   - Response includes dependency status

3. Include response schemas for health endpoints in components

This completes the documented API surface.
  </action>
  <verify>
Health endpoints included in OpenAPI spec with correct responses
  </verify>
  <done>OpenAPI spec includes all API and health endpoints</done>
</task>

</tasks>

## Verification

- OpenAPI spec validates: `openapi-generator-cli validate -i openapi/v1/openapi.yaml`
- Swagger UI can render the spec (optional, during testing)

## Success Criteria

1. OpenAPI spec is syntactically valid YAML
2. All Phase 3 endpoints documented with request/response examples
3. Authentication requirement clearly marked on protected endpoints

---

phase: 03-rest-api-layer
plan: 04
type: execute
wave: 2
depends_on:
  - 03-01
  - 03-02
files_modified:
  - Dockerfile
  - config/production.yaml
  - src/bin/server.rs
autonomous: true
requirements:
  - DEPLOY-01
user_setup: []
must_haves:
  truths:
    - Docker image uses distroless base (no shell/pkg manager)
    - Container exposes only required port (8080)
    - Application starts as non-root user
    - Separate /health/live and /health/ready endpoints implemented
    - Configuration loaded from mounted config file or environment variables
  artifacts:
    - path: "Dockerfile"
      provides: "Production Docker image with distroless base"
      contains: ["FROM gcr.io/distroless"]
    - path: "config/production.yaml"
      provides: "Production configuration (port, logging, database)"
    - path: "src/bin/server.rs"
      provides: "Binary entry point for server"
  key_links:
    - from: "Dockerfile"
      to: "openapi/v1/openapi.yaml"
      via: "API serves OpenAPI at /swagger-ui"
---

## Objective

Create production-ready Docker deployment configuration with distroless base image and health endpoints.

### Purpose

Container deployment is essential for Kubernetes orchestration. Distroless images provide the smallest attack surface by excluding shell and package manager. Health endpoints enable proper K8s lifecycle management.

### Output

- `/Dockerfile` - Production Docker build
- `/config/production.yaml` - Configuration file
- `/src/bin/server.rs` - Binary entry point

## Tasks

<tasks>

<task type="auto">
  <name>Task 1: Create production Dockerfile with distroless base</name>
  <files>
    Dockerfile
  </files>
  <action>
Create the production Dockerfile:

1. Multi-stage build:
```dockerfile
# Build stage
FROM rust:1.79-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin rrules

# Production stage - distroless base
FROM gcr.io/distroless/base-debian12
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/rrules .

# Create non-root user (distroless compatible approach)
USER 65532:65532

# Expose only required port
EXPOSE 8080

# Health check command (exec format, no shell)
HEALTHCHECK --interval=10s --timeout=3s \
  CMD ["wget", "--quiet", "--tries=1", "--spider", "http://localhost:8080/health/live"] || exit 1

ENTRYPOINT ["/app/rrules"]
```

2. Key security properties:
   - No shell (dash/sh not present)
   - No package manager
   - Non-root user
   - Minimal attack surface

3. Note: wget needed for health check; if unavailable, use alternative approach
  </action>
  <verify>
File exists at Dockerfile with distroless base image and multi-stage build
  </verify>
  <done>Docker build succeeds and creates minimal production image</done>
</task>

<task type="auto">
  <name>Task 2: Create production configuration file</name>
  <files>
    config/production.yaml
  </files>
  <action>
Create the production configuration:

```yaml
# Server configuration
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

# Logging configuration
logging:
  level: "info"
  format: "json"

# Database (PostgreSQL)
database:
  url: "${DATABASE_URL}"
  pool_size: 10

# Session store (Redis)
sessions:
  redis_url: "${REDIS_URL}"

# Feature flags
features:
  graphql_enabled: true
  async_execution_enabled: true
```

1. Use environment variable substitution for secrets
2. Production-tuned defaults (workers, pool size)
3. JSON logging format for structured output

Create config directory structure if needed.
  </action>
  <verify>
File exists at config/production.yaml with all required settings
  </verify>
  <done>Configuration file loaded successfully on startup</done>
</task>

<task type="auto">
  <name>Task 3: Create server binary entry point</name>
  <files>
    src/bin/server.rs
  </files>
  <action>
Create the server binary:

1. Create `src/bin/server.rs`:
```rust
use actix_web::HttpServer;
use rules_api::{create_http_server, build_middleware_stack};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load configuration (from env or config file)
    let config = load_config();
    
    // Create HTTP server
    HttpServer::new(move || {
        build_middleware_stack(
            App::new()
                .configure(configure_v1_routes)
        )
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
```

2. Add health endpoint registration (separate from v1 for no-auth requirement):
```rust
app.route("/health/live", web::get().to(health_live))
 .route("/health/ready", web::get().to(health_ready));
```

3. Implement health handlers:
   - `/health/live`: Simple OK response, always returns 200
   - `/health/ready`: Checks database connectivity and Redis availability

This binary will be compiled as the production server.
  </action>
  <verify>
File exists at src/bin/server.rs with HTTP server setup and health endpoints
  </verify>
  <done>Binary compiles and starts successfully</done>
</task>

<task type="auto">
  <name>Task 4: Create Kubernetes manifests for orchestration</name>
  <files>
    k8s/deployment.yaml
    k8s/service.yaml
    k8s/ingress.yaml
  </files>
  <action>
Create Kubernetes deployment resources:

1. `k8s/deployment.yaml`:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rrules
spec:
  replicas: 3
  selector:
    matchLabels:
      app: rrules
  template:
    metadata:
      labels:
        app: rrules
    spec:
      containers:
      - name: rrules
        image: rrules:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: rrules-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: rrules-secrets
              key: redis-url
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

2. `k8s/service.yaml`:
```yaml
apiVersion: v1
kind: Service
metadata:
  name: rrules-service
spec:
  selector:
    app: rrules
  ports:
  - port: 80
    targetPort: 8080
  type: ClusterIP
```

3. `k8s/ingress.yaml` (Nginx Ingress):
```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: rrules-ingress
  annotations:
    kubernetes.io/ingress.class: nginx
spec:
  rules:
  - host: rrules.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: rrules-service
            port:
              number: 80
```

4. Add Secret manifest for credentials (with placeholder values)

These manifests enable K8s deployment with proper health checks and ingress.
  </action>
  <verify>
Files exist at k8s/deployment.yaml, k8s/service.yaml, k8s/ingress.yaml
  </verify>
  <done>All K8s resources deploy successfully</done>
</task>

<task type="auto">
  <name>Task 5: Build hot deployment mechanism for rule updates</name>
  <files>
    src/api/hot_reload.rs
    src/storage/rule_store.rs
  </files>
  <action>
Implement hot deployment for rule updates without service restart:

1. Create `src/api/hot_reload.rs`:
   - `ReloadRulesRequest` and `ReloadRulesResponse` DTOs
   - `POST /v1/rules/reload` endpoint to trigger reload

2. Implement in RuleStore:
   - `reload_rules()` method that:
     - Reads rule files from Git repository
     - Rebuilds KIE base with new rules
     - Updates shared state atomically (RwLock or similar)

3. Hot reload process:
   - Load new rules into temporary store
   - Validate rules against engine
   - Atomically swap pointer to live store
   - Return success/failure response

4. Optional: Watch file changes for automatic reload in dev mode

This enables rule updates without container restart.
  </action>
  <verify>
File exists at src/api/hot_reload.rs with reload endpoint, compiles successfully
  </verify>
  <done>Hot reload endpoint triggers rule recompilation without restart</done>
</task>

</tasks>

## Threat Model

### Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Container→Host filesystem | Limited by distroless constraints |

### STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-03-05 | E | Container image supply chain | mitigate | Use pinned base images, verify digests in CI/CD |
| T-03-06 | D | Health endpoint flood | accept | Rate limiting applied at ingress layer |

## Verification

- Docker build succeeds: `docker build -t rrules:test .`
- Container starts with health checks: `docker run --rm -p 8080:8080 rrules:test`
- /health/live returns 200
- /health/ready returns 200 (or 503 if dependencies unavailable)

## Success Criteria

1. Docker image built successfully with distroless base
2. Container runs without root privileges
3. Health endpoints respond correctly
4. Kubernetes manifests validate: `kubectl apply --dry-run=client -f k8s/`

---

phase: 03-rest-api-layer
plan: 05
type: tdd
wave: 3
depends_on:
  - 03-01
  - 03-04
files_modified:
  - tests/integration/api_rules.rs
  - tests/integration/api_execution.rs
autonomous: true
requirements:
  - API-01
  - API-02
user_setup: []
must_haves:
  truths:
    - Integration tests cover all CRUD operations against /v1/rules endpoints
    - Rule execution endpoint accepts both facts and session_id modes
    - Full trace response is correctly formatted
    - Webhook callbacks trigger for async execution
    - Bulk operations work with and without transaction mode
  artifacts:
    - path: "tests/integration/api_rules.rs"
      provides: "Rule CRUD integration tests"
      min_lines: 50
    - path: "tests/integration/api_execution.rs"
      provides: "Execution endpoint integration tests"
      min_lines: 80
  key_links:
    - from: "src/api/v1/rules.rs"
      to: "tests/integration/api_rules.rs"
      via: "test suite validates all endpoints"
---

## Objective

Build comprehensive integration test suite for Phase 3 API endpoints covering CRUD operations, rule execution modes, and async callbacks.

### Purpose

Integration tests verify the complete HTTP stack works correctly with the underlying engine. TDD approach ensures tests drive implementation and catch integration issues early.

### Output

- `/tests/integration/api_rules.rs` - Rule CRUD test suite
- `/tests/integration/api_execution.rs` - Execution endpoint test suite

## Tasks

<tasks>

<task type="auto">
  <name>Task 1: Write failing rule CRUD integration tests</name>
  <files>
    tests/integration/api_rules.rs
  </files>
  <behavior>
# Integration Tests for Rule CRUD Endpoints

## Test Cases

### Create Rule (POST /v1/rules)
- Test: Valid request creates rule with 201 status
  - Input: {name, description, language, content}
  - Expected: Response body includes id and created_at

- Test: Missing required field returns 400
  - Input: {name} without content
  - Expected: Error response with validation message

### List Rules (GET /v1/rules)
- Test: Empty filter returns all rules
- Test: Filter by name works correctly
- Test: Filter by tag works correctly
- Test: Pagination parameters work (page, limit)

### Get Single Rule (GET /v1/rules/{id})
- Test: Valid ID returns rule details
- Test: Invalid UUID returns 404

### Update Rule (PUT /v1/rules/{id})
- Test: Full update replaces all fields
- Test: Partial update works (only specified fields change)

### Delete Rule (DELETE /v1/rules/{id})
- Test: Valid ID deletes rule, returns 204
- Test: Invalid ID returns 404

## Setup/Teardown
- Each test creates fresh in-memory database or uses unique rule names
- Cleanup removes created rules after each test
</behavior>
  <action>
Create integration tests for CRUD endpoints:

1. Set up actix-web test server with test configuration:
```rust
#[actix_web::test]
async fn test_create_rule() {
    let app = init_app().await;
    // Test implementation
}
```

2. Write tests for each CRUD operation as specified in behavior block

3. Use `awc::Client` to make HTTP requests
4. Validate response status codes and JSON bodies
5. Cleanup: Drop database or use unique test data per suite

Run tests with: `cargo test --test integration api_rules`
  </action>
  <verify>
All CRUD tests pass after implementation: `cargo test --test integration`
  </verify>
  <done>All rule CRUD operations tested and working</done>
</task>

<task type="auto">
  <name>Task 2: Write failing execution endpoint integration tests</name>
  <files>
    tests/integration/api_execution.rs
  </files>
  <behavior>
# Integration Tests for Rule Execution

## Test Cases

### Stateless Execution (facts mode)
- Test: Valid facts execute rules correctly
  - Input: {facts: {...}, actions: ["fire"]}
  - Expected: execution_id, fired_rules, facts_after in response

- Test: Multiple rules fire and facts are updated correctly

### Stateful Execution (session_id mode)
- Test: Session with existing working memory is used
  - First create session via POST /sessions
  - Then execute with {session_id, actions}
  - Expected: Rules use session's existing facts

### Full Trace Output
- Test: inference_trace includes step-by-step execution
  - Each entry has rule_name, conditions_met, actions_triggered

### Async with Webhook
- Test: Async request returns immediately with async response
  - Input: {facts, webhook_url}
  - Expected: Only execution_id in response

- Test: Webhook callback posts results when complete
  - Set up mock webhook endpoint using test server
  - Execute with that URL
  - Verify POST received with full result

### Bulk Operations (POST /v1/rules/bulk)
- Test: Multiple rules created atomically (transaction=true)
- Test: One failure rolls back all changes (transaction=true)
- Test: Individual creation works (transaction=false)

## Setup/Teardown
- Test server simulates webhook callbacks
- Sessions created in isolated database rows
</behavior>
  <action>
Create integration tests for execution endpoint:

1. Set up test server with Phase 1 engine integration

2. Write tests for:
   - Stateless mode with facts object
   - Stateful mode with session_id
   - Full trace response structure validation
   - Async webhook callback verification
   - Bulk operations with/without transaction

3. For webhook tests, start local HTTP server to receive callbacks

4. Validate JSON response structure matches ExecuteResponse DTO

Run tests with: `cargo test --test integration api_execution`
  </action>
  <verify>
All execution tests pass after implementation
  </verify>
  <done>Execution endpoint handles all modes correctly with full trace output</done>
</task>

<task type="auto">
  <name>Task 3: Add error handling and edge case tests</name>
  <files>
    tests/integration/api_errors.rs
  </files>
  <action>
Create tests for error scenarios:

1. Authentication errors (401)
   - Missing session cookie
   - Invalid session
   - Expired session

2. Validation errors (400)
   - Malformed JSON
   - Invalid UUID format
   - Rule content syntax errors

3. Resource not found (404)
   - Non-existent rule ID
   - Non-existent session ID

4. Server errors (500)
   - Engine execution failures
   - Storage connection errors

5. Rate limiting tests (to be implemented in Phase 4)

Each test verifies error response format includes appropriate fields.
  </action>
  <verify>
Error handling tests pass and return correct status codes
  </verify>
  <done>All error scenarios handled with proper HTTP status codes</done>
</task>

</tasks>

## Verification

- All integration tests pass: `cargo test --test integration`
- Test coverage reported: `cargo tarpaulin --tests`

## Success Criteria

1. 90%+ code coverage for API endpoints
2. All error cases return correct HTTP status codes
3. Async webhook callback mechanism verified

---

phase: 03-rest-api-layer
plan: 06
type: execute
wave: 3
depends_on:
  - 03-01
  - 03-02
files_modified:
  - src/api/v1/graphql.rs
autonomous: true
requirements:
  - API-02
user_setup: []
must_haves:
  truths:
    - GraphQL endpoint at /v1/graphql provides rule execution exploration
    - Query to list all rules with filtering and pagination
    - Query to get single rule details including version history
    - Mutation for executing rules with flexible payload support
    - Full trace output available in GraphQL response
  artifacts:
    - path: "src/api/v1/graphql.rs"
      provides: "GraphQL schema and resolvers"
      contains: ["build_schema", "execute"]
  key_links:
    - from: "src/api/models/execution.rs"
      to: "src/api/v1/graphql.rs"
      via: "ExecuteResponse used in GraphQL schema"
---

## Objective

Implement GraphQL endpoint for rule execution exploration as specified by the hybrid REST+GraphQL approach (D-04), complementing REST CRUD operations.

### Purpose

The GraphQL endpoint enables flexible querying of rules and exploration of execution results. While REST handles standard CRUD, GraphQL provides a richer interface for complex data fetching and execution exploration scenarios.

### Output

- `/src/api/v1/graphql.rs` - GraphQL schema and resolvers

## Tasks

<task type="auto">
  <name>Task 1: Design and implement GraphQL schema</name>
  <files>
    src/api/v1/graphql.rs
  </files>
  <action>
Create the GraphQL implementation:

1. Define schema types matching existing DTOs:
   - `Rule` type with fields from RuleResponse
   - `ExecuteRequest` input type with session_id or facts (union-like via input object)
   - `ExecuteResponse` type with execution_id, fired_rules, facts_after, inference_trace

2. Query operations:
   - `rules(filter: RuleFilter, pagination: Pagination): [Rule!]!`
   - `rule(id: ID!): Rule`

3. Mutation operations:
   - `execute(request: ExecuteRequestInput!): ExecuteResponse!`

4. Resolver implementation:
   - Use existing Phase 1 engine functions
   - Map GraphQL types to DTOs
   - Return same trace format as REST endpoint

5. Error handling:
   - GraphQL errors for validation failures
   - Proper status codes via error extensions

The schema should be consistent with the REST API models while leveraging GraphQL's strengths in flexible data fetching.
  </action>
  <verify>
GraphQL schema compiles and exports build_schema function
  </verify>
  <done>GraphQL endpoint provides rule query and execute mutation</done>
</task>

<task type="auto">
  <name>Task 2: Wire GraphQL endpoint into v1 router</name>
  <files>
    src/api/v1/mod.rs
  </files>
  <action>
Update the router to include GraphQL:

1. Import graphql module in `src/api/v1/graphql.rs`:
   ```rust
   pub mod graphql;
   ```

2. Register route in `configure_v1_routes`:
   - POST /v1/graphql -> graphql::execute_graphql

3. Ensure middleware (auth, logging) applies to GraphQL endpoint

4. Return consistent error format between REST and GraphQL

The hybrid approach is now complete with both endpoints available.
  </action>
  <verify>
GraphQL route registered and accessible at /v1/graphql
  </verify>
  <done>Hybrid REST+GraphQL API complete</done>
</task>

</tasks>

## Verification

- GraphQL endpoint responds to valid queries: `curl -X POST http://localhost:8080/v1/graphql`
- Query rules with filtering works correctly
- Execute mutation returns full trace output

## Success Criteria

1. GraphQL schema compiles and loads successfully
2. All rule queries return expected data structures
3. Execution mutation supports both session_id and facts modes

---

phase: 03-rest-api-layer
plan: 07
type: tdd
wave: 4
depends_on:
  - 03-01
  - 03-02
  - 03-05
files_modified:
  - tests/integration/api_graphql.rs
autonomous: true
requirements:
  - API-02
user_setup: []
must_haves:
  truths:
    - GraphQL endpoint executes all query and mutation operations correctly
    - Rule filtering works in GraphQL queries
    - Full trace output available via GraphQL execution
    - Error responses follow GraphQL error format
  artifacts:
    - path: "tests/integration/api_graphql.rs"
      provides: "GraphQL integration tests"
      min_lines: 60
---

## Objective

Build comprehensive integration test suite for the GraphQL endpoint to verify all query and mutation operations work correctly.

### Purpose

Verify GraphQL implementation matches REST behavior while testing GraphQL-specific features like complex filtering and nested data fetching.

### Output

- `/tests/integration/api_graphql.rs` - GraphQL integration tests

## Tasks

<task type="auto">
  <name>Task: Write failing GraphQL endpoint integration tests</name>
  <files>
    tests/integration/api_graphql.rs
  </files>
  <behavior>
# Integration Tests for GraphQL Endpoint

## Test Cases

### Query Operations

- Test: Query all rules with default pagination
- Test: Query rules with filter arguments (name, tag, status)
- Test: Query single rule by ID
- Test: Invalid ID returns GraphQL error (not 404)

### Mutation Operations

- Test: Execute mutation with facts object
- Test: Execute mutation with session_id
- Test: Verify trace output structure in response
- Test: Async execution with webhook_url triggers callback

### Error Handling

- Test: Missing authentication returns GraphQL error
- Test: Invalid rule content returns validation error
- Test: Syntax errors return proper GraphQL errors

## Setup/Teardown
- Use same test server as REST tests
- Clear database between test suites
</behavior>
  <action>
Create integration tests for GraphQL endpoint:

1. Set up actix-web test server with GraphQL middleware

2. Write tests using HTTP requests to /v1/graphql with JSON body:
```rust
let body = r#"{
  query: "query { rules { id name } }"
}"#;
```

3. Validate:
   - Correct data in `response.data`
   - Errors in `response.errors` when applicable

4. Test both REST-style execution and GraphQL-specific queries

Run tests with: `cargo test --test integration api_graphql`
  </action>
  <verify>
All GraphQL tests pass after implementation
  </verify>
  <done>GraphQL endpoint fully tested and working</done>
</task>

</tasks>

## Verification

- All GraphQL tests pass: `cargo test --test integration api_graphql`

## Success Criteria

1. All GraphQL queries return expected data structures
2. All mutations work correctly with full trace output
3. Error responses follow GraphQL error format

---

phase: 03-rest-api-layer
plan: 08
type: execute
wave: 5
depends_on:
  - 03-01
  - 03-04
files_modified:
  - docs/api.md
autonomous: true
requirements:
  - API-03
user_setup: []
must_haves:
  truths:
    - Complete API documentation includes all endpoints
    - Request/response examples for each endpoint
    - Authentication instructions
    - Error response reference
  artifacts:
    - path: "docs/api.md"
      provides: "Human-readable API documentation"
      contains: ["REST API Endpoints", "GraphQL Endpoint", "Authentication"]
---

## Objective

Create comprehensive API documentation that serves as the primary reference for developers integrating with rRules.

### Purpose

Well-documented APIs enable faster integration and reduce support burden. Documentation should cover REST, GraphQL, authentication, error handling, and deployment considerations.

### Output

- `/docs/api.md` - Complete API documentation

## Tasks

<task type="auto">
  <name>Task: Create comprehensive API documentation</name>
  <files>
    docs/api.md
  </files>
  <action>
Create the API documentation:

1. Structure:
   - Introduction (what rRules is, use cases)
   - Getting Started (setup, first request)
   - Authentication (session-based flow)
   - REST API
     - Overview
     - Rules CRUD endpoints
     - Execution endpoint
     - Bulk operations
   - GraphQL Endpoint
     - Schema overview
     - Query examples
     - Mutation examples
   - Error Handling
     - Status codes
     - Error response format
     - Common errors
   - Async Operations (webhook callbacks)
   - Deployment Notes (Docker, K8s)

2. Each endpoint includes:
   - HTTP method and path
   - Request parameters/body
   - Response structure with examples
   - Authentication requirement

3. Include cURL examples for quick testing:
```bash
curl -X POST http://localhost:8080/v1/rules \
  -H "Cookie: session=..." \
  -d '{"name": "Test Rule", ...}'
```

4. Link to OpenAPI spec for machine-readable documentation

The docs should be comprehensive but not overwhelming.
  </action>
  <verify>
Documentation file exists at docs/api.md with all sections
  </verify>
  <done>API documentation complete and accurate</done>
</task>

</tasks>

## Verification

- Documentation renders correctly as Markdown
- All examples are tested and functional
- OpenAPI link points to correct spec file

## Success Criteria

1. Documentation covers all Phase 3 endpoints
2. Examples work when copy-pasted
3. Authentication flow clearly explained

---

phase: 03-rest-api-layer
plan: 09
type: checkpoint:human-verify
wave: 6
depends_on:
  - 03-01
  - 03-02
  - 03-03
  - 03-04
  - 03-05
  - 03-06
autonomous: false
requirements:
  - API-01
  - API-02
  - API-03
  - DEPLOY-01
user_setup: []
must_haves:
  truths:
    - All Phase 3 endpoints working correctly
    - Docker container runs successfully
    - Health endpoints respond properly
    - Full trace output in execution responses
  artifacts:
    - path: "docs/api.md"
      provides: "API documentation reference"
---

## Objective

Final verification that Phase 3 implementation meets all requirements and is ready for production deployment.

### Purpose

Human verification catches issues that automated tests may miss, particularly around visual interfaces, API behavior, and deployment readiness.

## Tasks

<task type="checkpoint:human-verify">
  <what-built>
Phase 3: REST API Layer - Complete Implementation

**REST Endpoints:**
- POST /v1/rules - Create rule
- GET /v1/rules - List rules with filtering
- GET /v1/rules/{id} - Get single rule
- PUT /v1/rules/{id} - Update rule
- DELETE /v1/rules/{id} - Delete rule
- POST /v1/rules/execute - Execute rules (stateful/stateless)
- POST /v1/rules/bulk - Bulk operations with transaction option

**GraphQL Endpoint:**
- POST /v1/graphql - Query/mutate rules and execute

**Deployment:**
- Docker container with distroless base
- Health endpoints at /health/live and /health/ready
- Hot reload endpoint for rule updates without restart
</what-built>
  <how-to-verify>
1. **Start the server:**
   - Build: `cargo build --release`
   - Run: `DATABASE_URL=... REDIS_URL=... cargo run --release`

2. **Test health endpoints:**
   ```
   curl http://localhost:8080/health/live
   # Should return {"status": "ok"}

   curl http://localhost:8080/health/ready
   # Should return {"status": "ok", "dependencies": {...}}
   ```

3. **Test rule creation (stateless):**
   ```
   curl -X POST http://localhost:8080/v1/rules \
     -H "Cookie: session=..." \
     -d '{"name":"Test Rule","language":"rust","content":"fn evaluate() -> bool { true }"}'
   ```

4. **Test rule execution (stateless):**
   ```
   curl -X POST http://localhost:8080/v1/rules/execute \
     -H "Cookie: session=..." \
     -d '{"facts":{"input":42},"actions":["fire"]}'
   # Should return: {execution_id, fired_rules, facts_after, inference_trace}
   ```

5. **Test rule execution (stateful):**
   ```
   curl -X POST http://localhost:8080/v1/sessions
   # Copy session_id
   
   curl -X POST http://localhost:8080/v1/rules/execute \
     -H "Cookie: session=..." \
     -d '{"session_id":"...", "actions":["fire"]}'
   ```

6. **Test GraphQL:**
   ```
   curl -X POST http://localhost:8080/v1/graphql \
     -H "Cookie: session=..." \
     -d '{"query":"{ rules { id name } }"}'
   ```

7. **Verify Docker build:**
   ```
   docker build -t rrules:test .
   docker run --rm -p 8080:8080 rrules:test
   # Verify health endpoints work in container
   ```

8. **Test hot reload:**
   ```
   curl -X POST http://localhost:8080/v1/rules/reload \
     -H "Cookie: session=..."
   ```
  </how-to-verify>
  <resume-signal>Approve if all tests pass, or describe issues found</resume-signal>
</task>

</tasks>

## Verification

All manual verification steps completed successfully.

## Success Criteria

1. All endpoints return expected responses
2. Docker container runs without errors
3. Health endpoints respond correctly
4. Full trace output visible in execution results

---

phase: 03-rest-api-layer
plan: 10
type: checkpoint:decision
wave: 6
depends_on:
  - 03-09
autonomous: false
requirements:
  - API-04
user_setup: []
must_haves:
  truths:
    - Deployment configuration matches Phase 3 requirements
    - Production readiness verified
  artifacts:
    - path: "k8s/deployment.yaml"
      provides: "Kubernetes deployment configuration"
---

## Objective

Human decision checkpoint for production deployment approval and final configuration decisions.

### Purpose

Final go/no-go decision before declaring Phase 3 complete. Ensures all stakeholders confirm the implementation meets requirements.

## Tasks

<task type="checkpoint:decision">
  <decision>
Phase 3 Deployment Decision

**Context:** The REST API Layer (Phase 3) is complete with:
- Hybrid REST + GraphQL API
- URL path versioning (/v1/...)
- Session-based authentication (integration ready, Phase 4 RBAC)
- Flexible execution payload (facts or session_id auto-detection)
- Full trace output response format
- Webhook callbacks for async jobs
- Docker deployment with distroless base image
- Health endpoints (/health/live, /health/ready)
- Kubernetes manifests with Nginx Ingress

**Options:**
</decision>
  <context>
Phase 3 must ship to enable Phase 4 (Governance & Access Control) which depends on the API layer for rule management.
</context>
  <options>
    <option id="deploy">
      <name>Deploy Phase 3 to Production</name>
      <pros>
        All requirements met per CONTEXT.md and ROADMAP.md
        Hybrid REST+GraphQL approach implemented
        Security features (session auth, distroless image) in place
        OpenAPI spec generated for documentation
        Kubernetes deployment ready
      </pros>
      <cons>
        None identified - implementation verified
      </cons>
    </option>
    <option id="delay">
      <name>Delay Deployment - Fix Issues</name>
      <pros>
        Address any discovered issues before production
      </pros>
      <cons>
        Delays Phase 4 (Governance & Access Control)
        May block downstream integration work
      </cons>
    </option>
  </options>
  <resume-signal>Select: deploy, delay</resume-signal>
</task>

</tasks>

## Verification

Human approval obtained to proceed with production deployment.

## Success Criteria

Deployment decision made and documented.
---

phase: 03-rest-api-layer
plan: 10
type: execute
wave: 6
depends_on:
  - 03-08
files_modified:
  - .planning/ROADMAP.md
autonomous: true
requirements:
  - API-04
user_setup: []
must_haves:
  truths:
    - Phase 3 completion documented in ROADMAP
    - All plan files created and committed
    - Deployment configuration verified
  artifacts:
    - path: ".planning/ROADMAP.md"
      provides: "Updated project roadmap with Phase 3 complete"
---

## Objective

Complete Phase 3 planning by updating the roadmap to reflect completion and committing all plan files.

### Purpose

Formalize Phase 3 completion for tracking and enable progression to Phase 4 (Governance & Access Control).

### Output

- Updated `.planning/ROADMAP.md` with Phase 3 status
- All PLAN.md files committed to git

## Tasks

<task type="auto">
  <name>Task: Update ROADMAP.md and commit plan files</name>
  <files>
    .planning/ROADMAP.md
  </files>
  <action>
Finalize Phase 3:

1. Update `.planning/ROADMAP.md`:
   - Mark Phase 3 as complete in the progress table
   - Update plans section to show all 9 plans with checkboxes checked

2. Commit all PLAN.md files:
   ```
   .planning/phases/03-rest-api-layer/03-PLAN.md (this file)
   ```

3. Add to git history for traceability

This marks the end of planning for Phase 3.
  </action>
  <verify>
ROADMAP.md updated, plan files committed to git
  </verify>
  <done>Phase 3 marked complete in roadmap</done>
</task>

</tasks>

## Verification

- ROADMAP.md shows Phase 3 as planning complete
- All PLAN.md files present and committed

## Success Criteria

1. ROADMAP.md accurately reflects Phase 3 completion status
2. Git history includes all plan files
3. Next phase (Phase 4) can begin planning