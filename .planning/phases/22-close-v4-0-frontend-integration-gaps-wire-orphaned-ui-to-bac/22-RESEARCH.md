# Phase 22: Close v4.0 frontend integration gaps - wire orphaned UI to backend - Research

**Researched:** 2026-07-02
**Domain:** Frontend↔backend wiring/closure (Rust/actix-web backend + React/TS/TanStack frontend)
**Confidence:** HIGH (every audit file:line claim verified against the live codebase)

## Summary

Phase 22 is a **wiring/closure phase**, not a greenfield build. The v4.0 backend (phases 18–21) is substantially complete and reachable; the Phase 17 React frontend is a scaffolding skeleton whose components exist but are orphaned from the router and backend. A single systematic root cause — frontend `baseURL /api/v1` vs backend `/v1` scope (no `/api` anywhere) — breaks every axios flow at runtime (404 → falls through to `fs::Files`). This is compounded by an unmounted `AuthProvider`, an incomplete hand-maintained route tree (only `/` and `/rules`), three governance UIs that are stub/mock-only, an unregistered metrics WebSocket, and an unwrapped auth middleware.

I verified **every** file:line claim in `.planning/v4-MILESTONE-AUDIT.md` and `.planning/.continue-here.md` against the live code. All 8 integration gaps are confirmed present; two audit citations have minor drift (the execution stub function is `execute_single` at `execution.rs:312`, not `execute_rules`; the SPA fallback is at lines 155–159, not exactly 156). One critical contradiction is **resolved in D-22-01's favor**: the backend has **ZERO** auth endpoints today (no `/v1/auth/login`, `/v1/auth/me`, `/v1/auth/refresh` — grep of `api/src/` returns nothing). STATE.md's "P1 done — login/refresh endpoints at /v1/auth/*" claim is **false/stale** and must not be relied on. The auth middleware `requires_auth` (`api/src/middleware/auth.rs:6`) is confirmed a presence-only stub (checks `Bearer ` prefix, never validates a JWT, never `wrap()`ed).

I also surfaced three findings the audit did **not** flag, all load-bearing for the planner: (1) **`actix-cors` is NOT present** in `api/Cargo.toml`, `Cargo.lock`, or `api/src/` — STATE.md's P2 "actix-cors with configurable origins" claim is false; D-22-05's separate-host prod URL mode requires CORS or same-origin fallback; (2) the backend `create_branch` handler (`branches.rs:89`) **hardcodes `"feature/default"`** as the branch name — `CreateBranchRequest` has no `name` field, so the frontend's name input is silently ignored (a real backend gap blocking GOV-04 "create branches" end-to-end); (3) `@tanstack/react-router` is pinned at `^1.8.0` while the current `@tanstack/router-plugin` is `1.168.19` — a large version gap the planner must reconcile before codegen setup.

**Primary recommendation:** Break this large phase into **5 plans, one per gap cluster** in dependency order: (1) API path alignment `/v1` + vite proxy, (2) pragmatic auth (in-memory store + `/v1/auth/*` + JWT validation upgrade of `requires_auth`), (3) TanStack router codegen + mount all orphaned routes + AuthProvider mount, (4) real-time metrics WebSocket streaming, (5) governance UI wiring (audit export, branch merge + conflict resolution + side-by-side diff, approvals). Plan 1 is the foundation; plans 2–5 depend on it but can largely proceed in parallel after it lands.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-22-01:** Pragmatic minimal auth — in-memory user store seeded with 4 predefined roles (admin, author, reviewer, viewer) + demo users; build `/v1/auth/login`, `/v1/auth/me` (+refresh); signed JWTs using existing `API_SECRET_KEY` (HMAC); upgrade `requires_auth` from presence-only stub to JWT validation + role resolution; wrap on mutating `/v1/*` with public allowlist (login, /health, /swagger-ui, /api-docs). Backend currently has ZERO auth endpoints (STATE.md "P1 done" is false — verified). Passwords may be plaintext/lightly hashed for demo; production hashing deferred.
- **D-22-02:** Backend enforces "authenticated vs public" for v4.0 (public allowlist + authenticated on protected routes). Finer per-route role enforcement deferred to v5.0; frontend `usePermission` already gates UI by role.
- **D-22-03:** Split to v5.0 — full production auth (Postgres user store per D-18-01, argon2/bcrypt, refresh-token rotation, per-route role enforcement) AND migration of all v4.0 in-memory stores to Postgres → new milestone v5.0: Persistence & Identity. Phase 22 does NOT introduce Postgres and does NOT migrate existing stores. **Flag v5.0 milestone creation.**
- **D-22-04:** Align frontend to `/v1` — drop `/api` everywhere. `client.ts:6` baseURL → `/v1`; `auth-context.tsx` fetch paths → `/v1/auth/*`; `.env.*` `VITE_API_URL` → `/v1`-based. Retarget vite proxy from `/api` to `/v1` → `http://localhost:8080` (or add a `/v1` proxy). No backend route prefix change (stays `web::scope("/v1")`).
- **D-22-05:** Keep `VITE_API_URL` configurable absolute (default points at backend `/v1`; prod can override to separate API host like `https://api.rrules.example.com/v1`). Make `client.ts` (absolute) and `auth-context.tsx` (relative) consistent — both via the configured client / `VITE_API_URL`.
- **D-22-06:** Proper TanStack file-based routing with codegen — add `@tanstack/router-plugin` to vite; route files under `frontend/src/routes/` are source of truth; auto-generate the route tree so it cannot drift. Replace the hand-maintained `_layout.gen.ts` with generated output.
- **D-22-07:** Mount routes for every orphaned component — `/monitoring` (+ dashboard sub-routes for DeadRuleReport, RuleCoverage, EngineMetrics), `/governance/approvals`, `/governance/audit`, `/governance/branches`, `/governance/rbac`, and rule authoring routes (D-22-08). Mirror existing `features/{governance,versioning,monitoring,rules}` structure.
- **D-22-08:** Dedicated edit routes — `/rules/new` (create) and `/rules/:id/edit` (edit), rendering `RuleEditor` with `DecisionTable` and `HitPolicySelector` as sub-components/tabs. Deep-linkable. NOT a modal/drawer.
- **D-22-09:** Mount `AuthProvider` in `frontend/src/main.tsx:19-26` so `useAuth()`/`usePermission()` stop throwing.
- **D-22-10:** Implement real WebSocket streaming now — register `configure_metrics_ws` in `api/src/main.rs`; flesh out the handler to broadcast from the existing `metrics_tx` channel; wire `useEngineMetrics.ts` WS path; fix the frontend fetch path (`/api/v1/metrics` → `/v1/monitoring/engine-state` + WS). Data is empty while `execute_rules` is a stub (accepted tech debt).
- **D-22-11:** Full functional wiring + conflict resolution + side-by-side diff. AuditLogViewer calls `/v1/audit` (list) + `/v1/audit/export` (CSV/JSON). BranchManager calls `/v1/branches` CRUD + `/v1/branches/{name}/merge` AND implements interactive conflict-resolution UI + side-by-side diff view. ApprovalQueue mounted and calls `/approvals/*`. Real fetch, loading, error, empty states throughout.
- **D-22-12:** Phase 22 gets its own git branch `gsd/phase-22-frontend-wiring` off the current `gsd/phase-19-monitoring-metrics` (or `main`). **Pending user confirmation at plan time.**

