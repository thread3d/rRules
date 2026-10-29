import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { createRule } from '@/api/rules'
import {
  type ComparisonOperator,
  type RuleDraft,
  renderBody,
  toEngineRule,
} from '@/api/rule-builder'

// Creating a rule is a form, because that is what the API takes: a name plus a body
// of text, which it stores and never parses (`api/src/v1/rules.rs`). The page used to
// mount a JavaScript expression editor, a hit-policy selector and an empty decision
// table with no name field and no submit button, so a rule could not be created from
// the interface at all — while the rule list offered a "Create Rule" link to it.

const OPERATORS: Array<{ value: ComparisonOperator; label: string }> = [
  { value: 'Eq', label: '=' },
  { value: 'Ne', label: '!=' },
  { value: 'Gt', label: '>' },
  { value: 'Ge', label: '>=' },
  { value: 'Lt', label: '<' },
  { value: 'Le', label: '<=' },
]

function NewRulePage() {
  const navigate = useNavigate()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [factType, setFactType] = useState('customer')
  const [field, setField] = useState('status')
  const [op, setOp] = useState<ComparisonOperator>('Eq')
  const [value, setValue] = useState('premium')
  const [message, setMessage] = useState('High status customer detected')
  const [salience, setSalience] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    setError(null)
    setSubmitting(true)
    try {
      const draft: RuleDraft = { factType, field, op, value, message, salience }
      const trimmed = name.trim()
      await createRule({
        name: trimmed,
        description: description.trim() || undefined,
        // A description of the rule that runs, regenerated from the same draft — not a second syntax.
        dsl_body: renderBody(draft, trimmed),
        tags: [],
        salience,
        // The engine's own rule JSON: this is what makes the created document executable.
        rule: toEngineRule(draft, trimmed),
      })
      // Back to the list, where the new rule is the first row.
      await navigate({ to: '/rules' })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'The rule could not be created')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-bold">New Rule</h1>
        <p className="text-gray-500">
          The rule is sent as the engine's own structured form and written into the rules
          directory, so it can be executed after the next reload. The body shown on the edit
          page is a description of that same rule.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <label className="block">
          <span className="mb-1 block text-sm font-medium">Name</span>
          <input
            name="name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            placeholder="high-status-customer"
            className="w-full rounded-md border px-3 py-2"
          />
        </label>

        <label className="block">
          <span className="mb-1 block text-sm font-medium">Description</span>
          <input
            name="description"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="Optional"
            className="w-full rounded-md border px-3 py-2"
          />
        </label>

        <label className="block">
          <span className="mb-1 block text-sm font-medium">Salience</span>
          <input
            name="salience"
            type="number"
            min={-1000}
            max={1000}
            value={salience}
            onChange={(event) => setSalience(Number(event.target.value))}
            className="w-32 rounded-md border px-3 py-2"
          />
        </label>

        <fieldset className="space-y-3 rounded-md border p-4">
          <legend className="px-1 text-sm font-medium">When</legend>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-4">
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Fact type</span>
              <input
                name="fact_type"
                value={factType}
                onChange={(event) => setFactType(event.target.value)}
                required
                className="w-full rounded-md border px-3 py-2"
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Field</span>
              <input
                name="field"
                value={field}
                onChange={(event) => setField(event.target.value)}
                required
                className="w-full rounded-md border px-3 py-2"
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Operator</span>
              <select
                name="operator"
                value={op}
                onChange={(event) => setOp(event.target.value as ComparisonOperator)}
                className="w-full rounded-md border px-3 py-2"
              >
                {OPERATORS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Value</span>
              <input
                name="value"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                required
                className="w-full rounded-md border px-3 py-2"
              />
            </label>
          </div>
        </fieldset>

        <fieldset className="space-y-3 rounded-md border p-4">
          <legend className="px-1 text-sm font-medium">Then</legend>
          <label className="block">
            <span className="mb-1 block text-xs text-gray-500">Log message</span>
            <input
              name="message"
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              required
              className="w-full rounded-md border px-3 py-2"
            />
          </label>
        </fieldset>

        {error && (
          <div className="rounded-md bg-red-50 p-4 dark:bg-red-900/20">
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
        )}

        <div className="flex gap-3">
          <button
            type="submit"
            disabled={submitting || !name.trim() || !factType.trim() || !field.trim()}
            className="rounded-md bg-blue-600 px-4 py-2 font-medium text-white disabled:opacity-50"
          >
            {submitting ? 'Creating…' : 'Create rule'}
          </button>
          <button
            type="button"
            onClick={() => navigate({ to: '/rules' })}
            className="rounded-md border px-4 py-2"
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  )
}

export const Route = createFileRoute('/rules/new')({
  component: NewRulePage,
})
