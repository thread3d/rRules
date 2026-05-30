// Enterprise-grade decision table with ag-Grid

import { AgGridReact } from 'ag-grid-react';
import { GridOptions, ColDef, RowNode, CellEditingStartedEvent, CellEditingStoppedEvent } from 'ag-grid-community';
import { useState, useCallback, useRef, useEffect } from 'react';

/**
 * Decision table row structure
 */
export interface DecisionTableRow {
  id: string;
  conditions: Record<string, string>;
  actions: Record<string, string>;
  enabled: boolean;
}

/**
 * Props for AgGridTable component
 */
export interface AgGridTableProps {
  rowData: DecisionTableRow[];
  onRowDataChange?: (rows: DecisionTableRow[]) => void;
  columnDefs?: ColDef[];
  onDragEnd?: (fromIndex: number, toIndex: number) => void;
}

/**
 * AgGridTable - Enterprise-grade decision table component
 */
export function AgGridTable({
  rowData,
  onRowDataChange,
  columnDefs: externalColumnDefs,
  onDragEnd,
}: AgGridTableProps) {
  const gridRef = useRef<AgGridReact>(null);
  const [columnDefs, setColumnDefs] = useState<ColDef[]>([
    { field: 'id', hide: true },
    { field: 'enabled', checkboxSelection: true, headerCheckboxSelection: true, width: 60 },
    { field: 'conditions', headerName: 'Conditions', editable: true },
    { field: 'actions', headerName: 'Actions', editable: true },
  ]);

  const gridOptions: GridOptions = {
    animateRows: true,
    rowSelection: 'multiple',
    suppressRowClickSelection: false,
    defaultColDef: {
      editable: true,
      filter: true,
      sortable: true,
    },
    theme: 'ag-theme-alpine',
  };

  // Default columns with dropdown and formula types
  const defaultColumnDefs = [
    {
      headerName: 'Condition',
      field: 'condition',
      cellEditor: 'agSelectCellEditor',
      cellEditorParams: { values: ['>', '<', '==', '!=', '>=', '<='] },
    },
    {
      headerName: 'Value',
      field: 'value',
      cellEditor: 'agTextCellEditor',
    },
    {
      headerName: 'Action',
      field: 'action',
      cellEditor: 'agLargeTextCellEditor',
      cellEditorParams: { rows: 3, cols: 50 },
    },
  ];

  useEffect(() => {
    if (externalColumnDefs) {
      setColumnDefs(externalColumnDefs);
    } else {
      setColumnDefs(defaultColumnDefs);
    }
  }, [externalColumnDefs]);

  const handleDragEnd = useCallback((params: RowNode) => {
    const oldIndex = params.rowIndex;
    const newIndex = params.rowPinned ? -1 : params.rowIndex;
    onDragEnd?.(oldIndex ?? 0, newIndex ?? 0);
  }, [onDragEnd]);

  // CRUD operations exposed via grid API
  const insertRow = useCallback(() => {
    if (gridRef.current) {
      const api = gridRef.current.api;
      const newRow: DecisionTableRow = {
        id: `row-${Date.now()}`,
        conditions: {},
        actions: {},
        enabled: true,
      };
      api.applyTransaction({ add: [newRow] });
    }
  }, []);

  const deleteSelectedRows = useCallback(() => {
    if (gridRef.current) {
      const api = gridRef.current.api;
      const selectedNodes = api.getSelectedNodes();
      if (selectedNodes.length > 0) {
        if (confirm(`Delete ${selectedNodes.length} row(s)?`)) {
          api.applyTransaction({ remove: selectedNodes });
        }
      }
    }
  }, []);

  const duplicateRow = useCallback((rowIndex: number) => {
    if (gridRef.current) {
      const api = gridRef.current.api;
      const rowNode = api.getDisplayedRowAtIndex(rowIndex);
      if (rowNode) {
        const newRow = { ...rowNode.data, id: `row-${Date.now()}` };
        api.applyTransaction({ addAfter: [newRow], addIndex: rowIndex + 1 });
      }
    }
  }, []);

  const moveRowUp = useCallback((index: number) => {
    if (gridRef.current && index > 0) {
      const api = gridRef.current.api;
      const rowNode = api.getDisplayedRowAtIndex(index);
      if (rowNode) {
        api.applyTransaction({ move: [rowNode.id], toIndex: index - 1 });
      }
    }
  }, []);

  const moveRowDown = useCallback((index: number) => {
    if (gridRef.current) {
      const api = gridRef.current.api;
      const totalRows = api.getDisplayedRowCount();
      if (index < totalRows - 1) {
        const rowNode = api.getDisplayedRowAtIndex(index);
        if (rowNode) {
          api.applyTransaction({ move: [rowNode.id], toIndex: index + 1 });
        }
      }
    }
  }, []);

  // Get current grid data
  const getCurrentRows = useCallback((): DecisionTableRow[] => {
    if (gridRef.current) {
      const api = gridRef.current.api;
      const rows: DecisionTableRow[] = [];
      api.forEachNode((node) => rows.push(node.data));
      return rows;
    }
    return [];
  }, []);

  // Expose methods via ref
  useEffect(() => {
    if (gridRef.current && onRowDataChange) {
      const intervalId = setInterval(() => {
        const currentRows = getCurrentRows();
        if (JSON.stringify(currentRows) !== JSON.stringify(rowData)) {
          onRowDataChange(currentRows);
        }
      }, 500);
      return () => clearInterval(intervalId);
    }
  }, [onRowDataChange, rowData, getCurrentRows]);

  return (
    <div className="ag-theme-alpine" style={{ height: '600px', width: '100%' }}>
      <AgGridReact
        ref={gridRef}
        rowData={rowData}
        columnDefs={columnDefs}
        gridOptions={gridOptions}
        onDragEnd={handleDragEnd}
      />
    </div>
  );
}

export default AgGridTable;
