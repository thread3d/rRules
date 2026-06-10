// Simplified Select component using native HTML select

export interface SelectProps {
  value: string
  onChange: (value: string) => void
  children: React.ReactNode
  className?: string
}

export function Select({ value, onChange, children, className }: SelectProps) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className={`px-4 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-ring ${className}`}
    >
      {children}
    </select>
  )
}

export interface SelectContentProps {
  children: React.ReactNode
}

export function SelectContent({ children }: SelectContentProps) {
  return <>{children}</>
}

export interface SelectItemProps {
  value: string
  children: React.ReactNode
}

export function SelectItem({ value, children }: SelectItemProps) {
  return <option value={value}>{children}</option>
}

export interface SelectTriggerProps {
  children: React.ReactNode
  className?: string
}

export function SelectTrigger({ children, className }: SelectTriggerProps) {
  return (
    <div className={`px-4 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-ring ${className}`}>
      {children}
    </div>
  )
}

export interface SelectValueProps {
  placeholder?: string
}

export function SelectValue({ placeholder }: SelectValueProps) {
  return <>{placeholder}</>
}
