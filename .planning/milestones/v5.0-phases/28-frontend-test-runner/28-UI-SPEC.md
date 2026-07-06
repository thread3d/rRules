---
phase: 28
slug: frontend-test-runner
status: draft
shadcn_initialized: false
preset: none
created: 2026-07-06
---

# Phase 28 — UI Design Contract

> **RETROACTIVE** contract, derived from the as-built `frontend/src/components/TestRunner.tsx` (277 lines, merged to master) and its mount site `frontend/src/routes/rules/$id.edit.tsx`. CONTEXT.md decisions D-01..D-10 + L-01..L-09 are LOCKED and treated as the upstream source of truth; the as-built code is the visual source of truth. No user questions were asked — every field below is pre-populated from the implementation or the locked decisions.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none (manual shadcn-style token system — no `components.json`, no registry) |
| Preset | not applicable |
| Component library | none (hand-rolled React components on Tailwind utility classes; `class-variance-authority` + `clsx` + `tailwind-merge` available but unused by TestRunner) |
| Icon library | `lucide-react` ^0.453.0 — `Play`, `RefreshCw` imported in TestRunner; `h-4 w-4` (16px) inline icons |
| Font | `Inter, system-ui, sans-serif` (Tailwind `font-sans`, `tailwind.config.ts:11`) |
| Styling | Tailwind CSS 3.4.19 + `tailwindcss-animate`; dark mode via `.dark` class; shadcn-style HSL CSS vars in `frontend/src/global.css` |
| Test stack | Vitest ^2.0.0 + `@testing-library/react` ^16.0.0 + `@testing-library/jest-dom` ^6.9.1 + `jsdom` ^25.0.0 — 4 component tests GREEN (`TestRunner.test.tsx`) |

**shadcn gate result:** No `components.json` found. The project already carries shadcn-compatible design tokens in `global.css` (`--background`, `--foreground`, `--card`, `--primary`, `--secondary`, `--accent`, `--destructive`, `--border`, `--radius`) and the shadcn utility deps (`cva`, `clsx`, `tailwind-merge`), so the token system is effectively shadcn-aligned without the registry CLI. No initialization performed; `Tool: none`. Registry safety gate: not applicable.

---

## Spacing Scale

Tailwind default spacing scale (1 unit = 4px). The as-built TestRunner + edit page use the following values — all multiples of 4 except `space-y-3` (12px), which is a Tailwind default used for the results-panel inner stack.

| Token | Value | Usage (as-built) |
|-------|-------|------------------|
| xs | 4px | `space-y-1` label↔control pair; `mr-1` icon↔label gap; `space-y-2` edit-page inner section rhythm |
| sm | 8px | `gap-2` button row + trace-toggle row; `px-2` select horizontal padding; `py-1` select vertical padding; `p-2` facts_after `<pre>` padding |
| md | 12px | `space-y-3` results-panel inner stack (Fired / Not-fired / Facts after / Trace); `px-3` error banner horizontal padding; `py-2` error banner vertical padding |
| lg | 16px | `space-y-4` TestRunner root section vertical rhythm; `h-4 w-4` lucide icon size |
| xl | 24px | `p-6` edit-page container padding; `space-y-6` edit-page top-level stack |
| 2xl | 32px | (available, unused in TestRunner) |
| 3xl | 48px | (available, unused) |
| 4xl | 64px | (available, unused) |

**Exceptions:**
- `CodeMirror height="120px"` — explicit pixel height for the facts JSON editor (30px = ~3 lines at 1.5 line-height × ~13px font). Not on the 4-px grid; justified by the CodeMirror widget API requiring a px height.
- `space-y-3` (12px) — used only inside the results panel to tighten the stacked Fired/Not-fired/Facts-after/Trace sections. 12px is a Tailwind default but not a multiple of 8; flagged as a minor deviation from the strict 8-pt scale. Acceptable for the dense results panel.

---

## Typography

