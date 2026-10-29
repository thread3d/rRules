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
  /** Value to compare against, as typed by the user. */
  value: string
  /** What kind of value that text is. */
  valueKind: RuleValueKind
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
      value: Record<string, number | string | boolean>
      op: ComparisonOperator
    }
  }
  actions: Array<{ LogMessage: { level: string; message: string } }>
}

/** The kinds of value the form can express, each mapping to an engine `Value` carrier. */
export type RuleValueKind = 'number' | 'text' | 'boolean'

/** Which comparison operators make sense for a value kind. Ordering a boolean is meaningless. */
export function operatorsFor(kind: RuleValueKind): ComparisonOperator[] {
  return kind === 'boolean' ? ['Eq', 'Ne'] : ['Eq', 'Ne', 'Gt', 'Ge', 'Lt', 'Le']
}

/**
 * The engine value for the form's text, or `null` when the text is not that kind of value.
 *
 * **Typed, not guessed.** The caller says which kind it is collecting; a text that does not parse as that
 * kind is an error the form reports, rather than a value silently reinterpreted. An earlier version had no
 * kind at all and made every non-integer a string, so `1.5` — which a user plainly meant as a number —
 * became `"1.5"` and compared against nothing.
 */
export function encodeValue(
  raw: string,
  kind: RuleValueKind,
): Record<string, number | string | boolean> | null {
  const trimmed = raw.trim()
  switch (kind) {
    case 'number': {
      // `Number('')` is `0`, so an empty box would otherwise become a comparison against zero — the worst
      // kind of wrong answer, because it looks like a deliberate one.
      if (trimmed.length === 0) return null
      if (/^-?\d+$/.test(trimmed)) {
        const parsed = Number.parseInt(trimmed, 10)
        return Number.isSafeInteger(parsed) ? { Integer: parsed } : null
      }
      const fraction = Number(trimmed)
      return Number.isFinite(fraction) ? { Float: fraction } : null
    }
    case 'boolean': {
      if (/^true$/i.test(trimmed)) return { Bool: true }
      if (/^false$/i.test(trimmed)) return { Bool: false }
      return null
    }
    case 'text':
      return trimmed.length > 0 ? { String: trimmed } : null
  }
}

/**
 * Build the engine rule.
 *
 * `id` and `name` are placeholders: the API **imposes the document's identity** on whatever arrives
 * (`write_executable_rule`), so the id the caller later sees is the document's and the engine reports it.
 * They are set here anyway, because a rule without them is not a rule.
 */
export function toEngineRule(draft: RuleDraft, name: string): EngineRule {
  const value = encodeValue(draft.value, draft.valueKind)
  if (!value) {
    throw new Error(
      `"${draft.value}" is not a ${draft.valueKind} value — check the value and its type`,
    )
  }
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
        value,
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
  const value = encodeValue(draft.value, draft.valueKind)
  const shown =
    value === null
      ? draft.value.trim()
      : 'String' in value
        ? JSON.stringify(value.String)
        : 'Bool' in value
          ? String(value.Bool)
          : String('Integer' in value ? value.Integer : value.Float)
  return [
    `rule "${name}"`,
    `when ${draft.factType.trim()}.${draft.field.trim()} ${OPERATOR_TEXT[draft.op]} ${shown}`,
    `then log(${JSON.stringify(draft.message.trim())})`,
  ].join('\n')
}
