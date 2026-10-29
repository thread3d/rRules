import { useEffect, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { getRule, updateRule } from '@/api/rules'
import {
  type ComparisonOperator,
  type RuleDraft,
  type RuleValueKind,
  operatorsFor,
  renderBody,
  toEngineRule,
} from '@/api/rule-builder'
import { TestRunner } from '@/components/TestRunner'

// Editing a rule: load it, change it, save it.
//
// The page used to invent its content — a hard-coded expression with the id spliced in, a
// hit-policy selector, an empty decision table — and save a description with a timestamp,
// never reading the rule it claimed to edit. It now shows the rule's own name, description
// and body, and saves what the API actually stores.

function EditRulePage() {
  const { id } = Route.useParams()
  const navigate = useNavigate()

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [body, setBody] = useState('')
  // The structured form, used to make a text-only document executable (migration) or to re-declare the rule
  // behind an existing one. Left empty, the save touches only the body.
  const [factType, setFactType] = useState('')
  const [field, setField] = useState('')
  const [op, setOp] = useState<ComparisonOperator>('Eq')
  const [value, setValue] = useState('')
  const [valueKind, setValueKind] = useState<RuleValueKind>('text')
  const [message, setMessage] = useState('')
  const [structureError, setStructureError] = useState<string | null>(null)
  const [detail, setDetail] = useState('')
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [savedAt, setSavedAt] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    getRule(id)
      .then((rule) => {
        if (cancelled) return
        setName(rule.name)
        setDescription(rule.description ?? '')
        setBody(rule.body ?? '')
        setDetail(
          `status ${rule.status}, version ${rule.version}, ${
            rule.executable ? 'executable' : 'not executable — no engine can run it yet'
          }`,
        )
        setLoadError(null)
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLoadError(error instanceof Error ? error.message : 'The rule could not be loaded')
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [id])

  async function handleSave(event: React.FormEvent) {
    event.preventDefault()
    setSaveError(null)
    setSaving(true)
    try {
      // A filled-in structure is what makes the document executable; the body is then re-rendered from it so
      // the text describes the rule that runs. An empty structure leaves both alone.
      const hasStructure = Boolean(factType.trim() && field.trim() && value.trim() && message.trim())
      let structured: unknown
      let renderedBody = body
      if (hasStructure) {
        const draft: RuleDraft = { factType, field, op, value, valueKind, message, salience: 0 }
        try {
          structured = toEngineRule(draft, name.trim())
          renderedBody = renderBody(draft, name.trim())
        } catch (error) {
          setStructureError(
            error instanceof Error ? error.message : 'The rule could not be built',
          )
          setSaving(false)
          return
        }
      }
      setStructureError(null)
      await updateRule(id, {
        name: name.trim(),
        description: description.trim() || undefined,
        dsl_body: renderedBody,
        rule: structured,
      })
      setSavedAt(new Date().toLocaleTimeString())
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : 'The rule could not be saved')
    } finally {
      setSaving(false)
    }
  }

  if (loading) {
    return <p className="p-6 text-gray-500">Loading rule…</p>
  }

  if (loadError) {
    return (
      <div className="mx-auto max-w-2xl space-y-3 p-6">
        <h1 className="text-2xl font-bold">Rule not available</h1>
        <p className="text-red-600 dark:text-red-400">{loadError}</p>
        <button
          type="button"
          onClick={() => navigate({ to: '/rules' })}
          className="rounded-md border px-4 py-2"
        >
          Back to the rule list
        </button>
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-bold">Edit rule</h1>
        <p className="text-gray-500">
          {id} — {detail}
        </p>
      </div>

      <form onSubmit={handleSave} className="space-y-4">
        <label className="block">
          <span className="mb-1 block text-sm font-medium">Name</span>
          <input
            name="name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            className="w-full rounded-md border px-3 py-2"
          />
        </label>

        <label className="block">
          <span className="mb-1 block text-sm font-medium">Description</span>
          <input
            name="description"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            className="w-full rounded-md border px-3 py-2"
          />
        </label>

        <label className="block">
          <span className="mb-1 block text-sm font-medium">Rule body</span>
          <textarea
            name="body"
            value={body}
            onChange={(event) => setBody(event.target.value)}
            rows={10}
            placeholder="The rule's body text. Fill in the structure below to make the rule executable; the body is then re-rendered to describe it."
            className="w-full rounded-md border px-3 py-2 font-mono text-sm"
          />
        </label>

        <fieldset className="space-y-3 rounded-md border p-4">
          <legend className="px-1 text-sm font-medium">
            Executable rule (optional)
          </legend>
          <p className="text-xs text-gray-500">
            A document with only a body cannot be executed. Fill these in to give it the condition and action
            it describes, and the API will write it into the rules directory — the document keeps its id,
            history and approvals.
          </p>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-5">
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Fact type</span>
              <input
                name="fact_type"
                value={factType}
                onChange={(event) => setFactType(event.target.value)}
                placeholder="customer"
                className="w-full rounded-md border px-3 py-2"
              />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Field</span>
              <input
                name="field"
                value={field}
                onChange={(event) => setField(event.target.value)}
                placeholder="status"
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
                {operatorsFor(valueKind).map((operator) => (
                  <option key={operator} value={operator}>
                    {operator}
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Type</span>
              <select
                name="value_kind"
                value={valueKind}
                onChange={(event) => {
                  const kind = event.target.value as RuleValueKind
                  setValueKind(kind)
                  if (!operatorsFor(kind).includes(op)) setOp('Eq')
                }}
                className="w-full rounded-md border px-3 py-2"
              >
                <option value="text">Text</option>
                <option value="number">Number</option>
                <option value="boolean">True / false</option>
              </select>
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-gray-500">Value</span>
              <input
                name="value"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder="premium"
                className="w-full rounded-md border px-3 py-2"
              />
            </label>
          </div>
          <label className="block">
            <span className="mb-1 block text-xs text-gray-500">Log message</span>
            <input
              name="message"
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              placeholder="What the rule records when it fires"
              className="w-full rounded-md border px-3 py-2"
            />
          </label>
          {structureError && (
            <p className="text-sm text-red-600 dark:text-red-400">{structureError}</p>
          )}
        </fieldset>

        {saveError && (
          <div className="rounded-md bg-red-50 p-4 dark:bg-red-900/20">
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {saveError}
            </p>
          </div>
        )}

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={saving}
            className="rounded-md bg-blue-600 px-4 py-2 font-medium text-white disabled:opacity-50"
          >
            {saving ? 'Saving…' : 'Save changes'}
          </button>
          <button
            type="button"
            onClick={() => navigate({ to: '/rules' })}
            className="rounded-md border px-4 py-2"
          >
            Back to the rule list
          </button>
          {savedAt && <span className="text-xs text-gray-500">Saved at {savedAt}</span>}
        </div>
      </form>

      <section className="space-y-2">
        <h2 className="font-semibold">Run this rule</h2>
        <TestRunner ruleId={id} />
      </section>
    </div>
  )
}

export const Route = createFileRoute('/rules/$id/edit')({
  component: EditRulePage,
})
