// Phase 28 (Plan 28-02, Task 2): RED TestRunner component test scaffolds.
//
// These four tests pin the D-04/D-07/D-08/D-09 contract for the TestRunner
// component that Plan 28-05 will implement. They are expected to FAIL RED
// in this plan because `@/components/TestRunner` does not exist yet (the
// import resolves to a missing module). Plan 28-05 creates TestRunner.tsx
// and flips these GREEN.
//
// Contract references:
//  - D-04: TestRunner mounts in the edit page, takes a `ruleId` prop.
//  - D-07: "Reload then Run" polls getJobStatus then executeRule.
//  - D-08: Engine dropdown populated from GET /v1/engines (useQuery).
//  - D-09: Stacked sections; trace section hidden when inference_trace is None.
//
// The tests mock @/api/execution (the module Plan 28-04 creates) so the
// component's data layer is isolated from the network. The mock functions
// are created via vi.hoisted so they are available inside the vi.mock
// factory (which is hoisted above all imports) and the test file does NOT
// statically import @/api/execution — that module does not exist yet, and
// a static import would fail at Vite's import-analysis before vi.mock could
// virtualize it. Each test wraps TestRunner in a fresh QueryClientProvider
// (per-test QueryClient to avoid cross-test cache bleed).

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'

// vi.hoisted runs before module setup and gives the vi.mock factory a
// reference to the mock functions. The factory returns them as the module's
// exports; when TestRunner (Plan 28-05) imports @/api/execution it receives
// these mocks. Because the test file itself does not statically import
// @/api/execution, Vite's import-analysis never tries to resolve the
// not-yet-created module — only @/components/TestRunner is unresolved (the
// intended RED trigger).
const mocks = vi.hoisted(() => ({
  executeRule: vi.fn(),
  listEngines: vi.fn(),
  getJobStatus: vi.fn(),
  reloadRules: vi.fn(),
}))

vi.mock('@/api/execution', () => ({
  executeRule: mocks.executeRule,
  listEngines: mocks.listEngines,
  getJobStatus: mocks.getJobStatus,
  reloadRules: mocks.reloadRules,
}))

// TestRunner.tsx does not exist yet (Plan 28-05). Importing it here is the
// RED trigger: the module cannot be resolved, so every test fails at the
// import boundary until Plan 28-05 lands the component.
import { TestRunner } from '@/components/TestRunner'

// Helper: wrap a node in a fresh QueryClientProvider per test so cache state
// from one test cannot bleed into another (D-08 useQuery isolation).
function renderWithClient(ui: ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  })
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  )
}

beforeEach(() => {
  // Reset mock call counts + implementations between tests so each test
  // starts from a clean baseline. Default implementations match <behavior>.
  mocks.executeRule.mockClear().mockResolvedValue({
    execution_id: 'e1',
    rules_fired: 1,
    rules_not_fired: ['r2'],
    facts_after: { scorecard: { score: 10 } },
    inference_trace: null,
    engine_generation: 1,
    session_id: null,
  })
  mocks.listEngines.mockClear().mockResolvedValue({
    engines: ['rete', 'treat'],
    default: 'rete',
  })
  mocks.getJobStatus.mockClear().mockResolvedValue({
    job_id: 'j1',
    status: 'completed',
  })
  mocks.reloadRules.mockClear().mockResolvedValue({
    job_id: 'j1',
    status: 'pending',
  })
})

