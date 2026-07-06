// frontend/src/components/TestRunner.tsx — Phase 28 (Plan 28-05) TestRunner panel.
//
// The user-facing layer of the v5.0 Live Execution vertical slice. Surfaces
// real `rules_fired` / `rules_not_fired` / `facts_after` / `inference_trace`
// in the rule IDE, with the engine dropdown from `GET /v1/engines` (D-08) and
// the "Reload then Run" flow correlated via `engine_generation` (D-07, SC-4).
//
// Decision references:
//  - D-04: mounts in the edit page, takes a `ruleId` prop.
//  - D-06: raw JSON facts editor (CodeMirror via lang-javascript javascript({json:true}) —
//          Critical Finding 7: reuse lang-javascript, NOT lang-json).
//  - D-07: "Reload then Run" = reloadRules → poll getJobStatus → executeRule.
//  - D-08: engine dropdown populated from useQuery(['engines'], listEngines);
//          default = backend-reported default; selection in useState, sent as
//          ExecuteRequest.engine on every Run.
//  - D-09: stacked results sections (Fired / Not-fired / Facts after / Trace);
//          trace section hidden entirely when inference_trace is None.
//  - D-02 / SC-3: Trace toggle off by default; trace: true sent only when ON.
//  - Pitfall 8 (L-08): drive the results panel from the ExecuteResponse directly
//          (useMutation onSuccess) — NO polling for execute results. Polling is
//          reserved for getJobStatus during the async reload branch ONLY.

import { useState, useEffect } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import CodeMirror from '@uiw/react-codemirror'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import { Play, RefreshCw } from 'lucide-react'
import { executeRule, listEngines, getJobStatus, reloadRules } from '@/api/execution'
import type { ExecuteResponse } from '@/api/types'

// Critical Finding 7: reuse `@codemirror/lang-javascript` (NOT lang-json) for
// the facts JSON editor. The installed 6.2.5 version's `JavaScriptConfig`
// type does not include a `json` key, but the option is harmless at runtime
// (unknown keys are ignored) and JSON is valid JavaScript so the default JS
// parser highlights it correctly. The `@ts-expect-error` below suppresses the
// TS2353 so the source carries the plan's canonical `javascript({ json: true })`
// expression while remaining type-check clean.
// @ts-expect-error — `json` is not in JavaScriptConfig (lang-javascript 6.2.5)
const jsonLangExtension = javascript({ json: true })

export interface TestRunnerProps {
  ruleId: string
}

