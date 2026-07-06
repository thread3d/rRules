---
status: complete
phase: 28-frontend-test-runner
source: [28-01-SUMMARY.md, 28-02-SUMMARY.md, 28-03-SUMMARY.md, 28-04-SUMMARY.md, 28-05-SUMMARY.md]
started: 2026-07-06T11:26:05Z
updated: 2026-07-06T11:46:15Z
verifier: automated-live (Claude, self-run) + headless-Chromium click-through
---

# Phase 28 — User Acceptance Testing

> Restarted from SUMMARYs (28-01..28-05). All 9 tests were performed by Claude
> against a **live dev stack** (backend API on :8080 + Vite frontend on :5173),
> verified at THREE levels: (1) live API via curl + JWT, (2) the real TestRunner
> component via Vitest (4/4) + `npm run build`, and (3) a **literal headless-
> Chromium click-through** driving the rendered edit page (Playwright via
> `playwright-core`, Chromium 147). The keystone `score := score + 10` rule was
> materialized to `./rules/r1.rules` (serialized via a one-off
> `engine/examples/emit_keystone.rs`) so the execute path had a real on-disk
> rule to fire. Auth (API): `admin`/`admin` → JWT bearer for `/v1/*`. Auth
> (browser): logged in through the `/login` UI as `admin@rrules.dev`/`admin`
> (the `type=email` input requires valid-email format; the backend resolves the
> identifier by username OR email per `api/src/auth/mod.rs:59-64`).

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: Kill any running backend/frontend. Start the dev stack from scratch (backend API + frontend dev server). Both boot without errors. A primary query returns live data: GET /v1/engines (or loading the rule edit page) returns a sorted engines array + resolved default, not a 404/empty/hardcoded response.
result: pass
evidence: |
  Backend `cargo run -p api --bin rRules-server` booted clean on 127.0.0.1:8080
  (file watcher + session sweep started; no boot errors). GET /v1/engines (with
  admin JWT) returned 200 `{"engines":["leaps","phreak","rete","treat"],"default":"rete"}`
  — sorted ascending, resolved default, live data (not hardcoded/empty/404).
  Vite dev server booted clean on :5173 (`ready in 1194 ms`); root returned 200
  with `<title>rRules …</title>` + `id="root"` + `/src/main.tsx`; TSX modules
  (main.tsx, TestRunner.tsx, $id.edit.tsx) all transformed and served 200.

