---
phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
threats_total: 6
threats_open: 0
status: passed
---

# Phase 22 Security Audit

**ASVS Level:** L1 (mitigation presence verified in cited file; boundary placement confirmed for auth/CORS)
**Block-on threshold:** high (default)
**Audit method:** Source-file verification of each declared threat mitigation in the phase-22
plan `<threat_model>` blocks. No documentation was accepted as evidence — every `mitigate`
disposition was confirmed by a grep/Read match in the cited implementation file.

## Threat Register

| Threat ID | Category | Severity | Disposition | Evidence (file:line) | Notes |
|-----------|----------|----------|-------------|----------------------|-------|
| T-22-01 | Tampering (JWT alg-confusion / `alg:none`) | high | mitigated | `api/src/middleware/auth.rs:37` — `Validation::new(Algorithm::HS256)` inside `decode_token` | Algorithm pinned; `Validation::default()` is NOT used. `jsonwebtoken` rejects `alg:none` and wrong-secret tokens. `decode_token` is the sole validation path invoked by `requires_auth` (auth.rs:106). |
| T-22-02 | Elevation of privilege (mutating `/v1/*` routes) | high | mitigated | `api/src/main.rs:142-144` — `.wrap(actix_web::middleware::from_fn(middleware::auth::requires_auth))` on the `web::scope("/v1")` | The wrap covers `/v1/branches/*`, `/v1/approvals/*`, `/v1/audit/*` (main.rs:192-201). Public allowlist bypass (`/v1/auth/login`, `/v1/metrics/ws`, root health/swagger/metrics) is applied INSIDE the middleware (auth.rs:53-65), so no mutating route is accidentally public. `/v1/auth/refresh` is NOT in the allowlist → requires a valid token. |
| T-22-03 | Information disclosure (JWT in localStorage) | medium | accepted | D-22-01 (v4.0 demo); documented in plan objective and 22-02-PLAN.md threat_model | Accepted for v4.0 demo. v5.0 remediation: httpOnly cookies. Severity (medium) is below the `block_on: high` threshold AND the threat is explicitly accepted, so it does not count toward `threats_open`. Frontend stores token in `localStorage` (`frontend/src/api/client.ts:21`) — consistent with the accepted-risk record. |
| T-22-04 | Tampering (create_branch name / Git ref injection) | high | mitigated | `api/src/v1/branches.rs:40-84` (`sanitize_branch_name`) called at `branches.rs:154` before `guard.create_branch` (`branches.rs:171`) | `CreateBranchRequest.name` field present (branches.rs:25). Sanitizer rejects: empty, leading/trailing whitespace, embedded whitespace, `HEAD`, `refs/` prefix, backslash (`\`), and `..` traversal. Hardcoded `"feature/default"` is gone — name flows from request body. **Documented deviation:** a lone forward slash `/` is intentionally PERMITTED (git uses it for namespaced branches like `feature/login`; the codebase's own tests create `feature/x`). The path-traversal vector (`..`) and ref-injection vector (`refs/`) are rejected, so the threat is closed. Unit tests at branches.rs:511-526 cover the malicious inputs. |
| T-22-05 | Spoofing (CORS) | high | mitigated | `api/src/main.rs:89-111` — `actix_cors::Cors::default()` with `allowed_origin(...)` per configured origin; no `allow_any_origin()` and no `allow_credentials(true)` | Origins read from `CORS_ALLOWED_ORIGINS` env (default `http://localhost:5173`). Comment at main.rs:88 explicitly states the forbidden combination. Credentials are not used by the JWT scheme, so `allow_credentials` is never enabled. |
| T-22-SC | Tampering (cargo/npm installs) | low | accepted | 22-05/22-06 threat_model: "No new installs this phase" | Accepted. No new installs introduced in 22-05/22-06. (22-01 installed `jsonwebtoken` + `actix-cors`, both audited in 22-RESEARCH.md.) Severity low and accepted → does not count toward `threats_open`. |

## Cross-Cutting Verification (verifier findings)

| Check | Evidence | Status |
|-------|----------|--------|
| Frontend API client base URL resolves via `VITE_API_URL` (not raw `/api/v1` strings) | `frontend/src/api/client.ts:12` — `baseURL: import.meta.env.VITE_API_URL \|\| '/v1'` | verified |
| Audit export fetch goes through authenticated client | `frontend/src/features/governance/audit/AuditLogViewer.tsx:2,77` — `import apiClient from '@/api/client'`, `apiClient.get('/audit/export', ...)` | verified |
| Approvals fetch goes through authenticated client | `frontend/src/features/governance/approval/ApprovalQueue.tsx:2,18,42` — `apiClient.get('/approvals/pending')`, `apiClient.post(...)` | verified |
| Branches fetch goes through authenticated client | `frontend/src/features/versioning/BranchManager.tsx:2,88,110,255` — `apiClient.get('/branches')`, `apiClient.post('/branches', ...)` | verified |
| Bearer token attached to all requests | `frontend/src/api/client.ts:19-30` — request interceptor reads `localStorage.auth_token` and sets `Authorization: Bearer <token>` | verified |

## Unregistered Threat Flags

None. No new attack surface was registered in SUMMARY.md `## Threat Flags` that lacks a threat-model mapping.

## Accepted Risks Log

| ID | Risk | Decision | Remediation |
|----|------|----------|-------------|
| T-22-03 | JWT stored in browser `localStorage` (XSS exfiltration risk) | Accept for v4.0 demo (D-22-01) | v5.0: migrate to httpOnly Secure SameSite cookies |
| T-22-SC | Supply-chain risk from new cargo/npm installs | Accept — no new installs in 22-05/22-06 | 22-01 installs (`jsonwebtoken`, `actix-cors`) pre-audited in 22-RESEARCH.md |

## Computation

- Threats total: 6
- Closed (mitigated): 4 (T-22-01, T-22-02, T-22-04, T-22-05)
- Closed (accepted): 2 (T-22-03 medium, T-22-SC low)
- Open (blocking, severity ≥ high): 0
- Open (non-blocking, severity < high): 0
- **`threats_open` = 0** → ship gate: PASSED