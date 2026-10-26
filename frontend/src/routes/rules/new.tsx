import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { createRule } from '@/api/rules'

// Creating a rule is a form, because that is what the API takes: a name plus a body
// of text, which it stores and never parses (`api/src/v1/rules.rs`). The page used to
// mount a JavaScript expression editor, a hit-policy selector and an empty decision
// table with no name field and no submit button, so a rule could not be created from
// the interface at all — while the rule list offered a "Create Rule" link to it.

const EXAMPLE_BODY = `// Rule body. Stored as text by the API; see docs/user-guide.md for the rule model.
rule "high-status-customer"
when customer.status = "premium"
then log("High status customer detected")
`

function NewRulePage() {
  const navigate = useNavigate()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [body, setBody] = useState(EXAMPLE_BODY)
  const [salience, setSalience] = useState(0)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    setError(null)
    setSubmitting(true)
    try {
      await createRule({
        name: name.trim(),
        description: description.trim() || undefined,
        dsl_body: body,
        tags: [],
        salience,
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
          The body is stored as text and shown when the rule is edited; the API does not
          compile or validate it.
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

        <label className="block">
          <span className="mb-1 block text-sm font-medium">Rule body</span>
          <textarea
            name="body"
            value={body}
            onChange={(event) => setBody(event.target.value)}
            rows={10}
            required
            className="w-full rounded-md border px-3 py-2 font-mono text-sm"
          />
        </label>

        {error && (
          <div className="rounded-md bg-red-50 p-4 dark:bg-red-900/20">
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
        )}

        <div className="flex gap-3">
          <button
            type="submit"
            disabled={submitting || !name.trim() || !body.trim()}
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
