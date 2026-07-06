# Phase 28 — UI Review

**Audited:** 2026-07-06
**Baseline:** `.planning/phases/28-frontend-test-runner/28-UI-SPEC.md` (retroactive contract, derived from as-built)
**Screenshots:** not captured (Vite dev server not running on 5173; port 3000 serves the Rust backend's static SPA build; Playwright not installed locally). Audit is code-only against the as-built TSX. Functional behavior was already verified end-to-end via the headless-Chromium click-through recorded in `28-UAT.md`; this review covers visual/UX quality only.
**Scope:** `frontend/src/components/TestRunner.tsx`, `frontend/src/routes/rules/$id.edit.tsx`, `frontend/src/api/execution.ts`, `frontend/src/api/types.ts`, `frontend/src/global.css`, `frontend/src/components/__tests__/TestRunner.test.tsx`.

**Verdict: FLAG** — No pillar scored 1 (no BLOCKER), but two pillars scored 2 with concrete defects that should be fixed before this surface is considered production-quality: dark-mode color breakage from hardcoded Tailwind palette classes, and missing loading/disabled states on the Run/Reload buttons plus unassociated form labels. The panel is functionally sound and appropriate for a developer-facing IDE, but it does not meet its own token system's contract in dark mode and has real interaction/a11y gaps.

---

## Pillar Scores

| Pillar | Score | Key Finding |
|--------|-------|-------------|
| 1. Copywriting | 3/4 | Verb-based CTAs and clear labels, but no empty-state copy before first Run (results panel is simply absent). |
| 2. Visuals | 2/4 | Run vs "Reload then Run" are visually identical unstyled browser buttons; the primary CTA does not distinguish itself. |
| 3. Color | 2/4 | `bg-gray-50` / `text-gray-500` / `bg-red-100` / `border-red-400` / `text-red-700` / `bg-blue-600` are hardcoded Tailwind palette values that bypass the `--muted` / `--destructive` / `--primary` tokens and break in `.dark` mode. |
| 4. Typography | 3/4 | Within the scoped weight/scale contract; the facts-after `<pre>` uses the sans font (no `font-mono`) for JSON, hurting readability. |
| 5. Spacing | 3/4 | On the 4-px grid with two acknowledged justified exceptions (`space-y-3`, CodeMirror `120px`); TestRunner buttons have no padding class while the Save button uses `px-3 py-1` — inconsistent button treatment. |
| 6. Experience Design | 2/4 | Run/Reload buttons have no disabled/loading state during in-flight mutations (double-fire risk); no reload-in-progress indicator; "Facts (JSON)" and "Engine" `<label>`s are not associated with their controls (no `htmlFor`/`id`); error-banner branch is not covered by the 4-test suite. |

**Overall: 15/24**

---

## Top 3 Priority Fixes

1. **Hardcoded Tailwind palette classes bypass the token system and break dark mode** — In `.dark`, `bg-gray-50` renders a near-white facts-after box on a dark page, and `bg-red-100` renders a light-pink error banner on dark — both are glaring. — Replace with token-backed classes: `bg-gray-50` → `bg-muted`, `text-gray-500` → `text-muted-foreground`, the error banner → `bg-destructive/10 border-destructive/40 text-destructive` (or wire `--destructive`), and the Save CTA's `bg-blue-600` → `bg-primary text-primary-foreground`. This also fixes the spec's documented "accent inconsistency" (conformance note #1).

2. **Run button is not visually the primary CTA and has no in-flight state** — Both Run and "Reload then Run" render as default browser buttons with no `bg-*`, no padding, and no `disabled` binding, so the user cannot tell which is primary and can double-click to fire duplicate mutations; "Reload then Run" polls for ~seconds with no progress indicator. — Promote Run to `bg-primary text-primary-foreground px-3 py-1 rounded disabled:opacity-50` with `disabled={executeMutation.isPending}`; demote "Reload then Run" to an outline/ghost variant (`border border-border`) with `disabled={!!jobId}`; add a small "Reloading…" label or spinner while `jobQuery.data?.status` is `'pending'`/`'running'`.

3. **Form labels are not programmatically associated with their controls** — "Facts (JSON)" (`TestRunner.tsx:157`) and "Engine" (`TestRunner.tsx:170`) are `<label>` elements without `htmlFor`, and their CodeMirror wrapper / `<select>` have no `id`, so screen readers do not announce the label-control relationship; CodeMirror's contenteditable has no accessible name. — Add `htmlFor="facts-json-{ruleId}"` + an `id` on the CodeMirror wrapper (and an `aria-label` on the editor), and `htmlFor="engine-{ruleId}"` + `id="engine-{ruleId}"` on the `<select>`. The trace toggle already does this correctly (`htmlFor={`trace-toggle-${ruleId}`}`) — replicate that pattern.

---

## Detailed Findings

### Pillar 1: Copywriting (3/4)

**PASS-worthy:** CTAs are verb-based and unambiguous — "Run" (`TestRunner.tsx:200`), "Reload then Run" (`:203`), "Save" → "Saving…" (`$id.edit.tsx:84-85`). Section headings ("Test Runner", "Fired", "Not fired", "Facts after", "Inference trace") are clear and developer-appropriate. The parse-error copy "Invalid facts JSON: {msg}" (`TestRunner.tsx:120`) is actionable. "Saved at {ISO timestamp}" (`$id.edit.tsx:87`) is appropriate for a dev tool.

**WARNING — no empty-state copy (conformance note #3):** Before the first Run, the results panel is conditionally absent (`{lastResult && ...}`, `TestRunner.tsx:218`). The user sees the facts editor initialized to `'{}'` (`:48`), the engine dropdown, the trace toggle, and two buttons — but no guidance on what Run will produce. The spec explicitly defers this ("A future polish phase may add helper copy…"), but for a developer landing on the panel for the first time, a one-line placeholder ("No runs yet — click Run to execute this rule.") would materially help. Severity: WARNING.

**WARNING — raw error strings in the banner:** Execute/reload failures surface the raw `Error.message` (or `String(err)`) in the `role="alert"` banner (`TestRunner.tsx:84-86, 147, 208-215`). For a dev tool this is acceptable and arguably desirable (it surfaces backend detail), but it is not user-friendly copy. Acceptable for the audience; flagged for honesty.

**INFO — no destructive-action confirmation:** Save overwrites a governed rule's `hitPolicy` + `description` with no confirm dialog (`$id.edit.tsx:79-86`). The spec records this as D-05 minimal-enabler scope with server-side governance still applying; not a copywriting defect, but the absence of a "Save changes?" confirmation is noted.

### Pillar 2: Visuals (2/4)

**WARNING — primary CTA has no visual distinction (conformance note #1):** Both Run and "Reload then Run" are unstyled `<button type="button">` elements with no `bg-*` or padding class (`TestRunner.tsx:199-204`). They render as the browser's default gray button. The user cannot visually tell which is the primary action. The spec itself flags this: "the primary CTA ('Run') has no accent treatment, so it does not visually distinguish itself from the secondary 'Reload then Run' button." This is the single biggest visual-hierarchy defect in the panel.

**PASS-worthy hierarchy above the panel:** The edit page has a clear H1 (`text-2xl font-bold`) → H2 sections (`font-semibold`) → H3 results subsections (`font-semibold`) ladder (`$id.edit.tsx:50-93`, `TestRunner.tsx:153, 221, 228, 237, 246`). The facts_after `<pre>` uses `bg-gray-50` to differentiate it from surrounding text (`TestRunner.tsx:238`). The CodeMirror `oneDark` theme gives the facts editor a distinct code-editor appearance. Icon usage is paired with text labels (Play, RefreshCw at `h-4 w-4`), no icon-only buttons.

**INFO — focal point:** The TestRunner is one of five stacked sections on the edit page (Expression, Hit Policy, Decision Table, Save, Test Runner). Within the panel, there is no strong focal point because neither button is accent-treated. Fixing priority fix #2 resolves this.

### Pillar 3: Color (2/4)

**WARNING — hardcoded palette classes bypass the token system and break dark mode:**
- `bg-gray-50` on the facts_after `<pre>` (`TestRunner.tsx:238`) — near-white background that does not respond to `.dark`. In dark mode this renders as a bright white box on a `#0a0a0c` page. The token system provides `--muted` / `--secondary` (`240 4.8% 95.9%` light → `240 3.7% 15.9%` dark) for exactly this purpose.
- `text-gray-500` on the subtitle, `engine_generation` hint, and `savedAt` (`$id.edit.tsx:53, 87`; `TestRunner.tsx:265`) — same problem; should be `text-muted-foreground` (token `--muted-foreground`).
- `bg-red-100 border border-red-400 text-red-700` on the error banner (`TestRunner.tsx:211`) and `text-red-600` on `saveError` (`$id.edit.tsx:89`) — hardcoded red scale that does not track `--destructive` (`0 72.22% 50.59%` light → `0 62.8% 30.6%` dark). In dark mode the light-pink banner will be jarring.
- `bg-blue-600` on the Save button (`$id.edit.tsx:83`) — a one-off Tailwind palette pick, NOT the `--primary` token (which is `240 5.9% 10%` = near-black). The spec calls this out explicitly (conformance note #1, Color section note).

**WARNING — accent distribution:** The TestRunner panel itself contains zero accent color. The only accent on the entire edit page is the Save button's `bg-blue-600`. The 60/30/10 split is approximately upheld at the page level (dominant background, secondary grays, ~10% blue accent on Save), but the TestRunner sub-panel has no accent to draw the eye to its primary action.

**PASS-worthy:** No hardcoded hex/rgb literals in scope (grep confirmed none). Contrast of default `text-foreground` on `bg-background` is fine. The `text-gray-500` (#6b7280) on white is borderline AA for 12px text (~4.48:1) — minor, and only affects meta/hint copy.

### Pillar 4: Typography (3/4)

**PASS-worthy scale/weight adherence:** 4 distinct sizes in scope (`text-xs`, `text-sm`, default 16px, `text-2xl`) — within the spec's "<4 sizes" threshold. 4 weights (regular, `font-medium`, `font-semibold`, `font-bold`) — the spec's canonical contract is regular + semibold, with `font-medium` scoped to form labels and `font-bold` to the page H1 only; the as-built stays within these scoped exceptions (verified: `font-medium` only on "Facts (JSON)" and "Engine" labels, `font-bold` only on the H1). Section headings (`font-semibold`, no `text-*` → 16px) create a clear ladder against the 24px H1.

**WARNING — facts_after `<pre>` uses the sans font (conformance note #7):** `TestRunner.tsx:238` renders `JSON.stringify(facts_after, null, 2)` in a `<pre>` with no `font-mono` class. The JSON is indentation-formatted but in `Inter, system-ui, sans-serif`. For a developer tool displaying structured facts, monospace is the convention and materially aids scanning of keys/values. The spec marks this as "acceptable for the panel size; future polish," but it is a real readability gap for the panel's primary content.

**INFO — CodeMirror oneDark:** The facts editor uses the `oneDark` theme (`TestRunner.tsx:161`), which carries its own monospace font — so the input side is monospace while the output side (facts_after) is sans. This input/output asymmetry is mildly inconsistent.

### Pillar 5: Spacing (3/4)

**PASS-worthy scale adherence:** All spacing values are on the 4-px grid except the two spec-acknowledged exceptions: `space-y-3` (12px, results-panel inner stack, `TestRunner.tsx:219`) and CodeMirror `height="120px"` (off-grid px required by the widget API, `:160`). Vertical rhythm is consistent within each container: `space-y-4` for the TestRunner root (`:152`), `space-y-1` for label↔control pairs (`:156, 169`), `gap-2` for the button and trace rows (`:185, 198`), `space-y-6` / `space-y-2` for the edit page (`$id.edit.tsx:50, 56, 61, 66, 77`).

**WARNING — inconsistent button padding:** The Save button has `px-3 py-1` (`$id.edit.tsx:83`); the Run and "Reload then Run" buttons have no padding class at all (`TestRunner.tsx:199-204`) and fall back to the browser default. On the same page, two different button treatments. This is both a spacing and a visual defect (see Pillar 2). Fixing priority fix #2 resolves both.

**INFO — three section rhythms on one page:** `space-y-6` (page top), `space-y-2` (edit-page inner sections), `space-y-4` (TestRunner inner). Each is appropriate to its density, and the nesting is logical, so this is informational rather than a defect.

### Pillar 6: Experience Design (2/4)

**WARNING — no loading/disabled state on Run / Reload-then-Run:** `executeMutation` exposes `isPending` but it is not bound to either button's `disabled` attribute (`TestRunner.tsx:199-204`). A user can click Run twice and fire two concurrent `POST /v1/execute` requests. "Reload then Run" polls `getJobStatus` at 500ms (`:96-101`) for potentially several seconds with no visual indicator that a reload is in progress — the panel looks frozen. The Save button does this correctly (`disabled={saving}` + "Saving…" + `disabled:opacity-50`, `$id.edit.tsx:82-85`); the TestRunner buttons should follow the same pattern.

**WARNING — form labels not associated with controls (a11y):** "Facts (JSON)" (`TestRunner.tsx:157`) and "Engine" (`:170`) are `<label>` elements without `htmlFor`, and their controls (the CodeMirror wrapper and the `<select>`) have no `id`. Screen readers will not announce the label-control relationship. The trace toggle does this correctly (`htmlFor={`trace-toggle-${ruleId}`}` + matching `id`, `:187, 192`) — that pattern should be replicated. CodeMirror's contenteditable additionally needs an `aria-label` since `<label htmlFor>` does not associate to a contenteditable div.

**WARNING — error-banner branch untested:** The 4-test Vitest suite (`TestRunner.test.tsx`) covers: fired/facts_after render, engine dropdown, trace-hidden-when-null, and reload-then-run polling. No test exercises the `role="alert"` error banner — neither the JSON-parse error path (`TestRunner.tsx:120`) nor the `executeMutation.onError` / `reloadRules.catch` paths (`:84-86, 147`). This is an interaction-critical state (the only user-visible failure surface) and is uncovered. The user's brief explicitly flagged this; confirm it as a real coverage gap.

**PASS-worthy:** Error states use `role="alert"` for both the TestRunner banner (`TestRunner.tsx:210`) and the saveError paragraph (`$id.edit.tsx:89`) — correct for screen-reader announcement. The trace toggle is opt-in with `trace: traceEnabled || undefined` so the off state omits the key entirely (SC-3 latency neutrality, `:128`). Results render directly from the `ExecuteResponse` via `useMutation.onSuccess` with no polling (Pitfall 8 enforced, `:80-82`). The trace section is conditionally hidden when `inference_trace` is null (`:244`). The SC-4 correlation hint "engine_generation: {n} (pre-reload: {m})" is present (`:265-269`).

**INFO — no confirmation on Save:** See Pillar 1. D-05 minimal-enabler scope; server-side governance applies. Not an experience-design defect per se, but noted for completeness.

---

## Registry Safety

`components.json` not present (NO_SHADCN). The spec's Registry Safety table records `Tool: none` — no shadcn registry CLI, no third-party blocks. No `npx shadcn view` / `npx shadcn diff` vetting required. Registry audit: 0 third-party blocks checked, no flags.

---

## Files Audited

- `frontend/src/components/TestRunner.tsx` (277 lines) — the TestRunner panel
- `frontend/src/routes/rules/$id.edit.tsx` (102 lines) — mount site + Save enabler
- `frontend/src/api/execution.ts` (77 lines) — execution API client
- `frontend/src/api/types.ts` (Phase-28 block, `:112-206`) — ExecuteResponse / engine / trace types
- `frontend/src/global.css` (61 lines) — shadcn-style HSL token definitions (`:root` + `.dark`)
- `frontend/src/components/__tests__/TestRunner.test.tsx` (222 lines) — 4 GREEN component tests
- `.planning/phases/28-frontend-test-runner/28-UI-SPEC.md` — design contract (baseline)

---

## Severity Summary

| Severity | Count | Items |
|----------|-------|-------|
| BLOCKER | 0 | — |
| WARNING | 9 | empty-state gap; raw error strings; Run/Reload visual indistinction; hardcoded palette breaks dark mode; accent inconsistency; facts_after sans font; inconsistent button padding; missing loading/disabled on Run/Reload + no reload indicator; unassociated form labels; error-banner branch untested |
| INFO | 4 | no destructive-action confirmation; input/output font asymmetry; three section rhythms on one page; borderline AA contrast on `text-gray-500` meta copy |

**Recommendation: FLAG — fix the Top 3 before promoting this surface beyond the developer-IDE context.** The panel is functionally correct and appropriate for its audience, but it does not meet its own token system's dark-mode contract and has real interaction and a11y gaps that a half-day of polish (token wiring + button states + label associations) would close.