// The wire-to-view mapping, pinned.
//
// `api/rules.ts` translates the API's `RuleResponse` (snake_case, `dsl_body`, `version`) into the
// editor's `Rule` view model (camelCase, conditions, hit policy). Nothing tested that translation, and
// the defect it exists to prevent is invisible without a test: before it existed, "every field the API
// did return arrived as `undefined` — dates included, which is why sorting by date compared `NaN`". A
// row with a blank name and an invalid date looks like a rendering quirk, not a mapping bug, so a
// regression here would be silent.
//
// These tests drive the public functions against a stubbed client, so they cover the boundary a caller
// actually crosses rather than a private helper.

import { describe, expect, it, vi, beforeEach } from 'vitest'

const get = vi.hoisted(() => vi.fn())
const post = vi.hoisted(() => vi.fn())

vi.mock('@/api/client', () => ({
  default: { get, post },
}))

// Imported after the mock is registered.
import { getRules, getRule } from '@/api/rules'

/** A rule exactly as `api/src/v1/rules.rs` serialises it. */
const WIRE_RULE = {
  id: 'rule-1',
  name: 'Score plus ten',
  description: 'a rule',
  category: 'scorecards',
  dsl_body: 'rule "r" when true then log("ok")',
  tags: ['demo'],
  status: 'active',
  version: 3,
  created_at: '2026-10-01T10:00:00Z',
  updated_at: '2026-10-02T11:00:00Z',
}

beforeEach(() => {
  get.mockReset()
  post.mockReset()
})

describe('the rule mapping', () => {
  it('maps every field the API sends onto the view model', async () => {
    get.mockResolvedValue({ data: { rules: [WIRE_RULE] } })

    const [rule] = await getRules()

    expect(rule.id).toBe('rule-1')
    expect(rule.name).toBe('Score plus ten')
    expect(rule.description).toBe('a rule')
    expect(rule.status).toBe('active')
    // The two that were `undefined` before the mapping existed: a date compared as NaN, and a version
    // that fell back to the wrong number.
    expect(rule.createdAt).toBe('2026-10-01T10:00:00Z')
    expect(rule.updatedAt).toBe('2026-10-02T11:00:00Z')
    expect(rule.version).toBe(3)
  })

  it('reads the paginated envelope the API actually sends', async () => {
    // `RuleListResponse` renames its `items` field to `rules` on the wire; a client that read `items`
    // would show an empty list for a non-empty page.
    get.mockResolvedValue({ data: { total_count: 1, rules: [WIRE_RULE], page: 1, page_size: 20, total_pages: 1 } })

    const rules = await getRules()

    expect(rules).toHaveLength(1)
    expect(rules[0].name).toBe('Score plus ten')
  })

  it('defaults a status it does not know rather than passing it through', async () => {
    get.mockResolvedValue({ data: { ...WIRE_RULE, status: 'something-new' } })

    const rule = await getRule('rule-1')

    expect(rule.status).toBe('draft')
  })

  it('survives a rule with no optional fields', async () => {
    // The API omits `description`, `category` and `dsl_body` when they are empty
    // (`skip_serializing_if = "Option::is_none"`), so the mapping must not assume them.
    get.mockResolvedValue({
      data: { id: 'rule-2', name: 'Bare', tags: [], status: 'draft', version: 1 },
    })

    const rule = await getRule('rule-2')

    expect(rule.name).toBe('Bare')
    expect(rule.description).toBeUndefined()
    expect(rule.createdAt).toBe('')
    expect(rule.version).toBe(1)
  })
})