### Claude's Discretion
- JWT token lifetimes and exact demo-user credentials/roles for the in-memory store.
- Exact vite proxy retarget mechanics (new `/v1` proxy entry vs retarget `/api` with rewrite) — achieve "frontend targets `/v1`" any clean way.
- Route file layout/naming under TanStack file-based conventions.
- Polling interval and WS reconnect/backoff specifics in `useEngineMetrics`.
- Whether the metrics WS endpoint lives at `/v1/monitoring/ws` or `/metrics/ws` (pick one and align both sides).

### Deferred Ideas (OUT OF SCOPE)
- Full production auth: Postgres user/role/permission store (D-18-01), argon2/bcrypt password hashing, refresh-token rotation, per-route role enforcement (D-22-02) → v5.0.
- Migrate all v4.0 in-memory stores (rules, categories, tags, approvals, audit, branches) to PostgreSQL (D-22-03) → v5.0.
- Real-time RBAC sync via WebSocket subscriptions; multi-tenant governance isolation → v5.0.
- `execute_rules` stub (`api/src/v1/execution.rs:312`) stays a stub — separate future execution milestone. Downstream: metrics/dead-rules/coverage legitimately return empty until execution is wired.
- Verification debt: phases 20/21 have no formal VERIFICATION.md (optional to add during closure).
- `frontend/src/api/rules.ts:48,60` call unregistered backend routes (`PATCH /rules/{id}/status`, `GET /rules/search`) — wire or remove if reached, else leave documented.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FRONTEND-01 | Complete rule authoring interface (web IDE, decision table editor, guided editor, real-time validation, side-by-side version comparison) | `RuleEditor`/`DecisionTable`/`HitPolicySelector` exist but orphaned; D-22-06/07/08 mount dedicated `/rules/new` + `/rules/:id/edit` routes; D-22-11 adds side-by-side diff view for branches. Backend `RuleEditor` components + `/v1/rules` CRUD + `/v1/rules/{id}/git-log` registered. |
| FRONTEND-02 | Rule management dashboard (list w/ filters, edit/clone/delete, health indicators, bulk ops) | `useRules`→`getRules`→`/rules` is code-wired but 404s (baseURL) + `/rules` route crashes (no AuthProvider). D-22-04 fixes path; D-22-09 mounts AuthProvider; D-22-06/07 re-registers route. `rules.ts:48,60` latent unregistered routes flagged as tech debt. |
| GOV-01 | Complete RBAC (admin/author/reviewer/viewer) | `requires_auth` exists but never wrapped + presence-only stub. D-22-01 builds `/v1/auth/*`, upgrades to JWT validation + role resolution, wraps protected routes. D-22-09 mounts AuthProvider so frontend `usePermission` works. Per-route role enforcement deferred (D-22-02). |
| GOV-02 | Maker-checker approval workflow | Backend approval engine + `/v1/approvals/*` registered; `ApprovalQueue.tsx:17` contract matches (`approvals` array). Broken only by baseURL + unmounted. D-22-04 fixes path; D-22-07 mounts `/governance/approvals` route. |
| GOV-03 | Full audit trail (track modifications, exportable CSV/JSON) | Backend `/v1/audit` + `/v1/audit/export` registered. `AuditLogViewer.tsx:27-38` uses hardcoded mock. D-22-11 wires real fetch + export. **Contract mismatch** found (see Architecture Patterns) — needs mapping layer. |
| GOV-04 | Version control & branching (create branches, merge with conflict resolution, history, side-by-side diff) | Backend `/v1/branches` CRUD + `/merge` + `/v1/rules/{id}/git-log` registered; merge returns 409 with `conflicts` array (supports conflict UI). `BranchManager.tsx` is commented stubs. D-22-11 implements real wiring + conflict resolution + diff. **Backend gap** found: `create_branch` hardcodes name (see Pitfalls). |
| MON-02 | Real-time monitoring dashboard | Backend `/v1/monitoring/engine-state` registered (returns single object); `configure_metrics_ws` defined but never called; `useEngineMetrics.ts` fetches wrong path `/api/v1/metrics` and expects an array. D-22-10 registers WS, fixes paths. **Contract mismatch** found (array vs object) — needs adapter. |
| MON-03 | Dead rule detection | Backend `/metrics/dead-rules` (root, via `configure_metrics`) reads `RuleExecutionTracker`; `DeadRuleReportDashboard.tsx` orphaned. D-22-07 mounts `/monitoring` dashboard sub-route. Data empty (execution stub — accepted). |
| MON-04 | Rule coverage reporting | Backend `/metrics/coverage` (root) reads `CoverageTracking` singleton; `RuleCoverageDashboard.tsx` orphaned. D-22-07 mounts `/monitoring` dashboard sub-route. Data empty (execution stub — accepted). |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| API path alignment (`/v1`) | Frontend Server (vite proxy) / Browser (client.ts) | API Backend (no change) | The mismatch is entirely a frontend/proxy config problem; backend stays `/v1`. Dev uses vite proxy; prod uses `VITE_API_URL` absolute. |
| Auth endpoints (`/v1/auth/*`) | API / Backend | Browser (auth-context consumer) | New backend handlers + JWT signing must live in the API tier; frontend only consumes. |
| JWT validation / role resolution | API / Backend (middleware) | — | `requires_auth` middleware upgrade is backend-only request gating. |
| AuthProvider mount | Browser / Client | — | `main.tsx` provider composition is client-side React tree assembly. |
| Route tree codegen | Build tooling (vite plugin) | Browser (consumes generated tree) | `@tanstack/router-plugin` runs at vite build time; browser consumes `routeTree.gen.ts`. |
| Orphaned route mounting | Browser / Client | — | Route files under `frontend/src/routes/` are client-side. |
| Metrics WebSocket streaming | API / Backend (ws handler) | Browser (useEngineMetrics hook) | Backend owns the broadcast channel + WS upgrade; browser subscribes. |
| Governance UI wiring | Browser / Client | API / Backend (data source) | Components fetch from existing backend endpoints; backend already complete. |
| Conflict resolution + diff UI | Browser / Client | API / Backend (merge 409 + git-log) | Interactive UI is client-side; backend surfaces conflict data via 409. |
| Static SPA serving | API / Backend (`fs::Files`) | CDN (prod, optional) | Single-container deployment serves SPA at root; relevant to same-origin vs separate-host URL decision (D-22-05). |

## Standard Stack

