import { useEffect, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { getRule, updateRule } from '@/api/rules'
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
        setDetail(`status ${rule.status}, version ${rule.version}`)
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
      await updateRule(id, {
        name: name.trim(),
        description: description.trim() || undefined,
        dsl_body: body,
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
            placeholder="The rule's body, stored as text and never compiled by the API."
            className="w-full rounded-md border px-3 py-2 font-mono text-sm"
          />
        </label>

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
