# Phase 28: Frontend test-runner - Research

**Researched:** 2026-07-06
**Domain:** Brownfield frontend (React 19 + TanStack) + small backend additions (Rust/actix) — live execution surfaced in the rule IDE
**Confidence:** HIGH (every recommendation grounded in file:line evidence read this session)

## Summary

Phase 28 closes the v5.0 vertical slice by making live execution user-facing. It crosses two layers: (1) a **small backend addition** that populates `inference_trace` for real (EXEC-04) and exposes `GET /v1/engines` (D-03), and (2) a **frontend `TestRunner` panel** mounted in the rule edit page that runs the on-disk rule via the existing stateless `POST /v1/execute` and renders `rules_fired`/`rules_not_fired`/`facts_after`/`inference_trace`, with an engine dropdown populated from the new endpoint and a "Reload then Run" flow that polls `getJobStatus`.

The backend work is well-scoped and additive: `ExecuteRequest` gains one `trace: Option<bool>` field; `execute_single` resolves it and threads `collect_trace: bool` into the evaluator; `inference_trace: None` is replaced at two sites (`execution.rs:730` stateless, `:950` sessionful). The trace types (`InferenceTrace`/`InferenceTraceEntry`/`TraceSummary`) already exist and serialize (`execution.rs:189-213`). The `GET /v1/engines` handler reuses the already-shared `web::Data<EngineRegistry>` + `web::Data<EngineSelector>` and the same default-resolution expression `execute_single` already uses.

**Primary recommendation:** Thread trace emission through an **engine-side trace type** (the engine crate cannot import `api::v1::execution::InferenceTraceEntry` — dependency direction is `api → engine`), emit one entry per fired rule in firing order at BOTH evaluator sites (`direct_eval.rs:54` shared helper AND `rete/mod.rs:341` inlined path — the Phase-26 C1 two-site pattern), and map it to the api struct in `execute_single`. On the frontend, set up a `QueryClientProvider` (currently absent — see Critical Finding 2) and add `execution.ts` mirroring `rules.ts`'s one-function-per-endpoint pattern with snake_case types matching `ApprovalState`/`ApprovalsResponse`.

### Critical findings the planner MUST address

1. **Trace-type boundary (D-01 architecture).** `InferenceTraceEntry` is defined in `api/src/v1/execution.rs:197`. The engine crate cannot import it (`api` depends on `engine`, never the reverse — confirmed: `api/Cargo.toml` pulls `engine` as a path dep; `engine/Cargo.toml` has no `api` dep). The evaluator (`engine/src/engine/direct_eval.rs` + `engine/src/engine/rete/mod.rs`) therefore cannot emit `InferenceTraceEntry` directly. The clean design is an **engine-side trace type** added to `engine/src/engine/registry.rs` (or a new `engine/src/engine/trace.rs`), emitted by the evaluator, returned through `EngineResult`, and mapped to `api::v1::execution::InferenceTraceEntry` in `execute_single`. `[VERIFIED: api/src/v1/execution.rs:197, engine/src/engine/registry.rs:112, engine/Cargo.toml]`
2. **TanStack Query is installed but UNUSED — no `QueryClientProvider`.** D-08/D-10 specify `useQuery`/`useMutation`, and CONTEXT L-06 states TanStack Query is "already used by `useRules.ts`." That is **inaccurate**: `frontend/src/features/rules/useRules.ts` uses plain `useState`+`useEffect` (read in full this session), and `frontend/src/main.tsx` has **no `QueryClientProvider`** — only `AuthProvider` + `RouterProvider`. A codebase-wide grep for `useQuery`/`useMutation`/`QueryClient` returns zero hits. The planner must EITHER (a) add a `QueryClient` + wrap the app in `QueryClientProvider` in `main.tsx` as a Wave-0 enabler (faithful to D-08/D-10), OR (b) fall back to the actual established pattern (`useState`+`useEffect` like `useRules.ts`) and update the decision. Option (a) is the locked-decision-faithful path. `[VERIFIED: frontend/src/main.tsx, frontend/src/features/rules/useRules.ts, grep -r useQuery frontend/src → 0 hits]`
3. **D-10's 5-function list omits `reloadRules`.** D-07 "Reload then Run" requires calling `POST /v1/rules/reload` with `{ async_reload: true }` (handler at `execution.rs:1120`, returns `202` + `JobResponse { job_id, status }` at `:1202`). `rules.ts` has no reload function (read in full). So `execution.ts` needs a **6th function** `reloadRules({ asyncReload: true })` despite D-10 naming only five. `[VERIFIED: api/src/v1/execution.rs:1120-1203, frontend/src/api/rules.ts]`
4. **`@codemirror/lang-json` is NOT installed.** Only `@codemirror/lang-javascript` (^6.2.5) and `@codemirror/theme-one-dark` are present (`frontend/package.json`). `RuleEditor.tsx` uses `javascript({ typescript: true })`. For the JSON facts editor (D-06), either (a) reuse the existing dep via `javascript({ json: true })`, or (b) add `@codemirror/lang-json` (npm `6.0.2`, official CodeMirror package — `[VERIFIED: npm registry, npm view @codemirror/lang-json version → 6.0.2]`). Option (a) avoids a new dep entirely and is recommended. `[VERIFIED: frontend/package.json, frontend/src/components/RuleEditor.tsx:3]`
5. **No frontend test infrastructure exists.** `vitest` (^2.0.0) and `@testing-library/react` (^16.0.0) are in devDeps, but there are **zero `*.test.*`/`*.spec.*` files** in `frontend/src`, and `vite.config.ts` has **no `test` block** and no test-setup file. Wave 0 must scaffold: a `test` block in `vite.config.ts` (environment `jsdom`, globals, setup file), a `src/test/setup.ts` (imports `@testing-library/jest-dom` if present — verify), and the TestRunner test files. Verify `@testing-library/jest-dom` and `jsdom` are installed; if not, they are Wave-0 installs. `[VERIFIED: frontend/vite.config.ts, find frontend/src -name *.test.* → empty]`

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Trace emission (one entry per fired rule) | Engine (`direct_eval.rs` + `rete/mod.rs`) | — | The evaluator owns the fixpoint loop and fires rules; only it knows firing order + timestamps. Cannot live in api (no engine dep reverse). |
| Trace type definition + mapping to API shape | API (`execution.rs`) | Engine (engine-side trace type) | Engine emits an engine-side type; `execute_single` maps to the serde `InferenceTraceEntry`. API owns the wire schema. |
| `trace: Option<bool>` opt-in + `collect_trace` resolution | API (`execute_single`) | — | Request boundary; resolves the flag and skips engine trace work when false (SC-3 latency). |
| `GET /v1/engines` route | API (`execution.rs` handler + `main.rs` route) | — | Reads shared `EngineRegistry`/`EngineSelector` already in `web::Data`. No engine change. |
| Engine-selector dropdown data | Frontend (TestRunner) | API (`/v1/engines`) | UI renders the list; data comes from the real backend endpoint (never hardcoded — D-03). |
| Facts editor + results panel rendering | Frontend (TestRunner) | — | Pure client-side; renders `ExecuteResponse` directly (Pitfall 8). |
| "Reload then Run" orchestration | Frontend (TestRunner) | API (`/rules/reload` + `/jobs/{id}`) | Frontend triggers async reload, polls job status, then executes. Polling ONLY for job status (Pitfall 8 carve-out). |
| ExecuteResponse correlation (`execution_id`/`engine_generation`) | Frontend (TestRunner) | API (already populates both) | Frontend reads the fields from the response to detect staleness after reload — no polling for results. |

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Instrument the live direct evaluator (`engine/src/engine/direct_eval.rs` + `engine/src/engine/rete/mod.rs` inlined path) to emit one real `InferenceTraceEntry` per fired rule when `trace == true`. Thread `collect_trace: bool` through. Do NOT synthesize from coverage. Do NOT wire `InferenceTraceCollector` from `chaining/mode_selector.rs` (L-02).
- **D-02:** Add `#[serde(default)] pub trace: Option<bool>` to `ExecuteRequest` (`api/src/v1/execution.rs:21`), off by default. `execute_single` resolves `let collect_trace = req.trace.unwrap_or(false)`; `inference_trace: Some(...)` only when `collect_trace && entries non-empty`, else `None`. Evaluator skips trace work entirely when false (SC-3 latency neutrality).
- **D-03:** NEW `GET /v1/engines` route returning `{ engines: [...], default: "..." }` from `EngineRegistry::available_engines()` + resolved default. Register in `/v1` scope (`api/src/main.rs:208-267`). Reuse `web::Data<EngineRegistry>` + `web::Data<EngineSelector>` (already shared).
- **D-04:** Mount `TestRunner` as a new `<section>` in `frontend/src/routes/rules/$id.edit.tsx`; run the on-disk rule via stateless `POST /v1/execute` (`session_id` absent).
- **D-05:** Run already-saved rules; "Reload then Run" bridges edits. A minimal Save→`PUT /v1/rules/{id}` enabler is allowed but NO full authoring UX.
- **D-06:** Raw JSON facts editor (reuse CodeMirror from `RuleEditor.tsx`); shape `{ fact_type_id: serde_json::Value }`.
- **D-07:** Ship "Reload then Run" (SC-4 requires it): `reloadRules({ async: true })` → poll `getJobStatus` until `Completed`/`Failed` → `executeRule`. Polling ONLY for job status (Pitfall 8 carve-out).
- **D-08:** `useQuery` over `GET /v1/engines` for the dropdown; default from backend `default` field; selection in local `useState`, sent as `ExecuteRequest.engine`.
- **D-09:** Stacked sections (Fired / Not-fired / Facts after / Trace); trace section hidden when `inference_trace` is `None`.
- **D-10:** NEW `frontend/src/api/execution.ts` (`executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines` — **plus `reloadRules` per Critical Finding 3**) + types in `frontend/src/api/types.ts` mirroring backend snake_case serde. `useMutation` for execute, `useQuery` for engines/job-status.