### Core (Frontend — already installed, no new install unless noted)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@tanstack/react-router` | `^1.8.0` (installed; **STALE — latest `1.168.19`**) | Type-safe React router | Project standard (CLAUDE.md stack). Needs upgrade to a current `1.x` compatible with the router-plugin. `[VERIFIED: package.json + npm view]` |
| `@tanstack/router-plugin` | `1.168.19` (NOT installed — **ADD**) | Vite plugin for file-based routing codegen | The official TanStack codegen plugin; generates `routeTree.gen.ts` so the tree cannot drift. `[VERIFIED: ctx7 /tanstack/router + npm view]` |
| `@tanstack/react-query` | `^5.50.0` (installed) | Server-state / data fetching | Already in use via `apiClient`; no change. `[VERIFIED: package.json]` |
| `axios` | `^1.6.0` (installed) | HTTP client (`apiClient`) | Existing `client.ts` uses axios; D-22-04 retargets baseURL. `[VERIFIED: package.json]` |
| `react` / `react-dom` | `19.2.0` (installed) | UI runtime | Existing. `[VERIFIED: package.json]` |
| `vite` | `^5.4.21` (installed) | Build/dev server + proxy | Existing; `@vitejs/plugin-react ^4.7.0` also installed. `[VERIFIED: package.json]` |
| `zustand` | `^4.4.0` (installed) | Client state | Existing stack. `[VERIFIED: package.json]` |

### Core (Backend — Rust)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `actix-web` | `4.4` with `ws` feature (installed) | HTTP server + WebSocket upgrade | Existing server; **`ws` feature already enabled** — no new dep for WS. `[VERIFIED: api/Cargo.toml]` |
| `jsonwebtoken` (`/keats/jsonwebtoken`) | `10.4.0` (NOT installed — **ADD**) | HS256 HMAC JWT encode/decode + validation | The standard, widely-used Rust JWT crate (Keats Xu; 10 yrs on crates.io, 2.7M downloads/wk, verdict OK). D-22-01 needs it for signing with `API_SECRET_KEY`. `[VERIFIED: ctx7 /keats/jsonwebtoken + cargo search]` |
| `actix-cors` | latest (NOT installed — **ADD** to honor D-22-05) | CORS middleware | Required for D-22-05's separate-host prod URL mode. **Verified ABSENT** despite STATE.md's P2 claim. `[VERIFIED: api/Cargo.toml + Cargo.toml + Cargo.lock grep — none]` |
| `futures-util` | `0.3` (installed) | Stream utilities for WS | Existing; used in WS handler patterns. `[VERIFIED: api/Cargo.toml]` |
| `utoipa` / `utoipa-swagger-ui` | `4` / `7` (installed) | OpenAPI docs | Existing; add utoipa `#[path]` attrs to new auth endpoints. `[VERIFIED: api/Cargo.toml]` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@tanstack/router-devtools` | `^1.8.0` (installed) | Router devtools | Keep; upgrade alongside react-router. `[VERIFIED: package.json]` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `@tanstack/router-plugin` (codegen) | Hand-maintained `_layout.gen.ts` | **Rejected (D-22-06)** — hand-maintaining is exactly why the tree drifted to only `/` and `/rules`. Codegen is the fix. |
| `jsonwebtoken` (Keats) | `jwt` crate | Keats `jsonwebtoken` is the de-facto standard (20× more usage, official docs, 10 yrs). `[VERIFIED]` |
| Separate-host prod (`VITE_API_URL` absolute) | Same-origin (vite proxy / `fs::Files`) | Same-origin is the Phase 21 deployment default (single container serves SPA at root). Separate-host needs `actix-cors`. D-22-05 keeps it configurable — add CORS to support it. |

**Installation:**
```bash
# Frontend (new deps)
cd frontend && npm install @tanstack/router-plugin
# Also upgrade @tanstack/react-router to a current 1.x compatible with the plugin
# (installed ^1.8.0 is far behind latest 1.168.19)
npm install @tanstack/react-router@latest @tanstack/router-devtools@latest

# Backend (new deps)
# In api/Cargo.toml [dependencies]:
#   jsonwebtoken = "10.4"
#   actix-cors = "0.7"   # to honor D-22-05 separate-host mode
```

**Version verification:**
```bash
npm view @tanstack/router-plugin version      # → 1.168.19  [VERIFIED: npm registry]
cargo search jsonwebtoken                     # → 10.4.0   [VERIFIED: crates.io]
# actix-web ws feature already in api/Cargo.toml: actix-web = { version = "4.4", features = ["macros","ws"] }  [VERIFIED]
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `@tanstack/router-plugin` | npm | recent republish 2026-07-01 | ~20M/wk | github.com/TanStack/router | SUS (false positive — "too-new" is latest-publish date, not package age) | Approved — official TanStack org, 20M weekly downloads. Planner should add a `checkpoint:human-verify` before install per protocol, but signals strongly confirm legitimacy. |
| `jsonwebtoken` (Keats) | crates.io | 10 yrs (since 2015) | ~2.7M/wk | github.com/Keats/jsonwebtoken | OK | Approved |
| `actix-cors` | crates.io | (verify at install) | high | github.com/actix/actix-net (actix-cors) | OK (assumed — official actix org) | `[ASSUMED]` — verify via `cargo search actix-cors` at install time |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** `@tanstack/router-plugin` — flagged SUS by the seam only because the latest version was published 2026-07-01 (same-day). This is a false positive: the package has ~20M weekly downloads, the official `TanStack/router` repo, and is the documented codegen plugin. The `publishedAt` reflects the latest version publish, not package creation. Planner should still add `checkpoint:human-verify` before install per protocol.

*`@tanstack/router-plugin` was discovered via the project CLAUDE.md ctx7 rule and confirmed via `npm view` and the ctx7 `/tanstack/router` docs; tagged `[VERIFIED: ctx7 + npm registry]`.*

## Architecture Patterns

### System Architecture Diagram

