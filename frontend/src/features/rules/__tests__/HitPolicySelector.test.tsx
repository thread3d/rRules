import { render, screen } from '@testing-library/react'
import '@testing-library/jest-dom'
import { HitPolicySelector } from '../HitPolicySelector'

describe('HitPolicySelector', () => {
  it('renders without crashing with a valid hit policy value', () => {
    const onChange = vi.fn()
    render(
      <HitPolicySelector value="unique" onChange={onChange} />
    )
    expect(screen.getByRole('combobox')).toBeInTheDocument()
  })
})
