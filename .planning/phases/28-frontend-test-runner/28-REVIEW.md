---
phase: 28-frontend-test-runner
reviewed: 2026-07-06T00:00:00Z
depth: standard
files_reviewed: 18
files_reviewed_list:
  - api/src/v1/execution.rs
  - api/src/main.rs
  - api/src/openapi.rs
  - api/tests/execute_integration.rs
  - engine/src/engine/direct_eval.rs
  - engine/src/engine/registry.rs
  - engine/src/engine/rete/mod.rs
  - engine/src/engine/treat/mod.rs
  - engine/src/engine/leaps/mod.rs
  - engine/src/engine/phreak/mod.rs
  - engine/src/engine/mod.rs
  - frontend/src/components/TestRunner.tsx
  - frontend/src/api/execution.ts
  - frontend/src/api/types.ts
  - frontend/src/components/__tests__/TestRunner.test.tsx
  - frontend/src/routes/rules/$id.edit.tsx
  - frontend/src/main.tsx
  - frontend/vite.config.ts
findings:
  critical: 0
  warning: 2
  info: 3
  total: 5
status: issues_found
---

# Phase 28: Code Review Report

**Reviewed:** 2026-07-06T00:00:00Z
**Depth:** standard
**Files Reviewed:** 18
**Status:** issues_found

## Summary

Phase 28 (frontend-test-runner) wires the v5.0 Live Execution vertical slice: the `trace: Option<bool>` opt-in on `ExecuteRequest` (D-02), two-site ordered trace emission in `direct_eval.rs` (Treat/Leaps/Phreak) and `rete/mod.rs` (Rete inlined loop) per Phase 26 C1, the `GET /v1/engines` endpoint (D-03), the `TestRunner.tsx` component with engine dropdown + trace toggle + results panel, the `execution.ts` API client (6 functions), and 4 GREEN Vitest component tests plus 4 backend integration scaffolds. Route registration (`/v1/engines` in main.rs and configure_test_routes) and OpenAPI schema registration (`EnginesResponse`, `list_engines`) are correctly wired.

The locked decisions in phase_context are respected: `trace` off-by-default with `unwrap_or(false)` (SC-3 latency neutrality), two-site trace emission is the REQUIRED Phase 26 C1 pattern (NOT duplication), `useMutation(executeRule)` onSuccess renders directly with no polling (Pitfall 8), `@ts-expect-error` on `javascript({json:true})` is required, `mode_selector.rs` is untouched, and `rule_filter.categories` is optional. Pre-existing `get_job_status`/`list_jobs` `.unwrap()` lock patterns are NOT attributed to Phase 28.

The review found 0 Critical, 2 Warning, and 3 Info findings. The two warnings are both correctness gaps in newly-introduced Phase 28 code: (1) `list_engines` re-reads `RRULES_DEFAULT_ENGINE` directly, bypassing the startup validation in `main.rs`, and can return a `default` value not present in the `engines` array; (2) the TestRunner "Reload then Run" branch silently swallows failed reload jobs with no user feedback.

## Warnings

### WR-01: `list_engines` bypasses startup engine-name validation — can return a `default` not in `engines`

**File:** `api/src/v1/execution.rs:1101-1102`
**Issue:** The `list_engines` handler resolves the default engine name by reading the `RRULES_DEFAULT_ENGINE` env var directly:

```rust
let default = std::env::var("RRULES_DEFAULT_ENGINE")
    .unwrap_or_else(|_| selector.current_engine().to_string());
```

`main.rs` (lines 59-67) validates this env var ONCE at startup via `selector.select_engine(&default_engine)`, logs a warning on invalid values, and falls back to `"rete"`. But `list_engines` re-reads the raw env var on every request and returns it verbatim whenever it is present — even if the value is an unregistered engine name (e.g. `RRULES_DEFAULT_ENGINE=phreak` with `engine-phreak` feature off, or a typo like `RRULES_DEFAULT_ENGINE=rete2`). In that case `EnginesResponse.default` is a string that does NOT appear in `EnginesResponse.engines`.

The frontend `TestRunner.tsx` useEffect (lines 70-74) sets `selectedEngine = enginesQuery.data.default` on first load, and the `<select>` (lines 171-181) only renders `<option>` elements for names in `enginesQuery.data?.engines`. A `default` not in `engines` produces an empty dropdown selection: `select.value` is set to a non-existent option, so the browser shows nothing selected, and the next Run sends `engine: selectedEngine ?? undefined` with `selectedEngine` equal to the invalid name — which `execute_single` then rejects with HTTP 400 (unknown engine). The user cannot run anything until manually picking a valid engine from the dropdown.

The `list_engines_returns_registry` integration test (line 2251) asserts `engines` includes `default`, but `reset_test_env` pins `RRULES_DEFAULT_ENGINE=rete` and all features are enabled in the test build, so this edge case is not covered.

**Fix:** Use the already-validated `selector.current_engine()` as the source of truth instead of re-reading the env var, OR validate the env-var value against `registry.available_engines()` and fall back to `selector.current_engine()` when it is absent or invalid:

```rust
let default = std::env::var("RRULES_DEFAULT_ENGINE")
    .filter(|v| registry.available_engines().iter().any(|e| e == v))
    .unwrap_or_else(|_| selector.current_engine().to_string());
```