```
                      ┌─────────────────────────────────────────────────────────────┐
                      │                     BROWSER (React SPA)                      │
                      │                                                              │
                      │  main.tsx: <AuthProvider> → <RouterProvider>                  │
                      │     └─ routeTree.gen.ts (CODEGEN from src/routes/*)           │
                      │           ├─ /                (index)                         │
                      │           ├─ /rules           (list)                          │
                      │           ├─ /rules/new       (create — D-22-08)             │
                      │           ├─ /rules/:id/edit  (edit — D-22-08)              │
                      │           ├─ /monitoring/*    (dashboards — D-22-07)        │
                      │           └─ /governance/{approvals,audit,branches,rbac}    │
                      │                                                              │
                      │  apiClient.ts (axios, baseURL=VITE_API_URL=/v1) ──┐           │
                      │  auth-context.tsx (via client, →/v1/auth/*) ──────┤           │
                      │  useEngineMetrics (WS + HTTP poll →/v1/monitoring)┤           │
                      └──────────────────────────────────────────────────┼───────────┘
                                                                       │
                          ┌────────────────────────────────────────────┴───────────────┐
                          │   DEV: vite proxy /v1 → http://localhost:8080             │
                          │   PROD: VITE_API_URL absolute (needs actix-cors) OR same-origin │
                          └────────────────────────────────────────────────┬───────────┘
                                                                          │
              ┌───────────────────────────────────────────────────────────┴───────────────┐
              │                  API BACKEND (actix-web 4.4, 127.0.0.1:8080)               │
              │                                                                          │
              │  Public allowlist: /health, /swagger-ui, /api-docs, /v1/auth/login         │
              │  wrap(requires_auth) on protected /v1/*  (D-22-01 — JWT validation)        │
              │                                                                          │
              │  web::scope("/v1"):                                                      │
              │    /auth/login, /auth/me, /auth/refresh   (NEW — D-22-01)                │
              │    /rules CRUD, /rules/{id}/git-log                                      │
              │    /approvals/{pending,/{id}/approve,/{id}/reject}                       │
              │    /branches CRUD + /{name}/merge (409 → conflicts[])                    │
              │    /audit, /audit/export                                                  │
              │    /monitoring/engine-state, /monitoring/health                          │
              │    /metrics/ws  (NEW WS — D-22-10, broadcast from metrics_tx)            │
              │  configure_metrics (root): /metrics, /metrics/dead-rules, /metrics/coverage │
              │  fs::Files::new("/") serves SPA (last)                                   │
              └──────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
frontend/src/
├── main.tsx                  # mount <AuthProvider><RouterProvider/></AuthProvider>
├── routeTree.gen.ts          # GENERATED by @tanstack/router-plugin (replaces _layout.gen.ts)
├── routes/                   # source of truth for routes (file-based)
│   ├── __root.tsx            # root layout (replaces _layout.tsx)
│   ├── index.tsx             # /
│   ├── rules/
│   │   ├── index.tsx         # /rules (list)
│   │   ├── new.tsx           # /rules/new (create — D-22-08)
│   │   └── $id.edit.tsx      # /rules/:id/edit (edit — D-22-08)
│   ├── monitoring/
│   │   ├── index.tsx         # /monitoring (dashboard shell)
│   │   ├── dead-rules.tsx    # /monitoring/dead-rules
│   │   ├── coverage.tsx      # /monitoring/coverage
│   │   └── engine.tsx        # /monitoring/engine (metrics)
│   └── governance/
│       ├── approvals.tsx     # /governance/approvals
│       ├── audit.tsx         # /governance/audit
│       ├── branches.tsx      # /governance/branches
│       └── rbac.tsx          # /governance/rbac
├── api/
│   ├── client.ts             # baseURL → /v1 (via VITE_API_URL)
│   ├── rules.ts              # fix latent routes (lines 48,60) or document
│   └── auth.ts               # NEW (optional) — auth API via client
├── lib/
│   └── auth-context.tsx      # fetch paths → /v1/auth/* via client
├── hooks/
│   └── useEngineMetrics.ts   # WS + HTTP paths fixed
└── features/                 # existing components — now mounted

api/src/
├── main.rs                   # register configure_metrics_ws; wrap requires_auth; add /v1/auth/*
├── middleware/
│   └── auth.rs               # upgrade requires_auth → JWT validation + role
├── auth/                     # NEW — auth endpoints (login/me/refresh) + in-memory user store
│   └── mod.rs
├── websocket.rs              # flesh out metrics_ws (real WS upgrade + broadcast)
└── v1/
    ├── execution.rs:312      # execute_single STUB — out of scope (accepted tech debt)
    ├── audit.rs              # contract: AuditEvent {id,timestamp,user_id,action,resource_type,resource_id,resource_name,details,ip_address,user_agent}
    ├── branches.rs           # GAP: create_branch hardcodes "feature/default" (line 89)
    └── routes/monitoring.rs  # /v1/monitoring/engine-state (single object response)
```

### Pattern 1: TanStack Router Vite Plugin (file-based codegen)
**What:** The vite plugin watches `src/routes/` and auto-generates `routeTree.gen.ts`; route files use `createFileRoute` and are the source of truth.
**When to use:** D-22-06 — replace the hand-maintained `_layout.gen.ts`.
**Example:**
```typescript
// vite.config.ts
// Source: https://github.com/tanstack/router/blob/main/docs/router/installation/with-vite.md
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { tanstackRouter } from '@tanstack/router-plugin/vite'

export default defineConfig({
  plugins: [
    // MUST come before react()
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
  ],
  server: {
    port: 5173,
    proxy: {
      '/v1': { target: 'http://localhost:8080', changeOrigin: true }, // D-22-04
    },
  },
})
```
```typescript
// main.tsx — consume generated tree
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { routeTree } from './routeTree.gen'   // generated, not hand-maintained
const router = createRouter({ routeTree })
declare module '@tanstack/react-router' { interface Register { router: typeof router } }
// Wrap with AuthProvider (D-22-09):
// <AuthProvider><RouterProvider router={router} /></AuthProvider>
```
```typescript
// routes/rules/new.tsx — file-based route (D-22-08)
import { createFileRoute } from '@tanstack/react-router'
export const Route = createFileRoute('/rules/new')({ component: RuleEditor })
```

### Pattern 2: JWT signing/validation (Rust, HS256)
**What:** Sign JWTs with `API_SECRET_KEY` (HMAC HS256); validate in `requires_auth` middleware; extract role claim.
**When to use:** D-22-01.
**Example:**
```rust
// Source: https://github.com/keats/jsonwebtoken (ctx7 /keats/jsonwebtoken)
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, Algorithm};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,       // user id
    role: String,      // admin | author | reviewer | viewer
    exp: usize,        // required — expiration (UTC timestamp)
    iat: usize,        // issued at
}

// Sign (login endpoint)
let claims = Claims {
    sub: user_id.to_string(),
    role: user.role.clone(),
    exp: (now + 3600) as usize,  // 1h lifetime (planner picks default)
    iat: now as usize,
};
let token = encode(
    &Header::default(),  // HS256 by default
    &claims,
    &EncodingKey::from_secret(std::env::var("API_SECRET_KEY")?.as_bytes()),
)?;

// Validate (upgraded requires_auth middleware)
let token = /* extract Bearer token */;
let data = decode::<Claims>(
    &token,
    &DecodingKey::from_secret(std::env::var("API_SECRET_KEY")?.as_bytes()),
    &Validation::new(Algorithm::HS256),  // validates exp by default
)?;
let role = data.claims.role;  // resolved role for request gating
```

### Pattern 3: actix-web WebSocket upgrade (metrics streaming)
**What:** Real WS handler broadcasting from the existing `metrics_tx` channel.
**When to use:** D-22-10.
**Example:**
```rust
// Source: actix-web ws feature (enabled in api/Cargo.toml)
use actix_web_actors::ws;
use actix::Actor;

// metrics_ws: subscribe to metrics_tx broadcast, push EngineState to client
async fn metrics_ws(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    metrics_tx: web::Data<MetricsTx>,  // already app_data in main.rs:78
) -> Result<HttpResponse, actix_web::Error> {
    let session = MetricsWsSession { rx: metrics_tx.subscribe() };
    ws::start(session, &req, stream)
}
// Register: configure_metrics_ws (websocket.rs:30) → call it in main.rs /v1 scope
```

