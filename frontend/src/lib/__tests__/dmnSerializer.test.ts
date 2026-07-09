import { editorShapeToDmnIr, dmnIrToEditorShape } from '@/lib/dmnSerializer'
import type { EditorShapeColumn, DmnDecisionTable } from '@/lib/dmnSerializer'

describe('dmnSerializer', () => {
  describe('editorShapeToDmnIr', () => {
    it('maps column accessorKey directly to input_expression (Pitfall 1)', () => {
      const columns: EditorShapeColumn[] = [
        { id: 'c1', label: 'Age', accessorKey: 'age', type: 'condition' },
        { id: 'c2', label: 'Score', accessorKey: 'score', type: 'condition' },
        { id: 'c3', label: 'Result', accessorKey: 'result', type: 'action' },
      ]
      const rows = [
        {
          id: 'r1',
          conditions: { age: '25', score: '80' },
          actions: { result: 'pass' },
        },
      ]

      const ir = editorShapeToDmnIr(columns, rows, 'UNIQUE')

      expect(ir.input_clauses).toHaveLength(2)
      expect(ir.input_clauses[0].input_expression).toBe('age')
      expect(ir.input_clauses[1].input_expression).toBe('score')
      // Never uses input_${idx}_${key} prefix.
      expect(ir.input_clauses.every((c) => !c.input_expression.startsWith('input_'))).toBe(true)
    })

    it('preserves hit policy as canonical DMN spec string (Pitfall 2)', () => {
      const columns: EditorShapeColumn[] = [
        { id: 'c1', label: 'X', accessorKey: 'x', type: 'condition' },
        { id: 'c2', label: 'Y', accessorKey: 'y', type: 'action' },
      ]

      const ir = editorShapeToDmnIr(columns, [], 'RULE ORDER')
      expect(ir.hit_policy).toBe('RULE ORDER') // uppercase with space — exact DMN spec value

      const ir2 = editorShapeToDmnIr(columns, [], 'OUTPUT ORDER')
      expect(ir2.hit_policy).toBe('OUTPUT ORDER')
    })

    it('sets aggregation only for COLLECT hit policy', () => {
      const columns: EditorShapeColumn[] = [
        { id: 'c1', label: 'X', accessorKey: 'x', type: 'condition' },
        { id: 'c2', label: 'Y', accessorKey: 'y', type: 'action' },
      ]

      const irCollect = editorShapeToDmnIr(columns, [], 'COLLECT', 'SUM')
      expect(irCollect.aggregation).toBe('SUM')

      const irUnique = editorShapeToDmnIr(columns, [], 'UNIQUE')
      expect(irUnique.aggregation).toBeUndefined()
    })

    it('maps cell text content through TextContent wrapper (is_cdata: false)', () => {
      const columns: EditorShapeColumn[] = [
        { id: 'c1', label: 'Age', accessorKey: 'age', type: 'condition' },
        { id: 'c2', label: 'Result', accessorKey: 'result', type: 'action' },
      ]
      const rows = [
        {
          id: 'r1',
          conditions: { age: '30' },
          actions: { result: 'approved' },
        },
      ]

      const ir = editorShapeToDmnIr(columns, rows, 'UNIQUE')

      expect(ir.rules).toHaveLength(1)
      expect(ir.rules[0].input_entries[0]).toEqual({
        text: { content: '30', is_cdata: false },
      })
      expect(ir.rules[0].output_entries[0]).toEqual({
        text: { content: 'approved', is_cdata: false },
      })
    })

    it('maps output column allowedValues into output_values enumeration', () => {
      const columns = [
        { id: 'c1', label: 'Risk', accessorKey: 'risk', type: 'condition' as const },
        {
          id: 'c2',
          label: 'Priority',
          accessorKey: 'priority',
          type: 'action' as const,
          allowedValues: [
            { id: 'v1', text: 'Low' },
            { id: 'v2', text: 'Medium' },
            { id: 'v3', text: 'High' },
          ],
        },
      ] as EditorShapeColumn[]
      const rows = [{ id: 'r1', conditions: { risk: 'high' }, actions: { priority: 'High' } }]

      const ir = editorShapeToDmnIr(columns, rows, 'PRIORITY')

      expect(ir.output_clauses[0].output_values).toEqual({
        kind: 'enumeration',
        values: [
          { content: 'Low', is_cdata: false },
          { content: 'Medium', is_cdata: false },
          { content: 'High', is_cdata: false },
        ],
      })
    })
  })

  describe('dmnIrToEditorShape', () => {
    it('reverses editorShapeToDmnIr faithfully (round-trip)', () => {
      const originalColumns: EditorShapeColumn[] = [
        { id: 'c1', label: 'Age', accessorKey: 'age', type: 'condition' },
        { id: 'c2', label: 'Result', accessorKey: 'result', type: 'action' },
      ]
      const originalRows = [
        {
          id: 'r1',
          conditions: { age: '25' },
          actions: { result: 'pass' },
        },
        {
          id: 'r2',
          conditions: { age: '40' },
          actions: { result: 'fail' },
        },
      ]

      const ir = editorShapeToDmnIr(originalColumns, originalRows, 'ANY')
      const roundTrip = dmnIrToEditorShape(ir)

      expect(roundTrip.columns).toHaveLength(2)
      expect(roundTrip.rows).toHaveLength(2)
      expect(roundTrip.hitPolicy).toBe('ANY')

      // Column mapping: accessorKey preserved.
      expect(roundTrip.columns[0].accessorKey).toBe('age')
      expect(roundTrip.columns[1].accessorKey).toBe('result')

      // Row data preserved through TextContent wrapper.
      expect(roundTrip.rows[0].conditions['age']).toBe('25')
      expect(roundTrip.rows[0].actions['result']).toBe('pass')
      expect(roundTrip.rows[1].conditions['age']).toBe('40')
      expect(roundTrip.rows[1].actions['result']).toBe('fail')
    })

    it('preserves aggregation field through round-trip', () => {
      const ir: DmnDecisionTable = {
        hit_policy: 'COLLECT',
        aggregation: 'MAX',
        input_clauses: [],
        output_clauses: [],
        rules: [],
      }

      const shape = dmnIrToEditorShape(ir)
      expect(shape.aggregator).toBe('MAX')
    })
  })

  describe('round-trip fidelity', () => {
    it('preserves hit_policy, input_expression names, cell content, and aggregation through full round-trip', () => {
      const columns: EditorShapeColumn[] = [
        { id: 'c1', label: 'Temperature', accessorKey: 'temp_celsius', type: 'condition' },
        { id: 'c2', label: 'Pressure', accessorKey: 'pressure_psi', type: 'condition' },
        { id: 'c3', label: 'Status', accessorKey: 'status_code', type: 'action' },
      ]
      const rows = [
        {
          id: 'r1',
          conditions: { temp_celsius: '100', pressure_psi: '14.7' },
          actions: { status_code: 'NOMINAL' },
        },
        {
          id: 'r2',
          conditions: { temp_celsius: '200', pressure_psi: '30' },
          actions: { status_code: 'WARNING' },
        },
      ]

      const ir = editorShapeToDmnIr(columns, rows, 'COLLECT', 'COUNT')
      const shape = dmnIrToEditorShape(ir)

      // Hit policy preserved.
      expect(shape.hitPolicy).toBe('COLLECT')
      // Aggregator preserved.
      expect(shape.aggregator).toBe('COUNT')
      // Input expression names from accessorKey preserved.
      expect(ir.input_clauses[0].input_expression).toBe('temp_celsius')
      expect(ir.input_clauses[1].input_expression).toBe('pressure_psi')
      // Rule count preserved.
      expect(shape.rows).toHaveLength(2)
      // Cell content preserved through TextContent wrapper.
      expect(shape.rows[0].conditions['temp_celsius']).toBe('100')
      expect(shape.rows[1].actions['status_code']).toBe('WARNING')
    })

    it('extracts enumeration from output_values for PRIORITY/OUTPUT ORDER columns', () => {
      const ir: DmnDecisionTable = {
        hit_policy: 'PRIORITY',
        input_clauses: [{ label: 'Risk', input_expression: 'risk' }],
        output_clauses: [
          {
            label: 'Priority',
            name: 'priority',
            output_values: {
              kind: 'enumeration',
              values: [
                { content: 'Low', is_cdata: false },
                { content: 'High', is_cdata: false },
              ],
            },
          },
        ],
        rules: [],
      }

      const shape = dmnIrToEditorShape(ir)
      const actionCol = shape.columns.find((c) => c.type === 'action')!
      expect(actionCol.allowedValues).toEqual([
        { id: expect.any(String), text: 'Low' },
        { id: expect.any(String), text: 'High' },
      ])
    })
  })
})
