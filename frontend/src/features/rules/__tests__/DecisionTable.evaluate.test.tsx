import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'
import { DecisionTable } from '../DecisionTable'

describe('DecisionTable.evaluate', () => {
  it('renders without crashing with minimal props', () => {
    const mockProps = {
      hitPolicy: 'unique' as const,
      columns: [],
      rows: [],
    }
    render(<DecisionTable {...mockProps} />)
    expect(screen.getByRole('table')).toBeInTheDocument()
  })
})