### Anti-Patterns to Avoid
- **Hand-maintaining `_layout.gen.ts`:** This is the exact cause of the orphaned-component bug. The codegen plugin exists to make the tree self-maintaining. Do NOT edit the generated file by hand.
- **Splitting auth-context fetch from the apiClient:** `auth-context.tsx` currently uses raw `fetch('/api/v1/auth/*')` while `client.ts` uses axios — two code paths, two baseURL sources of truth. D-22-05 requires both to go through the configured client / `VITE_API_URL`.
- **Adding a backend `/api` scope:** D-22-04 explicitly chose "align frontend to `/v1`" — do NOT add `/api` to the backend to "match" the frontend; that preserves the mismatch at a different layer and complicates prod.
- **Treating `create_branch` as working:** The handler hardcodes `"feature/default"` (verified). Wiring the frontend modal naively will silently ignore the user's branch name. Fix the backend or document as tech debt.
- **Expecting metrics HTTP polling to match the WS payload shape:** `useEngineMetrics` HTTP fallback expects a `MetricsData[]` array; `/v1/monitoring/engine-state` returns a single `EngineStateResponse` object. These are different shapes — needs an adapter, not a direct swap.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Route tree management | Hand-maintained route array (`_layout.gen.ts`) | `@tanstack/router-plugin` codegen | Hand-maintaining is what caused the drift (only `/`, `/rules`). Codegen scales and cannot forget a route. |
| JWT signing/validation | Custom HMAC+base64 token code | `jsonwebtoken` (Keats) crate | Token validation has many edge cases (alg confusion, exp, nbf, leeway). Battle-tested crate avoids them. |
| WebSocket framing | Raw TCP framing | `actix-web` `ws` feature (already enabled) | WS handshake/frame protocol is deceptively complex; actix `ws::start` handles it. |
| CORS (if separate-host prod) | Manual `Access-Control-Allow-*` headers | `actix-cors` middleware | Preflight handling, origin matching, credential rules are easy to get wrong. |
| Password hashing (demo) | Custom hash | `String` compare for demo (D-22-01 permits plaintext) OR a crate if hashing — **defer real hashing to v5.0** | D-22-01 explicitly allows plaintext for the demo; do not over-build. |

**Key insight:** This phase is wiring, not invention. Every backend endpoint the frontend needs already exists and is registered (verified). The work is configuration (paths/proxy), composition (mount providers/routes), and replacing stubs with real fetch calls. Resist scope creep into execution engine, Postgres, or production auth.

## Runtime State Inventory

This phase is primarily a wiring/closure phase (not a rename/refactor/migration), but it touches runtime configuration that drifts between git and the live process. Categories checked:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | In-memory stores only (rules, approvals, audit, branches, tags, categories, jobs) — all `init_store()` in `main.rs:32-38`. No database. No data migration needed (D-22-03 defers Postgres to v5.0). | None — new in-memory auth store follows the same pattern (D-22-01). |
| Live service config | `API_SECRET_KEY` env var (Helm secret placeholder from Phase 21) — reused for JWT signing, key unchanged. `RULE_STORAGE_PATH`, `FRONTEND_DIST`, `WATCH_RULES_DIR` env vars (`main.rs:41,60,66`). | None — reuse `API_SECRET_KEY` for JWT (D-22-01). |
| OS-registered state | None — no Task Scheduler/launchd/systemd/pm2 registrations embed the changed strings. | None — verified by grep (no platform-specific registrations in repo). |
| Secrets/env vars | `.env.development` / `.env.production` `VITE_API_URL` (currently `/api/v1`-based) — code edit. `API_SECRET_KEY` (backend) — unchanged. | Code edit — update `VITE_API_URL` to `/v1`-based (D-22-04/05). |
| Build artifacts | `frontend/dist/` (built SPA served by `fs::Files`) — regenerated by `npm run build`. `target/` (Rust build) — regenerated by `cargo build`. `routeTree.gen.ts` will be newly generated. | Rebuild after wiring; the generated route tree is a new build artifact (gitignore or commit per project convention). |

**Nothing found in category:** No OS-registered state, no external-service config outside git. The only "runtime state" is in-memory and resets on restart — acceptable for v4.0 (D-22-03 defers persistence).

## Common Pitfalls

### Pitfall 1: The auth-endpoint contradiction (STATE.md vs reality)
**What goes wrong:** STATE.md claims "P1 done — login/refresh endpoints at /v1/auth/*". The planner might assume auth endpoints exist and only need wiring. They do NOT exist (verified: grep of `api/src/` for auth routes returns nothing).
**Why it happens:** STATE.md's "Multi-Track Execution Progress" section is stale/aspirational — it lists P1 as complete, but the code never landed (or was lost). The milestone audit (B2) did not catch this because it focused on Phase 17 frontend.
**How to avoid:** Treat D-22-01 as **building auth from scratch** (in-memory store + 3 endpoints + JWT). Do not plan a "wire existing auth" task. **Verified: backend has ZERO auth endpoints.**
**Warning signs:** Any plan task that says "connect to existing /v1/auth/login" is wrong — it must say "create /v1/auth/login".

### Pitfall 2: `actix-cors` absence breaks D-22-05 prod mode
**What goes wrong:** D-22-05 allows `VITE_API_URL=https://api.rrules.example.com/v1` (separate host). The browser will block cross-origin requests without CORS headers. STATE.md's P2 claims "actix-cors with configurable origins" — **this is false** (verified absent in `api/Cargo.toml`, `Cargo.lock`, and `api/src/`).
**Why it happens:** Same stale STATE.md as Pitfall 1 — the P2 hardening item did not actually land.
**How to avoid:** Add `actix-cors` as part of the auth/path plan (small, ~5 lines) to honor D-22-05. OR default to same-origin (Phase 21 deploys a single container serving the SPA via `fs::Files::new("/")`, so same-origin works out of the box in prod too). Recommend: add `actix-cors` so D-22-05's configurability is real, not theoretical.
**Warning signs:** Frontend login works in dev (vite proxy, same-origin) but fails in prod with CORS errors in the browser console.

### Pitfall 3: `create_branch` hardcodes the branch name
**What goes wrong:** The frontend `CreateBranchModal` captures a `name`, but the backend `create_branch` handler (`branches.rs:89`) calls `vcs.lock().await.create_branch("feature/default", source)` — a hardcoded `"feature/default"`. `CreateBranchRequest` (`branches.rs:22-29`) has only `source_branch` and `from_commit`, no `name`. Every "New Branch" creates a branch named `feature/default` regardless of input; creating a second one fails (branch already exists → 409).
**Why it happens:** The backend handler is a partial implementation — the name field was never wired through.
**How to avoid:** The governance UI plan (cluster 5) MUST include a backend fix: add `name` to `CreateBranchRequest` and pass it to `create_branch`. This is in-scope for D-22-11 ("calls `/v1/branches` CRUD") — CRUD implies working create. Flag as a required backend task, not just frontend wiring.
**Warning signs:** Creating two branches in a row fails; the branch list always shows `feature/default`.

