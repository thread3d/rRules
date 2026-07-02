---
phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
plan: 05
subsystem: frontend-governance
tags: [governance, audit, approvals, frontend, react, typescript, goV-02, GOV-03]
requires:
  - 22-01 (apiClient baseURL + CORS)
  - 22-02 (JWT auth context)
  - 22-03 (router mount + AuthProvider)
provides:
  - "AuditLogViewer wired to /v1/audit + /v1/audit/export (GOV-03)"
  - "ApprovalQueue wired to /v1/approvals/* with real fetch (GOV-02)"
affects:
  - frontend/src/features/governance/audit/AuditLogViewer.tsx
  - frontend/src/features/governance/approval/ApprovalQueue.tsx
tech-stack:
  added: []
  patterns:
    - "Backend→frontend contract adapter (Pitfall 4 / P-6 backstop)"
    - "Shared apiClient (VITE_API_URL /v1) with auth interceptor"
    - "Authenticated blob download for CSV/JSON export"
key-files:
  created: []
  modified:
    - frontend/src/features/governance/audit/AuditLogViewer.tsx
    - frontend/src/features/governance/approval/ApprovalQueue.tsx
decisions:
  - "Used shared apiClient (axios) for both audit + approval fetch instead of raw fetch — keeps auth token + baseURL handling in one place (D-22-04/D-22-05)"
  - "Export implemented as authenticated apiClient blob download rather than window.open — window.open cannot send the Authorization header; blob download keeps JWT auth on the export endpoint"
  - "ApprovalQueue approver_id resolved from useAuth() user.id with 'current-user' fallback for the demo (RESEARCH A4)"
  - "Refresh pending approvals from backend after approve/reject instead of local filter — keeps list state authoritative"
metrics:
  duration: ~12m
  completed: 2026-07-02
  tasks: 2
  files: 2
status: complete
---

# Phase 22 Plan 05: Governance UI Wiring — Audit + Approvals Summary

JWT-authenticated audit log viewer and approval queue wired to real backend endpoints via the shared apiClient, with backend→frontend contract adapters and loading/error/empty states. Closes GOV-02 (maker-checker approval workflow) and GOV-03 (audit trail + CSV/JSON export) end-to-end.

## What Was Built

### Task 1: AuditLogViewer real fetch + export + contract adapter (GOV-03, Pitfall 4)

Replaced the hardcoded `mockEvents` array in `frontend/src/features/governance/audit/AuditLogViewer.tsx` with a real fetch against `GET /v1/audit` through the shared `apiClient` (baseURL = `VITE_API_URL`, default `/v1`, auth token auto-injected).

- **Contract adapter** `mapBackendAuditEvent` maps the backend `AuditEvent` shape (`id, timestamp, user_id, action, resource_type, resource_id, resource_name, details, ip_address, user_agent` — see `api/src/v1/audit.rs:35-46`) to the frontend display shape (`eventId, eventType, actorId, actorName, subjectType, subjectId, resourceName, occurredAt, payload`). This is the Pitfall 4 / P-6 backstop — no `undefined` field renders after the mock→real swap.
- **Filter mapping**: frontend `eventType` → backend `action` query param; `startTime` → `since`; added `resourceType` → `resource_type` and `actorId` → `user_id`. Includes `page`/`page_size` pagination params.
- **Export**: CSV and JSON export via authenticated `apiClient.get('/audit/export', { params: { format }, responseType: 'blob' })` triggering a blob download. Chose blob download over `window.open` because `window.open` cannot attach the `Authorization` header (Rule 2 — security/correctness: export endpoint is auth-protected by 22-02).
- **States**: loading message, error banner with Retry button, empty-state message, pagination controls (Previous/Next + count).
- No `mockEvents`, no `/api/v1` strings.

### Task 2: ApprovalQueue real fetch + loading/error/empty states (GOV-02)

`frontend/src/features/governance/approval/ApprovalQueue.tsx` already used the shared `apiClient` and had loading/empty states; completed the wiring:

- **Approver id**: resolved from `useAuth()` `user.id` with `'current-user'` fallback for the demo (RESEARCH A4 / `ApprovalActionRequest.approver_id`).
- **Refresh after action**: replaced the local `setApprovals(prev => prev.filter(...))` with a full `fetchPendingApprovals()` refresh from the backend after a successful approve/reject, so the pending list stays authoritative.
- **States**: kept loading + empty states; added an error banner with Retry button and a per-row `acting` state that disables Approve/Reject buttons while a mutation is in flight.
- No `/api/v1` strings, no mock data. Calls resolve to `/v1/approvals/pending`, `/v1/approvals/{id}/approve`, `/v1/approvals/{id}/reject`.

## Verification

| Check | Result |
|-------|--------|
| `cd frontend && npx tsc --noEmit` | PASS (0 errors) |
| `cd frontend && npm run build` | PASS (built in ~5.3s, only chunk-size advisory warnings) |
| `cargo test -p api --lib` | PASS (25 passed, 0 failed — incl. `v1::audit::tests::*` and `v1::approvals::tests::*`) |
| No `mockEvents` / mock arrays in AuditLogViewer | PASS |
| No `/api/v1` strings in either file | PASS |
| AuditLogViewer references `/audit` + `/audit/export` | PASS |
| AuditLogViewer contains `mapBackendAuditEvent` adapter | PASS |
| ApprovalQueue references `/approvals/pending` + `/approve` + `/reject` | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Security/correctness] Export uses authenticated blob download instead of `window.open`**
- **Found during:** Task 1
- **Issue:** Plan action specified `window.open('/v1/audit/export?format=csv', '_blank')`, but the export endpoint is auth-protected (JWT from 22-02). `window.open` cannot attach the `Authorization: Bearer` header, so the export would 401.
- **Fix:** Implemented export as `apiClient.get('/audit/export', { responseType: 'blob' })` (auth interceptor injects the token) and triggered a download via a generated object URL. Still goes through `VITE_API_URL` (/v1) and opens `/audit/export?format=csv|json` per the success criterion.
- **Files modified:** `frontend/src/features/governance/audit/AuditLogViewer.tsx`
- **Commit:** 8b891051a

No other deviations — plan executed as written.

## Known Stubs

None. Both files now fetch real backend data via authenticated `apiClient`; no hardcoded/mock data remains.

## Threat Flags

None. No new trust boundaries beyond those already in the plan's `<threat_model>` (Browser → /v1/audit, /v1/approvals/* — authenticated via 22-02 JWT, mutating routes protected by `wrap(requires_auth)`).

## TDD Gate Compliance

Plan `type: execute` (not `tdd`) — no RED/GREEN/REFACTOR gate required.

## Self-Check: PASSED

- `frontend/src/features/governance/audit/AuditLogViewer.tsx` — FOUND
- `frontend/src/features/governance/approval/ApprovalQueue.tsx` — FOUND
- Commit `8b891051a` (feat 22-05 audit) — FOUND
- Commit `c8ee62ced` (feat 22-05 approvals) — FOUND