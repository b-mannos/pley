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

function dragEvent(preventDefault = vi.fn()) {
  return {
    preventDefault,
    dataTransfer: { effectAllowed: "", dropEffect: "", setData: vi.fn() },
  } as unknown as DragEvent<HTMLElement>;
}

describe("useDragReorder", () => {
  it("marks where the row would land and moves it on drop", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDragReorder(onMove));

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
    const { result } = renderHook(() => useDragReorder(vi.fn()));
    act(() => result.current.rowProps(3).onDragStart(dragEvent()));
    act(() => result.current.rowProps(1).onDragOver(dragEvent()));
    expect(result.current.dropMark(1)).toBe("before");
  });

  it("does nothing when dropped in place or when the drag ends", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDragReorder(onMove));
    act(() => result.current.rowProps(1).onDragStart(dragEvent()));
    act(() => result.current.rowProps(1).onDrop(dragEvent()));
    act(() => result.current.rowProps(0).onDragStart(dragEvent()));
    act(() => result.current.rowProps(0).onDragEnd());
    expect(onMove).not.toHaveBeenCalled();
    expect(result.current.dragging).toBeNull();
  });

  it("ignores drags that didn't start on a row", () => {
    const { result } = renderHook(() => useDragReorder(vi.fn()));
    const preventDefault = vi.fn();
    act(() => result.current.rowProps(1).onDragOver(dragEvent(preventDefault)));
    expect(preventDefault).not.toHaveBeenCalled();
  });
});