### Pitfall 4: Contract mismatches between frontend types and backend responses
**What goes wrong:** Several frontend interfaces don't match the backend JSON. Direct swap of mock→real fetch will produce runtime undefined-field errors.
**Why it happens:** The frontend was scaffolded against assumed shapes; backend evolved separately.
- **Audit:** frontend `AuditEvent` (`AuditLogViewer.tsx:121-130`: `eventId`, `eventType`, `actorId`, `actorName`, `subjectType`, `subjectId`, `occurredAt`, `payload`) vs backend `AuditEvent` (`audit.rs:35-46`: `id`, `timestamp`, `user_id`, `action`, `resource_type`, `resource_id`, `resource_name`, `details`, `ip_address`, `user_agent`). Completely different field names.
- **Branches:** frontend `Branch` (`BranchManager.tsx:86-92`: `id`, `name`, `isDefault`, `createdAt`, `lastCommitSha?`) vs backend `BranchResponse` (`branches.rs:33-39`: `name`, `is_default`, `commit_sha`, `last_commit_message`, `created_at`). Backend has no `id`; `commit_sha` vs `lastCommitSha`; `BranchListResponse` wraps in `{total_count, default_branch, branches[]}`.
- **Metrics:** `useEngineMetrics` HTTP fallback expects `MetricsData[]` (array) but `/v1/monitoring/engine-state` returns a single `EngineStateResponse` object.
**How to avoid:** Each wiring task needs a mapping/adapter layer (or update the frontend types to match the backend). Verify the backend contract by reading the `#[derive(Serialize)]` struct, not by assuming.
**Warning signs:** UI renders `undefined` for fields after switching from mock to real data.

### Pitfall 5: `@tanstack/react-router` version is far behind the plugin
**What goes wrong:** Installing `@tanstack/router-plugin@1.168.19` alongside `@tanstack/react-router@^1.8.0` may produce type/runtime incompatibilities (the plugin generates code targeting the current router API).
**Why it happens:** The codebase pins an early `1.8.0`; the router has progressed to `1.168.x`.
**How to avoid:** Upgrade `@tanstack/react-router` and `@tanstack/router-devtools` to a current `1.x` in the same plan that adds the plugin. Verify the generated `routeTree.gen.ts` compiles before proceeding.
**Warning signs:** TypeScript errors in `routeTree.gen.ts`, or `createFileRoute` not exported from the installed react-router version.

### Pitfall 6: WebSocket handler is a stub returning JSON, not a real WS upgrade
**What goes wrong:** The current `metrics_ws` (`websocket.rs:18-27`) returns `HttpResponse::Ok().json({...})` — a plain JSON response, NOT a WebSocket upgrade. Even if `configure_metrics_ws` is registered, the frontend `new WebSocket(url)` will fail the handshake.
**Why it happens:** The handler was a placeholder.
**How to avoid:** D-22-10 must rewrite `metrics_ws` to use `actix_web_actors::ws::start` (the `ws` feature is already enabled in `api/Cargo.toml`). Registering the existing stub is not enough.
**Warning signs:** Frontend `ws.onerror` fires immediately; browser shows "WebSocket connection failed".

## Code Examples

### Verified metric HTTP path fix (D-22-10)
```typescript
// frontend/src/hooks/useEngineMetrics.ts
// CURRENT (wrong): fetch('/api/v1/metrics') at lines 110, 141
// Backend reality: /v1/monitoring/engine-state returns a SINGLE EngineStateResponse object
// Fix: target the real endpoint AND adapt the single object to the hook's MetricsData shape
const API_BASE = import.meta.env.VITE_API_URL || '/v1'
const response = await fetch(`${API_BASE}/monitoring/engine-state`)
if (response.ok) {
  const state: EngineStateResponse = await response.json()  // single object, NOT array
  // adapt: map EngineStateResponse -> MetricsData
  setData([{
    engineName: state.engine_name,
    metrics: [{
      timestamp: state.last_updated,
      rulesFired: state.rules_fired,
      factsInserted: state.facts_inserted,
      executionTimeMs: state.execution_time_ms,
    }],
  }])
}
```

### Verified audit fetch (D-22-11)
```typescript
// AuditLogViewer.tsx — replace mockEvents (lines 27-38) with real fetch
// Backend contract (audit.rs:48-56): { total_count, events: AuditEvent[], page, page_size, total_pages }
// Backend AuditEvent (audit.rs:35-46): { id, timestamp, user_id, action, resource_type,
//   resource_id, resource_name, details, ip_address, user_agent }
const API_BASE = import.meta.env.VITE_API_URL || '/v1'
const params = new URLSearchParams()
if (filter.eventType) params.set('action', filter.eventType)  // backend uses 'action'
const res = await fetch(`${API_BASE}/audit?${params}`)
const data: { total_count: number; events: BackendAuditEvent[] } = await res.json()
// map backend -> frontend display shape
setEvents(data.events.map(mapBackendAuditEvent))
// Export:
window.open(`${API_BASE}/audit/export?format=csv`, '_blank')  // backend: GET /v1/audit/export
```

