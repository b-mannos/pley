import { useState, type DragEvent } from "react";

/** `ids` with the one at `from` moved to `to` (unchanged when `from` is out of range). */
export function moved<T>(ids: readonly T[], from: number, to: number): T[] {
  const next = ids.slice();
  if (from < 0 || from >= next.length) return next;
  const [id] = next.splice(from, 1) as [T];
  next.splice(to, 0, id);
  return next;
}

/**
 * Drag-and-drop reordering of a list's rows (HTML5 drag and drop; rows also
 * offer move up / down buttons for keyboard and touch).
 */
export function useDragReorder(onMove: (from: number, to: number) => void) {
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null);

  const rowProps = (index: number) => ({
    draggable: true,
    onDragStart: (e: DragEvent<HTMLElement>) => {
      e.dataTransfer.effectAllowed = "move";
      // Firefox only starts a drag that carries data.
      e.dataTransfer.setData("text/plain", String(index));
      setDrag({ from: index, over: index });
    },
    onDragOver: (e: DragEvent<HTMLElement>) => {
      if (!drag) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (drag.over !== index) setDrag({ from: drag.from, over: index });
    },
    onDrop: (e: DragEvent<HTMLElement>) => {
      e.preventDefault();
      if (drag && drag.from !== index) onMove(drag.from, index);
      setDrag(null);
    },
    onDragEnd: () => setDrag(null),
  });

  /** Where a dragged row would land, relative to row `index`. */
  const dropMark = (index: number): "before" | "after" | null => {
    if (!drag || drag.over !== index || drag.from === index) return null;
    return drag.from > index ? "before" : "after";
  };

  return { dragging: drag?.from ?? null, rowProps, dropMark };
}
