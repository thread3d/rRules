// The decision client, pinned to what the endpoints actually send.
//
// `specs/http-api.md`'s client table recorded the mismatch for as long as it existed: `listDecisions`
// expected a `model_version` the backend never sends, and `getDecision` expected a `decision_table` that
// only `/ir` returns. Nothing failed, because nothing called them — the defect was a type that
// described an API which did not exist, waiting for the first caller to be silently wrong.
//
// These tests read the wrappers through a stubbed client, so the shapes are checked against the
// endpoints' own field names (`api/src/v1/decisions.rs`) rather than against hope.

import { describe, expect, it, vi, beforeEach } from 'vitest'

const get = vi.hoisted(() => vi.fn())
const post = vi.hoisted(() => vi.fn())

vi.mock('@/api/client', () => ({
  default: { get, post },
}))

import {
  evaluateDecision,
  getDecision,
  getDecisionIr,
  listDecisions,
  reloadDmn,
} from '@/api/decisions'

beforeEach(() => {
  get.mockReset()
  post.mockReset()
})

describe('the decision client', () => {
  it('evaluates and hands back the API’s own response', async () => {
    // `EvaluateDecisionResponse` in `decisions.rs`, field for field.
    const wire = {
      decision_id: 'discount',
      outputs: { rate: 20 },
      rules_fired: 1,
      duration_ms: 0.42,
    }
    post.mockResolvedValue({ data: wire })

    const result = await evaluateDecision('discount', { inputs: { customer: 'premium' } })

    expect(post).toHaveBeenCalledWith('/decisions/discount/evaluate', {
      inputs: { customer: 'premium' },
    })
    expect(result).toEqual(wire)
  })

  it('lists decisions with the envelope the server sends', async () => {
    // `list_decisions` answers `{decisions, total, page, per_page, pages}` — there is no
    // `model_version`, and the items are `DmnSummary` (`id`, `name`, `version`, `updated_at`).
    const wire = {
      decisions: [
        {
          id: 'discount',
          name: 'Discount',
          version: '1',
          updated_at: '2026-10-01T10:00:00Z',
        },
      ],
      total: 1,
      page: 1,
      per_page: 20,
      pages: 1,
    }
    get.mockResolvedValue({ data: wire })

    const listed = await listDecisions()

    expect(listed.total).toBe(1)
    expect(listed.pages).toBe(1)
    expect(listed.decisions[0].version).toBe('1')
    expect(listed.decisions[0]).not.toHaveProperty('hit_policy')
  })

  it('reads a decision document’s metadata, not its table', async () => {
    // `GET /v1/decisions/{id}` returns the stored document: length, metadata, timestamps.
    const wire = {
      id: 'discount',
      name: 'Discount',
      content_length: 325,
      metadata: {},
      created_at: '2026-10-01T10:00:00Z',
      updated_at: '2026-10-02T11:00:00Z',
    }
    get.mockResolvedValue({ data: wire })

    const document = await getDecision('discount')

    expect(document.content_length).toBe(325)
    expect(document).not.toHaveProperty('decision_table')
  })

  it('reads the decision table from the IR endpoint', async () => {
    // The table lives here, and its `hit_policy` is the DMN string — `HitPolicy` serialises as
    // `"RULE ORDER"`, not `"RuleOrder"` (pinned on the engine side by `enum_wire_tests`).
    const wire = {
      decision_id: 'discount',
      name: 'Discount',
      decision_table: {
        id: 't1',
        name: null,
        hit_policy: 'RULE ORDER',
        input_clauses: [{ input_expression: 'customer' }],
        output_clauses: [{ name: 'rate', type_ref: 'number' }],
        rules: [],
      },
    }
    get.mockResolvedValue({ data: wire })

    const ir = await getDecisionIr('discount')

    expect(ir.decision_table?.hit_policy).toBe('RULE ORDER')
    expect(ir.decision_table?.input_clauses[0].input_expression).toBe('customer')
  })

  it('reloads a model and reports the server’s status', async () => {
    post.mockResolvedValue({ data: { status: 'reloaded', message: 'DMN model reloaded successfully' } })

    const reloaded = await reloadDmn('<definitions/>')

    expect(reloaded.status).toBe('reloaded')
  })
})
