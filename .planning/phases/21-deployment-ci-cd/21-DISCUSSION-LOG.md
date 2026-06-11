# Phase 21: Deployment & CI/CD - Discussion Log

**Date:** 2026-06-11  
**Mode:** discuss (default)  
**User selected to update existing context**

---

## Summary

Phase 21 already had CONTEXT.md from a prior session. User chose to update it with additional implementation decisions.

---

## Gray Areas Discussed

### 1. BuildKit Cache Optimizations

| Question | Options | Decision |
|----------|---------|----------|
| Which dependency caching approach? | BuildKit cache mounts / cargo-chef recipes / No optimization (current) | **No optimization (current)** |

**Notes:** Current Dockerfile already implements good layer ordering with Cargo.toml copied first. BuildKit optimization can be added later if build times become a bottleneck.

---

### 2. Reload Status Tracking Storage

| Question | Options | Decision |
|----------|---------|----------|
| How should reload job status be stored? | In-memory storage / PostgreSQL persistence | **PostgreSQL persistence** |

**Rationale:**
- Audit trail required for governance (Phase 18)
- Reload history survives container restarts
- Enables compliance reporting on rule changes

**Implementation plan:**
- Create `reload_jobs` table with columns: job_id, status, rules_count, error_message, created_at, completed_at
- POST /v1/rules/reload creates record and returns job_id
- GET /v1/rules/reload/{job_id} queries database for status

---

### 3. Helm Chart Environment Configurations

| Question | Options Selected | Decision |
|----------|------------------|----------|
| What Helm values should support environment-specific configuration? | Replica scaling, Resource limits, Ingress configuration, Feature flags | **All four selected** |

**Implementation plan:**
- `values-dev.yaml`: 1 replica, minimal resources, dev.example.com
- `values-staging.yaml`: 2 replicas, moderate resources, staging.example.com
- `values-prod.yaml`: 3+ replicas, configured resources, prod.example.com

---

## Decisions Summary

| Decision ID | Topic | Selection |
|-------------|-------|-----------|
| D-21-05 | Dependency caching | No optimization (current approach) |
| D-21-06 | Reload status storage | PostgreSQL persistence |
| D-21-07 | Helm configuration | All four features: replicas, resources, ingress, feature flags |

---

## Next Steps

1. **Plan Phase 21** with new decisions:
   - Add PostgreSQL schema migration for reload_jobs table
   - Extend Helm chart values templates for environment-specific configs
   - Update Dockerfile if BuildKit optimization is added later

---

*End of discussion log*