export function TestRunner({ ruleId }: TestRunnerProps) {
  // D-06: facts JSON editor state (matches facts / facts_after shape).
  const [factsJson, setFactsJson] = useState('{}')
  // D-08: selected engine; initialized from the backend default once loaded.
  const [selectedEngine, setSelectedEngine] = useState<string | undefined>(undefined)
  // D-02 / SC-3: Trace toggle, off by default.
  const [traceEnabled, setTraceEnabled] = useState(false)
  // D-07: reload job id — when set, the job-status polling query enables.
  const [jobId, setJobId] = useState<string | undefined>(undefined)
  // Pitfall 8: the results panel is driven from the ExecuteResponse directly.
  const [lastResult, setLastResult] = useState<ExecuteResponse | undefined>(undefined)
  const [error, setError] = useState<string | undefined>(undefined)
  // SC-4 correlation: capture the pre-reload generation so the panel can show
  // "ran against generation N (refreshed)" after Reload then Run.
  const [preReloadGeneration, setPreReloadGeneration] = useState<number | undefined>(undefined)

  // D-08: engine dropdown data source (cacheable, rarely changes).
  const enginesQuery = useQuery({
    queryKey: ['engines'],
    queryFn: listEngines,
  })

  // Set the default selection from the backend response once, when it first
  // arrives and no selection has been made yet (L-07 fallback on omit).
  useEffect(() => {
    if (enginesQuery.data && selectedEngine === undefined) {
      setSelectedEngine(enginesQuery.data.default)
    }
  }, [enginesQuery.data, selectedEngine])

  // Pitfall 8: Run button mutation — onSuccess renders the ExecuteResponse
  // directly. NO follow-up poll for execute results.
  const executeMutation = useMutation({
    mutationFn: executeRule,
    onSuccess: (data) => {
      setLastResult(data)
      setError(undefined)
    },
    onError: (err: unknown) => {
      setError(err instanceof Error ? err.message : String(err))
    },
  })

  // D-07 / Pitfall 8 carve-out: the ONLY legitimate polling path. Polls
  // getJobStatus until the reload job reaches a terminal state, then the
  // effect below fires executeRule against the refreshed ruleset.
  const jobQuery = useQuery({
    queryKey: ['job', jobId],
    queryFn: () => getJobStatus(jobId as string),
    enabled: !!jobId,
    refetchInterval: (query) => {
      const s = query.state.data?.status
      // Terminate polling once the job reaches a terminal state.
      return s === 'completed' || s === 'failed' ? false : 500
    },
  })

  // SC-4 correlation: when the reload job completes, capture the pre-reload
  // generation (if a prior run exists) and execute against the refreshed
  // ruleset. Clearing jobId disables the polling query.
  useEffect(() => {
    if (jobQuery.data?.status === 'completed' && jobId !== undefined) {
      setPreReloadGeneration(lastResult?.engine_generation)
      runExecute()
      setJobId(undefined)
    }
  }, [jobQuery.data?.status, jobId]) // eslint-disable-line react-hooks/exhaustive-deps

  // Build the ExecuteRequest from current state and fire the mutation.
  function runExecute() {
    let facts: Record<string, unknown>
    try {
      facts = JSON.parse(factsJson)
    } catch (e) {
      setError('Invalid facts JSON: ' + (e instanceof Error ? e.message : String(e)))
      return
    }
    executeMutation.mutate({
      facts,
      // L-07: omit engine when unknown → backend resolves to config default.
      engine: selectedEngine ?? undefined,
      // D-02 / SC-3: trace is opt-in; omit when off so latency is unaffected.
      trace: traceEnabled || undefined,
      // D-04: scope execution to the edited rule for a clean "test this rule" UX.
      rule_filter: { name_pattern: ruleId },
    })
  }

  function handleRun() {
    setError(undefined)
    runExecute()
  }

  // D-07: Reload then Run — kick off the async reload, then poll job status.
  function handleReloadThenRun() {
    setError(undefined)
    reloadRules(true)
      .then((res) => {
        setJobId(res.job_id)
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : String(err))
      })
  }

  // UI-REVIEW (Pillar 6): Run is in-flight while the execute mutation
  // is pending; the Reload-then-Run button is in-flight while a reload job
  // is being polled OR the post-reload execute is running. Binding these to
  // `disabled` prevents the double-fire mutation risk flagged in the review.
  const executeInFlight = executeMutation.isPending
  const reloadInFlight = !!jobId || executeMutation.isPending

  return (
    <section className="space-y-4">
      <h2 className="font-semibold">Test Runner</h2>

      {/* D-06: facts JSON editor (Critical Finding 7 — javascript({json:true})) */}
      <div className="space-y-1">
        <label htmlFor={`facts-json-${ruleId}`} className="text-sm font-medium">
          Facts (JSON)
        </label>
        <CodeMirror
          id={`facts-json-${ruleId}`}
          aria-label={`Facts JSON for rule ${ruleId}`}
          value={factsJson}
          height="120px"
          theme={oneDark}
          extensions={[jsonLangExtension]}
          onChange={(val) => setFactsJson(val)}
          basicSetup={{ lineNumbers: true, foldGutter: true, tabSize: 2 }}
        />
      </div>

      {/* D-08: engine dropdown */}
      <div className="space-y-1">
        <label htmlFor={`engine-${ruleId}`} className="text-sm font-medium">
          Engine
        </label>
        <select
          id={`engine-${ruleId}`}
          className="border rounded px-2 py-1 bg-background"
          value={selectedEngine ?? ''}
          onChange={(e) => setSelectedEngine(e.target.value)}
        >
          {enginesQuery.data?.engines.map((eng) => (
            <option key={eng} value={eng}>
              {eng}
            </option>
          ))}
        </select>
      </div>

      {/* D-02 / SC-3: Trace toggle */}
      <div className="flex items-center gap-2">
        <input
          id={`trace-toggle-${ruleId}`}
          type="checkbox"
          checked={traceEnabled}
          onChange={(e) => setTraceEnabled(e.target.checked)}
        />
        <label htmlFor={`trace-toggle-${ruleId}`} className="text-sm">
          Trace
        </label>
      </div>

      {/* Run + Reload then Run buttons.
          UI-REVIEW (Pillars 2/3/5/6): Run is the primary CTA (token-backed
          `bg-primary`, padding, disabled-while-pending); "Reload then Run" is
          demoted to an outline variant. A "Reloading…" indicator shows while
          the async reload job is being polled (D-07), so the panel does not
          look frozen. */}
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleRun}
          disabled={executeInFlight}
          className="px-3 py-1 rounded bg-primary text-primary-foreground disabled:opacity-50 inline-flex items-center"
        >
          <Play className="inline mr-1 h-4 w-4" /> Run
        </button>
        <button
          type="button"
          onClick={handleReloadThenRun}
          disabled={reloadInFlight}
          className="px-3 py-1 rounded border border-border disabled:opacity-50 inline-flex items-center"
        >
          <RefreshCw className="inline mr-1 h-4 w-4" /> Reload then Run
        </button>
        {reloadInFlight && (
          <span className="text-sm text-muted-foreground">Reloading…</span>
        )}
      </div>

      {/* Error banner. UI-REVIEW (Pillar 3): token-backed destructive classes
          so the banner tracks --destructive in both light and dark mode. */}
      {error && (
        <div
          role="alert"
          className="bg-destructive/10 border border-destructive/40 text-destructive px-3 py-2 rounded"
        >
          {error}
        </div>
      )}

      {/* D-09: stacked results sections (response-driven — Pitfall 8).
          UI-REVIEW (Pillar 1): empty-state copy before the first Run, so a
          developer landing on the panel has guidance instead of an absent
          results panel. */}
      {!lastResult && !error && (
        <p className="text-sm text-muted-foreground">
          No runs yet — click Run to execute this rule.
        </p>
      )}

      {lastResult && (
        <div className="space-y-3">
          <section>
            <h3 className="font-semibold">Fired</h3>
            <p>
              Count: <span>{lastResult.rules_fired}</span>
            </p>
          </section>

          <section>
            <h3 className="font-semibold">Not fired</h3>
            <ul>
              {lastResult.rules_not_fired.map((r) => (
                <li key={r}>{r}</li>
              ))}
            </ul>
          </section>

          <section>
            <h3 className="font-semibold">Facts after</h3>
            <pre className="bg-muted p-2 rounded text-sm font-mono overflow-auto">
              {JSON.stringify(lastResult.facts_after, null, 2)}
            </pre>
          </section>

          {/* D-09: trace section hidden entirely when inference_trace is None */}
          {lastResult.inference_trace && (
            <section>
              <h3 className="font-semibold">Inference trace</h3>
              <ul className="text-sm">
                {lastResult.inference_trace.entries.map((entry, i) => (
                  <li key={i}>
                    {entry.timestamp} — {entry.event_type} — {entry.rule_name}
                  </li>
                ))}
              </ul>
              {lastResult.inference_trace.summary && (
                <p className="text-sm">
                  Duration: {lastResult.inference_trace.summary.total_duration_ms}ms
                  {' — '}
                  Matched: {lastResult.inference_trace.summary.facts_matched}
                </p>
              )}
            </section>
          )}

          {/* SC-4 correlation hint. UI-REVIEW (Pillar 3): token-backed muted
              foreground instead of the hardcoded gray-500. */}
          <p className="text-xs text-muted-foreground">
            engine_generation: {lastResult.engine_generation}
            {preReloadGeneration !== undefined && (
              <> (pre-reload: {preReloadGeneration})</>
            )}
          </p>
        </div>
      )}
    </section>
  )
}

export default TestRunner