| Role | Size | Weight | Line Height | Usage (as-built) |
|------|------|--------|-------------|------------------|
| Body | 16px | 400 (regular) | 1.5 (Tailwind `leading-normal` default) | Default text — button labels ("Run", "Reload then Run"), trace toggle label, select option text, run-result count text, not-fired list items |
| Label | 14px | 500 (`font-medium`) | 1.5 | Form labels: "Facts (JSON)", "Engine" (`text-sm font-medium`) |
| Section heading | 14px | 600 (`font-semibold`) | 1.5 | TestRunner inner section headings: h3 "Fired" / "Not fired" / "Facts after" / "Inference trace" (`font-semibold`, inherits default size — no `text-*` class) |
| Page heading | 24px (`text-2xl`) | 700 (`font-bold`) | 1.2 (Tailwind heading default) | Edit-page H1 "Edit Rule {id}" |
| Trace entry / meta | 12px (`text-xs`) | 400 | 1.5 | `engine_generation` correlation hint; `savedAt` timestamp; `saveError` inline error |
| Trace list | 14px (`text-sm`) | 400 | 1.5 | Trace entry rows (`timestamp — event_type — rule_name`); trace summary "Duration / Matched" |
| Facts after block | 14px (`text-sm`) | 400 | 1.5 (monospace via `<pre>`) | `JSON.stringify(facts_after, null, 2)` in a `<pre>` |

**Canonical weight contract: regular (400) + semibold (600).** The as-built uses two additional weights:
- `font-medium` (500) — form labels only ("Facts (JSON)", "Engine")
- `font-bold` (700) — the edit-page H1 only

These are established shadcn/Tailwind defaults and are flagged as scoped exceptions, not a general license to introduce new weights. New Phase-28-scope components MUST stay within regular + semibold except for page-level H1s.

**Font family:** `Inter, system-ui, sans-serif` for all text. The facts_after `<pre>` inherits the same sans font (no monospace class applied) — JSON is formatted via `JSON.stringify(..., null, 2)` indentation only.

---

## Color

HSL token values from `frontend/src/global.css` (`:root`, light mode). The TestRunner uses Tailwind utility classes (`bg-red-100`, `border-red-400`, `text-red-700`, `bg-gray-50`, `text-gray-500`) plus the token-backed `bg-background`/`text-foreground`/`border-border` defaults.

| Role | Value (light) | Value (dark) | Usage |
|------|---------------|--------------|-------|
| Dominant (60%) | `--background: 0 0% 100%` → `#ffffff` | `240 10% 3.9%` → `#0a0a0c` | Page background; card surfaces; TestRunner section background (inherits `bg-background` from `body`) |
| Secondary (30%) | `--secondary / --muted: 240 4.8% 95.9%` → `#f4f4f5` | `240 3.7% 15.9%` → `#27272a` | `bg-gray-50` facts_after `<pre>` background; `text-gray-500` muted subtitle ("Editing existing rule definition.") + `engine_generation` hint + `savedAt` |
| Accent (10%) | `bg-blue-600` → `#2563eb` (one-off, NOT from the token system) | (same) | **Reserved for the edit-page Save CTA only** (`$id.edit.tsx:83`). NOT used inside TestRunner. |
| Destructive | `--destructive: 0 72.22% 50.59%` → `#dc2626` (token); as-built uses Tailwind red scale `bg-red-100` / `border-red-400` / `text-red-700` / `text-red-600` | `0 62.8% 30.6%` | Error banner (`role="alert"`, `bg-red-100 border-red-400 text-red-700`); `saveError` (`text-red-600`) |
| Border | `--border: 240 5.9% 90%` → `#e4e4e7` | `240 3.7% 15.9%` | `border` on the engine `<select>`; `body *` default border color |
| Primary text | `--foreground: 240 10% 3.9%` → `#0a0a0c` | `0 0% 98%` | All default body text |

