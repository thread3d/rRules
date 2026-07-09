import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import { AllowedValuesEditor } from '../AllowedValuesEditor'

describe('AllowedValuesEditor', () => {
  it('renders inline text inputs for enumeration items', () => {
    const items = [
      { id: 'v1', text: 'Low' },
      { id: 'v2', text: 'Medium' },
      { id: 'v3', text: 'High' },
    ]
    const onChange = vi.fn()

    render(<AllowedValuesEditor items={items} onChange={onChange} />)

    expect(screen.getByPlaceholderText('Item 1')).toHaveValue('Low')
    expect(screen.getByPlaceholderText('Item 2')).toHaveValue('Medium')
    expect(screen.getByPlaceholderText('Item 3')).toHaveValue('High')
  })

  it('"Add Item" button appends new empty input', () => {
    const items = [{ id: 'v1', text: 'Low' }]
    const onChange = vi.fn()

    render(<AllowedValuesEditor items={items} onChange={onChange} />)

    fireEvent.click(screen.getByText('+ Add Item'))

    expect(onChange).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({ text: 'Low' }),
        expect.objectContaining({ text: '' }),
      ]),
    )
  })

  it('each item has remove (×) button that deletes from list', () => {
    const items = [
      { id: 'v1', text: 'Low' },
      { id: 'v2', text: 'Medium' },
      { id: 'v3', text: 'High' },
    ]
    const onChange = vi.fn()

    render(<AllowedValuesEditor items={items} onChange={onChange} />)

    // Click the remove button on "Medium" (second item).
    const removeButtons = screen.getAllByLabelText(/Remove/)
    fireEvent.click(removeButtons[1])

    expect(onChange).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({ text: 'Low' }),
        expect.objectContaining({ text: 'High' }),
      ]),
    )
  })

  it('onChange fires on every edit with updated items array', () => {
    const items = [{ id: 'v1', text: 'Low' }]
    const onChange = vi.fn()

    render(<AllowedValuesEditor items={items} onChange={onChange} />)

    const input = screen.getByPlaceholderText('Item 1') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'Changed' } })

    expect(onChange).toHaveBeenCalledWith(
      expect.arrayContaining([expect.objectContaining({ text: 'Changed' })]),
    )
  })

  it('renders read-only display when readonly=true (no edit controls)', () => {
    const items = [{ id: 'v1', text: 'Low' }, { id: 'v2', text: 'High' }]

    render(<AllowedValuesEditor items={items} onChange={() => {}} readonly />)

    // Inputs should be present with the readonly attribute.
    expect(screen.getByDisplayValue('Low')).toHaveAttribute('readonly')
    expect(screen.getByDisplayValue('High')).toHaveAttribute('readonly')
    // No add button or remove buttons in readonly mode.
    expect(screen.queryByText('+ Add Item')).not.toBeInTheDocument()
    expect(screen.queryAllByLabelText(/Remove/)).toHaveLength(0)
  })

  it('renders no inputs when items array is empty', () => {
    const onChange = vi.fn()

    render(<AllowedValuesEditor items={[]} onChange={onChange} />)

    // Only the "Add Item" button should be visible.
    expect(screen.getByText('+ Add Item')).toBeInTheDocument()
    expect(screen.queryByPlaceholderText('Item 1')).not.toBeInTheDocument()
  })
})
