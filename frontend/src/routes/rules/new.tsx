import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { RuleEditor } from '@/components/RuleEditor'
import { DecisionTable } from '@/features/rules/DecisionTable'
import { HitPolicySelector } from '@/features/rules/HitPolicySelector'
import type { HitPolicy, DecisionTableColumn, DecisionTableRow } from '@/features/rules/types'

function NewRulePage() {
  const [expression, setExpression] = useState('// New rule expression\nreturn true;')
  const [hitPolicy, setHitPolicy] = useState<HitPolicy>('UNIQUE')
  // TODO(22-05/22-06): wire to rule create API. Using empty placeholder columns/
  // rows so the editor mounts end-to-end; the real save + table data lands in
  // the governance/authoring wiring plans.
  const [columns] = useState<DecisionTableColumn[]>([])
  const [rows] = useState<DecisionTableRow[]>([])

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">New Rule</h1>
        <p className="text-gray-500">Define a new business rule.</p>
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

export const Route = createFileRoute('/rules/new')({
  component: NewRulePage,
})