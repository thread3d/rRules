# Phase 22: Close v4.0 frontend integration gaps - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-01
**Phase:** 22-Close v4.0 frontend integration gaps
**Areas discussed:** Auth depth & enforcement, API path strategy, Router strategy, Real-time metrics (MON-02), Governance UI depth, Branch

---

## Auth depth & enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Pragmatic minimal auth | In-memory users + login/me + signed JWTs + upgraded requires_auth wrapped on mutating routes | ✓ (final, for P22) |
| Full production auth | JWT signing keys, password hashing, refresh rotation, Postgres user store, full middleware enforcement | ✓ (initial, then split) |
| Frontend-only (defer backend auth) | Mount AuthProvider + fix token flow; backend stays permissive | |

**User's choice:** Initially "Full production auth", then (after scope check) chose **Split into v5.0** — Phase 22 reverts to pragmatic minimal; full production auth + Postgres → new v5.0 milestone.
**Notes:** Scope check surfaced that "full auth + Postgres-for-everything" exceeds the "frontend wiring closure" boundary and re-platforms the backend. User split deliberately. Also surfaced a key scout finding: the backend has ZERO auth endpoints and `requires_auth` is only a presence-only stub — STATE.md's "P1 done" claim is false.

### User store (follow-up within auth)
| Option | Description | Selected |
|--------|-------------|----------|
| PostgreSQL (per D-18-01) | Postgres for users only | |
| In-memory store, prod-grade crypto | In-memory users + real crypto/enforcement | |
| Postgres for everything | Migrate all v4.0 stores to Postgres | ✓ (initial) |

**User's choice:** Initially "Postgres for everything", then reversed via the scope-check split → v5.0.

### P22 auth (after split)
| Option | Description | Selected |
|--------|-------------|----------|
| Pragmatic minimal | In-memory users + login/me + signed JWT + wrap requires_auth | ✓ |
| Frontend-only, defer to v5.0 | Mount AuthProvider only, no backend auth | |

---

## API path strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Align frontend to /v1 | Drop /api from frontend; retarget vite proxy | ✓ |
| Add /api scope to backend | Wrap /v1 in /api; keep frontend /api/v1 | |
| Vite proxy rewrite only | Rewrite /api→'' in dev; prod still breaks | |

### URL mode (follow-up)
| Option | Description | Selected |
|--------|-------------|----------|
| Relative /v1 everywhere | Same-origin; vite proxy dev, reverse proxy prod | |
| Configurable absolute host | VITE_API_URL override; separate API host + CORS in prod | ✓ |

**User's choice:** Align frontend to /v1 + keep VITE_API_URL configurable-absolute.

---

## Router strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Proper file-based codegen | @tanstack/router-plugin; route files as source of truth | ✓ |
| Extend route tree | Hand-extend _layout.gen.ts | |
| Manual route registry | Plain config, no codegen | |

### Rule editor access (follow-up, FRONTEND-01)
| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated edit routes | /rules/new + /rules/:id/edit | ✓ |
| Modal/drawer from list | No separate route | |

---

## Real-time metrics (MON-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Polling now, defer WS | Poll /v1/monitoring/engine-state; defer WS as tech debt | |
| Implement WS now | Register configure_metrics_ws, broadcast metrics_tx, wire useEngineMetrics WS | ✓ |

**User's choice:** Implement WS now (honors D-19-02; data empty while execute_rules stub is accepted tech debt).

---

## Governance UI depth

| Option | Description | Selected |
|--------|-------------|----------|
| Full functional wiring | Real fetch + export + merge + approve; defer conflict-resolution UI | |
| Minimal list-only wiring | Just list endpoints, no actions | |
| Full plus conflict resolution | Full wiring + interactive conflict resolution + side-by-side diff | ✓ |

**User's choice:** Full plus conflict resolution. Note: this is within-scope (GOV-04 + FRONTEND-01 acceptance explicitly require conflict resolution + side-by-side diff), not scope creep.

---

## Branch

| Option | Description | Selected |
|--------|-------------|----------|
| Own branch for Phase 22 | gsd/phase-22-frontend-wiring | ✓ (default, accepted by silence) |
| Stay on current branch | gsd/phase-19-monitoring-metrics | |

**User's choice:** Recommended default accepted (pending explicit confirmation at plan time).

---

## Claude's Discretion

- JWT token lifetimes and exact demo-user credentials/roles
- Exact vite proxy retarget mechanics (achieve "frontend targets /v1" any clean way)
- Route file layout/naming under TanStack file-based conventions
- Polling interval and WS reconnect/backoff in useEngineMetrics
- Metrics WS endpoint path (`/v1/monitoring/ws` vs `/metrics/ws`) — pick one, align both sides

## Deferred Ideas

- **v5.0: Persistence & Identity** (new milestone — flag for creation): full production auth (Postgres user store, password hashing, refresh rotation, per-route role enforcement) + migration of all v4.0 in-memory stores to Postgres.
- Real-time RBAC sync via WebSocket; multi-tenant governance isolation (from 18-CONTEXT).
- Accepted tech debt (do not re-litigate): `execute_rules` stub; phases 20/21 verification debt; latent unregistered route calls in `frontend/src/api/rules.ts:48,60`.