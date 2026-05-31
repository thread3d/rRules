// Error reporting UI for validation issues

import React, { useState, useEffect, useCallback } from 'react';
import {
  ValidationResult,
  ValidationError,
  ValidationWarning,
  ValidationInfo,
  SchemaField,
} from '../features/validation/inline-validator';

/**
 * Props for ValidationError component
 */
interface ValidationErrorProps {
  error: ValidationError | ValidationWarning;
  onJumpTo?: (position: { line: number; column: number }) => void;
}

/**
 * ValidationError - Display a single validation issue with severity indicator
 */
export function ValidationError({ error, onJumpTo }: ValidationErrorProps) {
  const [isHovered, setIsHovered] = useState(false);

  const getSeverityColor = () => {
    switch (error.severity) {
      case 'error':
        return 'red';
      case 'warning':
        return '#f59e0b'; // amber-500
      case 'info':
        return '#3b82f6'; // blue-500
      default:
        return '#6b7280'; // gray-500
    }
  };

  const getSeverityIcon = () => {
    switch (error.severity) {
      case 'error':
        return (
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#ef4444">
            <circle cx="12" cy="12" r="10" strokeWidth="2" />
            <path d="M8 8l8 8M16 8l-8 8" strokeWidth="2" strokeLinecap="round" />
          </svg>
        );
      case 'warning':
        return (
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#f59e0b">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" strokeWidth="2" />
            <line x1="12" y1="9" x2="12" y2="13" strokeWidth="2" strokeLinecap="round" />
            <line x1="12" y1="17" x2="12.01" y2="17" strokeWidth="2" strokeLinecap="round" />
          </svg>
        );
      case 'info':
        return (
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#3b82f6">
            <circle cx="12" cy="12" r="10" strokeWidth="2" />
            <line x1="12" y1="16" x2="12" y2="12" strokeWidth="2" strokeLinecap="round" />
            <line x1="12" y1="8" x2="12.01" y2="8" strokeWidth="2" strokeLinecap="round" />
          </svg>
        );
      default:
        return null;
    }
  };

  const getSeverityLabel = () => {
    switch (error.severity) {
      case 'error':
        return 'Error';
      case 'warning':
        return 'Warning';
      case 'info':
        return 'Info';
      default:
        return '';
    }
  };

  const handleJumpClick = useCallback(() => {
    if (onJumpTo) {
      onJumpTo(error.position);
    }
  }, [onJumpTo, error.position]);

  return (
    <div
      className="validation-error"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleJumpClick}
    >
      <div className="error-icon">{getSeverityIcon()}</div>
      <div className="error-content">
        <span className={`severity-badge ${error.severity}`}>{getSeverityLabel()}</span>
        <p className="error-message">{error.message}</p>
        {error.hint && isHovered && (
          <p className="error-hint">{error.hint}</p>
        )}
        <span className="error-location">
          Line {error.position.line}, Column {error.position.column}
        </span>
      </div>
    </div>
  );
}

/**
 * Props for ValidationSummaryPanel component
 */
interface ValidationSummaryPanelProps {
  result: ValidationResult;
  onJumpTo?: (position: { line: number; column: number }) => void;
  showOnly?: 'all' | 'errors' | 'warnings';
}

/**
 * ValidationSummaryPanel - Collapsible section showing all validation issues
 */
