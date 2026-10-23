import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Menu, MenuItem, MenuSeparator } from "./Menu";

function renderMenu(onRefresh = vi.fn()) {
  render(
    <div>
      <p>elsewhere</p>
      <Menu
        trigger={({ open, toggle }) => (
          <button type="button" aria-expanded={open} onClick={toggle}>
            More
          </button>
        )}
      >
        {(close) => (
          <>
            <MenuItem
              onClick={() => {
                onRefresh();
                close();
              }}
            >
              Refresh metadata
            </MenuItem>
            <MenuSeparator />
            <MenuItem onClick={vi.fn()} danger disabled>
              Delete
            </MenuItem>
          </>
        )}
      </Menu>
    </div>,
  );
  return onRefresh;
}

describe("Menu", () => {
  it("opens from its trigger and closes after a choice", async () => {
    const user = userEvent.setup();
    const onRefresh = renderMenu();

    await user.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByRole("button", { name: "More" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menuitem", { name: "Delete" })).toBeDisabled();

    await user.click(screen.getByRole("menuitem", { name: "Refresh metadata" }));
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes on a click elsewhere, on Escape and on its trigger", async () => {
    const user = userEvent.setup();
    renderMenu();
    const trigger = screen.getByRole("button", { name: "More" });

    await user.click(trigger);
    await user.click(screen.getByText("elsewhere"));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    await user.click(trigger);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    await user.click(trigger);
    await user.click(trigger);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
