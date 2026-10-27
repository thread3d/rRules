# rRules REST API Reference

Manual reference for the rRules REST API (actix-web, default `http://127.0.0.1:8080`).
This document covers behavior, schemas, and the auth/tenancy model that the
generated OpenAPI spec (A6b, `/api-docs/openapi.json`) cannot easily express in
prose. For the live spec, see Swagger UI at `/swagger-ui`.

## Conventions

- **Base URL:** `http://127.0.0.1:8080`. All business endpoints live under `/v1`;
  health and metrics are root-level.
- **Content type:** request bodies are `application/json`; responses are JSON
  unless noted (metrics Prometheus text, audit CSV export).
- **IDs:** rules use UUID `id`; categories use UUID; tags are keyed by `name`;
  branches are keyed by `name`; approvals use UUID `approval_id`.
- **Pagination:** list endpoints accept `page` (1-indexed) and `page_size`
  query params and return `total_count` + `total_pages`.
- **Errors:** non-2xx responses return a JSON body, typically
  `{"error": "<message>"}` (some endpoints add `"status_code": <n>`).
- **Auth:** JWT (HS256) is enforced on the whole `/v1` scope by
  `middleware::auth::requires_auth`. Send `Authorization: Bearer <token>` on every
  `/v1` request. The public allowlist is `/v1/auth/login`, `/health`,
  `/swagger-ui`, `/api-docs`, `/metrics` and `/v1/metrics/ws` (a browser
  WebSocket upgrade cannot carry an Authorization header, and the stream is
  read-only). See the **Authentication** section below.
- **Multi-tenancy:** rule CRUD is scoped by the `X-Tenant-ID` header (defaults to
  `"default"` when absent); two tenants never see each other's rules. Tenants
  themselves are managed through `/v1/tenants` (see **Tenants** below).
- **Rule execution is real:** `POST /v1/execute` builds a fresh engine from
  `EngineRegistry` per request and fires the rules in the shared `RuleSet`, so
  `rules_fired`, `rules_not_fired` and `facts_after` reflect actual inference.

## Status: registered vs. unregistered

Everything documented below is registered in `api/src/app.rs`. The metric surfaces are
`/v1/metrics/dead-rules`, `/v1/metrics/coverage`, `/v1/metrics/dmn` and `/v1/metrics/ws`,
plus the root `/metrics`. (`/v1/metrics/engine` and `/v1/metrics/coverage/reset` used to be
listed here as unregistered placeholders; their module returned hard-coded zeroes and has been
deleted rather than registered.) The two dashboard endpoints carry a
`/v1` prefix and require a token like every other business route: they are documented
once, under **Metrics under /v1**.

---

# Health

Root-level liveness/readiness probes (registered before `/v1`).

## GET /live
Liveness probe. **Auth:** none. **Response:** `200`
```json
{ "status": "ok", "service": "rrules-api" }
```

## GET /ready
Readiness probe. **Auth:** none. **Response:** `200`
```json
{ "status": "ready", "service": "rrules-api" }
```

---

# Metrics

Root-level `/metrics` scope (outside `/v1`). These read from the engine
monitoring singletons.

## GET /metrics
Engine metrics, content-negotiated via the `Accept` header.

| Accept | Response |
|--------|----------|
| `text/html` or `*/*` | HTML metrics dashboard (`text/html`) |
| `application/json` | JSON engine state |
| `text/plain` (or no Accept) | Prometheus exposition text |

**Auth:** none. **Query params:** none.

**JSON response** (`application/json`):
```json
{ "engine_name": "default", "rules_fired": 0, "facts_inserted": 0,
  "active_sessions": 0, "memory_bytes": 0 }
```
**Prometheus response** (`text/plain; version=0.0.4`):
```
# HELP engine_rules_fired Total number of rules fired
# TYPE engine_rules_fired counter
engine_rules_fired 0
# HELP engine_memory_bytes Current memory usage in bytes
# TYPE engine_memory_bytes gauge
engine_memory_bytes 0
```

# Metrics under /v1