**Accent reserved for:** the **Save CTA button** in `$id.edit.tsx` only (`bg-blue-600 text-white`). The TestRunner itself does NOT use the accent color — its Run / Reload-then-Run buttons are unstyled (default browser button appearance, no `bg-*` class). This is an as-built gap: the primary CTA ("Run") has no accent treatment, so it does not visually distinguish itself from the secondary "Reload then Run" button. Flagged for the auditor — the contract records the as-built state; a future polish phase should promote "Run" to `bg-primary text-primary-foreground` (or `bg-blue-600`) and demote "Reload then Run" to an outline/ghost variant.

**Note:** `bg-blue-600` is a one-off utility class, NOT wired to the `--primary` token. The token system's `--primary: 240 5.9% 10%` is near-black, not blue. The Save button's blue is a direct Tailwind palette pick. This is an as-built inconsistency against the shadcn token system.

---

## Copywriting Contract

All copy extracted verbatim from the as-built code.

| Element | Copy | Source |
|---------|------|--------|
| Primary CTA | **"Run"** (verb; `Play` icon prefix) | `TestRunner.tsx:200` |
| Secondary CTA | **"Reload then Run"** (verb phrase; `RefreshCw` icon prefix) | `TestRunner.tsx:203` |
| Save CTA (edit-page enabler) | **"Save"** → **"Saving…"** (disabled state) | `$id.edit.tsx:84-85` |
| Section heading | "Test Runner" | `TestRunner.tsx:153` |
| Facts editor label | "Facts (JSON)" | `TestRunner.tsx:157` |
| Engine selector label | "Engine" | `TestRunner.tsx:170` |
| Trace toggle label | "Trace" | `TestRunner.tsx:193` |
| Results: fired heading | "Fired" + body "Count: {n}" | `TestRunner.tsx:221-224` |
| Results: not-fired heading | "Not fired" + `<ul>` of rule ids | `TestRunner.tsx:228-234` |
| Results: facts-after heading | "Facts after" + `<pre>` JSON | `TestRunner.tsx:237-241` |
| Results: trace heading | "Inference trace" + `<ul>` of "{timestamp} — {event_type} — {rule_name}" + "Duration: {ms}ms — Matched: {n}" | `TestRunner.tsx:246-260` |
| Generation correlation hint | "engine_generation: {n}" + optional " (pre-reload: {n})" | `TestRunner.tsx:265-269` |
| Empty state (no run yet) | **No empty-state copy** — the results panel is conditionally rendered only when `lastResult` is set. Before the first Run, the panel is absent (not shown with placeholder copy). The facts editor initializes to `'{}'`. | `TestRunner.tsx:218` (`{lastResult && ...}`), `:48` (`useState('{}')`) |
| Error state — invalid facts JSON | "Invalid facts JSON: {Error.message}" | `TestRunner.tsx:120` |
| Error state — execute / reload failure | The raw `Error.message` (or `String(err)`) rendered in the `role="alert"` banner: `bg-red-100 border border-red-400 text-red-700` | `TestRunner.tsx:84-86, 147, 208-215` |
| Error state — save failure | The raw `Error.message` (or `String(err)`) in `text-red-600` below the Save button | `$id.edit.tsx:43, 88-91` |
| Save success | "Saved at {ISO timestamp}" | `$id.edit.tsx:87` |
| Page heading | "Edit Rule {id}" | `$id.edit.tsx:52` |
| Page subtitle | "Editing existing rule definition." | `$id.edit.tsx:53` |

**Destructive actions:** NONE in Phase 28 scope. The Save button (`PUT /v1/rules/{id}`) overwrites the rule's `hitPolicy` + `description` but carries **no confirmation dialog** — it is a minimal D-05 enabler, single-click. "Reload then Run" triggers an async reload from disk (non-destructive — it re-reads authored rules). Neither action is gated by a confirm step in the as-built UI. Flagged for the auditor: a production Save that overwrites a governed rule would normally route through the maker-checker approval flow (Phase 18), but Phase 28's D-05 explicitly scopes this to a minimal enabler; server-side governance still applies.