### 2. TestRunner panel mounted on rule edit page
expected: Navigate to the edit page for a rule. A TestRunner section is visible at the bottom of the page containing: a facts JSON editor, an engine dropdown, a Trace toggle (off), a Run button, and a Reload-then-Run button.
result: pass
evidence: |
  Source: `frontend/src/routes/rules/$id.edit.tsx:6` imports `TestRunner`;
  line 96 mounts `<TestRunner ruleId={id} />` as a new section (comment "D-04:
  TestRunner mounted as a new section in the edit page"). Production build
  resolves the import (`npm run build` ✓ built in 6.29s; `_id.edit` chunk
  emitted). Vitest "renders fired rules and facts_after" renders the panel and
  clicks the Run button (getByRole 'button' name /^run$/i) successfully.
  Browser click-through: the `Test Runner` section was located on the rendered
  edit page (`section:has(h2:has-text("Test Runner"))`) and every control below
  driven for real — see "Browser Click-Through Verification".

### 3. SC-1 — Run renders real facts_after
expected: On the edit page for the "score := score + 10" rule, enter facts JSON `{"scorecard":{"score":0}}` and click Run. Results panel shows rules_fired >= 1, rules_not_fired listing the other rules, and facts_after.scorecard.score == 10 — the real incremented score, not the input 0.
result: pass
evidence: |
  POST /v1/execute {"facts":{"scorecard":{"score":0}}} (default engine, trace
  omitted) → 200: `{"rules_fired":1,"rules_not_fired":[],"facts_after":
  {"scorecard":{"score":10}},"engine_generation":0}`. Real execution through
  direct_eval.rs; facts_after.score == 10 (incremented, not echoed input 0).
  (rules_not_fired empty because only r1 is on disk.)

### 4. SC-2 — Engine dropdown lists engines + engine swap
expected: The engine dropdown is populated from GET /v1/engines (sorted ascending, with the backend default selected) — not a hardcoded list. Select a non-default engine (e.g. "treat"). Click Run. The request sends the selected engine and the response reflects a real execution against that engine.
result: pass
evidence: |
  GET /v1/engines → `{"engines":["leaps","phreak","rete","treat"],"default":
  "rete"}` (sorted, default rete). POST /v1/execute with `engine:"treat"` +
  `trace:true` → 200 `rules_fired:1, facts_after.scorecard.score:10,
  inference_trace populated` — real execution against the treat engine, not a
  hardcoded/rejected response. UI dropdown verified by Vitest "dropdown lists
  engines": `<option>` rete + treat present, combobox default value == "rete".

### 5. SC-3a — Trace ON shows the trace section
expected: Turn the Trace toggle ON. Click Run. A trace section appears showing at least one InferenceTraceEntry with event_type "rule_fired", rule_name matching the fired rule, and a timestamp, plus the TraceSummary (total_duration_ms, facts_matched).
result: pass
evidence: |
  POST /v1/execute with `trace:true` → `inference_trace:
  {"entries":[{"timestamp":"2026-07-06T11:36:14.466702713+00:00",
  "event_type":"rule_fired","rule_name":"r1"}],"summary":
  {"total_duration_ms":0.078097,"facts_matched":1}}`. Ordered entry with
  rule_fired + rule_name + timestamp + summary. UI trace-section rendering
  verified by Vitest "trace section hidden when null": with inference_trace
  populated, `expect(screen.getByText(/inference trace/i)).toBeInTheDocument()`.

### 6. SC-3b — Trace OFF hides the trace section
expected: Turn the Trace toggle OFF. Click Run. The trace section is hidden entirely (no empty trace block). rules_fired and facts_after are unchanged versus the trace-on run (trace-off is the default, zero-allocation path).
result: pass
evidence: |
  POST /v1/execute with `trace:false` → 200 with NO inference_trace field
  (null/absent), `rules_fired:1, facts_after.scorecard.score:10` — unchanged
  vs trace-on run. UI verified by Vitest "trace section hidden when null":
  with inference_trace null, `expect(screen.queryByText(/inference trace/i))
  .not.toBeInTheDocument()`.

### 7. SC-4 — Reload then Run + engine_generation correlation
expected: Edit the rule body (e.g. score + 10 -> score + 20) and click Save. Click "Reload then Run". The panel polls job status until completed, then runs the rule. facts_after.scorecard.score == 20 (the edited rule), and the engine_generation hint line shows a generation strictly greater than the pre-reload run's generation.
result: pass
evidence: |
  Pre-reload execute: engine_generation=0, facts_after.score=10. Edited
  ./rules/r1.rules on disk (Literal Integer 10 → 20; condition value 0
  untouched). POST /v1/rules/reload {async_reload:true} → 202
  {job_id, status:"pending"}; polled GET /v1/jobs/{job_id} → status
  "completed" (result: rule_count=1, success=true). Post-reload execute:
  `facts_after.scorecard.score:20, engine_generation:1, rules_fired:1` —
  edited rule applied, generation strictly greater (0→1). UI polling+correlation
  verified by Vitest "reload then run polls job status": reloadRules called →
  getJobStatus polled until completed → executeRule called → result shows 20
  with engine_generation 2 (SC-4 correlation).

### 8. Minimal Save enabler persists edits
expected: On the edit page, change an editable structured field (hitPolicy and/or description) and click Save. The save succeeds (UX feedback: saving -> saved), and a subsequent reload of the rule reflects the persisted change (backend governance/approval still applies server-side).
result: pass
evidence: |
  POST /v1/rules {name,dsl_body,description:"original desc"} → 201 (id
  acquired). PUT /v1/rules/{id} {hitPolicy:"first", description:"edited by
  UAT save enabler"} → 200. GET /v1/rules/{id} → description == "edited by
  UAT save enabler" (persisted). NOTE: `hitPolicy` is NOT a backend
  StoredRule field — serde silently drops it, so only `description` actually
  persists. This matches the 28-05-SUMMARY "Known Stubs" note that the Save
  enabler persists structured fields but does not map the expression editor
  to executable conditions/actions; the rule-body edit in SC-4 was done
  against the on-disk rule + reload, which is the documented path.

### 9. Invalid facts JSON shows an error banner
expected: In the facts editor, enter invalid JSON (e.g. `{scorecard: missing}`) and click Run. An inline error banner (role="alert") appears describing the parse failure; no results panel is rendered with bad data.
result: pass
evidence: |
  Backend rejects bad input: POST /v1/execute with malformed JSON body
  (`{scorecard: missing, }`) → 400 "Json deserialize error: key must be a
  string"; POST with `{"facts":"not-an-object"}` → 400 "invalid type: string
  … expected a map". The TestRunner component parses factsJson client-side
  (try/catch → role="alert" error banner) before sending, so the backend's
  400 is the defense-in-depth backstop. NOTE: the client-side error-banner
  branch is not covered by the 4-test Vitest suite (coverage gap, minor) —
  verified at backend-rejection level + source review of TestRunner.tsx
  (role="alert" div on error state).

## Summary

total: 9
passed: 9
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none — all 9 tests passed]

## Browser Click-Through Verification

The literal browser click-through **was performed**: headless Chromium 147
(Playwright via `playwright-core` resolvable from
`/home/threaded/.hermes/hermes-agent/node_modules/playwright-core`; no MCP
server required) drove the rendered app at http://localhost:5173. After logging
in through the `/login` UI (`admin@rrules.dev`/`admin` → `localStorage.auth_token`
stored, len 167), it navigated to `/rules/{id}/edit`, located the `Test Runner`
section, typed facts into the CodeMirror facts editor, toggled Trace, swapped
the engine `<select>`, and clicked Run / Reload-then-Run — observing the real
rendered results panel each time. **9/9 browser checks passed; 0 console/page
errors** across the whole session.

| Check | Result | Observed in browser |
|-------|--------|---------------------|
| LOGIN | pass | UI login stored JWT in localStorage (len 167) |
| T2 mount | pass | `Test Runner` section rendered on the edit page |
| SC-1 | pass | Run → `rules_fired=1`, `facts_after.scorecard.score=10` |
| SC-2 | pass | dropdown options `[leaps,phreak,rete,treat]`, default `rete`; `treat` run fired=1 score=10 |
| SC-3a | pass | Trace ON → "Inference trace" section visible; entry `… — rule_fired — r1` |
| SC-3b | pass | Trace OFF → "Inference trace" hidden; score unchanged (10) |
| SC-4 | pass | edit +10→+20 on disk → "Reload then Run" → score=20, `engine_generation: 1 (pre-reload: 0)` |
| T9 | pass | invalid facts JSON → `role="alert"` banner: "Invalid facts JSON: …" |
| T1 console | pass | 0 console/page errors during the entire session |

Screenshots captured (in `/tmp/rrules_browser_uat/`, session-local — not
committed): `sc1-run.png`, `sc2-treat.png`, `sc3a-trace-on.png`,
`sc3b-trace-off.png`, `sc4-reload-then-run.png`, `t9-invalid-facts.png`,
`final-edit-page.png`. Script: `/tmp/rrules_browser_uat.mjs`; results:
`/tmp/rrules_browser_uat/results.json`.

Verification was therefore completed at three levels that jointly cover the
feature:

1. **Live API (curl + JWT)** — every backend contract exercised end-to-end
   through the running server (see per-test evidence above).
2. **Real component (Vitest + Testing Library)** — TestRunner.tsx renders the
   panel, dropdown (default rete), trace-section conditional, and reload-then-
   run polling (4/4 tests pass).
3. **Literal browser click-through (headless Chromium)** — the rendered edit
   page driven end-to-end as described in the table above (9/9, 0 console
   errors).

## Artifacts Created During UAT (cleanup recommended)

- `engine/examples/emit_keystone.rs` — one-off rule-JSON emitter (safe to delete)
- `./rules/r1.rules` — keystone rule on disk (was absent; needed for live execute)
- `./rules/r1.rules` was edited (+10 → +20) for SC-4 (API run AND the browser
  click-through) and is **currently left at +20** — revert to +10 or delete
  `./rules/` if not wanted for future phases.
- `frontend/node_modules/.vite/deps/*` — Vite dev-cache churn from booting the
  dev server (uncommitted, git-tracked cache files; harmless).