Live monitoring surfaces (JWT required, except the WebSocket streams).

## GET /v1/metrics/dead-rules
Rule ids that were evaluated but never fired. Reads the process-wide
`RuleExecutionTracker` singleton, so it reflects live executions.
**Response:** `200` `["rule-id", ...]`.

## GET /v1/metrics/coverage
Coverage data from the `CoverageTracking` singleton.
**Response:** `200` `application/json`
```json
{ "rules": [ { "rule_id": "discount-rule", "evaluated": true, "fired": true,
    "evaluation_count": 3, "fire_count": 2, "skipped_reason": null } ],
  "summary": { "total_rules": 1, "evaluated_count": 1, "fired_count": 1,
    "coverage_percent": 100.0 } }
```

## GET /v1/metrics/dmn
DMN evaluation metrics (per-output counters, DRD chain depth, dead decisions).
**Response:** `200` `DmnMetricsSnapshot` JSON.

## GET /v1/metrics/dmn/ws — WebSocket
Live `DmnMetricsSnapshot` broadcast after each successful decision evaluation
(`decision_id`, `rules_fired`, `duration_ms`, `timestamp`). A failed evaluation is counted in the
Prometheus metrics but does not publish a snapshot, because a snapshot describes a completed
evaluation.
**Auth:** none (read-only stream, same rationale as `/v1/metrics/ws`).

## GET /v1/metrics/ws — WebSocket
Live `MetricsSnapshot` broadcast after each `/v1/execute` call.
**Auth:** none (read-only stream).

---

# Authentication

JWT (HS256) with a 1-hour token lifetime. All `/v1` routes except
`/v1/auth/login` require `Authorization: Bearer <token>`.

## POST /v1/auth/login
**Auth:** none. **Request:**
```json
{ "username": "admin", "email": null, "password": "..." }
```
`username` or `email` identifies the user. **Response:** `200`
```json
{ "token": "<jwt>", "user": { "id": "...", "email": "...",
  "displayName": "...", "role": "admin", "permissions": ["..."] } }
```
`401` on bad credentials.

## GET /v1/auth/me
Current user, derived from the bearer token. **Response:** `200` `UserResponse`.

## POST /v1/auth/refresh
Issue a fresh token for a still-valid one. **Response:** `200`
`{ "token": "<jwt>" }`.

Token claims: `sub` (user id), `role`, `iat`, `exp`. The signing key comes from
the auth environment/secret configuration; algorithm pinning rejects `alg:none`
and wrong-secret tokens.

---

# Rules

Rule CRUD with multi-tenant isolation via the `X-Tenant-ID` header.

## GET /v1/rules
List rules for the calling tenant, with filtering and pagination.

**Auth:** `X-Tenant-ID` header (optional, defaults to `"default"`). **Query params** (`RuleListRequest`):

| Param | Type | Default | Notes |
|-------|------|---------|-------|
| `page` | u32 | 1 | 1-indexed |
| `page_size` | u32 | 50 | clamped to 1–100 |
| `category` | string | — | filter by category |
| `status` | string | — | `draft`/`active`/`inactive`/`archived` |
| `tag` | string | — | filter by tag |
| `sort_by` | string | — | `name`/`salience`/`created_at`/`updated_at` |
| `sort_order` | string | `asc` | `asc`/`desc` |

**Response:** `200` `RuleListResponse`
```json
{ "total_count": 1, "rules": [ { /* RuleResponse */ } ],
  "page": 1, "page_size": 50, "total_pages": 1 }
```

## POST /v1/rules
Create a rule. **Auth:** `X-Tenant-ID`.

**Request** (`CreateRuleRequest`):
```json
{ "name": "gold-discount", "description": "Apply 10% to Gold tier",
  "category": "pricing", "dsl_body": "rule ...", "tags": ["promo"],
  "salience": 10, "status": "draft" }
```
| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | unique within category |
| `description` | string | no | |
| `category` | string | no | |
| `dsl_body` | string | yes | rule DSL source |
| `tags` | [string] | no | default `[]` |
| `salience` | i32 | no | default `0` |
| `status` | string | no | default `draft` |

