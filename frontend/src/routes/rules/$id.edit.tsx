import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { RuleEditor } from '@/components/RuleEditor'
import { DecisionTable } from '@/features/rules/DecisionTable'
import { HitPolicySelector } from '@/features/rules/HitPolicySelector'
import { TestRunner } from '@/components/TestRunner'
import { updateRule } from '@/api/rules'
import type { HitPolicyId, DecisionTableColumn, DecisionTableRow } from '@/features/rules/types'

function EditRulePage() {
  const { id } = Route.useParams()
  // TODO(22-05/22-06): load rule `id` via the rule fetch API and persist edits.
  // Mounting the editor + table + hit-policy selector deep-linkably (D-22-08)
  // is this plan's bar; fetch/save wiring lands in the governance/authoring
  // plans.
  const [expression, setExpression] = useState(`// Editing rule ${id}\nreturn true;`)
  const [hitPolicy, setHitPolicy] = useState<HitPolicyId>('unique')
  const [columns] = useState<DecisionTableColumn[]>([])
  const [rows] = useState<DecisionTableRow[]>([])
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | undefined>(undefined)
  const [savedAt, setSavedAt] = useState<string | undefined>(undefined)

  // D-05: minimal Save → PUT /v1/rules/{id} enabler. Reuses the existing
  // authenticated `updateRule` wrapper (Phase 20 management endpoint) — no
  // new auth path; backend governance/approval still applies server-side.
  // This is a MINIMAL enabler (D-05 scope): it persists the structured
  // editable fields so "edit → save → reload → run" (SC-4) can observe a
  // changed hitPolicy. A full authoring UX (expression ↔ conditions/actions
  // mapping, validation, governance approval integration) is explicitly OUT
  // of scope for Phase 28 — the expression editor remains a scaffold pending
  // the future authoring phase.
  async function handleSave() {
    setSaving(true)
    setSaveError(undefined)
    try {
      await updateRule(id, {
        hitPolicy,
        description: `Edited via edit page at ${new Date().toISOString()}`,
      })
      setSavedAt(new Date().toISOString())
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Edit Rule {id}</h1>
        <p className="text-muted-foreground">Editing existing rule definition.</p>
      </div>

      <section className="space-y-2">
        <h2 className="font-semibold">Expression</h2>
        <RuleEditor value={expression} onChange={setExpression} />
      </section>

      <section className="space-y-2">
        <h2 className="font-semibold">Hit Policy</h2>
        <HitPolicySelector value={hitPolicy} onChange={setHitPolicy} />
      </section>

      <section className="space-y-2">
        <h2 className="font-semibold">Decision Table</h2>
        <DecisionTable
          hitPolicy={hitPolicy}
          columns={columns}
          rows={rows}
          onHitPolicyChange={setHitPolicy}
        />
      </section>

      {/* D-05: minimal Save → PUT /v1/rules/{id} enabler (SC-4 bridge).
          UI-REVIEW (Pillar 3): Save now uses the --primary token instead of
          the one-off bg-blue-600; meta/error copy uses muted/destructive
          tokens so it tracks dark mode. */}
      <section className="space-y-2">
        <h2 className="font-semibold">Save</h2>
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="px-3 py-1 bg-primary text-primary-foreground rounded disabled:opacity-50"
        >
          {saving ? 'Saving…' : 'Save'}
        </button>
        {savedAt && <p className="text-xs text-muted-foreground">Saved at {savedAt}</p>}
        {saveError && (
          <p role="alert" className="text-xs text-destructive">
            {saveError}
          </p>
        )}
      </section>

      {/* D-04: TestRunner mounted as a new section in the edit page. */}
      <TestRunner ruleId={id} />
    </div>
  )
}

export const Route = createFileRoute('/rules/$id/edit')({
  component: EditRulePage,
})