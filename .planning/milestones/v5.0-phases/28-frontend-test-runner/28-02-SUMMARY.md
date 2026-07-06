---
phase: 28-frontend-test-runner
plan: 02
subsystem: frontend-test-infra
tags: [frontend, vitest, testing-library, tanstack-query, tdd-red, wave-0]
requires:
  - frontend/package.json (devDeps: vitest, @testing-library/react, jsdom present)
provides:
  - frontend Vitest config (jsdom + globals + setup file)
  - QueryClientProvider-wrapped app shell (enables D-08/D-10 useQuery/useMutation)
  - four RED TestRunner component test scaffolds (D-04/D-07/D-08/D-09 contract)
  - tsconfig.json test-file exclusion (unblocks Plan 28-04 / 28-05 production tsc -b)
affects:
  - Plan 28-04 (execution.ts can rely on QueryClientProvider + vitest config)
  - Plan 28-05 (TestRunner.tsx flips the four RED tests GREEN)
tech-stack:
  added: ["@testing-library/jest-dom ^6.9.1 (devDep)"]
  patterns:
    - "Vitest jsdom + globals + setupFiles (standard Vite+Vitest component-test config)"
    - "vi.hoisted + vi.mock factory for not-yet-created api module (avoids Vite import-analysis failure on non-existent module)"
    - "QueryClientProvider outermost inside StrictMode (TanStack Query app-shell wrap)"
    - "tsconfig exclude src/**/*.test.* + src/**/*.spec.* (production tsc -b skips test files)"
key-files:
  created:
    - frontend/src/test/setup.ts
    - frontend/src/components/__tests__/TestRunner.test.tsx
  modified:
    - frontend/vite.config.ts
    - frontend/tsconfig.json
    - frontend/src/main.tsx
    - frontend/package.json
    - frontend/package-lock.json
decisions:
  - "Used vi.hoisted + vi.mock factory (not static import of @/api/execution) so Vite import-analysis does not fail on the not-yet-created module — the RED trigger is isolated to @/components/TestRunner as the plan intends"
  - "Wrapped app tree as StrictMode > QueryClientProvider > AuthProvider > RouterProvider (QueryClientProvider outermost inside StrictMode so the whole tree sees the client)"
  - "Extended tsconfig.json exclude with src/**/*.test.* and src/**/*.spec.* so the RED test scaffold's import of a not-yet-created component does not break production tsc -b (unblocks Plan 28-04 / 28-05 Task 3)"
  - "Verified @testing-library/jest-dom legitimacy (github.com/testing-library/jest-dom, v6.9.1) before install — T-28-02-SC mitigated"
metrics:
  duration: ~5m
  completed: 2026-07-06
  tasks: 2
  files: 7
status: complete
---

# Phase 28 Plan 02: Frontend test infra + RED TestRunner scaffolds Summary

Vitest jsdom + globals + setup scaffolded, QueryClientProvider wrap added (Critical Finding 2: TanStack Query was installed but unused), @testing-library/jest-dom installed, and four RED TestRunner component test scaffolds pinning the D-04/D-07/D-08/D-09 contract for Plan 28-05 to flip GREEN.

## What Was Built

### Task 1 — Vitest config + setup + QueryClientProvider + jest-dom install (feat)
- **frontend/vite.config.ts**: added `test` block — `environment: 'jsdom'`, `globals: true`, `setupFiles: ['./src/test/setup.ts']` (plus `/// <reference types="vitest" />` directive).
- **frontend/src/test/setup.ts** (NEW): imports `@testing-library/jest-dom` (registers matchers globally).
- **frontend/src/main.tsx**: constructed `const queryClient = new QueryClient()` and wrapped the render tree as `StrictMode > QueryClientProvider > AuthProvider > RouterProvider` (Critical Finding 2 fix — enables D-08 `useQuery` over `GET /v1/engines` and D-10 `useMutation` for `executeRule`). Added a brief comment referencing D-08.
- **frontend/tsconfig.json**: extended `exclude` with `src/**/*.test.*` and `src/**/*.spec.*` (kept existing `node_modules` and `dist`). Standard Vite+Vitest pattern — test files are type-checked by Vitest's own transform, not by the production `tsc -b` build. This unblocks Plan 28-04 and Plan 28-05 Task 3 (the RED test scaffold imports `@/components/TestRunner` which does not exist until Plan 28-05; without the exclude, `tsc --noEmit` / `npm run build` would fail with `Cannot find module '@/components/TestRunner'`).
- **frontend/package.json** + **package-lock.json**: installed `@testing-library/jest-dom ^6.9.1` as a devDep. Package legitimacy verified before install via `npm view @testing-library/jest-dom` → `git+https://github.com/testing-library/jest-dom.git` (official testing-library package, T-28-02-SC mitigated).

### Task 2 — Four RED TestRunner component test scaffolds (test / TDD RED)
- **frontend/src/components/__tests__/TestRunner.test.tsx** (NEW): four `it(...)` blocks with the EXACT test-name substrings from 28-VALIDATION.md:
  1. `renders fired rules and facts_after` (D-04 ruleId prop, D-09 results panel)
  2. `dropdown lists engines` (D-08 useQuery over GET /v1/engines)
  3. `trace section hidden when null` (D-09 trace section conditional on inference_trace)
  4. `reload then run polls job status` (D-07 reload → poll getJobStatus → execute, SC-4 engine_generation correlation)
- Each test renders `<TestRunner ruleId="r1" />` wrapped in a fresh `QueryClientProvider` (per-test `QueryClient` with `retry: false`, `gcTime: 0`, `staleTime: 0` to avoid cross-test cache bleed).
- Mocks `@/api/execution` (executeRule, listEngines, getJobStatus, reloadRules) via `vi.hoisted` + `vi.mock` factory so the not-yet-created api module (Plan 28-04) does not trip Vite's static import-analysis.
- The suite fails RED exactly as required: `Failed to resolve import "@/components/TestRunner"` — the import of the not-yet-implemented component is the RED trigger. Config + setup load correctly (jsdom env + setup file confirmed in the run output).

