// Complete decision table UI with toolbar and validation indicators

import React, { useState, useCallback } from 'react';
import { AgGridReact } from 'ag-grid-react';
import { ColDef } from 'ag-grid-community';

import { AgGridTable } from './ag-grid-table';
import { MixedCellEditor } from './cell-editors';
import { useDecisionTableRows, DecisionTableRow } from './row-operations';

/**
 * Props for DecisionTableView
 */
export interface DecisionTableViewProps {
  initialRules?: Rule[];
  onSave?: (rules: Rule[]) => void;
}

/**
 * Rule type for conversion between grid and rule formats
 */
export interface Rule {
  id: string;
  name?: string;
  condition: string;
  action: string;
  enabled: boolean;
}

/**
 * Convert rules to grid row format
 */
export function convertRulesToGrid(rules: Rule[]): DecisionTableRow[] {
  return rules.map((rule) => ({
    id: rule.id,
    conditions: { default: rule.condition },
    actions: { default: rule.action },
    enabled: rule.enabled,
  }));
}

/**
 * Convert grid rows back to rule objects
 */
export function convertGridToRules(rows: DecisionTableRow[]): Rule[] {
  return rows.map((row) => ({
    id: row.id,
    condition: Object.values(row.conditions).join(' ') || '',
    action: Object.values(row.actions).join(' ') || '',
    enabled: row.enabled,
  }));
}

/**
 * DecisionTableView - Complete decision table UI
 */
export function DecisionTableView({
  initialRules = [],
  onSave,
}: DecisionTableViewProps) {
  const [rules, setRules] = useState<Rule[]>(initialRules);
  const { rows, addRow, removeRow, duplicateRow, moveRowUp, moveRowDown } =
    useDecisionTableRows(convertRulesToGrid(initialRules));

  const [validationStatus, setValidationStatus] = useState<'valid' | 'invalid' | 'pending'>('pending');
  const [errorCount, setErrorCount] = useState(0);

  // Handle row data changes from grid
  const handleRowDataChange = useCallback(
    (gridRows: DecisionTableRow[]) => {
      const convertedRules = convertGridToRules(gridRows);
      setRules(convertedRules);

      if (onSave) {
        onSave(convertedRules);
      }
    },
    [onSave]
  );

  // Validation handler
  const validateTable = useCallback(() => {
    let errors = 0;
    rows.forEach((row, index) => {
      const conditionCount = Object.keys(row.conditions).length;
      if (conditionCount === 0) {
        errors++;
      }
    });
    setValidationStatus(errors > 0 ? 'invalid' : 'valid');
    setErrorCount(errors);
  }, [rows]);

  // Export to JSON
  const handleExport = useCallback(() => {
    const json = JSON.stringify(rules, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `decision-table-${new Date().toISOString()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [rules]);

  // Import from JSON
  const handleImport = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (event) => {
      try {
        const importedRules: Rule[] = JSON.parse(event.target?.result as string);
        setRules(importedRules);
        handleRowDataChange(convertRulesToGrid(importedRules));
      } catch (error) {
        console.error('Failed to import rules:', error);
      }
    };
    reader.readAsText(file);
  }, [handleRowDataChange]);

  // Toolbar controls
  const toolbar = (
    <div className="decision-table-toolbar">
      <button onClick={addRow} disabled={false}>
        + Add Row
      </button>
      <button onClick={() => rows.forEach((_, i) => removeRow(i))} disabled={rows.length === 0}>
        - Delete All
      </button>
      <div className="toolbar-spacer" />
      <button onClick={handleExport}>Export JSON</button>
      <label className="import-label">
        Import JSON
        <input type="file" accept=".json" onChange={handleImport} style={{ display: 'none' }} />
      </label>
      <button onClick={validateTable}>Validate</button>
    </div>
  );

  // Row actions menu (per row)
  const renderRowActions = useCallback((params: any) => {
    return (
      <div className="row-actions">
        <button
          type="button"
          onClick={() => moveRowUp(params.rowIndex)}
          disabled={params.rowIndex === 0}
        >
          ^
        </button>
        <button
          type="button"
          onClick={() => moveRowDown(params.rowIndex)}
          disabled={params.rowIndex === rows.length - 1}
        >
          v
        </button>
        <button type="button" onClick={() => duplicateRow(params.data.id)}>
          Duplicate
        </button>
        <button
          type="button"
          onClick={() => removeRow(params.rowIndex)}
          style={{ color: 'red' }}
        >
          Delete
        </button>
      </div>
    );
  }, [moveRowUp, moveRowDown, duplicateRow, removeRow, rows.length]);

  // Column definitions with row number and actions
  const columnDefs: ColDef[] = [
    { headerName: '#', valueGetter: 'node.rowIndex + 1', width: 50 },
    {
      headerName: 'Conditions',
      field: 'condition',
      editable: true,
      cellEditor: 'agSelectCellEditor',
      cellEditorParams: { values: ['>', '<', '==', '!='] },
    },
    { headerName: 'Actions', field: 'action', editable: true, flex: 1 },
    {
      headerName: 'Actions',
      cellRenderer: renderRowActions,
      width: 150,
      sortable: false,
      filter: false,
    },
  ];

  // Validation feedback
  const validationIndicator = (
    <div className={`validation-indicator ${validationStatus}`}>
      {validationStatus === 'valid' && <span>✓ Table valid</span>}
      {validationStatus === 'invalid' && <span>✗ {errorCount} issues found</span>}
      {validationStatus === 'pending' && <span>Click Validate to check</span>}
    </div>
  );

  return (
    <div className="decision-table-view">
      {/* Toolbar */}
      {toolbar}

      {/* Grid */}
      <AgGridTable
        rowData={rows}
        onRowDataChange={handleRowDataChange}
        columnDefs={columnDefs}
      />

      {/* Validation feedback */}
      {validationIndicator}
    </div>
  );
}

export default DecisionTableView;