**Response:** `201` `RuleResponse` (see below).

## GET /v1/rules/{id}
Get a single rule. **Auth:** `X-Tenant-ID`. **Response:** `200` `RuleResponse`,
`404` if not found (or wrong tenant).

## PUT /v1/rules/{id}
Full/partial update. **Auth:** `X-Tenant-ID`. **Request** (`UpdateRuleRequest`):
```json
{ "description": "new desc", "dsl_body": "rule ...", "tags": ["a","b"],
  "salience": 20, "status": "active" }
```
All fields optional; `tags` replaces the existing set. **Response:** `200`
`RuleResponse`.

## PATCH /v1/rules/{id}
Alias of `PUT` (same handler + `UpdateRuleRequest`). **Response:** `200`
`RuleResponse`.

## DELETE /v1/rules/{id}
Soft-delete (marks archived). **Auth:** `X-Tenant-ID`. **Response:** `200`
with the deleted rule id/status.

## POST /v1/rules/{id}/undelete
Restore a soft-deleted rule. **Auth:** `X-Tenant-ID`. **Response:** `200`
`RuleResponse`-style body.

## POST /v1/rules/{id}/tags
Assign a tag to a rule. **Auth:** `X-Tenant-ID`. **Request** (`RuleTagActionRequest`):
```json
{ "name": "promo" }
```
**Response:** `200`.

## DELETE /v1/rules/{id}/tags/{tag_name}
Remove a tag from a rule. **Auth:** `X-Tenant-ID`. **Response:** `200`/`404`.

## GET /v1/rules/{id}/git-log
Commit history for a rule (backed by git). **Auth:** `X-Tenant-ID`.

**Query params** (`GitLogRequest`): `limit` (usize, default 50).

**Response:** `200` `GitLogResponse`
```json
{ "branch": "main", "total_commits": 2,
  "commits": [ { "sha": "abc123", "message": "update rule",
    "author_name": "jane", "committed_at": "2026-06-30T12:00:00Z" } ] }
```
`500` on git failure.

### `RuleResponse`
```json
{ "id": "uuid", "name": "gold-discount", "description": "...",
  "category": "pricing", "owner": null, "dsl_body": "rule ...",
  "tags": ["promo"], "status": "draft", "version": 1,
  "created_at": "2026-06-30T10:00:00Z", "updated_at": "2026-06-30T10:00:00Z" }
```

---

# Execution

## POST /v1/execute
Execute a rule set against supplied facts. **Auth:** Bearer JWT.

**Request** (`ExecuteRequest`):
```json
{ "session_id": "sess-1", "facts": { "customer": { "tier": "Gold" } },
  "rule_filter": { "name_pattern": "gold-*", "categories": ["pricing"] },
  "engine": "rete", "trace": false }
```
| Field | Type | Notes |
|-------|------|-------|
| `session_id` | string? | opt-in **stateful** execution: the working memory is kept under this id and resumed on the next call (server UUID returned as `session_id`) |
| `facts` | map<string, json> | fact type → object payload |
| `rule_filter.name_pattern` | string? | glob on rule name |
| `rule_filter.categories` | [string]? | restrict categories |
| `engine` | string? | `rete`/`treat`/`leaps`/`phreak`; defaults to `RRULES_DEFAULT_ENGINE` (or `rete`). Unknown names → `400` |
| `trace` | bool? | opt-in inference trace (off by default; no latency cost when off) |

**Response:** `200` `ExecuteResponse`
```json
{ "execution_id": "uuid", "rules_fired": 2, "rules_not_fired": ["r-other"],
  "facts_after": { "customer": { "tier": "Gold", "discount": 0.1 } },
  "inference_trace": null, "engine_generation": 7, "session_id": null }
```
- `facts_after` is post-execution working memory, not the echoed input.
- `inference_trace` is populated only when `trace: true`; it carries
  `entries[{timestamp, event_type, rule_name?, data?}]` and
  `summary{total_duration_ms, facts_matched}`.
