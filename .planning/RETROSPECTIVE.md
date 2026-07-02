# Retrospective

Living retrospective. A new section is appended per milestone at ship time.

## Milestone: v4.0 — Production Platform

**Shipped:** 2026-07-02
**Phases:** 6 (17–22) | **Plans:** ~12 | **Timeline:** 22 days, 89 commits

### What Was Built

- Production React/TypeScript frontend — CodeMirror rule IDE, decision-table editor (all DMN hit policies), guided editors, rule-management dashboard
- Enterprise governance backend — RBAC (admin/author/reviewer/viewer) with JWT auth, maker-checker approval workflow, immutable audit trail with CSV/JSON export, Git-backed branching/merging with conflict resolution
- Monitoring & metrics — Prometheus `/metrics`, dashboard API, dead-rule detection, coverage reporting
- Complete management API — rules/categories/tags CRUD, approvals, audit, branches
- Deployment & CI/CD — multi-stage Alpine Docker, Helm chart (dev/staging/prod), hot reload with async job tracking
- Frontend-backend integration closure (Phase 22) — aligned frontend to `/v1`, JWT-protected routes + AuthProvider, mounted orphaned routes, real WebSocket metrics streaming, audit/approval/branch UI wired to backend

### What Worked

- **Phase 22 wiring-first closure pattern:** A single dedicated phase that wired orphaned UI to the substantially-complete backend (configuration + composition + stub-replacement) closed 9 partial requirements in one verified pass — cheaper than re-building.
- **Goal-backward verification:** Phase 22's VERIFICATION.md cited `file:line` evidence against the live codebase rather than SUMMARY claims — caught real integration state and produced an honest 9/9 MET verdict.
- **Single root-cause framing:** Identifying the `/api/v1` vs `/v1` baseURL mismatch as the single root cause focused the closure phase on config/proxy/auth rather than scattered rework.

### What Was Inefficient

- **Stale tracking records at close:** Phase 17's UAT sat as `partial` (pre-Phase-22) and Phases 18/19 VERIFICATION files used the unnumbered `VERIFICATION.md` form the tooling doesn't recognize — both blocked a clean `verified_closeout` until manually normalized. Verification records should be flipped to terminal status (`complete`/`passed`) and use the numbered `{phase}-VERIFICATION.md` filename at phase close, not at milestone close.
- **Roadmap drift:** ROADMAP.md accumulated duplicated/stale `## Progress` sections and inconsistent plan counts across milestones — required judgment to collapse cleanly.
- **No prior tags:** v1.0/v2.0/v3.0 were never tagged, so historical releases aren't recoverable as git refs. v4.0 is the first tagged release.

### Patterns Established

- Numbered verification filename convention: `{phase}-VERIFICATION.md` with YAML frontmatter (`status`, `phase`, `verified`, `verdict`) is the form the tooling recognizes — adopt for all future phases.
- UAT terminal status is `complete` (not `passed`) for the artifact audit to clear.
- When audit finds a phase is a skeleton, a dedicated decimal/wiring closure phase (like Phase 22) is an effective inline remedy.

### Key Lessons

- Run `/gsd-verify-work` (or the verify step) and flip UAT/verification records to terminal at phase completion — deferring it to milestone close is costly.
- Keep ROADMAP.md milestone-grouped as you go (`<details>` collapse at milestone close) to avoid drift accumulating.

### Cost Observations

- Sessions: multi-session across 22 days; significant context spent on multi-track hardening/production-readiness work (2026-07-01) and the Phase 22 closure sprint.
- Notable: the heaviest single spend in this close was reverse-engineering the tooling's phase-state derivation to unblock the verified_closeout — a one-time cost now documented above as a pattern.

---

## Cross-Milestone Trends

| Milestone | Phases | Days | Commits | Closeout | Tagged |
|-----------|--------|------|---------|----------|-------|
| v1.0 | 6 | — | — | — | no |
| v2.0 | 3 | — | — | — | no |
| v3.0 | 1 | — | — | — | no |
| v4.0 | 6 | 22 | 89 | verified | yes |

---
*Last updated: 2026-07-02 after v4.0 milestone*