describe('TestRunner', () => {
  // D-04 (ruleId prop), D-09 (results panel renders rules_fired /
  // rules_not_fired / facts_after).
  it('renders fired rules and facts_after', async () => {
    mocks.executeRule.mockResolvedValue({
      execution_id: 'e1',
      rules_fired: 1,
      rules_not_fired: ['r2'],
      facts_after: { scorecard: { score: 10 } },
      inference_trace: null,
      engine_generation: 1,
      session_id: null,
    })

    renderWithClient(<TestRunner ruleId="r1" />)

    // Click the Run button to trigger executeRule.
    const runButton = screen.getByRole('button', { name: /run/i })
    fireEvent.click(runButton)

    // Assert the panel shows the fired count (1), the not-fired rule (r2),
    // and the facts_after JSON containing score: 10.
    await waitFor(() => {
      expect(screen.getByText(/1/)).toBeInTheDocument()
    })
    expect(screen.getByText(/r2/)).toBeInTheDocument()
    expect(screen.getByText(/10/)).toBeInTheDocument()
    expect(mocks.executeRule).toHaveBeenCalledTimes(1)
  })

  // D-08: dropdown populated from GET /v1/engines; default = backend default.
  it('dropdown lists engines', async () => {
    renderWithClient(<TestRunner ruleId="r1" />)

    // The dropdown/select is populated with both engine options and the
    // default selection is "rete".
    await waitFor(() => {
      expect(screen.getByRole('option', { name: /rete/i })).toBeInTheDocument()
      expect(screen.getByRole('option', { name: /treat/i })).toBeInTheDocument()
    })
    const select = screen.getByRole('combobox') as HTMLSelectElement
    expect(select.value).toBe('rete')
  })

  // D-09: trace section hidden when inference_trace is null; shown when present.
  it('trace section hidden when null', async () => {
    // First: inference_trace null -> trace section NOT in the document.
    mocks.executeRule.mockResolvedValue({
      execution_id: 'e1',
      rules_fired: 1,
      rules_not_fired: ['r2'],
      facts_after: { scorecard: { score: 10 } },
      inference_trace: null,
      engine_generation: 1,
      session_id: null,
    })
    renderWithClient(<TestRunner ruleId="r1" />)
    fireEvent.click(screen.getByRole('button', { name: /run/i }))
    await waitFor(() => {
      expect(screen.getByText(/10/)).toBeInTheDocument()
    })
    expect(screen.queryByText(/inference trace/i)).not.toBeInTheDocument()

    // Second: inference_trace populated -> trace section IS present.
    mocks.executeRule.mockResolvedValue({
      execution_id: 'e2',
      rules_fired: 1,
      rules_not_fired: ['r2'],
      facts_after: { scorecard: { score: 10 } },
      inference_trace: {
        entries: [
          {
            timestamp: '2026-07-06T00:00:00Z',
            event_type: 'rule_fired',
            rule_name: 'r1',
            data: null,
          },
        ],
        summary: { total_duration_ms: 1, facts_matched: 1 },
      },
      engine_generation: 1,
      session_id: null,
    })
    fireEvent.click(screen.getByRole('button', { name: /run/i }))
    await waitFor(() => {
      expect(screen.getByText(/inference trace/i)).toBeInTheDocument()
    })
  })

  // D-07: "Reload then Run" calls reloadRules, polls getJobStatus until
  // completed, then calls executeRule and the result reflects the response
  // (with engine_generation greater than the pre-reload generation).
  it('reload then run polls job status', async () => {
    mocks.reloadRules.mockResolvedValue({ job_id: 'j1', status: 'pending' })
    // First poll: running. Second poll: completed.
    mocks.getJobStatus
      .mockResolvedValueOnce({ job_id: 'j1', status: 'running' })
      .mockResolvedValueOnce({ job_id: 'j1', status: 'completed' })
    // Post-reload execute reflects a bumped engine_generation (SC-4).
    mocks.executeRule.mockResolvedValue({
      execution_id: 'e2',
      rules_fired: 1,
      rules_not_fired: ['r2'],
      facts_after: { scorecard: { score: 20 } },
      inference_trace: null,
      engine_generation: 2,
      session_id: null,
    })

    renderWithClient(<TestRunner ruleId="r1" />)

    // Click "Reload then Run" to trigger the reload -> poll -> execute flow.
    const reloadButton = screen.getByRole('button', { name: /reload then run/i })
    fireEvent.click(reloadButton)

    // reloadRules is called first.
    await waitFor(() => expect(mocks.reloadRules).toHaveBeenCalledTimes(1))
    // getJobStatus is polled until the job completes.
    await waitFor(() => expect(mocks.getJobStatus).toHaveBeenCalled())
    // executeRule is called after the reload completes and the result
    // reflects the bumped engine_generation (SC-4 correlation).
    await waitFor(() => expect(mocks.executeRule).toHaveBeenCalled())
    expect(screen.getByText(/20/)).toBeInTheDocument()
  })
})