- `session_id` is present once the stateful path created a session.

## GET /v1/engines
Registered engine names, for the frontend engine selector. **Auth:** Bearer JWT.
```json
{ "engines": ["leaps", "phreak", "rete", "treat"], "default": "rete" }
```

## DELETE /v1/execute/session/{id}
Dispose a stateful session (frees the retained engine + working memory).
**Auth:** Bearer JWT. **Response:** `200`/`404`.

## POST /v1/execute/session/{id}/reset
Clear a session's working memory without disposing it. **Auth:** Bearer JWT.
**Response:** `200`/`404`.

## POST /v1/rules/batch-execute
Execute multiple requests concurrently with bounded parallelism. Each request
runs in its own task; one failing request does **not** fail the batch.

**Auth:** none. **Request** (`BatchExecuteRequest`):
```json
{ "requests": [ {"facts": {"customer": {"tier": "Gold"}}},
                {"facts": {"customer": {"tier": "Silver"}}} ],
  "max_concurrency": 8 }
```
| Field | Type | Notes |
|-------|------|-------|
| `requests` | [`ExecuteRequest`] | each item is a single execute request |
| `max_concurrency` | usize? | default 8, minimum 1 |

**Response:** `200` `BatchExecuteResponse`
```json
{ "results": [ { "success": true,
    "response": { /* ExecuteResponse */ }, "error": null } ],
  "total_fired": 2, "succeeded": 2, "failed": 0 }
```
`results` preserves submission order. A task panic/JoinError yields
`{ "success": false, "response": null, "error": "<join error>" }`.

## POST /v1/rules/reload
Hot-reload rules from the configured directory (`RULE_STORAGE_PATH`).

**Auth:** none. **Request** (`ReloadRequest`):
```json
{ "rule_file": "gold.rules", "async_reload": false }
```
- `async_reload: false` → `200` `ReloadResponse`:
  `{ "reload_id", "rules_reloaded", "status" }`
- `async_reload: true` → `202` `JobResponse`: `{ "job_id", "status": "running" }`
  (poll via `GET /v1/jobs/{job_id}`)

## GET /v1/jobs/{job_id}
Reload job status. **Response:** `200` `ReloadJob`, `404` if unknown.
```json
{ "job_id": "uuid", "status": "completed", "result": { /* ReloadResult */ } }
```
(`status` ∈ `pending`/`running`/`completed`/`failed`; `result`/`error` optional.)

## GET /v1/jobs
List all reload jobs. **Response:** `200` `[ReloadJob]`.

---

# Decisions (DMN 1.5)

Manage and evaluate DMN decision documents. Documents are stored as `.dmn` XML
plus a `.meta.json` sidecar under `DMN_STORAGE_PATH` and parsed into the
canonical IR on read. **Auth:** Bearer JWT on every endpoint.

## GET /v1/decisions
List stored decisions. **Query:** `page` (default 1), `per_page` (default 50).
**Response:** `200`
```json
{ "decisions": [ { "id": "uuid", "name": "Loan approval",
  "version": null, "updated_at": "2026-07-10T10:00:00Z" } ] }
```

## POST /v1/decisions
**Request** (`CreateDecisionRequest`): `{ "name": "...", "content": "<dmn ...>", "id": "optional" }`
**Response:** `201` with the stored decision summary.

## GET /v1/decisions/{id}
**Response:** `200` `{ "id", "name", "content", "metadata", "created_at", "updated_at" }`,
`404` if unknown.

## PUT /v1/decisions/{id}
**Request** (`UpdateDecisionRequest`): `{ "content": "<dmn ...>", "metadata": {...}? }`

## DELETE /v1/decisions/{id}
**Response:** `200`/`404`.

