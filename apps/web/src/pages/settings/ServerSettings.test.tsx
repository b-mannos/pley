import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ServiceTestResult } from "@/api/types";
import { mockApi, renderWithProviders } from "@/test/render";
import { TestButton } from "./ServerSettings";

describe("TestButton", () => {
  it("tests the key as typed and says how it went", async () => {
    const user = userEvent.setup();
    const requests = mockApi({
      "POST /api/settings/test": ({ body }): ServiceTestResult =>
        (body as { api_key: string }).api_key === "good"
          ? { ok: true, message: "TMDB accepted the key.", ms: 182 }
          : { ok: false, message: "TMDB rejected this key.", ms: 95 },
    });
    const { rerender } = renderWithProviders(<TestButton label="Test the TMDB key" test={{ service: "tmdb", api_key: "good" }} />);

    await user.click(screen.getByRole("button", { name: "Test the TMDB key" }));
    expect(await screen.findByText("TMDB accepted the key.")).toBeInTheDocument();
    expect(screen.getByText("182 ms")).toBeInTheDocument();
    expect(requests.at(-1)?.body).toEqual({ service: "tmdb", api_key: "good" });

    // A new key, a new answer (the settings page remounts it by key).
    rerender(<TestButton key="bad" label="Test the TMDB key" test={{ service: "tmdb", api_key: "bad" }} />);
    expect(screen.queryByText("TMDB accepted the key.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Test the TMDB key" }));
    expect(await screen.findByText("TMDB rejected this key.")).toBeInTheDocument();
    expect(screen.queryByText("95 ms")).not.toBeInTheDocument();
  });
});
