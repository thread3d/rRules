---
phase: 28-frontend-test-runner
plan: 04
subsystem: frontend-api
tags: [frontend, typescript, api-client, execution, trace, engines]
requires:
  - 28-03 (GET /v1/engines route + ExecuteRequest.trace backend field — parallel wave; execution.ts anticipates these shapes)
provides:
  - frontend/src/api/execution.ts (6 typed async functions: executeRule, batchExecute, getJobStatus, listJobs, listEngines, reloadRules)
  - frontend/src/api/types.ts Phase 28 section (9+ interfaces: ExecuteRequest, ExecuteResponse, InferenceTrace, InferenceTraceEntry, TraceSummary, EnginesResponse, ReloadJob, BatchExecuteRequest, BatchItemResult, BatchExecuteResponse)
affects:
  - Plan 28-05 (TestRunner imports { executeRule, listEngines, getJobStatus, reloadRules } from '@/api/execution')
tech-stack:
  added: []
  patterns:
    - one-function-per-endpoint (mirrors frontend/src/api/rules.ts)
    - snake_case TS types mirroring backend serde (extends ApprovalState/ApprovalsResponse precedent)
    - single shared apiClient reuse (L-05 — no second axios instance)
key-files:
  created:
    - frontend/src/api/execution.ts
  modified:
    - frontend/src/api/types.ts