## POST /v1/decisions/{id}/evaluate
Evaluate one decision (DRD dependencies are resolved and evaluated first).
**Request** (`EvaluateDecisionRequest`):
```json
{ "inputs": { "Application risk score": 130, "Age": 35 }, "trace": false }
```
**Response:** `200` `EvaluateDecisionResponse`
```json
{ "decision_id": "decision_1", "outputs": { "Approval": "approved" },
  "rules_fired": 1, "duration_ms": 0.42,
  "trace": { "decision_id": "decision_1", "event_count": 6, "duration_ms": 0.42 } }
```
`trace` is present only when requested. Unknown decision / DRD cycle → `4xx`
with `{"error": "..."}`.

## POST /v1/decisions/import
Import a DMN XML document. **Request** (`ImportDmnRequest`):
`{ "id": "...", "content": "<dmn ...>" }` (`content` may be raw XML text or a
byte array — the handler accepts both). **Response:** `201`.

## GET /v1/decisions/{id}/export
Returns the stored document as `application/xml` (`Content-Disposition:
attachment`) for round-trip editing.

## GET /v1/decisions/{id}/ir
Parsed canonical IR as JSON — what the frontend decision-table editor binds to
(hit policy, rules, input/output clauses, DRD links).

## POST /v1/decisions/reload
Hot-reload the in-memory shared model after external file changes.
**Request** (`ReloadDmnRequest`): `{ "dmn_content": "<dmn ...>" }`.

---

# Tenants

Organisation/workspace registry. **Auth:** Bearer JWT on every endpoint.

## GET /v1/tenants
List tenants, newest first. **Response:** `200`
```json
{ "total_count": 1,
  "tenants": [ { "id": "uuid", "name": "Acme Corp",
    "created_at": "2026-10-25T07:00:00Z" } ] }
```

## POST /v1/tenants
**Request:** `{ "name": "Acme Corp" }` (names are trimmed).
**Response:** `201` `TenantResponse`. `400` for a blank name; `409` when the name
is already taken.

## GET /v1/tenants/{id}
**Response:** `200` `TenantResponse`, `404` if unknown.

## DELETE /v1/tenants/{id}
**Response:** `204` on delete, `404` if unknown (deleting twice is a `404`).

> The store is an in-memory `MemoryTenantStore`; the `SqlxTenantStore` backend in
> `engine::governance::tenant` is a placeholder pending a PostgreSQL-backed
> tenant table.

---

# Categories

Hierarchical rule grouping (UUID-keyed).

## GET /v1/categories
List all categories. **Response:** `200` `CategoryListResponse`
```json
{ "total_count": 1, "categories": [ { /* CategoryResponse */ } ] }
```

## POST /v1/categories
**Request** (`CreateCategoryRequest`):
```json
{ "name": "pricing", "description": "Pricing rules", "parent_id": null }
```
**Response:** `201` `CategoryResponse`: `{ "id", "name", "description", "parent_id", "created_at" }`

## GET /v1/categories/{id}
**Response:** `200` `CategoryResponse`, `404` if missing.

## PUT /v1/categories/{id} / PATCH /v1/categories/{id}
Update a category (same `CreateCategoryRequest` body). **Response:** `200`
`CategoryResponse`.

## DELETE /v1/categories/{id}
Delete a category. **Response:** `200`/`404`.

---

# Tags

Tags are keyed by `name` (string), with optional color/description.

## GET /v1/tags
List all tags. **Response:** `200` `[TagResponse]`.

## POST /v1/tags
**Request** (`CreateTagRequest`): `{ "name": "promo", "color": "#f00", "description": "..." }`
**Response:** `201` `TagResponse`: `{ "name", "color", "description", "created_at" }`

## GET /v1/tags/{name}
**Response:** `200` `TagResponse`, `404` if missing.

## PUT /v1/tags/{name}
Update a tag (same `CreateTagRequest` body). **Response:** `200` `TagResponse`.

## DELETE /v1/tags/{name}
Delete a tag. **Response:** `200`/`404`.

---

# Approvals

Maker-checker approval workflow (UUID-keyed `approval_id`).

## POST /v1/rules/{id}/submit
Submit a rule for approval. **Request** (`SubmitApprovalRequest`):
```json
{ "workflow_id": "uuid-or-omit", "maker_id": "jane" }
```
**Response:** `200`/`201` `SubmitApprovalResponse`:
`{ "approval_id", "rule_id", "status" }`

