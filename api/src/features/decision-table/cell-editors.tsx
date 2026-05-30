// Specialized cell editors for decision table mixed editing modes

import React, { useState, useEffect } from 'react';
import { ICellEditorProps } from 'ag-grid-community';

/**
 * Props for SimpleDropdownEditor
 */
export interface SimpleDropdownEditorProps {
  values: string[];
  value?: string;
  onChange: (value: string) => void;
}

/**
 * SimpleDropdownEditor - Dropdown cell editor for simple conditions
 */
export function SimpleDropdownEditor({
  values,
  value = '',
  onChange,
}: SimpleDropdownEditorProps) {
  const [selectedValue, setSelectedValue] = useState(value);

  useEffect(() => {
    if (value !== selectedValue) {
      setSelectedValue(value);
    }
  }, [value]);

  return (
    <select
      className="simple-dropdown-editor"
      value={selectedValue}
      onChange={(e) => {
        const newValue = e.target.value;
        setSelectedValue(newValue);
        onChange(newValue);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === 'Escape') {
          // Handle keyboard navigation
        }
      }}
    >
      <option value="">Select...</option>
      {values.map((opt) => (
        <option key={opt} value={opt}>
          {opt}
        </option>
      ))}
    </select>
  );
}

/**
 * Props for FormulaCellEditor
 */
export interface FormulaCellEditorProps {
  value?: string;
  onChange: (value: string) => void;
}

/**
 * FormulaCellEditor - Multi-line textarea for formula input with syntax validation
 */
export function FormulaCellEditor({ value = '', onChange }: FormulaCellEditorProps) {
  const [editValue, setEditValue] = useState(value);
  const [isValid, setIsValid] = useState(true);

  useEffect(() => {
    if (value !== editValue) {
      setEditValue(value);
    }
  }, [value]);

  // Validate formula syntax on blur
  const validateSyntax = useCallback((text: string): boolean => {
    // Simple validation for MVEL-like syntax
    // Check for balanced braces in placeholders
    const openBraces = (text.match(/\{/g) || []).length;
    const closeBraces = (text.match(/\}/g) || []).length;

    if (openBraces !== closeBraces) {
      setIsValid(false);
      return false;
    }

    setIsValid(true);
    return true;
  }, []);

  const handleBlur = () => {
    validateSyntax(editValue);
    onChange(editValue);
  };

  return (
    <div className="formula-cell-editor">
      <textarea
        className={`formula-textarea ${isValid ? '' : 'invalid'}`}
        value={editValue}
        onChange={(e) => setEditValue(e.target.value)}
        onBlur={handleBlur}
        rows={3}
        placeholder="Enter formula (e.g., age > 18 && income < 50k)"
      />
      <span className={`validation-status ${isValid ? 'valid' : 'invalid'}`}>
        {isValid ? (
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="green">
            <path d="M20 6L9 17l-5-5" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        ) : (
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="red">
            <path d="M18 6L6 18M6 6l12 12" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
      </span>
    </div>
  );
}

/**
 * MixedCellEditor - Wrapper that renders appropriate editor based on column type
 */
export function MixedCellEditor({ params }: { params: any }) {
  const [editorType, setEditorType] = useState<'dropdown' | 'formula'>('dropdown');

  useEffect(() => {
    // Detect cell type from column definition metadata
    if (params.colDef.cellEditorParams?.values) {
      setEditorType('dropdown');
    } else {
      setEditorType('formula');
    }
  }, [params.colDef]);

  const handleChange = (value: string) => {
    params.setValue(value);
  };

  return (
    <div className="mixed-cell-editor">
      {editorType === 'dropdown' ? (
        <SimpleDropdownEditor
          values={params.colDef.cellEditorParams?.values || []}
          value={params.value || ''}
          onChange={handleChange}
        />
      ) : (
        <FormulaCellEditor value={params.value || ''} onChange={handleChange} />
      )}
    </div>
  );
}

export default MixedCellEditor;
