import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'
import { HitPolicySelector } from '../HitPolicySelector'

describe('HitPolicySelector', () => {
  it('renders without crashing with a valid DMN hit policy value', () => {
    const onChange = vi.fn()
    render(
      <HitPolicySelector value="UNIQUE" onChange={onChange} />
    )
    expect(screen.getByRole('combobox')).toBeInTheDocument()
  })

  it('renders aggregator selector when COLLECT is selected', () => {
    const onChange = vi.fn()
    const onAggChange = vi.fn()
    render(
      <HitPolicySelector
        value="COLLECT"
        onChange={onChange}
        aggregator="SUM"
        onAggregatorChange={onAggChange}
      />
    )
    expect(screen.getAllByRole('combobox')).toHaveLength(2)
    expect(screen.getByText('Aggregator:')).toBeInTheDocument()
  })

  it('does not render aggregator selector for non-COLLECT policies', () => {
    const onChange = vi.fn()
    const onAggChange = vi.fn()
    render(
      <HitPolicySelector
        value="UNIQUE"
        onChange={onChange}
        aggregator="SUM"
        onAggregatorChange={onAggChange}
      />
    )
    expect(screen.getByRole('combobox')).toBeInTheDocument()
    expect(screen.queryByText('Aggregator:')).not.toBeInTheDocument()
  })

  it('renders spec-accurate description for each policy', () => {
    const onChange = vi.fn()
    render(
      <HitPolicySelector value="FIRST" onChange={onChange} />
    )
    expect(screen.getByText(/First matching rule in document order wins/)).toBeInTheDocument()
  })

  it('renders all 7 canonical DMN hit policies', () => {
    const onChange = vi.fn()
    render(
      <HitPolicySelector value="UNIQUE" onChange={onChange} />
    )
    expect(screen.getByText('Unique')).toBeInTheDocument()
    expect(screen.getByText('Any')).toBeInTheDocument()
    expect(screen.getByText('Priority')).toBeInTheDocument()
    expect(screen.getByText('First')).toBeInTheDocument()
    expect(screen.getByText('Rule Order')).toBeInTheDocument()
    expect(screen.getByText('Output Order')).toBeInTheDocument()
    expect(screen.getByText('Collect')).toBeInTheDocument()
  })
})
