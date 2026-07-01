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
- **Auth:** **not yet enforced.** No JWT/session is checked at the route layer.
  Multi-tenancy is header-based: send `X-Tenant-ID: <tenant>` to scope rule CRUD
  to a tenant (defaults to `"default"` when absent). Real RBAC/JWT enforcement
  is deferred to a future milestone.
- **Rule execution is a stub:** `POST /v1/execute` and
  `POST /v1/rules/batch-execute` build an isolated engine but do not fire rules,
  so `rules_fired` is always `0`. These endpoints deliver real request handling
  (and batch concurrency) but not functional inference. Functional rule firing
  is a future milestone.

## Status: registered vs. unregistered

Only the routes below are registered in `api/src/main.rs`. Note that
`/v1/metrics/*` exists in source (`configure_v1_metrics`) but is **never
registered** — it is dead code. Use the root `/metrics` endpoints instead.

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

## GET /metrics/dead-rules
Rules that were evaluated but never fired (dead code candidates).

**Auth:** none. **Response:** `200` JSON array of rule IDs.
```json
[]
```
> Note: the handler builds a **fresh** `RuleExecutionTracker` per request and
> nothing populates it (rule execution is a stub), so this currently always
> returns `[]`. This is deferred tech debt, not a v4.0 blocker.

## GET /metrics/coverage
Rule coverage data from the global `CoverageTracking` singleton. Rules
recorded via the engine (e.g. `CoverageTracking::record_rule_eval`) surface
here.

**Auth:** none. **Response:** `200` `application/json`
```json
{
  "rules": [
    { "rule_id": "discount-rule", "evaluated": true, "fired": false,
      "evaluation_count": 3, "fire_count": 0, "skipped_reason": null }
  ],
  "summary": { "total_rules": 1, "evaluated_count": 1, "fired_count": 0,
    "coverage_percent": 0.0 }
}
```

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
Execute a rule set against supplied facts. **Auth:** none.

**Request** (`ExecuteRequest`):
```json
{ "session_id": "sess-1", "facts": { "customer": { "tier": "Gold" } },
  "rule_filter": { "name_pattern": "gold-*", "categories": ["pricing"] } }
```
| Field | Type | Notes |
|-------|------|-------|
| `session_id` | string? | optional correlation id |
| `facts` | map<string, json> | fact type → object payload |
| `rule_filter.name_pattern` | string? | glob on rule name |
| `rule_filter.categories` | [string]? | restrict categories |

**Response:** `200` `ExecuteResponse`
```json
{ "execution_id": "uuid", "rules_fired": 0, "rules_not_fired": [],
  "facts_after": { "customer": { "tier": "Gold" } }, "inference_trace": null }
```
> `rules_fired` is `0` because execution is a stub (see Conventions).

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
  "total_fired": 0, "succeeded": 2, "failed": 0 }
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

Immutable audit trail (Phase 20-04).

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

| Variable | Default | Effect |
|----------|---------|--------|
| `RULE_STORAGE_PATH` | `./rules` | rules directory; also the git repo (`<path>/.git`) |
| `FRONTEND_DIST` | `../frontend/dist` | static frontend served at `/` |
| `WATCH_RULES_DIR` | `true` | start the rules file watcher |