### Carry-forwards (locked, not re-asked)
- **L-01:** `inference_trace: Option<InferenceTrace>` already on `ExecuteResponse` (`execution.rs:56`), always `None` today (`:730`, `:950`). Trace structs exist at `:189-213`.
- **L-02:** Do NOT wire `chaining/mode_selector.rs:135` `InferenceTraceCollector` (coupled to stub `environment::Engine`).
- **L-03:** `facts_after` shape = `HashMap<String, serde_json::Value>` keyed by `fact_type_id` (`execution.rs:53`).
- **L-04:** `ExecuteResponse` already has `execution_id: String` + `engine_generation: u64` + `session_id: Option<String>`.
- **L-05:** Single `apiClient` at `frontend/src/api/client.ts:11` (`baseURL: VITE_API_URL || '/v1'`).
- **L-06:** Stack: React 19, `@tanstack/react-query` ^5.50, Zustand, Tailwind, `@uiw/react-codemirror`, Vitest + Testing Library — all in `frontend/package.json`. **(Caveat: TanStack Query is installed but unused — see Critical Finding 2.)**
- **L-07:** `ExecuteRequest` already has `session_id`, `facts`, `rule_filter`, `engine`. Phase 28 adds only `trace`. Stateless path used; `session_id` absent.
- **L-08:** Pitfall 8 — drive results from `ExecuteResponse`; polling ONLY for `getJobStatus` during async reload.
- **L-09:** Pitfall 3 — resolved by Phase 26; dashboards read live data.

### Claude's Discretion
- Trace `data` payload per entry (lean: `None` or action summary — heavy risks SC-3 latency).
- `TraceSummary.facts_matched` semantics (just needs to be honest).
- Timestamp source: `chrono::Utc::now().to_rfc3339()` (established pattern from Phase 26 `RuleExecutionTracker`) vs. monotonic offset. **chrono is available in the engine crate** (`engine/Cargo.toml:12`).
- TestRunner state: `useState` (default) vs. small Zustand slice.
- Minimal Save wiring (D-05): wire Save button to `PUT /v1/rules/{id}` or document "save via management UI first."
- `rule_filter` scoping: scope to edited rule via `rule_filter: { name_pattern: id }` or run whole ruleset.
- Component test coverage depth.
- `GET /v1/engines` exact shape: `{ engines: string[], default: string }` is the contract; optional `description` per engine.
- Error rendering (inline banner).

### Deferred Ideas (OUT OF SCOPE)
- Batch-execute UI / results matrix (`batchExecute` API-only in `execution.ts`).
- Stateful-session test-runner UI (Phase 27 backend; `session_id` carried in types but unused).
- Full rule-authoring/save UX in the edit page (only minimal Save enabler per D-05).
- "Load from `facts_after`" convenience.
- Per-engine description/metadata in `GET /v1/engines`.
- Network-node-level Rete trace (alpha/beta propagation).
- Rich trace `data` payload (deferred to keep SC-3 latency honest).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FRONTEND-03 | Frontend test-runner: run a rule from the IDE and see results | D-04 (mount TestRunner in edit page), D-06 (facts editor), D-09 (results panel), D-10 (`execution.ts` + types). Backend `POST /v1/execute` already real (Phase 23). |
| FRONTEND-04 | Engine-selector dropdown populated from `GET /v1/engines` | D-03 (new backend route), D-08 (`useQuery` dropdown), D-10 (`listEngines` fn + `EnginesResponse` type). |
| EXEC-04 | `inference_trace` populated when `trace: true`; omitted + latency-neutral when off | D-01 (instrument evaluator at both sites), D-02 (`trace: Option<bool>` + `collect_trace` resolution), L-01 (trace structs already exist). SC-3 is the verifiability bar. |
</phase_requirements>

## Standard Stack

