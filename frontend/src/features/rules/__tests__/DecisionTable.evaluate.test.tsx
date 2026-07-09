import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'
import type { HitPolicy } from '../types'
import { DecisionTable } from '../DecisionTable'

describe('DecisionTable.evaluate', () => {
  it('renders without crashing with minimal props', () => {
    const mockProps = {
      hitPolicy: 'UNIQUE' as HitPolicy,
      columns: [],
      rows: [],
    }
    render(<DecisionTable {...mockProps} />)
    expect(screen.getByRole('table')).toBeInTheDocument()
  })

  it('renders aggregator selector when hit policy is COLLECT', () => {
    const onAggChange = vi.fn()
    const mockProps = {
      hitPolicy: 'COLLECT' as HitPolicy,
      columns: [],
      rows: [],
      aggregator: 'SUM',
      onAggregatorChange: onAggChange,
    }
    render(<DecisionTable {...mockProps} />)
    expect(screen.getByText('Agg:')).toBeInTheDocument()
  })

  it('does not render aggregator selector for non-COLLECT policies', () => {
    const mockProps = {
      hitPolicy: 'ANY' as HitPolicy,
      columns: [],
      rows: [],
    }
    render(<DecisionTable {...mockProps} />)
    expect(screen.queryByText('Agg:')).not.toBeInTheDocument()
  })
})
