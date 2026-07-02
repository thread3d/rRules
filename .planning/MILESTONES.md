# Milestones

## v4.0 Production Platform (Shipped: 2026-07-02)

**Closeout:** verified — all milestone phases verified PASS, 0 open artifacts, 0 deferred items.
**Scope:** Phases 17–22 (6 phases, ~12 plans, 27 tracked tasks)
**Timeline:** 2026-06-10 → 2026-07-02 (22 days, 89 commits)
**Codebase:** ~31,500 LOC (Rust + TypeScript)

**Delivered:** A complete production-ready BRMS platform — full React/TypeScript web frontend wired end-to-end to a Rust backend with enterprise governance, Prometheus monitoring, complete management API, and Docker/Kubernetes deployment with hot reload.

**Key accomplishments:**

- Production React/TypeScript frontend — CodeMirror rule-authoring IDE, decision-table editor with all DMN hit policies, guided editors, and rule-management dashboard with filtering/sorting (Phase 17)
- Enterprise governance — full RBAC (admin/author/reviewer/viewer) with JWT auth, maker-checker approval workflow, immutable audit trail with CSV/JSON export, and Git-backed branching/merging with conflict resolution (Phase 18)
- Monitoring & metrics — Prometheus `/metrics` endpoint (HTML/JSON/text), dashboard API (engine-state/health), dead-rule detection with configurable thresholds, and coverage reporting reading real tracker structures (Phase 19)
- Complete management API — rules/categories/tags CRUD, approvals, audit, and branches endpoints (Phase 20)
- Deployment & CI/CD — multi-stage Alpine Docker image, Helm chart with dev/staging/prod values, and hot-reload mechanism with async job tracking (Phase 21)
- Frontend-backend integration closure — aligned frontend to `/v1` (VITE_API_URL + vite proxy + actix-cors), JWT-protected routes with AuthProvider, mounted all orphaned routes, real WebSocket metrics streaming, audit/approval/branch UI wired to backend; 9/9 partial requirements verified MET end-to-end (Phase 22)

**Archived:** `.planning/milestones/v4.0-ROADMAP.md`, `.planning/milestones/v4.0-REQUIREMENTS.md`
**Tag:** `v4.0`

---