export function ValidationSummaryPanel({
  result,
  onJumpTo,
  showOnly = 'all',
}: ValidationSummaryPanelProps) {
  const [isExpanded, setIsExpanded] = useState(true);
  const [filter, setFilter] = useState<'all' | 'errors' | 'warnings'>('all');

  // Filter results based on current filter
  const filteredErrors = result.errors.filter((e) =>
    filter === 'all' ? true : e.severity === filter
  );
  const filteredWarnings = result.warnings.filter((w) =>
    filter === 'all' || filter === 'warnings'
  );

  // Combined list with proper ordering (errors first, then warnings)
  const issues = [
    ...filteredErrors.map((e) => ({ ...e, type: 'error' } as const)),
    ...filteredWarnings.map((w) => ({ ...w, type: 'warning' } as const)),
  ];

  const totalIssues = filteredErrors.length + filteredWarnings.length;

  return (
    <div className="validation-summary-panel">
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className="summary-toggle"
      >
        <span className={`issue-count ${totalIssues > 0 ? 'has-issues' : ''}`}>
          {totalIssues === 0 ? 'No Issues' : `${totalIssues} Issue${totalIssues !== 1 ? 's' : ''}`}
        </span>
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          className={`chevron ${isExpanded ? 'expanded' : ''}`}
        >
          <path d="M6 9l6 6 6-6" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {isExpanded && (
        <>
          {/* Filter Tabs */}
          <div className="filter-tabs">
            <button
              type="button"
              onClick={() => setFilter('all')}
              className={`tab-btn ${filter === 'all' ? 'active' : ''}`}
            >
              All
            </button>
            <button
              type="button"
              onClick={() => setFilter('errors')}
              className={`tab-btn ${filter === 'errors' ? 'active' : ''}`}
            >
              Errors ({filteredErrors.length})
            </button>
            <button
              type="button"
              onClick={() => setFilter('warnings')}
              className={`tab-btn ${filter === 'warnings' ? 'active' : ''}`}
            >
              Warnings ({filteredWarnings.length})
            </button>
          </div>

          {/* Issues List */}
          {issues.length > 0 ? (
            <ul className="issues-list">
              {issues.map((issue, index) => (
                <li key={index} className={`issue-item ${issue.type}`}>
                  <ValidationError
                    error={issue}
                    onJumpTo={onJumpTo}
                  />
                </li>
              ))}
            </ul>
          ) : (
            <div className="empty-state">
              <p>Validation passed with no issues!</p>
            </div>
          )}
        </>
      )}
    </div>
  );
}

/**
 * Props for HighlightErrorLine component
 */
interface HighlightErrorLineProps {
  line: number;
  column?: number;
  length?: number;
  severity?: 'error' | 'warning';
  onClick?: () => void;
}

/**
 * HighlightErrorLine - Marks and highlights error regions in the editor
 */
export function HighlightErrorLine({
  line,
  column = 1,
  length = 0,
  severity = 'error',
  onClick,
}: HighlightErrorLineProps) {
  const style: React.CSSProperties = {
    position: 'absolute',
    left: `${column}ch`,
    width: length > 0 ? `${length}ch` : 'auto',
    height: '1.2em',
    backgroundColor:
      severity === 'error'
        ? 'rgba(239, 68, 68, 0.2)'
        : 'rgba(245, 158, 11, 0.2)',
    borderTop:
      severity === 'error' ? '2px solid #ef4444' : '2px solid #f59e0b',
    borderRadius: '2px',
    cursor: onClick ? 'pointer' : 'default',
  };

  return (
    <div
      style={style}
      className="highlight-error-line"
      onClick={(e) => {
        e.stopPropagation();
        onClick?.();
      }}
      title={`Jump to line ${line}`}
    />
  );
}

/**
 * DiagnosticGutter - Shows diagnostic markers in editor gutter
 */
interface DiagnosticGutterProps {
  errors: ValidationError[];
  warnings: ValidationWarning[];
  onJumpTo?: (position: { line: number; column: number }) => void;
}

export function DiagnosticGutter({
  errors,
  warnings,
  onJumpTo,
}: DiagnosticGutterProps) {
  // Group issues by line
  const issuesByLine = new Map<number, Array<{ severity: 'error' | 'warning'; message: string }>>();

  for (const error of errors) {
    const key = error.position.line;
    if (!issuesByLine.has(key)) {
      issuesByLine.set(key, []);
    }
    issuesByLine.get(key)?.push({ severity: 'error', message: error.message });
  }

  for (const warning of warnings) {
    const key = warning.position.line;
    if (!issuesByLine.has(key)) {
      issuesByLine.set(key, []);
    }
    issuesByLine.get(key)?.push({ severity: 'warning', message: warning.message });
  }

  return (
    <div className="diagnostic-gutter">
      {Array.from(issuesByLine.entries()).map(([line, issues]) => (
        <div
          key={line}
          className={`gutter-marker ${issues.some(i => i.severity === 'error') ? 'error' : 'warning'}`}
          onClick={() => {
            const firstIssue = issues[0];
            if (onJumpTo && firstIssue) {
              onJumpTo({ line, column: 1 });
            }
          }}
          title={`${issues.length} issue(s) on line ${line}`}
        >
          {issues.some(i => i.severity === 'error') ? (
            <svg width="12" height="12" viewBox="0 0 24 24">
              <circle cx="12" cy="12" r="8" fill="#ef4444" />
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 24 24">
              <path
                d="M12 2L1 21h22L12 2zm0 3l8.5 16h-17l8.5-16z"
                fill="#f59e0b"
              />
            </svg>
          )}
        </div>
      ))}
    </div>
  );
}

