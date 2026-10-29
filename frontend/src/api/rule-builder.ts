// Turn what the New Rule form collects into the JSON the API accepts as `rule`.
//
// The form used to collect a free-text body in a pseudo-DSL (`rule "x" when customer.status =
// "premium" then log(…)`) that **nothing parsed**: the API stored it, and no engine could run it. So
// every rule created through the interface was a draft that could never fire, while the interface
// presented rule creation as the product's core action.
//
// This module is the bridge. It emits the engine's own rule JSON — the shape `POST /v1/execute` loads —
// and renders the same rule as text for `dsl_body`, so what the list and the detail page show is a
// description of the rule that actually runs rather than a separate document.

/** Comparison operators, spelled as the engine's `ComparisonOp` variants. */
export type ComparisonOperator = 'Eq' | 'Ne' | 'Lt' | 'Le' | 'Gt' | 'Ge'

/** The fields the form collects. */
export interface RuleDraft {
  /** Fact type the condition matches, e.g. `customer`. */
  factType: string
  /** Field on that fact, e.g. `status`. */
  field: string
  op: ComparisonOperator
  /**
   * Value to compare against, as typed by the user.
   *
   * A value that parses as an integer becomes `{"Integer": n}` and anything else `{"String": s}` —
   * the two carriers this form can express. Fractions and booleans are not offered yet, because the
   * form has no control that distinguishes them from text.
   */
  value: string
  /** Message logged when the rule fires. */
  message: string
  /** Rule salience (higher fires first). */
  salience: number
}

/** The engine rule the API writes into the executing set. */
export interface EngineRule {
  id: string
  name: string
  salience: number
  no_loop: boolean
  lock_on_active: boolean
  condition: {
    Pattern: {
      fact_type: string
      field: string
      value: Record<string, number | string>
      op: ComparisonOperator
    }
  }
  actions: Array<{ LogMessage: { level: string; message: string } }>
}

/**
 * The engine value for the form's text.
 *
 * `Integer` when the text is an integer — `Number`/`Float` and booleans are deliberately not guessed at,
 * so `1.5` becomes the *string* `"1.5"` rather than a silently truncated number.
 */
export function encodeValue(raw: string): Record<string, number | string> {
  const trimmed = raw.trim()
  if (/^-?\d+$/.test(trimmed)) {
    return { Integer: Number.parseInt(trimmed, 10) }
  }
  return { String: trimmed }
}

/**
 * Build the engine rule.
 *
 * `id` and `name` are placeholders: the API **imposes the document's identity** on whatever arrives
 * (`write_executable_rule`), so the id the caller later sees is the document's and the engine reports it.
 * They are set here anyway, because a rule without them is not a rule.
 */
export function toEngineRule(draft: RuleDraft, name: string): EngineRule {
  return {
    id: name,
    name,
    salience: draft.salience,
    no_loop: false,
    lock_on_active: false,
    condition: {
      Pattern: {
        fact_type: draft.factType.trim(),
        field: draft.field.trim(),
        value: encodeValue(draft.value),
        op: draft.op,
      },
    },
    actions: [
      { LogMessage: { level: 'Info', message: draft.message.trim() } },
    ],
  }
}

const OPERATOR_TEXT: Record<ComparisonOperator, string> = {
  Eq: '=',
  Ne: '!=',
  Lt: '<',
  Le: '<=',
  Gt: '>',
  Ge: '>=',
}

/**
 * Render the rule as text for `dsl_body`.
 *
 * This is a *description* of the rule that runs, not a second definition of it: the engine never reads
 * it, and it is regenerated from the same draft, so the two cannot disagree.
 */
export function renderBody(draft: RuleDraft, name: string): string {
  const value = encodeValue(draft.value)
  const shown = 'String' in value ? JSON.stringify(value.String) : String(value.Integer)
  return [
    `rule "${name}"`,
    `when ${draft.factType.trim()}.${draft.field.trim()} ${OPERATOR_TEXT[draft.op]} ${shown}`,
    `then log(${JSON.stringify(draft.message.trim())})`,
  ].join('\n')
}
