// Cross-component validation integration with shared context

import React, { useState, useCallback, useRef, useEffect } from 'react';
import { EditorState, StateEffect, Prec, Compartment } from '@codemirror/state';
import {
  lintView,
  lintState,
  Diagnostic,
  lintProvider,
} from '@codemirror/lint';
import { KeyBinding, keymap } from '@codemirror/view';
import {
  validateRule as inlineValidate,
  ValidationResult,
  SyntaxValidator,
  SemanticValidator,
  EntitySchema,
  SchemaField,
} from './inline-validator';

/**
 * Validation state interface
 */
interface ValidationState {
  result: ValidationResult;
  isChecking: boolean;
  lastChecked: Date | null;
}

/**
 * Debounced validation helper
 */
function useDebouncedCallback<T extends (...args: any[]) => void>(
  callback: T,
  delay: number
): { callback: (...args: Parameters<T>) => void; cancel: () => void } {
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const debouncedCallback = useCallback(
    (...args: Parameters<T>) => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      timeoutRef.current = window.setTimeout(() => {
        callback(...args);
      }, delay);
    },
    [callback, delay]
  );

  return {
    callback: debouncedCallback,
    cancel: () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    },
  };
}

/**
 * Props for useValidation hook
 */
interface UseValidationProps {
  initialText?: string;
  schema?: EntitySchema;
  debounceDelay?: number;
  onValidationChange?: (result: ValidationResult) => void;
  onSave?: (text: string) => Promise<void>;
}

/**
 * useValidation - Hook for managing validation state
 */
export function useValidation({
  initialText = '',
  schema,
  debounceDelay = 300,
  onValidationChange,
  onSave,
}: UseValidationProps) {
  const [ruleText, setRuleText] = useState(initialText);
  const [validationState, setValidationState] = useState<ValidationState>({
    result: { errors: [], warnings: [], infos: [] },
    isChecking: false,
    lastChecked: null,
  });
  const [isSaving, setIsSaving] = useState(false);

  // Debounced validation callback
  const debouncedValidate = useDebouncedCallback(
    useCallback(
      (text: string) => {
        validate(text);
      },
      [schema]
    ),
    debounceDelay
  );

  // Validate the current rule text
  const validate = useCallback(
    async (text: string, mode: 'syntax' | 'semantic' | 'full' = 'syntax') => {
      if (!text.trim()) {
        setValidationState({
          result: { errors: [], warnings: [], infos: [] },
          isChecking: false,
          lastChecked: new Date(),
        });
        onValidationChange?.({ errors: [], warnings: [], infos: [] });
        return;
      }

      setValidationState((prev) => ({ ...prev, isChecking: true }));

      const result = inlineValidate(text, schema || null, mode);
      setValidationState({
        result,
        isChecking: false,
        lastChecked: new Date(),
      });
      onValidationChange?.(result);
    },
    [schema, onValidationChange]
  );

  // Debounced validation handler
  const handleInputChange = useCallback(
    (newText: string) => {
      setRuleText(newText);
      debouncedValidate.callback(newText);
    },
    [debouncedValidate]
  );

  // Full save with semantic validation
  const validateAndSave = useCallback(async () => {
    setIsSaving(true);

    // Run full semantic validation before saving
    const result = inlineValidate(ruleText, schema || null, 'full');

    if (result.errors.length > 0) {
      setValidationState((prev) => ({
        ...prev,
        isChecking: false,
      }));
      window.alert(
        `Cannot save: ${result.errors.length} error(s) found.\n\n` +
          result.errors.map((e) => `- ${e.message}`).join('\n')
      );
      setIsSaving(false);
      return false;
    }

    try {
      if (onSave) {
        await onSave(ruleText);
        setValidationState({
          result: { ...result, errors: [], warnings: [] },
          isChecking: false,
          lastChecked: new Date(),
        });
        setIsSaving(false);
        return true;
      }
      return true;
    } catch (error) {
      console.error('Save failed:', error);
      setIsSaving(false);
      return false;
    }
  }, [ruleText, schema, onSave]);

  // Save without validation (force save)
  const forceSave = useCallback(async () => {
    setIsSaving(true);
    try {
      if (onSave) {
        await onSave(ruleText);
        setIsSaving(false);
        return true;
      }
      return true;
    } catch (error) {
      console.error('Force save failed:', error);
      setIsSaving(false);
      return false;
    }
  }, [ruleText, onSave]);

  // Reset validation
  const clearValidation = useCallback(() => {
    setValidationState({
      result: { errors: [], warnings: [], infos: [] },
      isChecking: false,
      lastChecked: null,
    });
    onValidationChange?.({ errors: [], warnings: [], infos: [] });
  }, [onValidationChange]);

  // Keyboard shortcut handler
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        validateAndSave();
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'V') {
        e.preventDefault();
        validate(ruleText, 'full');
      }
    },
    [validateAndSave, validate, ruleText]
  );

  // Initialize validation on mount
  useEffect(() => {
    if (initialText) {
      setRuleText(initialText);
      debouncedValidate.callback(initialText);
    }

    return () => {
      debouncedValidate.cancel();
    };
  }, [initialText, debouncedValidate]);

  return {
    ruleText,
    setRuleText: handleInputChange,
    validationState,
    isSaving,
    validate,
    validateAndSave,
    forceSave,
    clearValidation,
    handleKeyDown,
  };
}