/**
 * Props for InlineValidationStatus component
 */
interface InlineValidationStatusProps {
  errorCount: number;
  warningCount: number;
  onOpenSummary?: () => void;
}

/**
 * InlineValidationStatus - Small status indicator in editor header
 */
export function InlineValidationStatus({
  errorCount,
  warningCount,
  onOpenSummary,
}: InlineValidationStatusProps) {
  const totalIssues = errorCount + warningCount;

  if (totalIssues === 0) {
    return (
      <span className="validation-status valid">
        <svg width="14" height="14" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="10" fill="#22c55e" />
          <path d="M7 12l3 3L17 8" stroke="white" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Valid
      </span>
    );
  }

  return (
    <button type="button" onClick={onOpenSummary} className="validation-status invalid">
      {errorCount > 0 && (
        <span className="error-badge">{errorCount}</span>
      )}
      {warningCount > 0 && (
        <span className="warning-badge">{warningCount}</span>
      )}
      Issues
    </button>
  );
}

/**
 * Validation Context for sharing validation state across components
 */
interface ValidationContextType {
  currentValidation: ValidationResult | null;
  isSaving: boolean;
  lastSaved: Date | null;
  setValidation: (result: ValidationResult) => void;
  startSave: () => void;
  endSave: (success: boolean, result?: ValidationResult) => void;
}

export const ValidationContext = React.createContext<ValidationContextType>({
  currentValidation: null,
  isSaving: false,
  lastSaved: null,
  setValidation: () => {},
  startSave: () => {},
  endSave: () => {},
});

/**
 * Props for ValidationContextProvider
 */
interface ValidationContextProviderProps {
  children: React.ReactNode;
}

/**
 * ValidationContextProvider - Provides validation context to child components
 */
export function ValidationContextProvider({
  children,
}: ValidationContextProviderProps) {
  const [currentValidation, setCurrentValidation] = useState<ValidationResult | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [lastSaved, setLastSaved] = useState<Date | null>(null);

  const setValidation = useCallback((result: ValidationResult) => {
    setCurrentValidation(result);
  }, []);

  const startSave = useCallback(() => {
    setIsSaving(true);
  }, []);

  const endSave = useCallback(
    (success: boolean, result?: ValidationResult) => {
      setIsSaving(false);
      if (success && result) {
        setLastSaved(new Date());
      }
    },
    []
  );

  return (
    <ValidationContext.Provider
      value={{
        currentValidation,
        isSaving,
        lastSaved,
        setValidation,
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
  onSave: (ruleText: string) => Promise<void>;
  ruleText: string;
  schema?: EntitySchema;
  children: React.ReactNode;
}

/**
 * ValidateAndSave - Wrapper that validates before saving
 */
export function ValidateAndSave({
  onSave,
  ruleText,
  schema,
  children,
}: ValidateAndSaveProps) {
  const { setValidation, startSave, endSave } = useValidationContext();
  const [isSaving, setIsSaving] = useState(false);

  const handleValidateAndSave = useCallback(
    async (event?: React.MouseEvent<HTMLButtonElement>) => {
      if (event) {
        event.stopPropagation();
        event.preventDefault();
      }

      startSave();

      // Run semantic validation
      const result = validateRule(ruleText, schema || null, 'full');
      setValidation(result);

      // Only save if no errors
      if (result.errors.length === 0) {
        try {
          await onSave(ruleText);
          endSave(true, result);
        } catch (error) {
          console.error('Failed to save:', error);
          endSave(false);
        }
      } else {
        endSave(false, result);
        // Show modal or alert indicating validation errors
        window.alert(
          `Cannot save: ${result.errors.length} error(s) found. Please fix the issues before saving.`
        );
      }
    },
    [onSave, ruleText, schema, setValidation, startSave, endSave]
  );

  return React.cloneElement(children as React.ReactElement, {
    onClick: handleValidateAndSave,
    disabled: isSaving || (currentValidation?.errors.length || 0) > 0,
  });
}

// Re-export validateRule for convenience
import { validateRule } from '../features/validation/inline-validator';

export default ValidationSummaryPanel;