## GET /v1/approvals/pending
List pending approval requests. **Response:** `200` array of approval objects
(each has `id`, `rule_id`, `state`, `reviewers`, `required_approvals`,
`approved_by`, `created_at`).

## POST /v1/approvals/{id}/approve
Approve a pending request. **Request** (`ApprovalActionRequest`):
```json
{ "approver_id": "reviewer1", "reason": "looks good" }
```
**Response:** `200` with updated approval state.

## POST /v1/approvals/{id}/reject
Reject a pending request. **Request** (`ApprovalActionRequest`):
```json
{ "approver_id": "reviewer1", "reason": "missing test coverage" }
```
**Response:** `200` with rejected state.

`ApprovalState` ∈ `draft`/`submitted`/`in_review`/`approved`/`rejected`/`escalated`.

---

# Branches

Git-backed branch operations (`VersionControlRepo`).

## GET /v1/branches
List all branches. **Response:** `200` `BranchListResponse`
```json
{ "total_count": 1, "default_branch": "main",
  "branches": [ { /* BranchResponse */ } ] }
```

## POST /v1/branches
Create a branch. **Request** (`CreateBranchRequest`):
```json
{ "source_branch": "main", "from_commit": null }
```
Both fields optional. **Response:** `201` `BranchResponse`:
`{ "name", "is_default", "commit_sha", "last_commit_message", "created_at" }`

## GET /v1/branches/{name}
**Response:** `200` `BranchResponse`, `404` if missing.

## DELETE /v1/branches/{name}
Delete a branch. **Response:** `200`/`404`.

## POST /v1/branches/{name}/merge
Merge the named branch into main. **Response:** `200` (merge result), `500` on
failure.

---

# Audit

The trail is written by the mutating endpoints: creating, updating and deleting (archiving) a rule,
and approving or rejecting an approval request. Each event carries the acting user from the JWT, the
peer address and the `User-Agent`.

Not recorded yet: category, tag and tenant mutations, and executions. A deployment that needs those
audited should not assume they are.

## GET /v1/audit
Query audit events. **Query params** (`AuditQuery`):

| Param | Type | Notes |
|-------|------|-------|
| `page` | u32? | 1-indexed |
| `page_size` | u32? | |
| `resource_type` | string? | `rule`/`category`/`tag`/`user`/`approval` |
| `action` | string? | `create`/`read`/`update`/`delete`/`approve`/`reject`/`execute`/`export` |
| `user_id` | string? | |
| `since` | string? | ISO-8601 timestamp lower bound |

**Response:** `200` `AuditListResponse`
```json
{ "total_count": 1, "events": [ { /* AuditEvent */ } ],
  "page": 1, "page_size": 50, "total_pages": 1 }
```
`AuditEvent`: `{ "id", "timestamp", "user_id", "action", "resource_type",
"resource_id", "resource_name", "details", "ip_address", "user_agent" }`.

## GET /v1/audit/export
Export the full audit log as CSV. **Response:** `200`
`text/csv` with `Content-Disposition: attachment; filename="audit_log.csv"`.

---

# Monitoring

Engine monitoring probes under `/v1/monitoring`.

## GET /v1/monitoring/engine-state
Snapshot of engine runtime state. **Response:** `200` `EngineStateResponse`
```json
{ "engine_name": "default", "rules_fired": 0, "facts_inserted": 0,
  "active_sessions": 0, "memory_bytes": 0, "alpha_memory_size": 0,
  "beta_memory_size": 0, "network_traversals": 0, "execution_time_ms": 0.0,
  "last_updated": "2026-07-01T10:00:00Z" }
```

## GET /v1/monitoring/health
Monitoring subsystem health. **Response:** `200`
```json
{ "status": "healthy", "service": "rrules-monitoring" }
```

---

# Simulation

What-If analysis — run fact sets against a base vs. modified engine and diff
results, without touching production state.