**Empty-state gap:** The TestRunner has no explicit empty-state heading/body copy — before the first Run, only the input controls (facts editor, dropdown, trace toggle, buttons) are visible. The contract records this as the as-built state. A future polish phase may add helper copy such as heading "No runs yet" / body "Click Run to execute this rule and see fired rules, results, and facts." — deferred.

---

## Interaction Contract

Derived from the as-built `TestRunner.tsx` + the 4 GREEN component tests.

| Interaction | Trigger | Behavior | Source |
|-------------|---------|----------|--------|
| Run | Click "Run" button (`getByRole('button', { name: /^run$/i })`) | `handleRun` → parse `factsJson` as JSON → `executeMutation.mutate({ facts, engine, trace, rule_filter: { name_pattern: ruleId } })` → `onSuccess` sets `lastResult` (NO polling — Pitfall 8). On JSON parse error: set error banner, no mutation. | `TestRunner.tsx:78-87, 115-137` |
| Reload then Run | Click "Reload then Run" button | `handleReloadThenRun` → `reloadRules(true)` → set `jobId` → `useQuery(['job', jobId])` polls `getJobStatus` at 500ms `refetchInterval` until `status === 'completed'` or `'failed'` (the ONE legitimate polling path — Pitfall 8 carve-out) → on `completed`, capture `preReloadGeneration = lastResult?.engine_generation`, call `runExecute()`, clear `jobId`. | `TestRunner.tsx:92-112, 139-149` |
| Engine select | Change `<select>` | `setSelectedEngine(e.target.value)`; sent as `ExecuteRequest.engine` on next Run. Initial value set from `enginesQuery.data.default` via `useEffect` once loaded. Omit when `undefined` → backend resolves to `RRULES_DEFAULT_ENGINE` (L-07 fallback). | `TestRunner.tsx:63-74, 169-182, 126` |
| Trace toggle | Click checkbox | `setTraceEnabled(e.target.checked)`; when ON, `trace: true` sent on Run; when OFF, `trace: undefined` (omitted) so backend skips trace work (SC-3 latency neutrality). Off by default. | `TestRunner.tsx:52, 128, 184-195` |
| Facts edit | Type in CodeMirror | `setFactsJson(val)`; parsed as JSON on Run. Initial value `'{}'`. | `TestRunner.tsx:48, 158-166` |
| Save (edit page) | Click "Save" button | `handleSave` → `updateRule(id, { hitPolicy, description })` → `PUT /v1/rules/{id}`; disabled while `saving`; shows "Saved at {ts}" or inline error. | `$id.edit.tsx:33-47, 77-93` |
| Results render | `useMutation onSuccess` | Stacked sections render directly from `ExecuteResponse` — Fired (count), Not-fired (`<ul>`), Facts after (`<pre>` JSON), Inference trace (only if `inference_trace` non-null), engine_generation hint. | `TestRunner.tsx:80-82, 218-272` |

**Pitfall 8 enforcement:** The results panel is driven exclusively from the `ExecuteResponse` returned by `executeRule`'s `useMutation.onSuccess`. There is NO `refetchInterval` on any query other than `['job', jobId]`. Polling is reserved for `getJobStatus` during the async reload branch. Verified in code (`TestRunner.tsx:96-101`) and pinned by the "reload then run polls job status" component test.

**SC-3 enforcement:** `trace: traceEnabled || undefined` — when the toggle is off, the `trace` key is omitted from the request body entirely (not `false`), so the backend's `#[serde(default)] pub trace: Option<bool>` resolves to `None` → `unwrap_or(false)` → evaluator skips all trace allocation. The trace section in the results panel is conditionally rendered (`{lastResult.inference_trace && ...}`) — hidden entirely when `null`. Pinned by the "trace section hidden when null" component test.

**SC-4 correlation:** After "Reload then Run" completes, the panel shows "engine_generation: {n} (pre-reload: {m})" when a prior run existed, so the user can verify the run executed against the refreshed ruleset.

