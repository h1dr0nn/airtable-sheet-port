import { useCallback, useEffect, useRef, useState } from "react";

// Per-sheet undo/redo stack for cell edits. The Workbench records every applied
// cell edit; undo rewrites the previous value and redo rewrites the next one,
// both through the same write path but WITHOUT recording a fresh entry (so an
// undo does not itself become undoable). Append-row is intentionally out of
// scope for v1: the backend cannot delete a sheet row, so appends are not
// tracked here.

/** One applied cell edit, enough to replay it forwards or backwards. */
export type CellEdit = {
  rowIndex: number;
  columnId: string;
  prevValue: string;
  nextValue: string;
};

/** Writes a cell value through the mutation path, without recording history. */
export type ApplyCell = (rowIndex: number, columnId: string, value: string) => void;

export type SheetHistory = {
  canUndo: boolean;
  canRedo: boolean;
  /** Records a user-initiated edit; no-op edits (unchanged value) are ignored. */
  record: (edit: CellEdit) => void;
  /** Reverts the most recent edit and moves it onto the redo stack. */
  undo: () => void;
  /** Re-applies the most recently undone edit. */
  redo: () => void;
};

/**
 * Tracks cell-edit history for one sheet, resetting whenever `scopeKey`
 * (itemId + gid) changes. Stacks live in refs so undo/redo never run write side
 * effects inside a state updater; a version counter drives re-renders so the
 * `canUndo` / `canRedo` flags stay in sync with the toolbar buttons.
 */
export function useSheetHistory(scopeKey: string, applyCell: ApplyCell): SheetHistory {
  const undoStackRef = useRef<CellEdit[]>([]);
  const redoStackRef = useRef<CellEdit[]>([]);
  const [, setVersion] = useState(0);
  const bump = useCallback(() => setVersion((value) => value + 1), []);

  // Clearing on scope change keeps history strictly scoped to the active sheet.
  useEffect(() => {
    undoStackRef.current = [];
    redoStackRef.current = [];
    bump();
  }, [scopeKey, bump]);

  const record = useCallback(
    (edit: CellEdit) => {
      if (edit.prevValue === edit.nextValue) {
        return;
      }
      undoStackRef.current = [...undoStackRef.current, edit];
      // A fresh edit invalidates any redo branch, matching editor conventions.
      redoStackRef.current = [];
      bump();
    },
    [bump]
  );

  const undo = useCallback(() => {
    const stack = undoStackRef.current;
    const edit = stack[stack.length - 1];
    if (!edit) {
      return;
    }
    undoStackRef.current = stack.slice(0, -1);
    redoStackRef.current = [...redoStackRef.current, edit];
    applyCell(edit.rowIndex, edit.columnId, edit.prevValue);
    bump();
  }, [applyCell, bump]);

  const redo = useCallback(() => {
    const stack = redoStackRef.current;
    const edit = stack[stack.length - 1];
    if (!edit) {
      return;
    }
    redoStackRef.current = stack.slice(0, -1);
    undoStackRef.current = [...undoStackRef.current, edit];
    applyCell(edit.rowIndex, edit.columnId, edit.nextValue);
    bump();
  }, [applyCell, bump]);

  return {
    canUndo: undoStackRef.current.length > 0,
    canRedo: redoStackRef.current.length > 0,
    record,
    undo,
    redo
  };
}
