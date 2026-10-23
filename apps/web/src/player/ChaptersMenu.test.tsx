import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Chapter } from "@/api/types";
import { makeChapter } from "@/test/fixtures";
import { chapterIndexAt, ChaptersMenu, chapterTitle } from "./ChaptersMenu";

const chapters = [makeChapter(0, "Opening"), makeChapter(60_000, "  "), makeChapter(120_000, "Finale")];

describe("chapter helpers", () => {
  it("names untitled chapters by number", () => {
    expect(chapterTitle(makeChapter(0, "Opening"), 0)).toBe("Opening");
    expect(chapterTitle(makeChapter(0, "  "), 1)).toBe("Chapter 2");
    expect(chapterTitle(makeChapter(0, null), 4)).toBe("Chapter 5");
  });

  it("finds the chapter playing at a time", () => {
    expect(chapterIndexAt(chapters, 0)).toBe(0);
    expect(chapterIndexAt(chapters, 59_999)).toBe(0);
    expect(chapterIndexAt(chapters, 60_000)).toBe(1);
    expect(chapterIndexAt(chapters, 3_600_000)).toBe(2);
    expect(chapterIndexAt([makeChapter(5_000)], 1_000)).toBe(-1);
    expect(chapterIndexAt([], 1_000)).toBe(-1);
  });
});

function Menu({ list = chapters, current = 0, onSeek = vi.fn() }: { list?: Chapter[]; current?: number; onSeek?: (s: number) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <p>outside</p>
      <ChaptersMenu chapters={list} current={current} open={open} setOpen={setOpen} onSeek={onSeek} />
    </div>
  );
}

describe("ChaptersMenu", () => {
  it("lists the chapters and seeks to the one picked", async () => {
    const user = userEvent.setup();
    const onSeek = vi.fn();
    render(<Menu current={75} onSeek={onSeek} />);

    await user.click(screen.getByRole("button", { name: "Chapters" }));
    expect(screen.getByRole("button", { name: "Chapters" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: /Opening/ })).toHaveTextContent("0:00");
    // The chapter playing is highlighted.
    expect(screen.getByRole("button", { name: /Chapter 2/ })).toHaveClass("text-accent");

    await user.click(screen.getByRole("button", { name: /Finale/ }));
    expect(onSeek).toHaveBeenCalledWith(120);
    expect(screen.queryByRole("button", { name: /Finale/ })).not.toBeInTheDocument();
  });

  it("closes on Escape and on a click elsewhere", async () => {
    const user = userEvent.setup();
    render(<Menu />);

    await user.click(screen.getByRole("button", { name: "Chapters" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByText("Finale")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Chapters" }));
    await user.click(screen.getByText("outside"));
    expect(screen.queryByText("Finale")).not.toBeInTheDocument();
  });

  it("shows chapter pictures when there are any, and drops broken ones", async () => {
    const user = userEvent.setup();
    const list = [makeChapter(0, "Opening", "/api/files/1/chapters/0.jpg"), makeChapter(60_000, "Middle", "/api/files/1/chapters/1.jpg")];
    const { container } = render(<Menu list={list} />);

    await user.click(screen.getByRole("button", { name: "Chapters" }));
    const images = container.querySelectorAll("img");
    expect([...images].map((img) => img.getAttribute("src"))).toEqual([
      "/api/files/1/chapters/0.jpg",
      "/api/files/1/chapters/1.jpg",
    ]);
    fireEvent.error(images[0] as HTMLImageElement);
    expect(container.querySelectorAll("img")).toHaveLength(1);
  });
});