## Verification Evidence

### Task 1
- `grep -c "environment: 'jsdom'" frontend/vite.config.ts` == 1 ✓
- `grep -c "setupFiles" frontend/vite.config.ts` == 1 ✓
- `grep -c "globals: true" frontend/vite.config.ts` == 1 ✓
- `grep -c "@testing-library/jest-dom" frontend/src/test/setup.ts` == 1 ✓
- `grep -c "QueryClientProvider" frontend/src/main.tsx` == 1 ✓
- `grep -c "QueryClient" frontend/src/main.tsx` >= 1 ✓
- `grep -c "@testing-library/jest-dom" frontend/package.json` == 1 ✓
- `frontend/tsconfig.json` exclude contains `src/**/*.test.*` and `src/**/*.spec.*` ✓
- `npx tsc --noEmit` exits 0 ✓
- `npm run build` exits 0 (✓ built in 5.46s) ✓

### Task 2
- `test -f frontend/src/components/__tests__/TestRunner.test.tsx` ✓
- Each of the four test names appears exactly once in the file ✓
- `npm test -- --run TestRunner.test.tsx` exits NON-zero with:
  `Error: Failed to resolve import "@/components/TestRunner"` (RED — component not yet implemented, NOT a config error) ✓
- `grep -q "environment: 'jsdom'" frontend/vite.config.ts` (config correct — RED comes from missing component) ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Replaced static `import { executeRule, ... } from '@/api/execution'` with `vi.hoisted` + `vi.mock` factory**
- **Found during:** Task 2
- **Issue:** The plan's action said to `vi.mock('@/api/execution')` and import the mocked functions at the top of the test file. But `@/api/execution` does not exist yet (Plan 28-04 creates it), and a static `import { executeRule } from '@/api/execution'` triggers Vite's `vite:import-analysis` plugin at transform time, BEFORE vi.mock can virtualize the module. The first run failed with `Failed to resolve import "@/api/execution"` — at the wrong import, masking the intended RED trigger (`@/components/TestRunner`).
- **Fix:** Used `vi.hoisted(() => ({ executeRule: vi.fn(), ... }))` to create the mock functions, then `vi.mock('@/api/execution', () => ({ executeRule: mocks.executeRule, ... }))` to register the virtual module. The test file references `mocks.executeRule` directly — it does NOT statically import `@/api/execution`, so Vite's import-analysis never tries to resolve the not-yet-created module. The only failing import is now `@/components/TestRunner`, exactly as the plan intends. When Plan 28-05 implements TestRunner.tsx (which imports `@/api/execution`), it will receive the mock.
- **Files modified:** frontend/src/components/__tests__/TestRunner.test.tsx
- **Commit:** 3366336d1

No other deviations. The plan executed exactly as written otherwise.

## TDD Gate Compliance

This plan is `type: tdd` and its stated purpose is the **unblocking Wave-0 RED layer** — the GREEN gate (TestRunner.tsx implementation) is intentionally deferred to Plan 28-05.

- **RED gate**: ✓ `test(28-02): add four RED TestRunner component test scaffolds` (commit 3366336d1) — four failing TestRunner tests landed.
- **GREEN gate**: deferred to Plan 28-05 (this plan's objective explicitly says "Plan 28-05 creates TestRunner.tsx to flip these GREEN"). Task 1's `feat(28-02)` commit is the test-infra scaffold (Vitest config + QueryClientProvider + jest-dom), NOT the feature GREEN.
- **REFACTOR gate**: N/A for this plan.

This is the intended cross-plan RED→GREEN split for Wave 0; no gate violation.

## Threat Model Disposition

| Threat ID | Disposition | Result |
|-----------|-------------|--------|
| T-28-02-SC (Tampering: npm install @testing-library/jest-dom) | mitigate | Mitigated — verified package legitimacy via `npm view @testing-library/jest-dom` → `git+https://github.com/testing-library/jest-dom.git`, v6.9.1 (official testing-library package) before install. |
| T-28-02-provider (Tampering/logic: QueryClientProvider wrap) | accept | Accepted — provider wraps the existing tree with no auth/route logic changes; defaults are safe. |

## Authentication Gates

None encountered.

## Known Stubs

None. This plan lands test infrastructure and RED test scaffolds only; no production component with placeholder data was created. The RED tests intentionally import a not-yet-created `@/components/TestRunner` — that is the required RED state, not a stub.

## Threat Flags

None. No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries were introduced by this plan. (The `GET /v1/engines` backend route is Plan 28-01's scope; the `@/api/execution` module is Plan 28-04's scope.)

## Self-Check: PASSED

### Files
- FOUND: frontend/src/test/setup.ts
- FOUND: frontend/src/components/__tests__/TestRunner.test.tsx
- FOUND: frontend/vite.config.ts
- FOUND: frontend/tsconfig.json
- FOUND: frontend/src/main.tsx
- FOUND: frontend/package.json

### Commits
- FOUND: 8763c32d4 (feat(28-02): scaffold Vitest config + QueryClientProvider + jest-dom)
- FOUND: 3366336d1 (test(28-02): add four RED TestRunner component test scaffolds)

### TDD gate commits
- 8763c32d4: feat(28-02) — Task 1 infra scaffold
- 3366336d1: test(28-02) — Task 2 RED gate (four failing TestRunner tests)
- GREEN gate deferred to Plan 28-05 (intentional cross-plan RED→GREEN split)