### Core (no new backend deps; frontend adds only optional/test deps)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@tanstack/react-query` | ^5.50.0 (installed) | `useMutation` for execute, `useQuery` for engines/job-status | D-08/D-10 locked. **Requires adding `QueryClientProvider` in `main.tsx`** (Critical Finding 2). `[VERIFIED: frontend/package.json]` |
| `@uiw/react-codemirror` | ^4.25.10 (installed) | JSON facts editor (D-06) | Reused from `RuleEditor.tsx`. `[VERIFIED: frontend/package.json, frontend/src/components/RuleEditor.tsx:2]` |
| `@codemirror/lang-javascript` | ^6.2.5 (installed) | JSON mode via `javascript({ json: true })` — no new dep | Avoids adding `@codemirror/lang-json`. `[VERIFIED: frontend/package.json]` |
| `axios` | ^1.6.0 (installed) | `apiClient` (shared, `frontend/src/api/client.ts:11`) | L-05 — single instance. `[VERIFIED: frontend/src/api/client.ts]` |
| `chrono` | 0.4 (engine + api workspace dep) | Trace timestamps (`Utc::now().to_rfc3339()`) | Established Phase-26 pattern. `[VERIFIED: engine/Cargo.toml:12, api/Cargo.toml:38]` |
| `utoipa` | 4 (api dep) | `GET /v1/engines` OpenAPI annotation | Trace structs already `ToSchema` (`execution.rs:188-212`). `[VERIFIED: api/Cargo.toml:41]` |
| Vitest | ^2.0.0 (devDep) | TestRunner component tests | Already in devDeps; needs config + setup (Critical Finding 5). `[VERIFIED: frontend/package.json]` |
| `@testing-library/react` | ^16.0.0 (devDep) | Component assertions | Already in devDeps. `[VERIFIED: frontend/package.json]` |

### Optional (planner decides)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@codemirror/lang-json` | 6.0.2 | Pure JSON language support for facts editor | Only if `javascript({ json: true })` feels insufficient; otherwise skip. `[VERIFIED: npm registry — npm view → 6.0.2]` |
| `@testing-library/jest-dom` | (verify installed) | `toBeInTheDocument()` matchers | Wave-0 check; install if absent. `[ASSUMED — not confirmed in package.json grep]` |
| `jsdom` | (verify installed) | Vitest DOM environment | Wave-0 check; required for component testing. `[ASSUMED — not confirmed]` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `useQuery`/`useMutation` (D-08/D-10) | `useState`+`useEffect` (the actual `useRules.ts` pattern) | Faithful to locked decisions requires `QueryClientProvider` setup; the `useState` fallback contradicts D-08/D-10 and would need a decision amendment. |
| `@codemirror/lang-json` | `javascript({ json: true })` from existing dep | No new dep; identical JSON editing UX. Recommended. |
| Stacked results sections (D-09) | Tabbed panel | Tabs add complexity for little gain at this panel size — D-09 locks stacked. |

**Installation (Wave 0 frontend, only if gaps found):**
```bash
# Only if jsdom / jest-dom are missing from devDeps (verify first):
npm install -D jsdom @testing-library/jest-dom
# Only if planner chooses dedicated JSON lang (optional):
npm install @codemirror/lang-json
```
**No backend `cargo add` needed** — `chrono`, `utoipa`, `actix-web`, `serde_json` are all already in `api/Cargo.toml` and `engine/Cargo.toml`. `[VERIFIED]`

## Package Legitimacy Audit

> The only possibly-new packages are optional frontend devDeps (`jsdom`, `@testing-library/jest-dom`) and the optional `@codemirror/lang-json`. No backend packages are installed. Packages tagged `[ASSUMED]` below were checked via `npm view` (registry existence confirmed) but legitimacy signals (downloads/repo) were not run through the full gate because each is a well-known official package and installation is optional/planner-gated.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `@codemirror/lang-json` | npm | mature (6.x) | high (CodeMirror official) | github.com/codemirror/lang-json | OK `[CITED: npm registry — npm view → 6.0.2]` | Optional — planner may install or use existing `javascript({ json: true })` |
| `jsdom` | npm | mature | very high | github.com/jsdom/jsdom | OK `[ASSUMED]` | Wave-0 install only if absent; planner verifies |
| `@testing-library/jest-dom` | npm | mature | very high | github.com/testing-library/jest-dom | OK `[ASSUMED]` | Wave-0 install only if absent; planner verifies |

**Packages removed due to SLOP verdict:** none
**Packages flagged as suspicious:** none

*`jsdom` and `@testing-library/jest-dom` are tagged `[ASSUMED]` because the planner must first verify whether they are already in devDeps; if absent, they are standard, well-established testing utilities. The planner should run `npm ls jsdom @testing-library/jest-dom` as a Wave-0 check.*

## Architecture Patterns

### System Architecture Diagram (data flow: Run rule from IDE → results panel)

```
[Edit page /rules/{id}/edit]
   │
   ├─ TestRunner <section>
   │     │
   │     ├─ facts editor (CodeMirror JSON) ── useState factsJson
   │     ├─ engine dropdown ◄── useQuery(['engines']) ── GET /v1/engines
   │     │                                              │
   │     │                                              ▼
   │     │                            [API] GET /v1/engines handler
   │     │                              reads web::Data<EngineRegistry>.available_engines()
   │     │                              + RRULES_DEFAULT_ENGINE env > selector.current_engine()
   │     │                              → { engines: [...], default: "rete" }
   │     │
   │     ├─ "Run" button ──► useMutation(executeRule)
   │     │                       │  POST /v1/execute { facts, engine, trace }
   │     │                       ▼
   │     │                 [API] execute_single (execution.rs:512)
   │     │                       │  resolve collect_trace = trace.unwrap_or(false)
   │     │                       │  engine.load_ruleset(shared) ; insert facts
   │     │                       │  web::block(engine.execute()) ──► [Engine]
   │     │                       │    direct_execute OR ReteEngine::execute_rules
   │     │                       │      fixpoint loop: evaluate condition → apply action
   │     │                       │      if collect_trace: push trace event per fired rule
   │     │                       │      (timestamp, rule_id, in firing order)
   │     │                       │    returns EngineResult { fired_rule_ids, trace_events? }
   │     │                       │  map engine trace_events → InferenceTraceEntry
   │     │                       │  build facts_after from engine.get_facts()
   │     │                       │  return ExecuteResponse { rules_fired, rules_not_fired,
   │     │                       │                          facts_after, inference_trace?,
   │     │                       │                          execution_id, engine_generation }
   │     │                       ▼
   │     └─ results panel renders ExecuteResponse directly (NO polling — Pitfall 8)
   │           Fired / Not-fired / Facts after / Trace (hidden if None)
   │
   └─ "Reload then Run" button (D-07):
         reloadRules({ asyncReload: true })  ──► POST /v1/rules/reload → 202 { job_id, status }
              │
              ▼  poll (the ONE legitimate polling path — Pitfall 8 carve-out)
         useQuery(['job', job_id], { enabled: !!jobId, refetchInterval })
              │  GET /v1/jobs/{job_id} → { job_id, status, result?, error? }
              │  until status === 'completed' | 'failed'
              ▼
         executeRule(...) → render result (compare engine_generation to pre-reload gen)
```

