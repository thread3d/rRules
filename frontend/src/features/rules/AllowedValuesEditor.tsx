// AllowedValuesEditor — inline text inputs for enumeration items.
// Used by PRIORITY and OUTPUT ORDER hit policies which require an ordered
// list of allowed values on output columns.

import { useCallback } from 'react'

export interface AllowedValueItem {
  id: string // unique within this editor instance
  text: string // display value (e.g., "Low", "Medium", "High")
}

export interface AllowedValuesEditorProps {
  items: AllowedValueItem[]
  onChange: (items: AllowedValueItem[]) => void
  readonly?: boolean
}

function generateId(): string {
  return crypto.randomUUID?.() ?? `item-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

export function AllowedValuesEditor({ items, onChange, readonly }: AllowedValuesEditorProps) {
  const handleTextChange = useCallback(
    (index: number, newText: string) => {
      const next = items.map((item, i) =>
        i === index ? { ...item, text: newText } : item,
      )
      onChange(next)
    },
    [items, onChange],
  )

  const handleAdd = useCallback(() => {
    onChange([...items, { id: generateId(), text: '' }])
  }, [items, onChange])

  const handleRemove = useCallback(
    (index: number) => {
      onChange(items.filter((_, i) => i !== index))
    },
    [items, onChange],
  )

  return (
    <div className="space-y-1">
      {items.map((item, index) => (
        <div key={item.id} className="flex items-center gap-1">
          <input
            type="text"
            value={item.text}
            onChange={(e) => handleTextChange(index, e.target.value)}
            readOnly={readonly}
            className={`flex-1 px-2 py-1 border rounded text-sm ${readonly ? 'bg-gray-50 dark:bg-gray-800 cursor-default' : ''}`}
            placeholder={`Item ${index + 1}`}
          />
          {!readonly && (
            <button
              type="button"
              onClick={() => handleRemove(index)}
              className="px-1.5 py-0.5 text-red-600 hover:text-red-800 text-sm font-bold"
              aria-label={`Remove item ${index + 1}`}
              title="Remove"
            >
              ×
            </button>
          )}
        </div>
      ))}

      {!readonly && (
        <button
          type="button"
          onClick={handleAdd}
          className="text-sm text-blue-600 hover:text-blue-800 font-medium"
        >
          + Add Item
        </button>
      )}
    </div>
  )
}

export default AllowedValuesEditor
