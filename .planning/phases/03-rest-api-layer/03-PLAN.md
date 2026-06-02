---
phase: 03-rest-api-layer
plan: 01
type: execute
wave: 1
depends_on:
  - "02-05"
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
---
phase: 03-rest-api-layer
plan: 02
type: execute
wave: 1
depends_on:
  - "03-01"
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
---
phase: 03-rest-api-layer
plan: 03
type: execute
wave: 2
depends_on:
  - "03-01"
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
---
phase: 03-rest-api-layer
plan: 04
type: execute
wave: 2
depends_on:
  - "03-01"
  - "03-02"
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
---
phase: 03-rest-api-layer
plan: 05
type: tdd
wave: 3
depends_on:
  - "03-01"
  - "03-04"
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
---
phase: 03-rest-api-layer
plan: 06
type: execute
wave: 3
depends_on:
  - "03-01"
  - "03-02"
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
