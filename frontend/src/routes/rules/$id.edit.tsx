import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { RuleEditor } from '@/components/RuleEditor'
import { DecisionTable } from '@/features/rules/DecisionTable'
import { HitPolicySelector } from '@/features/rules/HitPolicySelector'
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

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Edit Rule {id}</h1>
        <p className="text-gray-500">Editing existing rule definition.</p>
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
    </div>
  )
}

export const Route = createFileRoute('/rules/$id/edit')({
  component: EditRulePage,
})