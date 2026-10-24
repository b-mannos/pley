import { act, renderHook } from "@testing-library/react";
import type { DragEvent } from "react";
import { describe, expect, it, vi } from "vitest";
import { moved, useDragReorder } from "./reorder";

describe("moved", () => {
  it("moves an entry to its new place", () => {
    expect(moved(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moved(["a", "b", "c", "d"], 3, 0)).toEqual(["d", "a", "b", "c"]);
    expect(moved(["a", "b", "c"], 1, 1)).toEqual(["a", "b", "c"]);
  });

  it("returns a copy and ignores out of range sources", () => {
    const ids = ["a", "b"];
    expect(moved(ids, 5, 0)).toEqual(["a", "b"]);
    expect(moved(ids, -1, 0)).not.toBe(ids);
  });
});

/** A drag event on `target`, handled by `row` (the element the handlers are on). */
function dragEvent({
  preventDefault = vi.fn(),
  stopPropagation = vi.fn(),
  setDragImage = vi.fn(),
  target = null as Element | null,
  row = document.createElement("li"),
} = {}) {
  return {
    preventDefault,
    stopPropagation,
    target: target ?? row,
    currentTarget: row,
    clientX: 0,
    clientY: 0,
    dataTransfer: { effectAllowed: "", dropEffect: "", setData: vi.fn(), setDragImage },
  } as unknown as DragEvent<HTMLElement>;
}

const ROWS = ["a", "b", "c", "d"];

describe("useDragReorder", () => {
  it("marks where the row would land and moves it on drop", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDragReorder(ROWS, onMove));

    act(() => result.current.rowProps(0).onDragStart(dragEvent()));
    expect(result.current.dragging).toBe(0);
    expect(result.current.dropMark(0)).toBeNull();

    act(() => result.current.rowProps(2).onDragOver(dragEvent()));
    expect(result.current.dropMark(2)).toBe("after");
    expect(result.current.dropMark(1)).toBeNull();

    act(() => result.current.rowProps(2).onDrop(dragEvent()));
    expect(onMove).toHaveBeenCalledWith(0, 2);
    expect(result.current.dragging).toBeNull();
  });

  it("marks rows above the dragged one as before", () => {
    const { result } = renderHook(() => useDragReorder(ROWS, vi.fn()));
    act(() => result.current.rowProps(3).onDragStart(dragEvent()));
    act(() => result.current.rowProps(1).onDragOver(dragEvent()));
    expect(result.current.dropMark(1)).toBe("before");
  });

  it("does nothing when dropped in place or when the drag ends", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDragReorder(ROWS, onMove));
    act(() => result.current.rowProps(1).onDragStart(dragEvent()));
    act(() => result.current.rowProps(1).onDrop(dragEvent()));
    act(() => result.current.rowProps(0).onDragStart(dragEvent()));
    act(() => result.current.rowProps(0).onDragEnd());
    expect(onMove).not.toHaveBeenCalled();
    expect(result.current.dragging).toBeNull();
  });

  it("ignores drags that didn't start on a row", () => {
    const { result } = renderHook(() => useDragReorder(ROWS, vi.fn()));
    const preventDefault = vi.fn();
    act(() => result.current.rowProps(1).onDragOver(dragEvent({ preventDefault })));
    expect(preventDefault).not.toHaveBeenCalled();
    act(() => result.current.listProps.onDragOver(dragEvent({ preventDefault })));
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("follows the row picked up when the list changes during the drag", () => {
    const onMove = vi.fn();
    const { result, rerender } = renderHook(({ rows }) => useDragReorder(rows, onMove), { initialProps: { rows: ROWS } });
    act(() => result.current.rowProps(1).onDragStart(dragEvent()));

    // "a" goes away: "b" is first now, and still the one dragged.
    rerender({ rows: ["b", "c", "d"] });
    expect(result.current.dragging).toBe(0);
    act(() => result.current.rowProps(2).onDragOver(dragEvent()));
    act(() => result.current.rowProps(2).onDrop(dragEvent()));
    expect(onMove).toHaveBeenCalledWith(0, 2);

    // Gone during the drag: nothing moves.
    act(() => result.current.rowProps(0).onDragStart(dragEvent()));
    rerender({ rows: ["c", "d"] });
    expect(result.current.dragging).toBeNull();
    act(() => result.current.rowProps(1).onDrop(dragEvent()));
    expect(onMove).toHaveBeenCalledTimes(1);
  });

  it("with a handle, picks a row up only by it, with the whole row as the image", () => {
    const { result } = renderHook(() => useDragReorder(ROWS, vi.fn()));
    expect(result.current.rowProps(0, { handle: true })).not.toHaveProperty("draggable");

    const row = document.createElement("li");
    const handle = row.appendChild(document.createElement("span"));
    handle.dataset.dragHandle = "";
    const text = row.appendChild(document.createElement("p"));

    act(() => result.current.rowProps(0, { handle: true }).onDragStart(dragEvent({ target: text, row })));
    expect(result.current.dragging).toBeNull();

    const setDragImage = vi.fn();
    act(() => result.current.rowProps(0, { handle: true }).onDragStart(dragEvent({ target: handle, row, setDragImage })));
    expect(result.current.dragging).toBe(0);
    expect(setDragImage).toHaveBeenCalledWith(row, 0, 0);
  });

  it("takes a drop between rows where the mark is, once", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDragReorder(ROWS, onMove));
    act(() => result.current.rowProps(0).onDragStart(dragEvent()));
    act(() => result.current.rowProps(2).onDragOver(dragEvent()));
    const preventDefault = vi.fn();
    act(() => result.current.listProps.onDragOver(dragEvent({ preventDefault })));
    expect(preventDefault).toHaveBeenCalled();
    act(() => result.current.listProps.onDrop(dragEvent()));
    expect(onMove).toHaveBeenCalledWith(0, 2);

    // A drop on a row doesn't reach the list.
    act(() => result.current.rowProps(3).onDragStart(dragEvent()));
    const stopPropagation = vi.fn();
    act(() => result.current.rowProps(1).onDrop(dragEvent({ stopPropagation })));
    expect(stopPropagation).toHaveBeenCalled();
    expect(onMove).toHaveBeenLastCalledWith(3, 1);
  });
});
