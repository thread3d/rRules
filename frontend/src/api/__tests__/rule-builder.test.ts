// The bridge from the form to an executable rule.
//
// The form used to collect free text in a pseudo-DSL that nothing parsed, so every rule created through the
// interface was a draft that could never fire. These tests pin the two things that matter: the JSON the API
// receives is the engine's own shape, and the text stored beside it describes that same rule.

import { describe, expect, it } from 'vitest'

import { encodeValue, renderBody, toEngineRule, type RuleDraft } from '@/api/rule-builder'

const draft: RuleDraft = {
  factType: 'customer',
  field: 'status',
  op: 'Eq',
  value: 'premium',
  message: 'High status customer detected',
  salience: 10,
}

describe('the rule builder', () => {
  it('emits the engine rule shape, not a DSL string', () => {
    const rule = toEngineRule(draft, 'high-status-customer')

    expect(rule.condition.Pattern).toEqual({
      fact_type: 'customer',
      field: 'status',
      value: { String: 'premium' },
      op: 'Eq',
    })
    expect(rule.actions).toEqual([
      { LogMessage: { level: 'Info', message: 'High status customer detected' } },
    ])
    expect(rule.salience).toBe(10)
    expect(rule.no_loop).toBe(false)
    expect(rule.lock_on_active).toBe(false)
  })

  it('encodes a value as an integer only when it is one', () => {
    expect(encodeValue('42')).toEqual({ Integer: 42 })
    expect(encodeValue('-7')).toEqual({ Integer: -7 })
    // Not a number the engine should silently round: `1.5` stays a string.
    expect(encodeValue('1.5')).toEqual({ String: '1.5' })
    expect(encodeValue('premium')).toEqual({ String: 'premium' })
    expect(encodeValue('  spaced  ')).toEqual({ String: 'spaced' })
  })

  it('renders the body from the same draft, so the two cannot disagree', () => {
    expect(renderBody(draft, 'high-status-customer')).toBe(
      [
        'rule "high-status-customer"',
        'when customer.status = "premium"',
        'then log("High status customer detected")',
      ].join('\n'),
    )
  })

  it('renders numbers without quotes and each operator as written', () => {
    const numeric: RuleDraft = { ...draft, field: 'score', op: 'Ge', value: '100' }
    expect(renderBody(numeric, 'big-spender')).toContain('when customer.score >= 100')
    expect(
      renderBody({ ...draft, op: 'Ne' }, 'x'),
    ).toContain('customer.status != "premium"')
  })

  it('trims the names it sends, because the engine matches on them exactly', () => {
    const rule = toEngineRule({ ...draft, factType: ' customer ', field: ' status ' }, 'n')
    expect(rule.condition.Pattern.fact_type).toBe('customer')
    expect(rule.condition.Pattern.field).toBe('status')
    expect(rule.actions[0].LogMessage.message).toBe('High status customer detected')
  })
})