### Recommended Project Structure
```
frontend/src/
├── api/
│   ├── execution.ts          # NEW (D-10): executeRule, batchExecute, getJobStatus,
│   │                         #   listJobs, listEngines, reloadRules (6th fn — CF#3)
│   ├── types.ts              # ADD: ExecuteRequest/Response, InferenceTrace*, EnginesResponse,
│   │                         #   ReloadJob, BatchExecute* (snake_case keys)
│   ├── client.ts             # unchanged — import apiClient
│   └── rules.ts              # unchanged (pattern reference; has updateRule for Save enabler)
├── components/
│   └── TestRunner.tsx        # NEW (D-04): facts editor, dropdown, buttons, results panel
├── routes/rules/
│   └── $id.edit.tsx          # EDIT: mount <TestRunner ruleId={id} /> as new <section>
│                             #   + optional minimal Save → updateRule (D-05)
└── main.tsx                  # EDIT (CF#2): wrap in <QueryClientProvider> if using TanStack Query

engine/src/engine/
├── registry.rs               # EDIT: add engine-side TraceEvent type + EngineResult.trace field
├── direct_eval.rs            # EDIT: thread collect_trace, emit TraceEvent per fired rule (site 1)
└── rete/mod.rs               # EDIT: thread collect_trace in execute_rules, emit TraceEvent (site 2 — C1)

api/src/
├── v1/execution.rs           # EDIT: ExecuteRequest.trace, execute_single collect_trace + mapping,
│                             #   GET /v1/engines handler, replace inference_trace: None at :730/:950
└── main.rs                   # EDIT: register GET /v1/engines in /v1 scope (~:261)
```

### Pattern 1: One-function-per-endpoint API module (mirror `rules.ts`)
**What:** Each backend endpoint gets one typed async function in `api/*.ts` importing the shared `apiClient`.
**When to use:** Always for new API surfaces.
**Example:**
```typescript
// Source: frontend/src/api/rules.ts (read in full this session) + D-10
import apiClient from './client'
import type { ExecuteRequest, ExecuteResponse, EnginesResponse, ReloadJob } from './types'

export async function executeRule(req: ExecuteRequest): Promise<ExecuteResponse> {
  const response = await apiClient.post<ExecuteResponse>('/execute', req)
  return response.data
}

export async function listEngines(): Promise<EnginesResponse> {
  const response = await apiClient.get<EnginesResponse>('/engines')
  return response.data
}

export async function getJobStatus(jobId: string): Promise<ReloadJob> {
  const response = await apiClient.get<ReloadJob>(`/jobs/${jobId}`)
  return response.data
}

export async function reloadRules(asyncReload = true): Promise<{ job_id: string; status: string }> {
  const response = await apiClient.post('/rules/reload', { async_reload: asyncReload })
  return response.data
}
```

### Pattern 2: snake_case TS types matching backend serde
**What:** The backend serializes with `serde::Serialize` defaults (snake_case). TS interfaces use the exact keys — no camelCase transform. Precedent: `ApprovalState`/`ApprovalsResponse` in `types.ts:70-110`.
```typescript
// Source: api/src/v1/execution.rs:47-73 (ExecuteResponse fields) + types.ts:101-110 precedent
export interface ExecuteResponse {
  execution_id: string
  rules_fired: number
  rules_not_fired: string[]
  facts_after: Record<string, unknown>
  inference_trace?: InferenceTrace | null
  engine_generation: number
  session_id?: string | null
}
export interface InferenceTrace {
  entries: InferenceTraceEntry[]
  summary?: TraceSummary | null
}
export interface InferenceTraceEntry {
  timestamp: string
  event_type: string
  rule_name?: string | null
  data?: unknown | null
}
export interface TraceSummary {
  total_duration_ms: number
  facts_matched: number
}
export interface EnginesResponse {
  engines: string[]
  default: string
}
export interface ReloadJob {
  job_id: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  result?: unknown
  error?: string | null
}
```

### Pattern 3: Engine-side trace type → API mapping (D-01, the cross-crate boundary)
**What:** The engine emits its own trace type; `execute_single` maps it to the wire schema.
```rust
// engine/src/engine/registry.rs — NEW engine-side type (engine cannot import api types)
#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub rule_id: String,
    pub timestamp: String,        // chrono::Utc::now().to_rfc3339()
    pub event_type: &'static str, // "rule_fired"
}

#[derive(Debug, Clone, Default)]
pub struct TraceData {
    pub events: Vec<TraceEvent>,
    pub total_duration_ms: f64,
    pub facts_matched: usize,
}

// Add to EngineResult:
pub struct EngineResult {
    // ...existing fields...
    pub fired_rule_ids: Vec<String>,
    #[deprecated = "use trace.events ordered"] // fired_rule_ids is HashSet-order today
    pub trace: Option<TraceData>,   // None when collect_trace == false (SC-3)
}
```
```rust
// direct_eval.rs — thread collect_trace, emit in firing order
pub fn direct_execute(
    rules: &[Rule],
    working_facts: &mut HashMap<String, Fact>,
    collect_trace: bool,           // NEW param
) -> (HashSet<String>, HashSet<String>, Option<TraceData>) { ... }
```
```rust
// api/src/v1/execution.rs execute_single — map engine TraceData → InferenceTrace
let inference_trace = if collect_trace {
    engine_result.trace.as_ref().map(|td| InferenceTrace {
        entries: td.events.iter().map(|e| InferenceTraceEntry {
            timestamp: e.timestamp.clone(),
            event_type: e.event_type.to_string(),
            rule_name: Some(e.rule_id.clone()),
            data: None,            // D-01 discretion: lean payload (SC-3)
        }).collect(),
        summary: Some(TraceSummary {
            total_duration_ms: td.total_duration_ms,
            facts_matched: td.facts_matched,
        }),
    })
} else { None };
```

### Pattern 4: `GET /v1/engines` handler (D-03)
```rust
// Source: reuse the exact default-resolution from execute_single (execution.rs:541-544)
#[utoipa::path(get, path = "/v1/engines", tag = "execution",
    responses((status = 200, description = "Available engines", body = EnginesResponse)))]
pub async fn list_engines(
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
) -> impl Responder {
    let engines: Vec<String> = registry.available_engines().iter().map(|s| s.to_string()).collect();
    let default = std::env::var("RRULES_DEFAULT_ENGINE")
        .unwrap_or_else(|_| selector.current_engine().to_string());
    HttpResponse::Ok().json(serde_json::json!({ "engines": engines, "default": default }))
}
// Register in main.rs /v1 scope: .route("/engines", web::get().to(v1::execution::list_engines))
```
`#[derive(serde::Serialize, utoipa::ToSchema)] struct EnginesResponse { engines: Vec<String>, default: String }` is cleaner than the inline `json!` — recommended.