## POST /v1/simulation
Run a simulation. (Also accepts `/v1/simulation/`.)

**Auth:** none. **Request** (`SimulationRunRequest`):
```json
{
  "name": "discount-whatif",
  "base_state": null,
  "proposed_changes": [
    { "RemoveRule": { "rule_id": "legacy-discount" } }
  ],
  "fact_sets": [
    [ { "id": "customer", "fields": { "tier": "Gold" } } ]
  ]
}
```
| Field | Type | Notes |
|-------|------|-------|
| `name` | string | required |
| `base_state` | `WorkingMemorySnapshot`? | optional seed facts |
| `proposed_changes` | [`RuleChange`] | each produces one variant |
| `fact_sets` | [[`TestFact`]] | required, non-empty; each inner vec is one scenario |

`RuleChange` is an externally-tagged enum: `AddCondition`, `RemoveCondition`,
`ModifyAction`, `RemoveRule`, `AddRule`, `DisableRule`.
`TestFact` = `{ "id": string, "fields": json }`.

**Response:** `200` `SimulationRunResponse`
```json
{
  "name": "discount-whatif",
  "test_cases": [ {
    "fact_set_index": 0,
    "variants": [ { "variant_label": "base", "rules_fired": [],
      "execution_time_ms": 0.1, "output": null, "success": true, "error": null } ],
    "differences": []
  } ],
  "total_execution_time_ms": 0.5,
  "summary": { "total_variants": 2, "total_fact_sets": 1,
    "affected_test_cases": 0, "rules_affected": 0 }
}
```
- `400` if `fact_sets` is empty: `{"error":"fact_sets must contain at least one fact group","status_code":400}`.
- `summary.total_variants = 1 + proposed_changes.len()` (base + one per change).

---

## Environment variables

Every variable the server reads, with the default that applies when it is unset.

| Variable | Default | Effect |
|----------|---------|--------|
| `API_SECRET_KEY` | `rrules-dev-secret-change-me` | HS256 signing key. The default is a known string; a warning is logged and **it must be set in any deployment** (the chart reads it from a Secret). |
| `RRULES_BIND_ADDRESS` | `127.0.0.1` | listening host. The default is loopback; the container image sets `0.0.0.0` so the port is reachable. |
| `PORT` | `8080` | listening port. |
| `CORS_ALLOWED_ORIGINS` | none (no cross-origin origin allowed) | comma-separated list of browser origins allowed to call the API. Origins are pinned, never a wildcard, so the Bearer-token flow works without allowing credentials. |
| `RULE_STORAGE_PATH` | `./rules` | rules directory; also the git repository the version control uses (`<path>/.git`). |
| `DMN_STORAGE_PATH` | `./dmn` | DMN decision model directory (the filesystem store). |
| `DMN_PERSISTENCE` | `filesystem` | Backend for decision documents: `filesystem` or `postgres`. `postgres` requires a build with `--features postgres` and a `DATABASE_URL`; an unusable choice fails startup instead of silently using the filesystem. |
| `DATABASE_URL` | unset | PostgreSQL connection string, read when `DMN_PERSISTENCE=postgres`. |
| `FRONTEND_DIST` | `../frontend/dist` | built interface served at `/` and for every client-side route. |
| `WATCH_RULES_DIR` | `true` | watch the rules directory and reload on change. Set `false` to disable. |
| `RRULES_DEFAULT_ENGINE` | `rete` | engine used when a request does not name one. |
| `RRULES_MAX_CHAIN_ITERATIONS` | engine default | bound on forward-chaining iterations for one execution. |
| `RRULES_SESSION_IDLE_TTL_SECS` | `900` | idle lifetime of a stateful session before it is swept. |
| `RRULES_SESSION_MAX_LIFETIME_SECS` | `86400` | absolute lifetime of a stateful session. |
| `RRULES_MAX_SESSIONS` | `1024` | cap on concurrent stateful sessions. |
| `RRULES_SESSION_SWEEP_INTERVAL_SECS` | `60` | how often the session sweeper runs. |