/**
 * Props for useCodeMirrorLinter hook
 */
interface UseCodeMirrorLinterProps {
  schema?: EntitySchema;
  debounceDelay?: number;
}

/**
 * useCodeMirrorLinter - Hook to create CodeMirror linter extension
 */
export function useCodeMirrorLinter({
  schema,
  debounceDelay = 300,
}: UseCodeMirrorLinterProps) {
  const syntaxValidatorRef = useRef(new SyntaxValidator());

  useEffect(() => {
    return () => {
      // Cleanup if needed
    };
  }, []);

  // Create the lint provider
  const lintProviderExtension = React.useMemo(() => {
    const createLintProvider = (state: EditorState) => {
      const diagnostics: Diagnostic[] = [];
      const doc = state.doc;

      for (let lineNum = 1; lineNum <= doc.lines; lineNum++) {
        const line = doc.line(lineNum);
        const text = line.text.trim();
        const lineNumber = lineNum - 1;

        // Skip empty lines and comments
        if (!text || text.startsWith('#') || text.startsWith('//')) {
          continue;
        }

        // Check for unmatched braces in placeholders
        const openBraces = (text.match(/\{/g) || []).length;
        const closeBraces = (text.match(/\}/g) || []).length;
        if (openBraces !== closeBraces) {
          diagnostics.push({
            from: line.from,
            to: line.to,
            message: 'Unmatched braces in placeholder syntax',
            severity: 'error',
          });
        }

        // Check for standalone operators
        const operators = ['>', '<', '==', '!=', '>=', '<='];
        for (const op of operators) {
          let index = text.indexOf(op);
          while (index !== -1) {
            const before = text.substring(0, index).trim();
            const after = text.substring(index + op.length).trim();

            if (!before || !after) {
              diagnostics.push({
                from: line.from + index,
                to: line.from + index + op.length,
                message: `Invalid operator usage with '${op}'`,
                severity: 'error',
              });
            }
            index = text.indexOf(op, index + 1);
          }
        }

        // Check IF-THEN structure
        if (text.toUpperCase().startsWith('IF')) {
          const remainingText = doc.toString().substring(line.to).toUpperCase();
          const hasThen = remainingText.includes('\nTHEN');
          if (!hasThen) {
            diagnostics.push({
              from: line.from,
              to: line.to,
              message: 'Missing THEN clause after IF',
              severity: 'warning',
            });
          }
        }
      }

      return diagnostics;
    };

    return lintProvider(createLintProvider);
  }, []);

  // Create semantic validation effect
  const semanticValidationEffect = React.useMemo(() => {
    return StateEffect.define<boolean>({
      map: (value, mapping) => value,
    });
  }, []);

  return {
    lintProviderExtension,
    semanticValidationEffect,
  };
}

/**
 * Props for ValidationIntegration component
 */
interface ValidationIntegrationProps {
  schema?: EntitySchema;
  children: React.ReactNode;
}

/**
 * ValidationIntegration - Context provider and shared validation state
 */