### Verified conflict-resolution fetch (D-22-11)
```typescript
// BranchManager.tsx — replace commented stubs (lines 70-83)
// Backend merge_branch (branches.rs:262+) returns 409 with conflicts[] on conflict
const API_BASE = import.meta.env.VITE_API_URL || '/v1'
async function handleMerge(branch: Branch) {
  const res = await fetch(`${API_BASE}/branches/${branch.name}/merge`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
  })
  if (res.status === 409) {
    const { conflicts } = await res.json()  // backend surfaces conflicts[]
    setConflictState({ branch: branch.name, conflicts })  // render conflict-resolution UI
    return
  }
  if (res.ok) { /* merged — refresh list */ fetchBranches() }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-maintained route tree (`_layout.gen.ts`) | `@tanstack/router-plugin` codegen | Current | Route tree auto-generates from files; cannot drift. This phase adopts it (D-22-06). |
| `requires_auth` presence-only stub | JWT validation with `jsonwebtoken` crate | This phase (D-22-01) | Real (demo-grade) auth enforcement for the first time. |
| Mock/stub governance UIs | Real fetch + loading/error/empty states | This phase (D-22-11) | GOV-02/03/04 become verifiable end-to-end. |

**Deprecated/outdated:**
- `_layout.gen.ts` (hand-maintained): replace with generated `routeTree.gen.ts`.
- `fetch('/api/v1/...')` direct calls in `auth-context.tsx`: route through `apiClient` / `VITE_API_URL` (D-22-05).
- STATE.md "P1/P2 done" claims (auth endpoints, actix-cors): **verified false** — do not rely on them.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `actix-cors` is the correct/official crate for CORS in actix-web 4.4 | Standard Stack | Low — it is the standard actix CORS crate, but version compatibility with actix-web 4.4 should be confirmed at install (`cargo add actix-cors`). |
| A2 | `@tanstack/router-plugin` latest (`1.168.19`) is compatible with `@tanstack/react-router` latest `1.x` (lockstep versioning) | Standard Stack / Pitfall 5 | Low — TanStack versions router-plugin and react-router together, but the planner should verify the generated `routeTree.gen.ts` compiles before building on it. |
| A3 | The in-memory auth store should follow the existing `init_store()` pattern (`Arc<Mutex<HashMap<...>>>`) | Architecture Patterns | Low — this is the established v4.0 pattern (D-22-01 confirms no Postgres). |
| A4 | `approvals.rs` `ApprovalActionRequest` accepts an `approver_id` field (frontend sends `{ approver_id: 'current-user' }`) | Code Examples | Low — the frontend already sends this (ApprovalQueue.tsx:31); the backend handler takes `web::Json<ApprovalActionRequest>`. Verify the struct field name at implementation time. |

**All other claims in this research were verified against the live codebase, ctx7 docs, or the npm/crates registries (tagged `[VERIFIED: source]` inline or in the audit table).**

## Open Questions (RESOLVED)

1. **`actix-cors` — add it, or default to same-origin prod?** — RESOLVED: implemented in plan 22-01 Task 3 (add `actix-cors = "0.7"`, register with configurable allowed origins, never `any()` + `allow_credentials(true)`).
   - What we know: `actix-cors` is absent (verified). Phase 21 deploys a single container serving the SPA at root (`fs::Files::new("/")`), so same-origin works in prod by default. D-22-05 wants `VITE_API_URL` to be configurable-absolute for a separate API host.
   - What's unclear: Whether the user's deployment actually splits SPA and API across hosts, or if D-22-05 is future-proofing.
   - Recommendation: Add `actix-cors` (small) so D-22-05's configurability is real. It is the declared locked decision. Flag for planner confirmation.

2. **`create_branch` name field — fix backend or document as tech debt?** — RESOLVED: implemented in plan 22-06 Task 1 (add `name` to `CreateBranchRequest`, pass through to `create_branch`, `sanitize_branch_name` guard, P-5 regression test).
   - What we know: `branches.rs:89` hardcodes `"feature/default"`; `CreateBranchRequest` has no `name`. GOV-04 acceptance criteria include "Create branches for rule development."
   - What's unclear: Whether this is a quick backend fix (add `name` to the request struct + pass through) or a deeper VCS limitation.
   - Recommendation: Treat as an in-scope backend fix in the governance UI plan (cluster 5) — D-22-11 says "calls `/v1/branches` CRUD", and working create is part of CRUD. Small change; don't defer.

3. **Metrics WS endpoint path (`/v1/metrics/ws` vs `/metrics/ws`)** — RESOLVED: implemented in plan 22-04 Task 1 (`configure_metrics_ws` registered inside the `/v1` scope → `/v1/metrics/ws`; frontend `useEngineMetrics` WS path updated to match in Task 2).
   - What's unclear: `websocket.rs` doc says `/v1/metrics/ws` but `configure_metrics_ws` scopes it as `/metrics/ws` (root). If registered inside the `/v1` scope it becomes `/v1/metrics/ws`; if at root, `/metrics/ws`. `useEngineMetrics.ts:26` defaults to `ws://localhost:8080/metrics/ws`.
   - Recommendation (Claude's discretion per CONTEXT): Register inside the `/v1` scope → `/v1/metrics/ws`, and update the frontend default to match. Keeps all API under `/v1` consistently.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js / npm | Frontend build | ✓ | (npm present) | — |
| Rust / cargo | Backend build | ✓ | (cargo present) | — |
| `actix-web` `ws` feature | Metrics WebSocket (D-22-10) | ✓ | 4.4 (with `ws`) | — |
| `jsonwebtoken` crate | JWT auth (D-22-01) | ✗ | — | Add as dependency |
| `actix-cors` crate | CORS for separate-host prod (D-22-05) | ✗ | — | Same-origin mode (but limits D-22-05) |
| `@tanstack/router-plugin` | Router codegen (D-22-06) | ✗ | — | None — required for D-22-06 |
| vite dev server proxy | Dev API routing (D-22-04) | ✓ | 5.4.21 | — |

**Missing dependencies with no fallback:**
- `@tanstack/router-plugin` — required for D-22-06 (codegen). Must install.
- `jsonwebtoken` — required for D-22-01 (JWT auth). Must install.

**Missing dependencies with fallback:**
- `actix-cors` — fallback is same-origin prod (works with Phase 21 single-container deploy), but this disables D-22-05's separate-host configurability. Recommend installing.

## Validation Architecture

**Nyquist validation is enabled** (`.planning/config.json` `workflow.nyquist_validation: true`).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust: `cargo test` (integration/unit, actix-web test utils); Frontend: vitest (verify presence at Wave 0 — not confirmed in package.json grep) |
| Config file | `api/Cargo.toml` (rust); `frontend/vite.config.ts` (no vitest config seen — **Wave 0 gap**) |
| Quick run command | `cargo test --workspace -p api --lib` ; `cd frontend && npm run build` (typecheck) |
| Full suite command | `cargo test --workspace` (currently 232 pass) ; `cd frontend && npm run build` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FRONTEND-01 | `/rules/new` and `/rules/:id/edit` routes render RuleEditor | smoke (build) | `cd frontend && npx tsc --noEmit && npm run build` | ❌ Wave 0 (route files to create) |
| FRONTEND-02 | `/rules` list loads via `/v1/rules` without crashing | integration | `cargo test -p api --test rules` (existing) | ✅ (existing api tests) |
| GOV-01 | `requires_auth` rejects missing/invalid JWT; accepts valid | unit | `cargo test -p api --lib auth` | ❌ Wave 0 (auth module to create) |
| GOV-01 | Protected `/v1/*` route returns 401 without token | integration | `cargo test -p api --test auth_protection` | ❌ Wave 0 |
| GOV-01 | `/v1/auth/login` returns signed JWT; `/v1/auth/me` resolves user | integration | `cargo test -p api --test auth_endpoints` | ❌ Wave 0 |
| GOV-02 | ApprovalQueue mounted; `/approvals/pending` returns list | integration/smoke | `cargo test -p api --test approvals` | ✅ (existing) + frontend mount smoke |
| GOV-03 | AuditLogViewer calls `/v1/audit`; export returns CSV/JSON | integration | `cargo test -p api --test audit` | ✅ (existing) + frontend wiring smoke |
| GOV-04 | BranchManager creates (with name), lists, merges; 409 surfaces conflicts | integration | `cargo test -p api --test branches` | ✅ (existing) + backend create-name fix test |
| MON-02 | WS `/v1/metrics/ws` upgrades and streams; frontend reconnects on drop | integration | `cargo test -p api --test metrics_ws` | ❌ Wave 0 (WS handler to implement) |
| MON-02 | `useEngineMetrics` targets `/v1/monitoring/engine-state` (not `/api/v1/metrics`) | smoke (build + grep) | `cd frontend && npm run build && ! grep -r "/api/v1/metrics" src/` | ❌ Wave 0 |
| MON-03/04 | Dashboards mounted under `/monitoring/*`; call `/metrics/dead-rules` + `/metrics/coverage` | smoke (build) | `cd frontend && npm run build` | ❌ Wave 0 (route files) |

### Sampling Rate
- **Per task commit:** `cargo test -p api --lib` (fast affected-module tests) + `cd frontend && npx tsc --noEmit` (typecheck). Frontend has no confirmed vitest setup — typecheck + build is the fast gate.
- **Per wave merge:** `cargo test --workspace` (full, 232 baseline — must not regress) + `cd frontend && npm run build` (full build green) + `cargo clippy --workspace --all-all-targets` (0 warnings baseline).
- **Phase gate:** Full suite green before `/gsd-verify-work`. Plus property-style checks: (a) every route in the generated `routeTree.gen.ts` resolves to a mounted component (no orphans); (b) `grep -r "/api/v1" frontend/src/` returns nothing (path alignment complete); (c) a request to a protected `/v1/*` route without a JWT returns 401; (d) a JWT with a wrong-signature secret is rejected; (e) WS connects to `/v1/metrics/ws` and receives a frame.

### Wave 0 Gaps
- [ ] Frontend test framework: confirm/install vitest (no vitest config seen in `vite.config.ts`); if absent, frontend validation is limited to typecheck + build + manual smoke. Planner should decide whether to add vitest or rely on build + manual/E2E.
- [ ] `api/tests/auth_*.rs` — covers GOV-01 (auth endpoints + middleware protection)
- [ ] `api/tests/metrics_ws.rs` — covers MON-02 (WS upgrade + broadcast)
- [ ] `api/tests/branches.rs` create-with-name test — covers GOV-04 (backend fix)
- [ ] Frontend route files under `frontend/src/routes/` (rules/new, rules/$id.edit, monitoring/*, governance/*) — covers FRONTEND-01, MON-02/03/04, GOV-02/03/04 mounting

## Security Domain

**Security enforcement is enabled** (config default). This phase introduces auth for the first time, so the security domain is directly relevant.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | yes | `jsonwebtoken` crate (HS256, `exp` validated by default); in-memory user store (demo). Plaintext passwords permitted by D-22-01 for demo only — production hashing deferred to v5.0. `[VERIFIED: ctx7 /keats/jsonwebtoken]` |
| V3 Session Management | yes | JWT in `localStorage` (existing `client.ts:15` + `auth-context.tsx:36`); token lifetime per planner (discretion). Note: `localStorage` JWT is XSS-exposable — accepted for v4.0 demo; review for v5.0. |
| V4 Access Control | yes (coarse) | `requires_auth` middleware upgraded to validate JWT + resolve role; wrap on protected `/v1/*` with public allowlist (D-22-01/02). Per-route role enforcement deferred (D-22-02). |
| V5 Input Validation | yes | actix-web `web::Json<T>` deserialization (existing pattern); utoipa schemas for new auth endpoints. |
| V6 Cryptography | yes | `jsonwebtoken` HS256 using `API_SECRET_KEY` — never hand-roll HMAC. `[VERIFIED: ctx7]` |

### Known Threat Patterns for Rust/actix-web + React/JWT

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| JWT alg-confusion (attacker sends `alg:none`) | Tampering | `Validation::new(Algorithm::HS256)` pins the algorithm; `jsonwebtoken` rejects `none`. Do not use `Validation::default()` without pinning. `[CITED: keats/jsonwebtoken validation docs]` |
| Missing auth on mutating routes | Elevation of privilege | `wrap(requires_auth)` on `/v1/*` with explicit public allowlist (D-22-01). Verify no mutating route is accidentally public. |
| XSS exfiltrating JWT from localStorage | Information disclosure | Accepted for v4.0 demo (D-22-01). v5.0 should move to httpOnly cookies. Document as accepted tech debt. `[ASSUMED — standard guidance]` |
| CORS misconfiguration (wildcard origin + credentials) | Spoofing | If adding `actix-cors`, never combine `any()` origin with `allow_credentials(true)`. Use configurable allowed origins. `[ASSUMED — standard guidance]` |
| Branch name injection (Git) | Tampering | `create_branch` fix (Pitfall 3) must validate/sanitize the name before passing to `VersionControlRepo::create_branch`. `[ASSUMED]` |

## Sources

### Primary (HIGH confidence)
- Live codebase (verified 2026-07-02): `api/src/main.rs`, `api/src/middleware/auth.rs`, `api/src/websocket.rs`, `api/src/v1/{execution,audit,approvals,branches,rules}.rs`, `api/src/v1/routes/monitoring.rs`, `api/src/metrics.rs`, `api/Cargo.toml`, `frontend/src/{main.tsx,api/client.ts,api/rules.ts,lib/auth-context.tsx,hooks/useEngineMetrics.ts,routes/_layout.gen.ts,features/**/*}`, `frontend/vite.config.ts`, `frontend/.env.*`, `frontend/package.json` — every audit file:line claim checked.
- ctx7 `/tanstack/router` docs — vite plugin codegen config, file-based conventions, `routeTree.gen.ts` consumption. `[CITED: github.com/tanstack/router docs]`
- ctx7 `/keats/jsonwebtoken` docs — HS256 encode/decode, Claims struct, Validation config. `[CITED: github.com/keats/jsonwebtoken]`
- `.planning/v4-MILESTONE-AUDIT.md`, `.planning/.continue-here.md`, `.planning/HANDOFF.json` — gap list, evidence, prior decisions.
- `.planning/phases/22-.../22-CONTEXT.md` — locked decisions D-22-01..12.

### Secondary (MEDIUM confidence)
- `npm view @tanstack/router-plugin version` → 1.168.19 (npm registry)
- `cargo search jsonwebtoken` → 10.4.0 (crates.io)
- gsd-tools `package-legitimacy check` → jsonwebtoken OK, router-plugin SUS (false positive, analyzed)

### Tertiary (LOW confidence)
- `actix-cors` compatibility with actix-web 4.4 (`[ASSUMED]` — verify at install)
- Vitest presence in frontend (`[ASSUMED]` not present — verify at Wave 0)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified against package.json/Cargo.toml/registries; library docs via ctx7.
- Architecture: HIGH — every file:line claim verified in the live code; contract mismatches found and documented.
- Pitfalls: HIGH — three new pitfalls (actix-cors absence, create_branch hardcode, version gap) discovered by direct code inspection, not assumed.
- Security: MEDIUM — JWT patterns verified via ctx7; localStorage-JWT XSS and CORS guidance is standard but not library-specific.

**Research date:** 2026-07-02
**Valid until:** 2026-08-01 (30 days — stable wiring phase; library versions are current as of research date)