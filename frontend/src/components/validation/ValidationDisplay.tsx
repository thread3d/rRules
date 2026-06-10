import { useMemo } from 'react'

export interface ValidationError {
  id: string
  message: string
  field?: string
}

export interface ValidationWarning {
  id: string
  message: string
  field?: string
}

export interface ValidationDisplayProps {
  errors: ValidationError[]
  warnings: ValidationWarning[]
  onDismissError?: (id: string) => void
  onDismissWarning?: (id: string) => void
}

export function ValidationDisplay({
  errors,
  warnings,
  onDismissError,
  onDismissWarning,
}: ValidationDisplayProps) {
  if (!errors.length && !warnings.length) {
    return null
  }

  const showErrorIcons = useMemo(() => {
    return (
      <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
      </svg>
    )
  }, [])

  const showWarningIcons = useMemo(() => {
    return (
      <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v4a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
      </svg>
    )
  }, [])

  return (
    <div className="space-y-2">
      {/* Error alerts */}
      {errors.map((error) => (
        <div
          key={error.id}
          className="flex items-center gap-2 px-3 py-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded"
        >
          {showErrorIcons}
          <span className="text-sm font-medium text-red-700 dark:text-red-300 flex-1">
            {error.message}
          </span>
          {onDismissError && (
            <button
              onClick={() => onDismissError(error.id)}
              className="text-gray-400 hover:text-gray-600"
            >
              <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                <path
                  fillRule="evenodd"
                  d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
                  clipRule="evenodd"
                />
              </svg>
            </button>
          )}
        </div>
      ))}

      {/* Warning alerts */}
      {warnings.map((warning) => (
        <div
          key={warning.id}
          className="flex items-center gap-2 px-3 py-2 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded"
        >
          {showWarningIcons}
          <span className="text-sm text-yellow-700 dark:text-yellow-300 flex-1">
            {warning.message}
          </span>
          {onDismissWarning && (
            <button
              onClick={() => onDismissWarning(warning.id)}
              className="text-gray-400 hover:text-gray-600"
            >
              <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                <path
                  fillRule="evenodd"
                  d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
                  clipRule="evenodd"
                />
              </svg>
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

export default ValidationDisplay