### Anti-Patterns to Avoid
- **Polling for execute results (Pitfall 8):** Do NOT poll `/v1/metrics/coverage` or the WS channel after Run. Render the `ExecuteResponse` directly. Polling is ONLY for `getJobStatus` during "Reload then Run". `[CITED: .planning/research/PITFALLS.md §Pitfall 8]`
- **Hardcoding engine names in the frontend:** Violates D-03's honesty contract. Always fetch from `GET /v1/engines`. `[CITED: 28-CONTEXT.md D-03]`
- **Deriving the trace from `rules_not_fired`/`CoverageTracking`:** Rejected by D-01 — the trace must reflect what the engine actually did, in firing order. `[CITED: 28-CONTEXT.md D-01]`
- **Emitting trace only at one evaluator site:** Phase 26 C1 established recording at BOTH `direct_eval.rs` (Treat/Leaps/Phreak) AND `rete/mod.rs` (Rete's inlined `execute_rules`). Trace emission must mirror this — otherwise Rete runs show no trace. `[VERIFIED: engine/src/engine/rete/mod.rs:341, engine/src/engine/direct_eval.rs:54]`
- **Adding `InferenceTraceEntry` import to the engine crate:** Impossible — dependency direction is `api → engine`. Use an engine-side type. `[VERIFIED: engine/Cargo.toml has no api dep]`
- **Echoing request facts as `facts_after`:** Already fixed in Phase 23 (Pitfall 11); do not regress. `[CITED: 23-CONTEXT D-08]`

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client + auth + 401 redirect | custom fetch wrapper | shared `apiClient` (`client.ts:11`) | Already handles Bearer token + 401 redirect (L-05). |
| JSON editor | custom textarea | `@uiw/react-codemirror` (`RuleEditor.tsx`) | Reuse existing wrapper; JSON via `javascript({ json: true })`. |
| Engine list | hardcoded array | `GET /v1/engines` (D-03) | Feature-gated engines would lie if hardcoded. |
| Polling loop | raw `setInterval` | TanStack `useQuery` `refetchInterval` (if QueryClient set up) | Standard, cancellable, cache-aware. |
| Firing-order tracking | reconstruct from `fired_rule_ids` (HashSet order!) | emit `TraceEvent` in the fixpoint loop | `fired_rule_ids` is `fired_set.iter().cloned().collect()` — **HashSet order is NOT firing order** (`rete/mod.rs:415`). `[VERIFIED]` |
| Trace timestamps | `SystemTime::now()` | `chrono::Utc::now().to_rfc3339()` | Established Phase-26 pattern; RFC3339 is the wire format. `[VERIFIED: engine/Cargo.toml:12]` |

**Key insight:** `EngineResult.fired_rule_ids` (`registry.rs:126`) is currently populated from a `HashSet` and is therefore **unordered**. A real trace needs firing order, which only the evaluator's fixpoint loop can capture. This is the single most important non-obvious finding for D-01.

## Runtime State Inventory

> Phase 28 is primarily additive (new route, new component, new trace field) — not a rename/refactor. A light inventory confirms no runtime-state migration is needed.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — `inference_trace` is a new response field; no persisted trace records. | None. |
| Live service config | None — `GET /v1/engines` reads already-shared `web::Data`; no service reconfiguration. | None. |
| OS-registered state | None. | None. |
| Secrets/env vars | `RRULES_DEFAULT_ENGINE` reused (existing, read by `execute_single:542` and the new handler). No new secrets. | None. |
| Build artifacts | None new. Frontend may add optional devDeps (`jsdom`/`jest-dom`) — `node_modules` regenerates from `package.json`. | `npm install` if gaps found. |

**Nothing found in any category requiring a data migration.** All changes are code edits + one optional dep install.

## Common Pitfalls

### Pitfall 1: Trace emission at only one evaluator site (C1 violation)
**What goes wrong:** Rete runs show `inference_trace: null` while Treat/Leaps/Phreak show the trace.
**Why it happens:** The shared `direct_execute` helper (`direct_eval.rs:54`) serves Treat/Leaps/Phreak, but `ReteEngine::execute_rules` (`rete/mod.rs:341`) is a separate inlined implementation that does NOT delegate. Phase 26 C1 documented this exact two-site requirement for coverage recording.
**How to avoid:** Thread `collect_trace` and emit `TraceEvent` at BOTH sites. The `direct_execute` signature gains a `collect_trace: bool` param; `ReteEngine::execute_rules` gains the same logic inline.
**Warning signs:** A trace-on test passes for `engine: "treat"` but fails for `engine: "rete"`. `[VERIFIED: engine/src/engine/rete/mod.rs:341-417, engine/src/engine/direct_eval.rs:54-125]`

### Pitfall 2: `fired_rule_ids` mistaken for firing order
**What goes wrong:** Trace entries appear in arbitrary order, not execution order.
**Why it happens:** `EngineResult.fired_rule_ids` is `fired_set.iter().cloned().collect()` — `HashSet` iteration is non-deterministic (`rete/mod.rs:415`, `direct_eval.rs` returns the set).
**How to avoid:** Emit `TraceEvent` in the fixpoint loop at the moment each rule fires; do NOT reconstruct order from `fired_rule_ids`.
**Warning signs:** Trace entry order changes between runs of the same request. `[VERIFIED: engine/src/engine/rete/mod.rs:415]`

### Pitfall 3: TanStack Query used without a QueryClientProvider
**What goes wrong:** `useQuery`/`useMutation` throw "No QueryClient set" at runtime.
**Why it happens:** `main.tsx` has no `QueryClientProvider`; the codebase has never used TanStack Query (despite the dep being installed).
**How to avoid:** Wave 0 wraps the app: `<QueryClientProvider client={queryClient}>...</QueryClientProvider>` in `main.tsx`. Alternatively, fall back to `useState`+`useEffect` (but that contradicts D-08/D-10).
**Warning signs:** Component tests fail with "No QueryClient set". `[VERIFIED: frontend/src/main.tsx, grep useQuery → 0 hits]`

### Pitfall 4: Polling for execute results (Pitfall 8 regression)
**What goes wrong:** Stale results panel after a reload; race between execute completion and a poll.
**Why it happens:** Developer adds a follow-up poll to "refresh" the panel after Run.
**How to avoid:** The `useMutation` `onSuccess` renders the `ExecuteResponse` directly. `execution_id`/`engine_generation` are the correlation fields. Polling is ONLY for `getJobStatus` during the async reload branch.
**Warning signs:** A `refetchInterval` on any query other than `['job', jobId]`. `[CITED: .planning/research/PITFALLS.md §Pitfall 8]`

### Pitfall 5: SC-3 latency regression from trace collection
**What goes wrong:** Trace-off runs are slower than Phase 23/24 baseline.
**Why it happens:** Trace `Vec`/`String` allocation happens unconditionally.
**How to avoid:** `collect_trace = req.trace.unwrap_or(false)`; the evaluator checks `if collect_trace` before any `TraceEvent` allocation or `chrono::Utc::now()` call. The `EngineResult.trace` field is `None` and `Vec` is never allocated when off.
**Warning signs:** Trace-off latency benchmark differs from Phase 24 baseline. `[CITED: 28-CONTEXT.md D-02]`

### Pitfall 6: Edit page can't save → "Run an edited rule" is impossible
**What goes wrong:** User edits a rule, clicks Run, and sees the OLD behavior (unsaved edits never reached the shared `RuleSet`).
**Why it happens:** `$id.edit.tsx` has `TODO(22-05/22-06)` — no fetch/save wiring. `/v1/execute` runs the on-disk shared `RuleSet`, not the in-editor text.
**How to avoid:** D-05 — either wire a minimal Save button to `PUT /v1/rules/{id}` (the Phase-20 endpoint `rules.ts:updateRule` already wraps), or document "save via management UI first, then Reload then Run." The "Reload then Run" flow (D-07) is the bridge: save → reload → run.
**Warning signs:** SC-4 fails because `facts_after` reflects the pre-edit rule. `[VERIFIED: frontend/src/routes/rules/$id.edit.tsx:10 TODO]`

## Code Examples

### Backend: `ExecuteRequest.trace` field (D-02)
```rust
// Source: api/src/v1/execution.rs:21-35 (current) + D-02
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct ExecuteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub facts: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_filter: Option<RuleFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Phase 28 (D-02 / EXEC-04): opt-in inference trace. Off by default
    /// for performance (SC-3). When absent/false the evaluator skips all
    /// trace collection and `inference_trace` is `None`.
    #[serde(default)]
    pub trace: Option<bool>,
}
```

### Backend: `collect_trace` resolution in `execute_single` (D-02)
```rust
// Source: api/src/v1/execution.rs:512-735 — insert after engine_name resolution
let collect_trace = req.trace.unwrap_or(false);
// ... pass `collect_trace` into the engine via a config or a new trait method.
// After web::block, build inference_trace from engine_result.trace:
let inference_trace = if collect_trace {
    engine_result.trace.as_ref().map(|td| InferenceTrace {
        entries: td.events.iter().map(|e| InferenceTraceEntry {
            timestamp: e.timestamp.clone(),
            event_type: e.event_type.to_string(),
            rule_name: Some(e.rule_id.clone()),
            data: None,
        }).collect(),
        summary: Some(TraceSummary {
            total_duration_ms: td.total_duration_ms,
            facts_matched: td.facts_matched,
        }),
    })
} else { None };
// Replace `inference_trace: None` at :730 (stateless) and :950 (sessionful).
```

> **Threading `collect_trace` into the engine:** The `ExecutionEngine` trait (`registry.rs:64`) `execute(&mut self) -> EngineResult` takes no params. Two options: (a) add a `set_collect_trace(&mut self, bool)` method (default `noop`, engines override) called before `execute()`, or (b) add a field on `ReteEngine`/the engines set before `execute()`. Option (a) is trait-consistent and mirrors `set_config`. The planner picks; both are honest. `[VERIFIED: engine/src/engine/registry.rs:64-108]`

### Frontend: TestRunner mounting (D-04)
```tsx
// Source: frontend/src/routes/rules/$id.edit.tsx (current) + D-04
import { TestRunner } from '@/components/TestRunner'
// ... inside EditRulePage return, after the DecisionTable <section>:
<section className="space-y-2">
  <h2 className="font-semibold">Test Runner</h2>
  <TestRunner ruleId={id} />
</section>
```

### Frontend: "Reload then Run" polling (D-07, Pitfall 8 carve-out)
```tsx
// useQuery for job-status polling — enabled only after a reload is triggered
const { data: job } = useQuery({
  queryKey: ['job', jobId],
  queryFn: () => getJobStatus(jobId!),
  enabled: !!jobId,
  refetchInterval: (query) => {
    const status = query.state.data?.status
    return status === 'completed' || status === 'failed' ? false : 500
  },
})
// When job.status === 'completed', call executeRule(...) and render result.
// Compare response.engine_generation to the pre-reload generation (SC-4).
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `inference_trace: None` always (Phase 23 L-04) | Populated when `trace: true` (Phase 28 EXEC-04) | Phase 28 | Trace is real per-fired-rule data, not derived. |
| `EngineResult.fired_rule_ids` (HashSet order) | Add `trace: Option<TraceData>` with ordered events | Phase 28 | Firing-order is now captured. |
| No engine-list endpoint | `GET /v1/engines` (D-03) | Phase 28 | Frontend dropdown is honest about feature-gated engines. |
| Edit page scaffold (TODO 22-05/22-06) | Minimal Save enabler (D-05) | Phase 28 | "Edit → save → reload → run" becomes possible. |

**Deprecated/outdated:**
- `InferenceTraceCollector` (`engine/src/chaining/mode_selector.rs:135`): NOT to be wired (L-02) — coupled to the stub `environment::Engine` surface.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `jsdom` and `@testing-library/jest-dom` may be missing from devDeps | Standard Stack / Validation | Wave-0 install needed; planner must `npm ls` first. |
| A2 | `set_collect_trace` trait method (or equivalent) is the threading mechanism | Code Examples | Planner may choose a different mechanism (engine field); contract is the same. |
| A3 | `EnginesResponse` struct is cleaner than inline `json!` | Pattern 4 | Minor; planner may inline. |
| A4 | `TraceSummary.facts_matched` counts matched facts (discretion) | Pattern 3 | Low — field just needs an honest value. |
| A5 | `@codemirror/lang-json` 6.0.2 is legitimate (official CodeMirror) | Package Audit | Verified via `npm view`; well-known package. |

**All other claims were verified against the codebase this session (file:line cited inline).**

## Open Questions (RESOLVED)

1. **How does `collect_trace` reach the engine's `execute()`?**
   - What we know: `ExecutionEngine::execute(&mut self) -> EngineResult` takes no params (`registry.rs:66`); `execute_single` calls `engine.execute()` inside `web::block` (`execution.rs:632`).
   - What's unclear: whether to add a `set_collect_trace(&mut self, bool)` trait method (default noop) or set a field directly on each engine before `execute()`.
   - RESOLVED: Add `fn set_collect_trace(&mut self, _enabled: bool) {}` to the trait with a default noop; `ReteEngine` and the `direct_execute` callers override. This mirrors `set_config` (`registry.rs:87`). For the shared `direct_execute` helper, pass `collect_trace: bool` as a third arg directly (cleaner than a field since the helper is a free function). → Plan 28-03 Task 1.

2. **Does the edit page's missing fetch/save wiring block "Run an edited rule" (SC-4)?**
   - What we know: `$id.edit.tsx:10` has `TODO(22-05/22-06)` — no fetch/save; `/v1/execute` runs the on-disk shared `RuleSet` (Phase 25).
   - What's unclear: whether D-05's "minimal Save enabler" is strictly required for SC-4 or whether documenting "save via management UI first" suffices.
   - RESOLVED: Wire a minimal Save button to `updateRule(id, ...)` (`rules.ts:33`) — it's a 1-call wiring reusing the existing Phase-20 endpoint, and SC-4's "edit → reload → run → see edited result" honesty check is otherwise unverifiable from the edit page. Included as a small enabler task. → Plan 28-05 Task 3.

3. **`rule_filter` scoping — scope Run to the edited rule or run the whole ruleset?**
   - What we know: `ExecuteRequest.rule_filter: Option<RuleFilter>` with `name_pattern: Option<String>` (`execution.rs:27-44`); `/v1/execute` supports it.
   - What's unclear: whether the TestRunner sends `rule_filter: { name_pattern: id }` to isolate the edited rule.
   - RESOLVED: Scope to the edited rule via `name_pattern` for the "test this rule" UX (cleaner results panel, fewer `rules_not_fired`). → Plan 28-05 Task 1.

4. **Trace `data` payload size vs SC-3 latency.**
   - What we know: D-01 leaves `data` to the planner; a rich payload risks latency.
   - RESOLVED: `data: None` for Phase 28 (lean). A rich fact/action snapshot is explicitly deferred. This keeps SC-3 honest. → Plan 28-03 Task 2.

5. **Does `available_engines()` return a deterministic order?**
   - What we know: `registry.rs:234` iterates `self.factories.keys()` (a `HashMap`) — order is NOT guaranteed. The dropdown may reorder between requests.
   - RESOLVED: Sort the `engines` array in the handler (`engines.sort()`) so the dropdown is stable. Low-effort, good UX. → Plan 28-03 Task 2.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `chrono` (engine) | Trace timestamps | ✓ | 0.4 | — |
| `utoipa` (api) | OpenAPI for new route | ✓ | 4 | — |
| `@tanstack/react-query` | D-08/D-10 queries | ✓ (installed, unused) | ^5.50.0 | `useState`+`useEffect` (contradicts D-08/D-10) |
| `@uiw/react-codemirror` | Facts editor (D-06) | ✓ | ^4.25.10 | — |
| `@codemirror/lang-javascript` | JSON mode | ✓ | ^6.2.5 | add `@codemirror/lang-json` |
| Vitest | Component tests | ✓ (devDep, no config) | ^2.0.0 | — |
| `@testing-library/react` | Component assertions | ✓ (devDep) | ^16.0.0 | — |
| `jsdom` | Vitest DOM env | `[ASSUMED — verify]` | — | happy-dom (if preferred) |
| `@testing-library/jest-dom` | DOM matchers | `[ASSUMED — verify]` | — | plain `expect` asserts |
| Rust toolchain | Backend build | ✓ | (project builds today) | — |
| Node/npm | Frontend build | ✓ | (project builds today) | — |

**Missing dependencies with no fallback:** none blocking — `jsdom`/`jest-dom` are Wave-0 installs if absent.
**Missing dependencies with fallback:** TanStack Query has a `useState` fallback but it contradicts locked D-08/D-10; prefer the `QueryClientProvider` enabler.

## Validation Architecture

> Nyquist validation is enabled (`.planning/config.json` → `workflow.nyquist_validation: true`). This phase touches BOTH backend (`api` + `engine` crates) and frontend (`frontend` Vitest). Two test surfaces.

### Test Framework
| Property | Value |
|----------|-------|
| Backend framework | `cargo test` (Rust integration tests in `api/tests/`, unit tests in `engine/`) |
| Backend config | `api/Cargo.toml`, `engine/Cargo.toml` — existing |
| Backend quick run | `cargo test -p api --test execute_integration` |
| Backend full suite | `cargo test -p api -p engine` |
| Frontend framework | Vitest ^2.0.0 + `@testing-library/react` ^16.0.0 (devDeps) |
| Frontend config | **NONE — `vite.config.ts` has no `test` block; no setup file; no tests exist** (Wave 0 gap) |
| Frontend quick run | `cd frontend && npm test -- --run` (after Wave 0 scaffolds config) |
| Frontend full suite | `cd frontend && npm test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXEC-04 | `inference_trace` is `Some` with one entry per fired rule when `trace: true`; `None` when `trace` absent/false | integration | `cargo test -p api --test execute_integration -- trace_on_populates_entries trace_off_is_none` | ❌ Wave 0 (RED) |
| EXEC-04 (Rete site) | Trace populated for `engine: "rete"` (C1 two-site) | integration | `cargo test -p api --test execute_integration -- rete_trace_populated` | ❌ Wave 0 (RED) |
| EXEC-04 (latency) | Trace-off latency ≈ Phase 24 baseline (SC-3) | integration/manual | `cargo test -p api --test execute_integration -- trace_off_latency_neutral` (or manual benchmark) | ❌ Wave 0 (RED) — may be manual-only with justification (timing assertions are flaky) |
| FRONTEND-04 / D-03 | `GET /v1/engines` returns `{ engines, default }` with `available_engines()` contents | integration | `cargo test -p api --test execute_integration -- list_engines_returns_registry` | ❌ Wave 0 (RED) |
| FRONTEND-03 | Run renders `rules_fired`/`rules_not_fired`/`facts_after` | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "renders fired rules and facts_after"` | ❌ Wave 0 (RED) |
| FRONTEND-04 | Engine dropdown populates from `GET /v1/engines` | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "dropdown lists engines"` | ❌ Wave 0 (RED) |
| EXEC-04 (UI) | Trace section hidden when `inference_trace` is `None`; shown when present | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "trace section hidden when null"` | ❌ Wave 0 (RED) |
| SC-4 (D-07) | "Reload then Run" polls `getJobStatus` then executes | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "reload then run polls job status"` | ❌ Wave 0 (RED) |
| SC-1 keystone | `score := score + 10` → `facts_after.scorecard.score == 10` through UI | e2e/manual | (manual or Playwright — not currently set up) | ❌ manual-only |

### Sampling Rate
- **Per task commit (backend):** `cargo test -p api --test execute_integration`
- **Per task commit (frontend):** `cd frontend && npm test -- --run`
- **Per wave merge:** `cargo test -p api -p engine && cd frontend && npm test`
- **Phase gate:** Full backend + frontend suites green before `/gsd-verify-work`; SC-1..SC-4 keystone e2e demonstrated.

### Wave 0 Gaps
- [ ] `api/tests/execute_integration.rs` — add RED scaffolds: `trace_on_populates_entries`, `trace_off_is_none`, `rete_trace_populated`, `list_engines_returns_registry` (extend existing test file — reuse `init_test_app!`/`configure_test_routes` fixtures at `:231-391`).
- [ ] `frontend/vite.config.ts` — add `test` block: `{ environment: 'jsdom', globals: true, setupFiles: ['./src/test/setup.ts'] }`.
- [ ] `frontend/src/test/setup.ts` — NEW: import `@testing-library/jest-dom` (install if missing).
- [ ] `frontend/src/components/__tests__/TestRunner.test.tsx` — NEW RED scaffolds for the 4 component behaviors above.
- [ ] `frontend/src/main.tsx` — add `QueryClientProvider` (Critical Finding 2) so `useQuery`/`useMutation` work in tests AND the app.
- [ ] Verify/install `jsdom` + `@testing-library/jest-dom` (`npm ls jsdom @testing-library/jest-dom`).
- [ ] No new backend framework install needed (`cargo test` already runs).

### SC-1..SC-4 Verifiability
- **SC-1** (Run shows real `rules_fired`/`rules_not_fired`/`facts_after`): the existing Phase-23 keystone rule (`score := score + 10`) re-exercised through the TestRunner. Verifiable by the `TestRunner.test.tsx` "renders fired rules and facts_after" test + a manual e2e click. Backend already proven by `execute_integration.rs`.
- **SC-2** (dropdown lists `GET /v1/engines`, selection sent): `list_engines_returns_registry` integration test + `TestRunner.test.tsx` "dropdown lists engines" component test. Honesty check: selecting `treat` sends `engine: "treat"` and the response reflects a real treat execution.
- **SC-3** (trace on shows trace; off omits + latency-neutral): `trace_on_populates_entries` + `trace_off_is_none` integration tests + `TestRunner.test.tsx` "trace section hidden when null". Latency neutrality: manual benchmark or a loose timing assertion (justification for manual-only: tight timing assertions are flaky in CI).
- **SC-4** (Reload then Run reflects edited rule via `engine_generation`): `TestRunner.test.tsx` "reload then run polls job status" component test (mock `getJobStatus` returning `completed` then `executeRule`) + the existing Phase-25 reload integration test. Honesty check: `engine_generation` increments after reload.

## Security Domain

> `security_enforcement` is not set in `.planning/config.json` (absent = enabled). Light security review — this phase adds one unauthenticated-behind-middleware route and no new data flows.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no (no new auth) | Existing JWT `requires_auth` middleware wraps `/v1` scope (`main.rs:251`) — `GET /v1/engines` inherits it. |
| V3 Session Management | no | No new sessions (TestRunner uses stateless path; `session_id` absent). |
| V4 Access Control | yes (light) | `GET /v1/engines` is behind `requires_auth` — any authenticated user can list engines. No role restriction needed (read-only metadata). |
| V5 Input Validation | yes | `trace: Option<bool>` — `#[serde(default)]` makes absent→`None`; `unwrap_or(false)` is total. No free-form input. `EnginesResponse` is server-derived. |
| V6 Cryptography | no | No new crypto. |

### Known Threat Patterns for the stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Unauthorized engine enumeration | Information disclosure | `requires_auth` middleware on `/v1` scope (existing). |
| Trace data leakage (rich `data` payload) | Information disclosure | `data: None` for Phase 28 (D-01 discretion — lean payload). |
| Trace-induced DoS (large trace) | Denial of Service | Trace is opt-in (`trace: Option<bool>`, off by default); one entry per fired rule bounded by `|rules|`; `RRULES_MAX_CHAIN_ITERATIONS` cap (existing). |
| Stale-result display (Pitfall 8) | Tampering (logic) | Drive from `ExecuteResponse`; `engine_generation` correlation. |

## Sources

### Primary (HIGH confidence — read this session)
- `api/src/v1/execution.rs` — `ExecuteRequest` (:21-35), `ExecuteResponse` (:47-73), `InferenceTrace`/`InferenceTraceEntry`/`TraceSummary` (:188-213), `execute_single` (:512-735), `execute_sessionful` (:750-955), `inference_trace: None` sites (:730, :950), `get_job_status` (:1240-1254), `list_jobs` (:1265-1271), `reload_rules` (:1120-1203), `ReloadJob`/`ReloadStatus` (:382-431), `BatchExecuteRequest` (:462-).
- `engine/src/engine/direct_eval.rs` — `direct_execute` (:54-125), two-site recording (:113-122).
- `engine/src/engine/rete/mod.rs` — `ReteEngine::execute_rules` (:341-417), `fired_rule_ids` HashSet-order (:415), C1 recording (:400-409).
- `engine/src/engine/registry.rs` — `ExecutionEngine` trait (:64-108), `EngineResult` (:112-127), `available_engines` (:234-239), `create_engine_by_name` (:222-231).
- `engine/src/engine/selector.rs` — `current_engine` (:82).
- `api/src/main.rs` — `/v1` scope + `web::Data` wiring (:208-299), `requires_auth` wrap (:251).
- `frontend/src/api/client.ts` — `apiClient` baseURL (:11).
- `frontend/src/api/rules.ts` — one-function-per-endpoint pattern (full file).
- `frontend/src/api/types.ts` — snake_case precedent `ApprovalState`/`ApprovalsResponse` (:70-110).
- `frontend/src/components/RuleEditor.tsx` — CodeMirror usage (:1-53).
- `frontend/src/routes/rules/$id.edit.tsx` — TODO(22-05/22-06) gap (:10), section pattern.
- `frontend/src/features/rules/useRules.ts` — plain `useState`/`useEffect` (NOT TanStack Query).
- `frontend/src/main.tsx` — no `QueryClientProvider`.
- `frontend/vite.config.ts` — no `test` block.
- `frontend/package.json` — React 19, TanStack ^5.50, Vitest ^2.0.0, `@testing-library/react` ^16.0.0, `@codemirror/lang-javascript` ^6.2.5 (no `lang-json`).
- `engine/Cargo.toml` / `api/Cargo.toml` — `chrono` 0.4, `utoipa` 4, `dashmap` 6.2.1 present; no `api` dep in `engine`.
- `api/tests/execute_integration.rs` — test app fixtures + `init_test_app!`/`configure_test_routes` (:231-418).
- `.planning/phases/28-frontend-test-runner/28-CONTEXT.md` — D-01..D-10, L-01..L-09, Specifics (honesty anchors).
- `.planning/ROADMAP.md` §Phase 28 (:252-280) — SC-1..SC-4, Pitfalls 8+3.
- `.planning/config.json` — `nyquist_validation: true`.

### Secondary (MEDIUM confidence)
- `npm view @codemirror/lang-json` → 6.0.2 (registry-confirmed, official CodeMirror package).
- `.planning/research/PITFALLS.md` §Pitfall 8 (cited via CONTEXT canonical_refs).

### Tertiary (LOW confidence)
- None — all claims verified or cited.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all deps verified in `package.json`/`Cargo.toml` this session.
- Architecture (D-01 trace boundary): HIGH — cross-crate dependency direction verified; two-site C1 pattern verified in live code.
- Architecture (D-03 engines route): HIGH — reuses verified `web::Data` wiring + verified default-resolution expression.
- Pitfalls: HIGH — grounded in file:line evidence (HashSet order at `rete/mod.rs:415`, two-site at `direct_eval.rs`+`rete/mod.rs`, no QueryClientProvider in `main.tsx`).
- Validation: HIGH for backend (existing test fixtures verified), MEDIUM for frontend (test infra must be scaffolded from zero; `jsdom`/`jest-dom` availability unverified).

**Research date:** 2026-07-06
**Valid until:** 2026-08-05 (30 days — stable brownfield codebase; no fast-moving external deps)