decisions:
  - D-10 (snake_case TS types mirror backend serde — no camelCase transform)
  - D-07 (reloadRules is the 6th function — Critical Finding 3; Reload-then-Run needs it)
  - Matched backend serde EXACTLY where plan prose diverged (Rule 1): BatchExecuteRequest.requests + max_concurrency (not plan's 'items'); BatchExecuteResponse.results is BatchItemResult[] (not ExecuteResponse[])
metrics:
  duration: ~12m
  completed: 2026-07-06
  tasks: 2
  files: 2
status: complete
---

# Phase 28 Plan 04: Execution API Client + Types Summary

Created the typed frontend bridge Plan 28-05's TestRunner consumes: 6 async functions in `frontend/src/api/execution.ts` (one per endpoint, reusing the shared `apiClient`) and 10 snake_case TS interfaces in `frontend/src/api/types.ts` mirroring the backend `serde` shapes in `api/src/v1/execution.rs`.

## What Was Built

### Task 1 — types.ts Phase 28 section (commit 9d90f690e)

Appended a clearly-delimited "Phase 28 — Execution / Trace / Engine / Job types" section to `frontend/src/api/types.ts` with these exported interfaces (snake_case keys matching backend serde):

- `ExecuteRequest` — `session_id?`, `facts`, `rule_filter?` (`name_pattern?` + `categories?`), `engine?`, `trace?` (anticipates Plan 28-03's `#[serde(default)] pub trace: Option<bool>`)
- `ExecuteResponse` — `execution_id`, `rules_fired`, `rules_not_fired`, `facts_after`, `inference_trace?`, `engine_generation`, `session_id?`
- `InferenceTrace` / `InferenceTraceEntry` / `TraceSummary` — the EXEC-04 trace wire schema
- `EnginesResponse` — `{ engines: string[], default: string }` (D-03)
- `ReloadJob` — `job_id`, `status` union, `result?`, `error?`
- `BatchExecuteRequest` — `requests: ExecuteRequest[]`, `max_concurrency?`
- `BatchItemResult` — `success`, `response?`, `error?` (helper, see Deviations)
- `BatchExecuteResponse` — `results: BatchItemResult[]`, `total_fired`, `succeeded`, `failed`

A doc-comment block references D-10 (snake_case mirroring) and EXEC-04 (trace types) for traceability. `tsc --noEmit` passes.

### Task 2 — execution.ts (commit c6e589597)

Created `frontend/src/api/execution.ts` importing `apiClient` from `./client` (the single shared axios instance — L-05) and the typed responses from `./types`. Six exported async functions mirroring `rules.ts`'s one-function-per-endpoint pattern:

| Function | Method + Path | Returns | Consumer |
|----------|---------------|---------|----------|
| `executeRule(req)` | POST `/execute` | `ExecuteResponse` | D-08 Run button |
| `batchExecute(req)` | POST `/rules/batch-execute` | `BatchExecuteResponse` | ROADMAP (no UI in Phase 28) |
| `getJobStatus(jobId)` | GET `/jobs/{jobId}` | `ReloadJob` | D-07 poll |
| `listJobs()` | GET `/jobs` | `ReloadJob[]` | ROADMAP (no UI in Phase 28) |
| `listEngines()` | GET `/engines` | `EnginesResponse` | D-08 dropdown data source |
| `reloadRules(asyncReload=true)` | POST `/rules/reload` | `{ job_id, status }` | D-07 Reload-then-Run kick-off |

`reloadRules` is the 6th function — Critical Finding 3: D-10's original 5-function list omitted it, but D-07 "Reload then Run" needs it to start the async reload job. A file-header comment documents the D-10/CF#3/D-07 trail. No interceptors or error handling added (the existing 401/auth interceptor on `apiClient` covers execution.ts — L-05).

`tsc --noEmit` and `npm run build` both exit 0. Plan 28-05 can `import { executeRule, listEngines, getJobStatus, reloadRules } from '@/api/execution'`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] BatchExecuteRequest / BatchExecuteResponse field shapes matched to backend serde, not plan prose**
- **Found during:** Task 1
- **Issue:** The plan's `<action>` prose specified `BatchExecuteRequest { items: ExecuteRequest[] }` and `BatchExecuteResponse { results: ExecuteResponse[] }`. The actual backend (`api/src/v1/execution.rs:462-494`) serializes `BatchExecuteRequest { requests: Vec<ExecuteRequest>, max_concurrency: Option<usize> }` and `BatchExecuteResponse { results: Vec<BatchItemResult>, total_fired, succeeded, failed }` where `BatchItemResult { success, response?, error? }` (execution.rs :474-481). The plan's same `<action>` explicitly instructed "Read api/src/v1/execution.rs to confirm the exact field names + nullability before writing — the keys must match the backend serde output exactly." Following that authoritative instruction, the types use `requests` + `max_concurrency` and `results: BatchItemResult[]`.
- **Fix:** Added a `BatchItemResult` helper interface (10th, beyond the 9 named in must_haves) and used `requests` / `max_concurrency` on `BatchExecuteRequest`. `execution.ts`'s `batchExecute` returns the corrected `BatchExecuteResponse`.
- **Files modified:** frontend/src/api/types.ts, frontend/src/api/execution.ts
- **Commit:** 9d90f690e (types), c6e589597 (execution.ts)

**2. [Rule 2 - Completeness] ExecuteRequest.rule_filter gained `categories?: string[]` to match backend**
- **Found during:** Task 1
- **Issue:** Plan prose listed `rule_filter?: { name_pattern?: string }` but backend `RuleFilter` (execution.rs :38-44) also has `categories: Option<Vec<String>>`. Matching backend exactly (per the plan's authoritative instruction) requires the `categories` field.
- **Fix:** Added `categories?: string[]` to the `rule_filter` inline type.
- **Files modified:** frontend/src/api/types.ts
- **Commit:** 9d90f690e

**3. [Documentation] Acceptance-criterion grep for "export interface InferenceTrace" matches 2 lines (substring of InferenceTraceEntry)**
- **Found during:** Task 1 verification
- **Issue:** The plan's acceptance criterion `grep -c "export interface InferenceTrace" ... == 1` is satisfied by `grep` substring matching — `InferenceTraceEntry` contains `InferenceTrace`, so the raw count is 2. The interfaces themselves are correctly defined as two separate exports (`InferenceTrace` at line 154, `InferenceTraceEntry` at line 160).
- **Fix:** None — the code is correct. A precise grep (`grep -nE "export interface InferenceTrace($|[ {])"`) confirms exactly 1 match for each. Recorded here so a verifier running the literal plan grep is not confused.
- **Files modified:** none

## Auth Gates

None — no auth required for this plan (types + client module only; no live API calls).

## Known Stubs

None — `execution.ts` wires real endpoints via the shared `apiClient`; `types.ts` defines real shapes mirroring backend serde. No placeholder/mock data.

## Threat Flags

None. No new network surface beyond reusing the existing authenticated `apiClient` (T-28-04-client: accept). Type shapes mirror backend serde exactly (T-28-04-types: accept).

## Verification

- `cd frontend && npx tsc --noEmit` → exit 0 (Task 1 and Task 2)
- `cd frontend && npm run build` → exit 0 (Task 2; `✓ built in 5.34s`)
- All 6 functions exported with correct signatures (grep-verified)
- `import apiClient from './client'` present exactly once; `apiClient` used in 6 call sites + 1 import = 7 non-comment occurrences (≥6 required)
- 9 named interfaces present (plus `BatchItemResult` helper — see Deviation 1)

## Self-Check: PASSED

- frontend/src/api/execution.ts — FOUND
- frontend/src/api/types.ts — FOUND
- commit 9d90f690e — FOUND
- commit c6e589597 — FOUND