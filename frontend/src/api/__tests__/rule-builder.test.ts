// The bridge from the form to an executable rule.
//
// The form used to collect free text in a pseudo-DSL that nothing parsed, so every rule created through the
// interface was a draft that could never fire. These tests pin the two things that matter: the JSON the API
// receives is the engine's own shape, and the text stored beside it describes that same rule.

import { describe, expect, it } from 'vitest'

import {
  encodeValue,
  operatorsFor,
  renderBody,
  toEngineRule,
  type RuleDraft,
} from '@/api/rule-builder'

const draft: RuleDraft = {
  factType: 'customer',
  field: 'status',
  op: 'Eq',
  value: 'premium',
  valueKind: 'text',
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

  it('encodes each kind of value as the carrier that kind means', () => {
    expect(encodeValue('42', 'number')).toEqual({ Integer: 42 })
    expect(encodeValue('-7', 'number')).toEqual({ Integer: -7 })
    // A fraction is a number, not a truncated integer and not a string.
    expect(encodeValue('1.5', 'number')).toEqual({ Float: 1.5 })
    expect(encodeValue('0.1', 'number')).toEqual({ Float: 0.1 })

    expect(encodeValue('premium', 'text')).toEqual({ String: 'premium' })
    // Text that looks like a number stays text when the caller says it is text.
    expect(encodeValue('42', 'text')).toEqual({ String: '42' })

    expect(encodeValue('true', 'boolean')).toEqual({ Bool: true })
    expect(encodeValue('FALSE', 'boolean')).toEqual({ Bool: false })
  })

  it('reports a value that is not the kind it claims', () => {
    // `null` rather than a guess: the form shows the error instead of sending a value the engine would
    // compare against nothing.
    expect(encodeValue('premium', 'number')).toBeNull()
    expect(encodeValue('1.5', 'boolean')).toBeNull()
    expect(encodeValue('   ', 'text')).toBeNull()
    expect(encodeValue('', 'number')).toBeNull()
  })

  it('refuses to build a rule from a value of the wrong kind', () => {
    expect(() => toEngineRule({ ...draft, valueKind: 'number' }, 'n')).toThrow(
      /is not a number value/,
    )
  })

  it('offers only the operators a kind can express', () => {
    expect(operatorsFor('number')).toEqual(['Eq', 'Ne', 'Gt', 'Ge', 'Lt', 'Le'])
    // Ordering a boolean is meaningless, so the form does not offer it.
    expect(operatorsFor('boolean')).toEqual(['Eq', 'Ne'])
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

  it('renders each kind as it would be written', () => {
    const numeric: RuleDraft = {
      ...draft,
      field: 'score',
      op: 'Ge',
      value: '100',
      valueKind: 'number',
    }
    expect(renderBody(numeric, 'big-spender')).toContain('when customer.score >= 100')
    expect(renderBody({ ...draft, op: 'Ne' }, 'x')).toContain(
      'customer.status != "premium"',
    )
    expect(
      renderBody({ ...draft, value: 'true', valueKind: 'boolean' }, 'flag'),
    ).toContain('customer.status = true')
    expect(
      renderBody({ ...draft, value: '1.5', valueKind: 'number' }, 'ratio'),
    ).toContain('customer.status = 1.5')
  })

  it('trims the names it sends, because the engine matches on them exactly', () => {
    const rule = toEngineRule({ ...draft, factType: ' customer ', field: ' status ' }, 'n')
    expect(rule.condition.Pattern.fact_type).toBe('customer')
    expect(rule.condition.Pattern.field).toBe('status')
    expect(rule.actions[0].LogMessage.message).toBe('High status customer detected')
  })
})
