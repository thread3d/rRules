import { useState, useEffect } from 'react'
import type { Rule } from '@/types/common'

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

export interface ValidationState {
  hasErrors: boolean
  errors: ValidationError[]
  warnings: ValidationWarning[]
}

export function useRuleValidation(rule?: Partial<Rule>): ValidationState {
  const [validationState, setValidationState] = useState<ValidationState>({
    hasErrors: false,
    errors: [],
    warnings: [],
  })

  useEffect(() => {
    if (!rule) {
      setValidationState({ hasErrors: false, errors: [], warnings: [] })
      return
    }

    const errors: ValidationError[] = []
    const warnings: ValidationWarning[] = []

    // Validate rule name (required)
    if (!rule.name || rule.name.trim() === '') {
      errors.push({
        id: 'rule-name',
        message: 'Rule name is required',
        field: 'name',
      })
    }

    // Validate hit policy
    if (!rule.hitPolicy) {
      errors.push({
        id: 'hit-policy',
        message: 'Hit policy must be selected',
        field: 'hitPolicy',
      })
    }

    // Validate conditions (at least one condition required for most policies)
    if (!rule.conditions || rule.conditions.length === 0) {
      warnings.push({
        id: 'no-conditions',
        message: 'No conditions defined - rule will fire on all inputs',
        field: 'conditions',
      })
    }

    // Validate actions
    if (!rule.actions || rule.actions.length === 0) {
      errors.push({
        id: 'no-actions',
        message: 'At least one action is required',
        field: 'actions',
      })
    }

    setValidationState({
      hasErrors: errors.length > 0,
      errors,
      warnings,
    })
  }, [rule])

  return validationState
}

export default useRuleValidation
