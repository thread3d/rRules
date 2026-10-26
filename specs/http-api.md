# HTTP API

Implementation-derived reference for the actix-web server in `api/`. Every
statement below is traceable to a `path:line`. Where `docs/API.md` disagrees
with the code, the code wins and the drift is recorded in
[Known gaps](#known-gaps).

- **Purpose:** REST surface over the engine crate (`/v1/execute`, sessions,
  DMN decisions), the governance stores (rules, categories, tags, approvals,
  audit, branches, tenants), and the monitoring surfaces (Prometheus, WS).
- **Base URL:** `http://127.0.0.1:8080` by default, from `RRULES_BIND_ADDRESS` and `PORT`
  (`api/src/main.rs:45-52`). The default is loopback because that is right for a laptop and
  wrong for a container, which is why the image sets `RRULES_BIND_ADDRESS=0.0.0.0`.
- **Versioning:** one `web::scope("/v1")` (`api/src/app.rs:147`). No `/v2`, no
  `Accept`-based version negotiation. The OpenAPI `info.version` is the crate
  version `9.0.18` (`api/src/openapi.rs:12`).
- **Content types:** request bodies are JSON. Responses are JSON except the
  four tabulated exceptions below.

| Surface | Content-Type | Source |
|---|---|---|
| Default handlers | `application/json` | every `HttpResponse::…json(...)` call |
| `GET /metrics` (Prometheus arm) | `text/plain; version=0.0.4` | `api/src/metrics.rs:380` |
| `GET /metrics` (HTML arm) | `text/html; charset=utf-8` | `api/src/metrics.rs:337` |
| `GET /v1/metrics/dmn` | `text/plain; version=0.0.4` | `api/src/v1/dmn_metrics.rs:124` |
| `GET /v1/decisions/{id}/export` | `application/xml` + `Content-Disposition: attachment` | `api/src/v1/decisions.rs:412-416` |
| `GET /v1/audit/export` | `text/csv` + `Content-Disposition: attachment` | `api/src/v1/audit.rs:291-292` |
| Extractor failures (`web::Json`/`Query`) | actix-web default (not the JSON envelope) — no `JsonConfig`/custom error handler is installed anywhere | `api/src/v1/*.rs` handler signatures |

## Server composition

### Startup sequence (`api/src/main.rs:42-283`, then `api/src/app.rs:60-322`)

1. Build `EngineRegistry` + `register_default_engines()` (`:56-58`); build
   `EngineSelector::with_defaults()` and select `RRULES_DEFAULT_ENGINE` or
   `rete` (`:60-70`); validate `RRULES_MAX_CHAIN_ITERATIONS` parses as `usize`,
   warn only (`:76-83`).
2. Load the executing `RuleSet` from `RULE_STORAGE_PATH` via
   `build_ruleset_from_dir`, wrapped as `web::Data<Arc<RwLock<SharedRuleSet>>>`
   with `generation: 0` (`:92-95`, `api/src/v1/execution.rs:168-229`).
3. Load **only the first** `.dmn` file in `DMN_STORAGE_PATH` into a
   `DecisionEvaluator`; missing dir / no files / parse error → empty model,
   never a crash (`:102-142`).
4. Create the DMN metrics channel (capacity 16) and spawn its forwarding task
   (`:149-150`); create the engine metrics channel and **drop its receiver
   immediately** (`:153`).
5. Build `ExecutionCounters` on a dedicated Prometheus registry and register
   `rrules_sessions_active` on it (`:161-178`); build `SessionManager` from
   `read_session_config()` (`:169-183`); initialize every in-memory store
   (`:186-195`).
6. Create `RULE_STORAGE_PATH` on disk and open/init the git repo at
   `<rules_dir>/.git` (`:198-212`); a VCS init failure aborts startup (`:210`).
7. `tokio::spawn(start_file_watcher(rules_dir))` when `WATCH_RULES_DIR` is
   truthy (`:223-230`); spawn the session sweep task (`:238`).
8. Parse `CORS_ALLOWED_ORIGINS` (`:247-253`), build `HttpServer` with a
   per-worker closure (`:255`).

### Middleware order

| Order | Layer | Scope | Source |
|---|---|---|---|
| 1 | `actix_cors::Cors` | whole app (outermost) | `api/src/main.rs:260-271` |
| 2 | `middleware::from_fn(middleware::auth::requires_auth)` | `/v1` scope only | `api/src/app.rs:144-146` |
| — | `CorrelationId` | **never installed** — file is orphaned (see gaps) | `api/src/middleware/correlation.rs:14` |

No logging middleware, no compression, no default headers, no rate limiting.

### Shared state (`app_data`)

Twenty `app_data` entries; all are per-worker clones of process-shared Arcs.

| Type | Notes | Source |
|---|---|---|
| `v1::execution::EngineState` | **never extracted by any handler** | `api/src/main.rs:48,272` |
| `EngineRegistry` | engine factories | `:56-58,274` |
| `EngineSelector` | default engine, read via `current_engine()` | `:60-70,275` |
| `Arc<RwLock<SharedRuleSet>>` | ruleset + generation | `:94-95,276` |
| `MetricsTx` (`Arc<broadcast::Sender<MetricsSnapshot>>`) | WS fan-out | `:153,277` |
| `ExecutionCounters` | 3 Prometheus metric families + 3 `AtomicU64` mirrors | `:161,280` |
| `Arc<SessionManager>` | stateful sessions | `:179-183,283` |
| `Arc<DecisionEvaluator>` | DMN evaluation + reload | `:142,286` |
| `DmnMetricsTx` | DMN WS fan-out | `:149,289` |
| `FileSystemDmnStore` | decision persistence (env-resolved) | `:291-293` |
| `RuleStore`, `CategoryStore`, `TagStore`, `RuleTagStore` | in-memory | `:294-297` |
| `TenantStore`, `ApprovalStore`, `AuditStore` | in-memory | `:298-300` |
| `tokio::sync::Mutex<VersionControlRepo>` | git branches | `:205-212,301` |
| `JobStore` | reload jobs | `:302` |
| `UserStore` | 4 seeded demo users | `:303` |

### CORS rules (`api/src/main.rs:260-269`)

- Methods: `GET, POST, PUT, PATCH, DELETE, OPTIONS`.
- Headers: `Authorization, Content-Type, Accept` — **`X-Tenant-ID` is not
  allowed**, so a cross-origin browser client cannot send the tenancy header
  (`:263`).
- `max_age(3600)`; one `allowed_origin` per comma-separated entry; default
  origin `http://localhost:5173` (`:247-252`).
- No `allow_credentials(true)` and no `expose_headers`.

### Static frontend (`api/src/app.rs:305-322`)

`actix_files::Files::new("/", &frontend_dist).index_file("index.html").show_files_listing()`
is mounted **last**, so any path not matched by a registered route reaches the
file service (it also enables directory listing). `FRONTEND_DIST` defaults to
`../frontend/dist` (`:217-218`). `GET /swagger-ui/{_:.*}` with spec at
`/api-docs/openapi.json` is registered before `/v1` (`:307-310`).

### Environment variables

| Variable | Default | Effect | Source |
|---|---|---|---|
| `RULE_STORAGE_PATH` | `./rules` | rules dir + `<dir>/.git` repo; read again per reload | `main.rs:92-93,198`, `v1/execution.rs:1295` |
| `DMN_STORAGE_PATH` | `./dmn` | startup model load; also read by the store | `main.rs:102-103`, `engine_state/src/lib.rs:104` |
| `FRONTEND_DIST` | `../frontend/dist` | static mount | `main.rs:217-218` |
| `WATCH_RULES_DIR` | `true` (`"true"`/`"1"` truthy) | spawns the watcher task — **which is a no-op stub** | `main.rs:223-230`, `v1/execution.rs:1444-1446` |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:5173` | comma-separated allowed origins | `main.rs:247-252` |
| `RRULES_DEFAULT_ENGINE` | `rete` | validated at startup; re-read per request | `main.rs:61-69`, `v1/execution.rs:600-603,1148`, `v1/sessions.rs:200-202` |
| `RRULES_MAX_CHAIN_ITERATIONS` | `1000` (engine-side) | main.rs only **warns** if unparseable; engine reads it per execution | `main.rs:76-83`, `engine/src/engine/rete/mod.rs:378` |
| `API_SECRET_KEY` | `rrules-dev-secret-change-me` | JWT HMAC secret; warns when unset | `api/src/auth/mod.rs:87-98` |
| `RRULES_SESSION_IDLE_TTL_SECS` | `900` | session idle TTL | `v1/sessions.rs:451-455` |
| `RRULES_SESSION_MAX_LIFETIME_SECS` | `86400` | session max age | `v1/sessions.rs:456-460` |
| `RRULES_MAX_SESSIONS` | `1024` | LRU cap | `v1/sessions.rs:461-464` |
| `RRULES_SESSION_SWEEP_INTERVAL_SECS` | `60` | sweep cadence | `v1/sessions.rs:465-469` |
| `VITE_API_URL` | `/v1` | **build-time, frontend only** | `frontend/src/api/client.ts:12`, `frontend/src/lib/auth-context.tsx:12` |

Read but effectively inert: **`WATCH_RULES_DIR`** — its only consumer spawns
`start_file_watcher`, whose body is a single `log::info!`
(`v1/execution.rs:1444-1446`), so no file watching happens. `DATABASE_URL` is
mentioned in `engine_state` docs but never read (`engine_state/src/lib.rs:338`).

## Authentication and authorisation

- **Algorithm:** HS256, pinned with `Validation::new(Algorithm::HS256)`, so
  `alg:none` and wrong-secret tokens are rejected
  (`api/src/middleware/auth.rs:33-40`). Signing uses `jsonwebtoken`'s
  `Header::default()` (`api/src/auth/mod.rs:101-107`).
- **Claims:** `sub` (user id), `role`, `exp`, `iat`, all UTC seconds
  (`api/src/auth/mod.rs:26-32`).
- **TTL:** 3600 s, set at issue and at refresh (`api/src/auth/mod.rs:80,114,247`).
- **Users:** in-memory store seeded with `admin`, `author`, `reviewer`,
  `viewer`; password == username, plaintext compare
  (`api/src/auth/store.rs:27-48`, `api/src/auth/mod.rs:154`).
- **Public allowlist** (`api/src/middleware/auth.rs:53-60`), matched with
  `path.starts_with(prefix)`: `/health`, `/swagger-ui`, `/api-docs`,
  `/v1/auth/login`, `/v1/metrics/ws`, `/metrics`. The middleware is only
  wrapped on the `/v1` scope, so in practice only `/v1/auth/login` and
  `/v1/metrics/ws` are bypassed.
- **Rejection:** no `Bearer ` prefix → `401 {"message":"Missing or invalid
  authorization token"}`; decode failure/expiry → `401 {"message":"Invalid or
  expired token"}` (`api/src/middleware/auth.rs:76-81,101-113`). Handlers that
  re-decode the token (`/v1/auth/me`, `/v1/auth/refresh`) produce their own
  `{"message":…}` 401s (`api/src/auth/mod.rs:186-201,229-244`).
- **Authorisation: none.** Successful auth inserts `Claims` into request
  extensions (`api/src/middleware/auth.rs:109`) but **no handler or middleware
  ever reads it** (only the doc comment at `:90` references
  `extensions().get::<Claims>()`). Roles are pure UI metadata: any
  authenticated user of any role may call every `/v1` route, including
  `POST /v1/tenants`, `DELETE /v1/rules/{id}` and `POST /v1/approvals/{id}/approve`.
  `permissions` are emitted only so the frontend `usePermission` can gate the UI
  (`api/src/auth/store.rs:64-90`).

## Endpoint reference

Auth column: **yes** = requires a valid Bearer JWT (inside the `/v1` wrap);
**no** = public allowlist or root-level. 73 route entries total: 67 inside
`/v1`, 6 at root.

### Root-level surfaces

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/health`, `/health/live` | — | `200`, empty body | — | no | `api/src/health.rs` |
| GET | `/health/ready`, `/ready` | — | `200`, empty body | — | no | No dependency checks: the stores are in-memory (`health.rs`). The flat `/health` and `/ready` are aliases, because the Kubernetes manifests and the Helm chart probe those paths. |
| POST | `/v1/execute` | `ExecuteRequest{facts, rule_filter?, engine?, trace?}` | `200 {execution_id, rules_fired, rules_matched, rules_not_fired[], facts_after, …}` | `400` on a malformed body | yes | Runs the engine's rule set, which is loaded from `RULE_STORAGE_PATH`; rules created through `/v1/rules` are stored as text and are **not** part of it. `rules_matched` reports how many rules the filter selected, so a filter that matches nothing is distinguishable from a run where nothing fired. |
| GET | `/metrics` | — | `200` HTML / JSON / Prometheus text | — | no | Content-negotiated on `Accept` (`api/src/metrics.rs:330-383`); see drift note. |
| GET | `/api-docs/openapi.json` | — | `200` OpenAPI 3 JSON | — | no | `api/src/app.rs:129-132` |
| GET | `/swagger-ui/{_:.*}` | — | `200` HTML | — | no | Same registration. |
| GET | `/` (and all unmatched paths) | — | `200` static file | 404 file-not-found | no | `actix_files` mount, `show_files_listing()` (`app.rs:305-322`) |

`GET /metrics` arm selection (`api/src/metrics.rs:330-382`): `Accept`
containing `text/html` **or `*/*`** → HTML; containing `application/json` →
JSON; otherwise Prometheus text. Because `*/*` wins before JSON, a plain
`curl /metrics` (curl sends `Accept: */*`) returns the HTML page.

### Authentication

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| POST | `/v1/auth/login` | `LoginRequest{username?,email?,password}` | `200 {token,user{id,email,displayName,role,permissions}}` | `401 {"message":"Invalid credentials"}`, `500 {"message":"Failed to issue token"}` | **no** | `username` takes precedence over `email` (`auth/mod.rs:140-144`). |
| GET | `/v1/auth/me` | — | `200 UserResponse` | `401`, `500 {"message":"Store unavailable"}`, `401 {"message":"User not found"}` | yes | Re-decodes the token itself (`auth/mod.rs:185-216`). |
| POST | `/v1/auth/refresh` | body ignored | `200 {token}` | `401`, `500` | yes | Handler takes only `HttpRequest`; claims `iat`/`exp` are refreshed, `sub`/`role` preserved (`auth/mod.rs:228-257`). |

### Execution and sessions

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| POST | `/v1/execute` | `ExecuteRequest{session_id?,facts,rule_filter?,engine?,trace?}` | `200 ExecuteResponse{execution_id,rules_fired,rules_not_fired,facts_after,inference_trace?,engine_generation,session_id?}` | `400 {"error":"unknown engine: X"}`, `400 {"error":"session engine is fixed: …"}`, `500 {"error":…}` | yes | Stateless vs sessionful branch on `session_id.is_some()` (`execution.rs:583-593`). Only JSON-object fact values are inserted (`execution.rs:680-694`). |
| GET | `/v1/engines` | — | `200 {engines:[…sorted…],default}` | — | yes | `execution.rs:1138-1151` |
| DELETE | `/v1/execute/session/{id}` | — | `200 {status:"disposed",session_id}` | `404 {"error":"session not found: X"}` | yes | `sessions.rs:508-522` |
| POST | `/v1/execute/session/{id}/reset` | — | `200 {status:"reset",session_id}` | `404`, `500` | yes | Rebuilds the engine and reloads the current ruleset (`sessions.rs:343-424,534-550`). |
| POST | `/v1/rules/batch-execute` | `BatchExecuteRequest{requests,max_concurrency?}` | `200 {results[],total_fired,succeeded,failed}` | always `200`; per-item errors are in the body | yes | `max_concurrency` defaults to 8, min 1 (`execution.rs:1181`). |

### Reload jobs

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| POST | `/v1/rules/reload` | `ReloadRequest{rule_file?,async_reload}` | `202 {job_id,status:"pending"}` when async; `200 {reload_id,rules_reloaded,status}` when sync | `500 {reload_id,rules_reloaded:0,status:"Reload failed: …"}` (lock poisoning only) | yes | `rule_file` is parsed but never used — reload always reads the whole `RULE_STORAGE_PATH` dir (`execution.rs:1290-1395`). |
| GET | `/v1/jobs/{job_id}` | — | `200 ReloadJob{job_id,status,result?,error?}` | `404 {"error":"Job not found"}` | yes | `execution.rs:1410-1424` |
| GET | `/v1/jobs` | — | `200 [ReloadJob]` (unordered map iteration) | — | yes | `execution.rs:1435-1441` |

### Rules

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/rules` | query `RuleListRequest` | `200 {total_count,rules[],page,page_size,total_pages}` | `400 {"error":"Invalid status filter"}` | yes | `page_size` defaults to **50** when absent (an omitted value deserialises to `0`, which used to clamp to 1 and return a single row); capped at 100 (`rules.rs`). `X-Tenant-ID` scoped. |
| POST | `/v1/rules` | `CreateRuleRequest{name,description?,category?,dsl_body,tags,salience,status?}` | `201 RuleResponse` | `400 {"error":"Rule name is required"}` / `"DSL body is required"`, `409 {"error":"A rule with this name already exists in the category"}` | yes | Duplicate check compares against `category.unwrap_or_default()` (empty string) while the stored rule uses `"default"` (`rules.rs:346-349,364`) — duplicates for uncategorised rules are never detected. |
| GET | `/v1/rules/{id}` | — | `200 RuleResponse` | `400 {"error":"Invalid rule ID format"}`, `404 {"error":"Rule not found"}` | yes | `rules.rs:514-537` |
| PUT | `/v1/rules/{id}` | `UpdateRuleRequest` | `200 RuleResponse` | `400` (bad uuid / bad status), `404`, `409` | yes | `rules.rs:555-646` |
| PATCH | `/v1/rules/{id}` | same | same | same | yes | Same handler as `PUT` (`app.rs:185`). |
| DELETE | `/v1/rules/{id}` | — | `200 {id,name,status:"archived",message}` | `400`, `404` | yes | Soft delete only (`rules.rs:662-697`). |
| POST | `/v1/rules/{id}/undelete` | — | `200 {id,name,status:"draft",message}` | `400 {"error":"Rule is not archived, cannot restore"}`, `404` | yes | `rules.rs:713-751` |
| POST | `/v1/rules/{id}/tags` | `CreateTagRequest{name,…}` | `201 {rule_id,tag_name,action:"assigned"}` | `400` (bad uuid) | yes | **Not tenant-scoped**; creates the tag if absent (`tags.rs:264-305`). |
| DELETE | `/v1/rules/{id}/tags/{tag_name}` | — | `200 {rule_id,tag_name,action:"removed"}` | `400`, `404 {"error":"Tag not assigned to rule"}` | yes | `tags.rs:322-349` |
| GET | `/v1/rules/{id}/git-log` | query `limit` (default 50) | `200 {branch:"main",total_commits,commits[{sha,message,author_name,committed_at}]}` | `500` | yes | `{id}` is ignored; the branch is hard-coded to `main` (`branches.rs:459-492`). |

`RuleResponse` projection (`rules.rs:71-138`): `id,name,description?,category?,owner?(always null),dsl_body?,tags,status,version` (always `1`), `created_at,updated_at`.

### Approvals

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| POST | `/v1/rules/{id}/submit` | `SubmitApprovalRequest{workflow_id?,maker_id?}` | `201 {approval_id,rule_id,status:"draft"}` | `400` (bad uuid) | yes | Always created as `Draft`; no transition to `Submitted` exists (`approvals.rs:107-127`). |
| GET | `/v1/approvals/pending` | — | `200 {total_count,approvals[{id,rule_id,maker_id,state,created_at}]}` | — | yes | Everything not `Approved`/`Rejected` (`approvals.rs:139-163`). |
| POST | `/v1/approvals/{id}/approve` | `ApprovalActionRequest{approver_id?,reason?}` | `200 {approval_id,status:"approved",approved_by[]}` | `400`, `404 {"error":"Approval not found","approval_id":…}` | yes | `reason` is pushed into the `approved_by` vector (`approvals.rs:206-208`), mixing a rationale into an identity list. |
| POST | `/v1/approvals/{id}/reject` | same | `200 {approval_id,status:"rejected"}` | `400`, `404` | yes | `approvals.rs:238-275` |

No tenant scoping; `required_approvals` is fixed at 1 and never enforced.

### Categories

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/categories` | — | `200 {total_count,categories[CategoryResponse]}` | — | yes | `categories.rs:119-127` |
| POST | `/v1/categories` | `CreateCategoryRequest{name,description?,parent_id?}` | `201 CategoryResponse` | `409 {"error":"Category with this name already exists"}` | yes | Case-insensitive name uniqueness (`categories.rs:97-101`). |
| GET | `/v1/categories/{id}` | — | `200 CategoryResponse` | `400`,`404` | yes | `categories.rs:143-164` |
| PUT | `/v1/categories/{id}` | `CreateCategoryRequest` | `200 CategoryResponse` | `400`,`404`,`409` | yes | Also overwrites `created_at` with "now" (`categories.rs:215`). |
| PATCH | `/v1/categories/{id}` | same | same | same | yes | Same handler (`app.rs:231`). |
| DELETE | `/v1/categories/{id}` | — | **`204` no body** | `400`,`404`,`409 {"error":"Cannot delete category with child categories"}` | yes | `categories.rs:240-268` |

`CategoryResponse` = `{id,name,description?,parent_id?,created_at}`. No tenant scoping.

### Tags

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/tags` | — | `200 {total_count,tags[TagResponse]}` (ad-hoc `json!`, not a typed struct) | — | yes | `tags.rs:120-128` |
| POST | `/v1/tags` | `CreateTagRequest{name,color?,description?}` | `201 TagResponse` | `409` | yes | `tags.rs:83-109` |
| GET | `/v1/tags/{name}` | — | `200 TagResponse` | `404 {"error":"Tag not found"}` | yes | `tags.rs:143-157` |
| PUT | `/v1/tags/{name}` | `CreateTagRequest` | `200 TagResponse` | `404`,`409` | yes | Rename loses the original colour/description on conflict rollback (`tags.rs:184-201`). No `PATCH` route. |
| DELETE | `/v1/tags/{name}` | — | **`204` no body** | `404` | yes | `tags.rs:234-248`. Rule-tag links are not cleaned up. |

`TagResponse` = `{name,color?,description?,created_at}`.

### Tenants

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/tenants` | — | `200 {total_count,tenants[{id,name,created_at}]}` | `503 {"error":"Resource temporarily unavailable"}` on poisoned lock | yes | Newest first (`tenants.rs:86-102`, `engine/src/governance/tenant/types.rs:46-50`). |
| POST | `/v1/tenants` | `CreateTenantRequest{name}` | `201 TenantResponse` | `400` blank name, `409` duplicate, `503` | yes | Names are trimmed (`tenants.rs:58-75`). |
| GET | `/v1/tenants/{id}` | — | `200 TenantResponse` | `404`,`503` | yes | `tenants.rs:115-136` |
| DELETE | `/v1/tenants/{id}` | — | **`204`** | `404`,`503` | yes | No `PUT`/`PATCH` route exists (`tenants.rs:149-171`). |

### Branches

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/branches` | — | `200 {total_count,default_branch,branches[BranchResponse]}` | `500` | yes | `branches.rs:263-281` |
| POST | `/v1/branches` | `CreateBranchRequest{name,source_branch?,from_commit?}` | `201 BranchResponse` | `400 {"error":…,"status_code":400}` (bad name), `409`, `500` | yes | Name is sanitised (`branches.rs:40-84`); `from_commit` is accepted but ignored, the branch is cut from the source HEAD (`branches.rs:210-251`). |
| GET | `/v1/branches/{name}` | — | `200 BranchResponse` | `404`,`500` | yes | `branches.rs:297-318` |
| DELETE | `/v1/branches/{name}` | — | **`204`** | `404`,`409`,`500` | yes | `branches.rs:335-358` |
| POST | `/v1/branches/{name}/merge` | — (no body) | `200 {success,commit_sha,conflicts_detected,conflicts}` | `409 {…conflicts}`, `500` | yes | Checks out `main` first; a non-existent `main` yields `500` (`branches.rs:374-407`). |

`BranchResponse` = `{name,is_default,commit_sha,last_commit_message,created_at}`.

### Audit

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/audit` | query `AuditQuery{page,page_size,resource_type,action,user_id,since}` | `200 {total_count,events[AuditEvent],page,page_size,total_pages}` | `400 {"error":"Invalid resource_type filter"}` / `"Invalid action filter"` | yes | `since` is parsed but **never applied** (`audit.rs:145-217`). Defaults: page 1, page_size 50 clamped 1-100. |
| GET | `/v1/audit/export` | — | `200 text/csv` | — | yes | Header row always emitted; CSV escaping only doubles quotes (`audit.rs:252-294`). |

`AuditEvent` = `{id,timestamp,user_id,action,resource_type,resource_id,resource_name,details,ip_address,user_agent}`. The store is never written (see gaps), so both endpoints always return an empty set.

### Monitoring and metrics under `/v1`

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/monitoring/engine-state` | — | `200 {engine_name:"default",rules_fired:0,facts_inserted:0,active_sessions:0,memory_bytes:0,alpha_memory_size:0,beta_memory_size:0,network_traversals:0,execution_time_ms:0.0,last_updated}` | — | yes | Every counter is hard-coded zero (`routes/monitoring.rs:30-43`). |
| GET | `/v1/monitoring/health` | — | `200 {status:"healthy",service:"rrules-monitoring"}` | — | yes | `routes/monitoring.rs:54-59` |
| GET | `/v1/metrics/dead-rules` | — | `200 ["rule-id",…]` | — | yes | Reads the process-wide `RuleExecutionTracker` (`metrics.rs:395-408`). |
| GET | `/v1/metrics/coverage` | — | `200` JSON from `CoverageTracking::to_json()` | — | yes | `metrics.rs:419-427` |
| GET | `/v1/metrics/dmn` | — | `200 text/plain; version=0.0.4` | — | yes | Dedicated `DMN_METRICS_REGISTRY` (`dmn_metrics.rs:115-126`). |
| GET | `/v1/metrics/ws` | WS upgrade | `101` then `MetricsSnapshot` text frames | — | **no** (allowlist) | `websocket.rs:89-101,109-111` |
| GET | `/v1/metrics/dmn/ws` | WS upgrade | `101` then frames | — | **yes** — not in the allowlist | `dmn_metrics_ws.rs:53-69`; the prefix list has no matching entry (`middleware/auth.rs:53-60`). |

### Simulation

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| POST | `/v1/simulation` | `SimulationRunRequest{name,base_state?,proposed_changes,fact_sets}` | `200 SimulationRunResponse` | `400 {"error":"fact_sets must contain at least one fact group","status_code":400}` | yes | `simulation.rs:133-211`. |
| POST | `/v1/simulation/` | same | same | same | yes | Trailing-slash alias registered in `v1/mod.rs:48-49`. |

Runs against a fresh empty `EngineBuilder` engine, so `rules_fired` is empty in practice (`simulation.rs:145`, `api/tests/simulation_integration.rs:7-9`).

### Decisions (DMN)

| Method | Path | Request | Success response | Errors | Auth | Notes |
|---|---|---|---|---|---|---|
| GET | `/v1/decisions` | query `page`,`per_page` | `200 {decisions[{id,name,version,updated_at}],total,page,per_page,pages}` | `500` | yes | `per_page` default is **20** (`decisions.rs:107`); `updated_at` in summaries is `Utc::now()` at scan time (`engine_state/src/lib.rs:191`). |
| GET | `/v1/decisions/` | same | same | same | yes | Trailing-slash alias (`decisions.rs:528`). |
| POST | `/v1/decisions` | `CreateDecisionRequest{name,content,id?}` | `201 {id,name,content_length,metadata,created_at,updated_at}` | `409 {"error_code":"DMN_CONFLICT","message":…}`, `500` | yes | Submitted `metadata` is discarded; the sidecar only holds timestamps (`engine_state/src/lib.rs:148-157`). |
| GET | `/v1/decisions/{id}` | — | `200 {id,name,content_length,metadata,created_at,updated_at}` | `404`,`500` | yes | Deliberately omits the XML body; use `export` or `ir` (`decisions.rs:236-260`). |
| PUT | `/v1/decisions/{id}` | `UpdateDecisionRequest{content,metadata?}` | `200 {id,name,content_length,…}` | `404`,`500` | yes | `name` cannot be changed (`decisions.rs:301-345`). |
| DELETE | `/v1/decisions/{id}` | — | **`204`** | `404`,`500` | yes | `decisions.rs:348-365` |
| POST | `/v1/decisions/{id}/evaluate` | `EvaluateDecisionRequest{inputs,trace}` | `200 {decision_id,outputs,rules_fired,duration_ms,trace?}` | `404 {"error":…}`, `400 {"error_code":"DMN_HIT_POLICY_VIOLATION","policy","message"}`, `500` | yes | `METRIC_EVALUATIONS_TOTAL` is incremented before evaluation, so failed calls are counted (`decisions.rs:139-141`). |
| POST | `/v1/decisions/import` | `ImportDmnRequest{id,content: [u8]}` | `201 {status:"imported",id,message}` | `409 {"error_code":"DMN_CONFLICT",…}`, `500` | yes | Written as `{id}.dmn` via `save_file`; no `.meta.json` is created, so a later `get` synthesises timestamps (`decisions.rs:368-400`). |
| GET | `/v1/decisions/{id}/export` | — | `200 application/xml` | `404`,`500` | yes | `decisions.rs:404-427` |
| GET | `/v1/decisions/{id}/ir` | — | `200 {decision_id,name,decision_table}` | `400` (no table / parse failure), `404`,`500` | yes | `decisions.rs:436-491` |
| POST | `/v1/decisions/reload` | `ReloadDmnRequest{dmn_content}` | `200 {status:"reloaded",message}` | `500 {"error":…}` | yes | The only way the evaluated model changes; CRUD does not touch it (`decisions.rs:494-507`). |

## Error model

Four JSON envelopes appear in the code, plus actix's own extractor/routing
defaults and one unreachable fifth shape:

| Shape | Produced by | Status | Example |
|---|---|---|---|
| `{"error": "<msg>"}` | rules, categories, tags, approvals, audit, branches (partly), execution, sessions, decisions, tenants, monitoring | 400/401/404/409/500 | `api/src/v1/rules.rs:323-325` |
| `{"message": "<msg>"}` | auth handlers and the auth middleware only | 401/500 | `api/src/middleware/auth.rs:76-81`, `api/src/auth/mod.rs:148-158` |
| `{"error": msg, "status_code": n}` | branches, simulation only | 400/404/409/500 | `api/src/v1/branches.rs:95-99`, `api/src/v1/simulation.rs:138-141` |
| `{"error_code": …, "message"/"policy": …}` | decisions conflict / hit-policy violation | 400/409 | `api/src/v1/decisions.rs:189-193,288-293` |
| `ApiError` variant of the above | **unreachable** — `api/src/error.rs` is not part of any module tree | — | `api/src/error.rs:39-78` |

Conventions actually observed: `400` malformed ids/validation, `404` unknown
ids, `409` duplicates and merge conflicts, `500` lock/engine/IO failures, `503`
poisoned tenant-store locks (`tenants.rs:62-69`), `204` for deletes in
decisions/categories/tags/tenants/branches. Inconsistencies:

- Same meaning, different key: `error` vs `message` vs `error_code`; approvals
  `404` adds an extra `approval_id` field (`approvals.rs:216-219`).
- The extra `status_code` field appears only in branches, simulation and two
  category handlers — a client cannot rely on it globally.
- `POST /v1/rules/reload` returns a **success-shaped** body with HTTP `500` on
  failure (`execution.rs:1388-1392`).
- `POST /v1/rules/batch-execute` never fails the request: per-item failures are
  embedded as `BatchItemResult{success:false,error}` in a `200`
  (`execution.rs:1240-1275`).
- Extractor failures (`web::Json` on a missing/invalid field) bypass all of the
  above — no `JsonConfig` or custom extractor error is registered.

## Tenancy

- **Extraction:** `extract_tenant_id` reads header `x-tenant-id`
  (case-insensitive), falling back to the literal string `"default"` on absence
  or non-UTF-8 (`api/src/v1/rules.rs:32-39`). No validation, no cross-check
  against `/v1/tenants`, no auth-derived tenant.
- **Scoped:** only the rule handlers — `create_rule`, `get_rules`, `get_rule`,
  `update_rule`, `delete_rule`, `undelete_rule`
  (`api/src/v1/rules.rs:317,407,519,561,667,718`); store key is
  `(tenant_id, Uuid)` (`rules.rs:293`).
- **Not scoped:** categories, tags, rule-tag links, approvals, audit, branches,
  tenants, jobs, metrics, decisions, sessions, and the executing ruleset. A rule
  created via `POST /v1/rules` is invisible to `POST /v1/execute`, which reads
  the on-disk `SharedRuleSet` (`execution.rs:658-675`).
- **Consequence:** the header is the only isolation mechanism and it is
  spoofable — any authenticated caller reads another tenant's rules by sending a
  different value. Cross-origin browser callers cannot send it at all (CORS
  header list, `main.rs:263`), and no frontend module sets it.

## Persistence

| Data | Backing store | Survives restart | Source |
|---|---|---|---|
| Rules CRUD (`StoredRule`) | `HashMap<(tenant,Uuid),…>` in a `Mutex` | **no** | `api/src/v1/rules.rs:293-298` |
| Executing ruleset | `.rules`/`.drl` JSON files under `RULE_STORAGE_PATH`, loaded at startup and on reload | yes (files) | `execution.rs:168-229` |
| Categories | `HashMap<Uuid,…>` | **no** | `categories.rs:11,65-68` |
| Tags / rule-tag links | `HashMap<String,…>` + `HashSet<(Uuid,String)>` | **no** | `tags.rs:11-12,63-70` |
| Approvals | `HashMap<Uuid,…>` | **no** | `approvals.rs:11,70-73` |
| Audit events | `HashMap<Uuid,…>` | **no** (and never written) | `audit.rs:72,122-125` |
| Tenants | `engine::governance::tenant::MemoryTenantStore` under a `Mutex` | **no** | `tenants.rs:23-28` |
| Reload jobs | `RwLock<HashMap<String,ReloadJob>>` | **no** | `execution.rs:490-495` |
| Users | seeded `HashMap` | recreated identical at boot | `auth/store.rs:27-48` |
| Sessions | `DashMap` in `SessionManager`, TTL/LRU swept | **no** | `sessions.rs:92-105,272-312` |
| Branches / commits | git repository at `<RULE_STORAGE_PATH>/.git` (libgit2) | yes | `main.rs:201-214` |
| DMN documents | `<DMN_STORAGE_PATH>/{id}.dmn` + `{id}.meta.json` | yes | `engine_state/src/lib.rs:96-166,243-305` |
| Evaluated DMN model | in-process snapshot, replaced only by `POST /v1/decisions/reload` | re-read at boot (first file only) | `main.rs:102-142` |
| Prometheus counters, gauges | process memory | **no** | `metrics.rs:96-162` |

SQL backends: `SqlxTenantStore` is referenced only in `docs/API.md:469-471`;
the reachable tenant backend is `MemoryTenantStore` (`tenants.rs:18-23`).
`PostgresDmnStore` exists but every trait method is `todo!()`
(`engine_state/src/lib.rs:360-388`), so calling it panics; it is never
constructed by the server. `migrations/` is not exercised by any API code path.

## Realtime surfaces

Two WebSocket endpoints, both read-only upgrade handlers built on
`actix_web_actors::ws`.

| Path | Actor | Payload | Trigger | Public |
|---|---|---|---|---|
| `/v1/metrics/ws` | `MetricsWsSession` (`websocket.rs:37-80`) | `MetricsSnapshot{engine_name,rules_fired,facts_inserted,active_sessions,memory_bytes:0,last_updated}` (`metrics.rs:19-27`) | `metrics_tx.send(...)` after every successful `POST /v1/execute` — once per stateless call (`execution.rs:751-762`) and once per sessionful call (`:1004-1012`), including once per batch item (`:1220`) | yes |
| `/v1/metrics/dmn/ws` | `DmnMetricsWsSession` (`dmn_metrics_ws.rs:19-50`) | `DmnMetricsSnapshot{decision_id,rules_fired,duration_ms,timestamp}` (`dmn_metrics.rs:67-77`) | **nothing sends** — `evaluate_decision` never touches `DmnMetricsTx`, and the broadcaster drains the channel only to log (`dmn_metrics.rs:92-103`) | no (JWT required) |

Delivery mechanics: each session calls `subscribe()` on the shared sender and
polls with `try_recv` every 50 ms, emitting one JSON text frame per snapshot
(`websocket.rs:44-59`, `dmn_metrics_ws.rs:26-34`). There is no history/replay
and no back-pressure beyond the channel capacity of 16 (`main.rs:149,153`).
Inbound frames are ignored except `Ping`→`Pong` and `Close`→echo
(`websocket.rs:67-80`).

## Frontend contract

Client modules under `frontend/src/api`:

| Module | Base | Endpoints called | Backend reality |
|---|---|---|---|
| `client.ts` | axios, `import.meta.env.VITE_API_URL || '/v1'` (`:12`); injects `Authorization` from `localStorage.auth_token` (`:19-30`); on `401` clears storage and redirects to `/login` (`:38-43`) | — | Matches the `{"message"}`-only 401 from the middleware. |
| `execution.ts` | `/v1` | `POST /execute`, `POST /rules/batch-execute`, `GET /jobs/{id}`, `GET /jobs`, `GET /engines`, `POST /rules/reload` (`:33-77`) | All registered and shapes match. `reloadRules` expects `{job_id,status}` — backend sends `status:"pending"` (`execution.rs:1372-1375`). |
| `rules.ts` | `/v1` | `GET /rules`, `GET/POST /rules`, `PUT /rules/{id}`, `DELETE /rules/{id}`, **`PATCH /rules/{id}/status`** (`:48`), **`GET /rules/search`** (`:60`) | Two calls are missing server-side: `PATCH /rules/{id}/status` has no route; `GET /rules/search` collides with `GET /rules/{id}` and returns `400 "Invalid rule ID format"` for id=`search`. Additionally `rules.ts` unwraps `response.data.data` (`:17,23,29,35`) but the backend returns `RuleListResponse` / bare `RuleResponse` — **no `data` envelope exists** anywhere in `api/src`. |
| `decisions.ts` | `/v1` | `POST /decisions/{id}/evaluate`, `GET /decisions`, `GET /decisions/{id}`, `POST /decisions/reload`, `GET /decisions/{id}/ir` (`:107-166`) | Registered. `listDecisions` expects `model_version` (`:122-131`) which the backend never sends. `getDecision` expects `description/question/decision_table` (`:39-51`); `GET /v1/decisions/{id}` returns `content_length/metadata/created_at/updated_at` — the table only comes from `/ir`. |
| `types.ts` | — | Type declarations for execution payloads | Consistent with `execution.rs` schemas. |

Auth is fetched with raw `fetch`, not the axios client
(`frontend/src/lib/auth-context.tsx:49,74`), against
`${VITE_API_URL || '/v1'}/auth/me` and `/auth/login` — both registered. The dev
proxy forwards only `/v1` (`frontend/vite.config.ts:26-37`), so in development
`/metrics`, `/health/live` and `/swagger-ui` are unreachable through the dev
server.

## Known gaps

Dead or unreachable code:

| Item | Evidence |
|---|---|
| `api/src/error.rs` (`ApiError`, `map_to_not_found`) is compiled by nothing — no `mod error` exists in `api/src/lib.rs` or `api/src/main.rs` | `api/src/error.rs:8-90` |
| `api/src/v1/auth.rs` is orphaned: `v1/mod.rs` never declares `pub mod auth`, and the file imports `JwtClaims`/`encode_token` from `middleware::auth`, which do not exist there | `api/src/v1/auth.rs:14,92,116`; `api/src/v1/mod.rs:5-23` |
| `api/src/middleware/correlation.rs` is orphaned — `middleware/mod.rs` declares only `auth`, and no `wrap(CorrelationId)` exists | `api/src/middleware/mod.rs:3`; `api/src/middleware/correlation.rs:14` |
| `configure_v1_metrics` is defined but never called, so `GET /v1/metrics` (engine summary) and `POST /v1/metrics/coverage/reset` do not exist despite the module being compiled | `api/src/v1/mod.rs:35-42`; `api/src/v1/metrics.rs:6-23` |
| `get_audit_event` (GET /v1/audit/{id}) is implemented but unregistered | `api/src/v1/audit.rs:220-241` |
| `record_audit_event` has zero callers, so `/v1/audit` and `/v1/audit/export` are permanently empty | `api/src/v1/audit.rs:298-326` |
| `build_ruleset_from_dir`'s counterpart `EngineState` (+ `reload_from_directory`, `reload_engine`) is registered as `app_data` but never extracted; the git-log `from_commit` parameter is accepted and ignored | `api/src/main.rs:48,272`; `api/src/v1/execution.rs:295-378`; `branches.rs:216-251` |
| `start_file_watcher` is a stub (one `log::info!`), so `WATCH_RULES_DIR` has no functional effect | `api/src/v1/execution.rs:1444-1446` |
| DMN WS broadcaster drains and logs but never forwards; nothing publishes to `DmnMetricsTx` | `api/src/v1/dmn_metrics.rs:92-103` |
| Startup loads only the first `.dmn` file in the directory (arbitrary `read_dir` order), so a second decision file is invisible to `/evaluate` | `api/src/main.rs:109-118` |

Stub / hard-coded responses: `GET /v1/monitoring/engine-state` returns all
zeros (`routes/monitoring.rs:31-42`); `GET /metrics` HTML/JSON arms report
`memory_bytes: 0` and `engine_name: "default"` (`metrics.rs:338-360`);
`POST /v1/simulation` runs an empty engine so no rules fire
(`simulation.rs:145`); `POST /v1/rules/{id}/submit` never leaves `Draft`
(`approvals.rs:112`); `required_approvals`/`reviewers` are never enforced
(`approvals.rs:113-115`); `POST /v1/approvals/{id}/approve` injects the free-text
reason into `approved_by` (`approvals.rs:206-208`); `GET /v1/rules` silently
defaults `page_size` to 1 (`rules.rs:409`); the rule-duplicate check compares
two different category defaults (`rules.rs:346-349` vs `:364`); batch results
are documented as "in submission order" but are pushed in `join_next()`
completion order (`execution.rs:545` vs `:1240-1268`); `GET /v1/audit`'s `since`
filter is parsed and dropped (`audit.rs:68` vs `:145-193`).

Doc/code drift in `docs/API.md` (each item verified against the handlers):

| Claim | Reality |
|---|---|
| `GET /live`, `GET /ready` returning `{"status":"ok","service":"rrules-api"}` (`docs/API.md:47-57`) | Routes are `GET /health/live`, `GET /health/ready`, plus the aliases `GET /health` and `GET /ready` the deployment probes; all return an empty 200 body, never that JSON (`api/src/health.rs`) |
| `/metrics/dead-rules`, `/metrics/coverage` at root (`docs/API.md:92-119`) | Registered only as `/v1/metrics/dead-rules`, `/v1/metrics/coverage` (`api/src/metrics.rs:452-455`); root `/metrics` scope has a single route (`:437-442`) |
| Prometheus samples named `engine_rules_fired`, `engine_memory_bytes` (`docs/API.md:84-90`) | Only `rrules_execute_requests_total`, `rrules_execute_rules_fired_total`, `rrules_execute_duration_seconds`, `rrules_sessions_active` exist (`api/src/metrics.rs:100-137`) |
| `GET /v1/metrics/dmn` returns `DmnMetricsSnapshot` JSON (`docs/API.md:142-144`) | Returns Prometheus text (`api/src/v1/dmn_metrics.rs:115-126`) |
| `GET /v1/metrics/dmn/ws` needs no auth (`docs/API.md:148`) | Not in `PUBLIC_PREFIXES`; the `/v1` wrap 401s the upgrade (`api/src/middleware/auth.rs:53-60`) |
| `batch-execute`, `rules/reload`, `simulation` "Auth: none" (`docs/API.md:340,363,651`) | All three sit inside the `/v1` auth wrap and require a JWT (`api/src/app.rs:154-168`) |
| Async reload returns `status:"running"` (`docs/API.md:369`) | Returns `"pending"` (`api/src/v1/execution.rs:1372-1375`) |
| Batch `results` preserve submission order (`docs/API.md:357`) | Completion order (`execution.rs:1240-1268`) |
| Decisions `per_page` default 50 (`docs/API.md:391`) | 20 (`api/src/v1/decisions.rs:107`) |
| `GET /v1/decisions/{id}` includes `content` (`docs/API.md:403`) | Returns `content_length` instead (`decisions.rs:243-250`) |
| `DELETE` on decisions/categories/tags/branches returns `200`/`404` (`docs/API.md:410,500,522,584`) | `204` with no body (`decisions.rs:355`, `categories.rs:263`, `tags.rs:243`, `branches.rs:342`) |
| `GET /v1/tags` returns `[TagResponse]` (`docs/API.md:509`) | Returns `{total_count,tags}` (`tags.rs:124-127`) |
| Pending approvals are an array of objects with `reviewers`/`required_approvals` (`docs/API.md:539-541`) | `{total_count,approvals[{id,rule_id,maker_id,state,created_at}]}` (`approvals.rs:153-162`) |
| `POST /v1/branches` body has only `source_branch`/`from_commit` (`docs/API.md:575`) | `name` is required and 400s when missing (`branches.rs:23-25,40-46`) |
| env table lists 3 variables (`docs/API.md:695-701`) | 13 are read by the server (see the env table above) |
| `SqlxTenantStore` is "a placeholder" for tenants (`docs/API.md:469-471`) | The API uses `MemoryTenantStore`; no SQL tenant code is referenced from `api/` (`tenants.rs:18-23`) |
