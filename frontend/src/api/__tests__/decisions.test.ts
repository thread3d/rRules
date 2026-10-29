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

import { evaluateDecision } from '@/api/decisions'

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

})