---

## Component Inventory

| Component | File | Props | Lines | Status |
|-----------|------|-------|-------|--------|
| `TestRunner` | `frontend/src/components/TestRunner.tsx` | `{ ruleId: string }` | 277 | as-built, mounted |
| `EditRulePage` (mount site) | `frontend/src/routes/rules/$id.edit.tsx` | — (route component) | 102 | as-built; mounts `<TestRunner ruleId={id} />` at line 96 |
| `execution.ts` API client | `frontend/src/api/execution.ts` | 6 functions: `executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines`, `reloadRules` | 77 | as-built |
| Execute types | `frontend/src/api/types.ts` | `ExecuteRequest`, `ExecuteResponse`, `InferenceTrace`, `InferenceTraceEntry`, `TraceSummary`, `EnginesResponse`, `ReloadJob`, `BatchExecuteRequest`, `BatchExecuteResponse`, `BatchItemResult` | 84 (Phase-28 block) | as-built, snake_case mirroring backend serde |
| `TestRunner.test.tsx` | `frontend/src/components/__tests__/TestRunner.test.tsx` | 4 tests | 222 | GREEN — `renders fired rules and facts_after`, `dropdown lists engines`, `trace section hidden when null`, `reload then run polls job status` |

**Reused assets:** `apiClient` (`frontend/src/api/client.ts:11`), `@uiw/react-codemirror` + `@codemirror/lang-javascript` (`javascript({ json: true })` per Critical Finding 7 — NOT `@codemirror/lang-json`), `@codemirror/theme-one-dark` (`oneDark` theme on the facts editor), `lucide-react` (`Play`, `RefreshCw`), `updateRule` from `@/api/rules` (D-05 Save enabler).

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| shadcn official | none — no `components.json`, no registry CLI in use | not applicable |
| third-party | none | not applicable |

**No `npx shadcn` invocations, no third-party registry blocks, no `shadcn view` vetting required.** The design system is a manual token setup in `global.css` + Tailwind utilities. All component code is hand-rolled in `frontend/src/components/`.

---

## As-Built Conformance Notes (for the auditor)

These are observed deviations from the template's idealized contract, recorded honestly so `gsd-ui-auditor` can compare against the as-built rather than against an aspirational spec:

1. **Accent inconsistency.** The Save button uses `bg-blue-600` (Tailwind palette), not the `--primary` token (`240 5.9% 10%` = near-black). The TestRunner's Run / Reload-then-Run buttons have no accent treatment at all (default browser button). The contract records this as as-built; a future polish phase should unify on `bg-primary text-primary-foreground` for primary CTAs.
2. **4 weights, not 2.** The as-built uses regular (400), medium (500), semibold (600), bold (700). The canonical contract is regular + semibold; medium is scoped to form labels, bold to the page H1 only.
3. **No empty-state copy.** Before the first Run, the results panel is absent (no placeholder heading/body). Deferred.
4. **No destructive-action confirmation.** The Save button overwrites a governed rule with no confirm dialog. D-05 explicitly scopes this to a minimal enabler; server-side governance applies. Flagged.
5. **`space-y-3` (12px)** is off the strict 8-pt grid but is a Tailwind default used for the results-panel inner stack.
6. **CodeMirror height `120px`** is an off-grid pixel value required by the widget API.
7. **Facts after `<pre>`** uses the sans font (no `font-mono` class) — JSON is formatted by `JSON.stringify(..., null, 2)` indentation only. Acceptable for the panel size; a monospace treatment is a future polish.

---

## Checker Sign-Off

- [ ] Dimension 1 Copywriting: PASS
- [ ] Dimension 2 Visuals: PASS
- [ ] Dimension 3 Color: PASS
- [ ] Dimension 4 Typography: PASS
- [ ] Dimension 5 Spacing: PASS
- [ ] Dimension 6 Registry Safety: PASS

**Approval:** pending