export function ValidationIntegration({
  schema,
  children,
}: ValidationIntegrationProps) {
  const [currentValidation, setCurrentValidation] = useState<ValidationResult | null>(null);
  const [isChecking, setIsChecking] = useState(false);

  // Set validation result
  const setValidationResult = useCallback((result: ValidationResult) => {
    setCurrentValidation(result);
  }, []);

  // Check if rule has critical errors
  const hasCriticalErrors = currentValidation
    ? currentValidation.errors.length > 0
    : false;

  return (
    <ValidationIntegrationContext.Provider
      value={{
        schema,
        currentValidation,
        isChecking,
        setValidationResult,
        setIsChecking,
        hasCriticalErrors,
      }}
    >
      {children}
    </ValidationIntegrationContext.Provider>
  );
}

/**
 * ValidationIntegrationContext - Context for sharing validation state
 */
interface ValidationIntegrationContextType {
  schema?: EntitySchema;
  currentValidation: ValidationResult | null;
  isChecking: boolean;
  setValidationResult: (result: ValidationResult) => void;
  setIsChecking: (checking: boolean) => void;
  hasCriticalErrors: boolean;
}

const ValidationIntegrationContext = React.createContext<ValidationIntegrationContextType>({
  schema: undefined,
  currentValidation: null,
  isChecking: false,
  setValidationResult: () => {},
  setIsChecking: () => {},
  hasCriticalErrors: false,
});

/**
 * useValidationIntegration - Hook to access validation integration context
 */
export function useValidationIntegration() {
  return React.useContext(ValidationIntegrationContext);
}

/**
 * Keyboard shortcuts for validation
 */
const VALIDATION_SHORTCUTS: KeyBinding[] = [
  {
    key: 'Mod-Shift-V',
    run: (view) => {
      // Run full semantic validation
      console.log('Running full semantic validation...');
      return true;
    },
  },
  {
    key: 'Escape',
    run: (view) => {
      // Clear diagnostics or close error panel
      const el = document.querySelector('.validation-summary-panel');
      if (el && el.classList.contains('expanded')) {
        el.classList.remove('expanded');
        return true;
      }
      return false;
    },
  },
];

/**
 * Debounce helper for validation calls
 */
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: number | null = null;

  return (...args: Parameters<T>) => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = window.setTimeout(() => {
      func(...args);
    }, delay);
  };
}

/**
 * Create validation context provider
 */
export function createValidationContext() {
  const ctx = React.createContext<ValidationContextType | null>(null);
  return ctx;
}

/**
 * Props for ValidationContextProvider
 */
interface ValidationContextType {
  schema?: EntitySchema;
  isSaving: boolean;
  lastSaved: Date | null;
  validationResult: ValidationResult | null;
  setValidationResult: (result: ValidationResult) => void;
  startSave: () => void;
  endSave: (success: boolean) => void;
}

/**
 * ValidationContextProvider - Provides shared validation context
 */
export function ValidationContextProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [schema, setSchema] = useState<EntitySchema | undefined>();
  const [isSaving, setIsSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const [validationResult, setValidationResult] = useState<ValidationResult | null>(null);

  const startSave = useCallback(() => {
    setIsSaving(true);
  }, []);

  const endSave = useCallback(
    (success: boolean) => {
      setIsSaving(false);
      if (success) {
        setLastSaved(new Date());
      }
    },
    []
  );

  return (
    <ValidationContext.Provider
      value={{
        schema,
        isSaving,
        lastSaved,
        validationResult,
        setValidationResult,
        startSave,
        endSave,
      }}
    >
      {children}
    </ValidationContext.Provider>
  );
}

/**
 * Hook to use validation context
 */
export function useValidationContext() {
  return React.useContext(ValidationContext);
}

/**
 * Props for validateAndSave wrapper component
 */
interface ValidateAndSaveProps {
  onSave: (text: string) => Promise<void>;
  ruleText: string;
  schema?: EntitySchema;
}

/**
 * validateAndSave - Function that validates before saving
 */
export async function validateAndSave(
  ruleText: string,
  schema: EntitySchema | undefined,
  onSave: (text: string) => Promise<void>
): Promise<{ success: boolean; result?: ValidationResult }> {
  // Run semantic validation
  const result = inlineValidate(ruleText, schema || null, 'full');

  if (result.errors.length > 0) {
    return { success: false, result };
  }

  try {
    await onSave(ruleText);
    return { success: true, result };
  } catch (error) {
    console.error('Save failed:', error);
    return { success: false };
  }
}

// Export main hook
export default useValidation;
