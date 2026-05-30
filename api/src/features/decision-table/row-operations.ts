// Row management functionality with drag-and-drop reordering

import { useState, useCallback } from 'react';
import { v4 as uuidv4 } from 'uuid';

/**
 * Decision table row type
 */
export interface DecisionTableRow {
  id: string;
  conditions: Record<string, string>;
  actions: Record<string, string>;
  enabled: boolean;
}

/**
 * Hook for managing decision table rows with CRUD and drag-and-drop
 */
export function useDecisionTableRows(initialRows: DecisionTableRow[] = []) {
  const [rows, setRows] = useState<DecisionTableRow[]>(initialRows);

  // Add a new row at the end or at specific index
  const addRow = useCallback((rowData?: Partial<DecisionTableRow>) => {
    setRows((prev) => [
      ...prev,
      {
        id: rowData?.id || uuidv4(),
        conditions: rowData?.conditions || {},
        actions: rowData?.actions || {},
        enabled: rowData?.enabled ?? true,
      },
    ]);
  }, []);

  // Remove a row by index
  const removeRow = useCallback((index: number) => {
    setRows((prev) => prev.filter((_, i) => i !== index));
  }, []);

  // Update a row at specific index
  const updateRow = useCallback(
    (index: number, rowData: Partial<DecisionTableRow>) => {
      setRows((prev) =>
        prev.map((row, i) =>
          i === index ? { ...row, ...rowData } : row
        )
      );
    },
    []
  );

  // Reorder rows array
  const reorderRows = useCallback(
    (fromIndex: number, toIndex: number) => {
      setRows((prev) => {
        const result = Array.from(prev);
        const [removed] = result.splice(fromIndex, 1);
        result.splice(toIndex, 0, removed);
        return result;
      });
    },
    []
  );

  // Drag-and-drop handlers using HTML5 API
  const useDragAndDrop = useCallback(() => {
    const [draggedRowIndex, setDraggedRowIndex] = useState<number | null>(null);

    const handleDragStart = useCallback((index: number) => {
      setDraggedRowIndex(index);
    }, []);

    const handleDragOver = useCallback((e: React.DragEvent) => {
      e.preventDefault();
    }, []);

    const handleDrop = useCallback(
      (targetIndex: number) => {
        if (draggedRowIndex !== null && draggedRowIndex !== targetIndex) {
          reorderRows(draggedRowIndex, targetIndex);
        }
        setDraggedRowIndex(null);
      },
      [draggedRowIndex, reorderRows]
    );

    const handleDragEnd = useCallback(() => {
      setDraggedRowIndex(null);
    }, []);

    return { handleDragStart, handleDragOver, handleDrop, handleDragEnd };
  }, [reorderRows]);

  // Move row up/down (keyboard navigation)
  const moveRowUp = useCallback(
    (index: number) => {
      if (index > 0) {
        reorderRows(index, index - 1);
      }
    },
    [reorderRows]
  );

  const moveRowDown = useCallback(
    (index: number) => {
      setRows((prev) => {
        if (index >= prev.length - 1) return prev;
        return reorderRows(index, index + 1);
      });
    },
    [reorderRows]
  );

  // Duplicate a row
  const duplicateRow = useCallback(
    (id: string) => {
      setRows((prev) => {
        const rowIndex = prev.findIndex((r) => r.id === id);
        if (rowIndex === -1) return prev;
        const original = prev[rowIndex];
        const duplicate = {
          ...original,
          id: uuidv4(),
          conditions: { ...original.conditions },
          actions: { ...original.actions },
        };
        return [...prev.slice(0, rowIndex + 1), duplicate, ...prev.slice(rowIndex + 1)];
      });
    },
    []
  );

  // Insert row at specific position
  const insertRowAt = useCallback(
    (index: number, rowData?: Partial<DecisionTableRow>) => {
      setRows((prev) => {
        const newRow: DecisionTableRow = {
          id: rowData?.id || uuidv4(),
          conditions: rowData?.conditions || {},
          actions: rowData?.actions || {},
          enabled: rowData?.enabled ?? true,
        };
        return [...prev.slice(0, index), newRow, ...prev.slice(index)];
      });
    },
    []
  );

  // Get row number for display (1-indexed)
  const getRowNumber = useCallback((index: number) => index + 1, []);

  return {
    rows,
    addRow,
    removeRow,
    updateRow,
    reorderRows,
    useDragAndDrop,
    moveRowUp,
    moveRowDown,
    duplicateRow,
    insertRowAt,
    getRowNumber,
  };
}

export { moveRowUp, moveRowDown, duplicateRow };