This makes `list_engines` consistent with the startup validation in `main.rs` and guarantees `default ∈ engines`.

### WR-02: TestRunner silently swallows failed reload jobs — no user feedback, polling stuck

**File:** `frontend/src/components/TestRunner.tsx:106-112`
**Issue:** The "Reload then Run" effect only handles the `completed` terminal state:

```tsx
useEffect(() => {
  if (jobQuery.data?.status === 'completed' && jobId !== undefined) {
    setPreReloadGeneration(lastResult?.engine_generation)
    runExecute()
    setJobId(undefined)
  }
}, [jobQuery.data?.status, jobId]) // eslint-disable-line react-hooks/exhaustive-deps
```

The `refetchInterval` callback (lines 96-100) correctly stops polling on BOTH `completed` and `failed`:

```tsx
return s === 'completed' || s === 'failed' ? false : 500
```

But the effect never checks `status === 'failed'`. When the reload job fails, polling stops, `jobId` is never cleared (the effect's `setJobId(undefined)` only runs on `completed`), no error is surfaced to the `error` banner, and `runExecute()` is never called. The user clicks "Reload then Run", the backend job fails (e.g. rules directory parse error, IO failure), and the UI appears frozen with no indication that anything happened or that a second reload attempt is impossible without a page reload (the `jobId` is still set, so a second click of "Reload then Run" would set a NEW jobId, but the failed query for the old jobId lingers in the cache).

This is a correctness gap: a failed reload is a real operational state that the user must see to act on.

**Fix:** Handle the `failed` branch in the effect, surface an error, and clear `jobId`:

```tsx
useEffect(() => {
  if (jobQuery.data?.status === 'completed' && jobId !== undefined) {
    setPreReloadGeneration(lastResult?.engine_generation)
    runExecute()
    setJobId(undefined)
  } else if (jobQuery.data?.status === 'failed' && jobId !== undefined) {
    setError('Reload job failed. See server logs.')
    setJobId(undefined)
  }
}, [jobQuery.data?.status, jobId]) // eslint-disable-line react-hooks/exhaustive-deps
```

Consider also extending the `reload then run polls job status` component test to cover the `failed` transition.

## Info

### IN-01: TestRunner does not surface `enginesQuery` errors — empty dropdown with no feedback

**File:** `frontend/src/components/TestRunner.tsx:63-66, 169-182`
**Issue:** `enginesQuery = useQuery({ queryKey: ['engines'], queryFn: listEngines })` has no error-handling branch. If `GET /v1/engines` fails (network error, 401, 500), `enginesQuery.data` is `undefined`, the dropdown renders with zero `<option>` elements, and `selectedEngine` is never initialized. The user sees an empty `<select>` and a Run button that sends `engine: undefined` (backend falls back to default — which works) but with no indication that the dropdown failed to populate. A `retry` affordance or an error hint near the dropdown would improve robustness.
**Fix:** Add an error state for `enginesQuery`:

```tsx
{enginesQuery.error && (
  <p className="text-xs text-red-600">Engine list unavailable; using backend default.</p>
)}
```

### IN-02: `react-hooks/exhaustive-deps` disabled on the reload-completion effect

**File:** `frontend/src/components/TestRunner.tsx:112`
**Issue:** The effect at lines 106-112 disables the exhaustive-deps lint rule because `runExecute` and `lastResult` are used but not listed in the deps array. Tracing through the effect's behavior today: the effect re-runs whenever `jobQuery.data?.status` or `jobId` changes, and at each re-run it captures the latest `runExecute` (redefined every render) and the latest `lastResult` from that render's closure — so the behavior is correct. However, the suppressed lint means a future refactor that adds a new state read inside `runExecute` (or changes when `lastResult` updates) could introduce a real stale-closure bug without any lint warning. Prefer restructuring to avoid the disable: either extract the completion logic into a stable callback via `useCallback` with proper deps, or move `runExecute`'s body inline into the effect with explicit deps.
**Fix:** No immediate change required (behavior is correct); flagged for future maintainability. If desired, inline the execute logic into the effect and list `factsJson`, `selectedEngine`, `traceEnabled`, `ruleId`, `lastResult` in the deps array, gating the body on `status === 'completed'`.

### IN-03: Failed `executeMutation` leaves stale `lastResult` visible alongside error banner

**File:** `frontend/src/components/TestRunner.tsx:78-87, 218`
**Issue:** `executeMutation.onSuccess` sets `lastResult` and clears `error`; `onError` sets `error` but does NOT clear `lastResult`. After a successful Run (panel populated) followed by a failed Run (e.g. invalid facts JSON is caught before mutate, but a network/500 failure hits `onError`), the panel continues to display the previous run's `rules_fired` / `facts_after` / `inference_trace` while the error banner shows below. The user may mistake the stale panel for the current run's result. Clearing `lastResult` on error (or rendering the panel only when there is no error) would avoid confusion.
**Fix:**

```tsx
onError: (err: unknown) => {
  setError(err instanceof Error ? err.message : String(err))
  setLastResult(undefined)
},
```

---

_Reviewed: 2026-07-06T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_