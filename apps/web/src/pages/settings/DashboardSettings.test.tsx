import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Activity, SessionInfo } from "@/api/types";
import { makeItem } from "@/test/fixtures";
import { apiError, mockApi, renderWithProviders } from "@/test/render";
import { DashboardSettings } from "./DashboardSettings";

const playing: SessionInfo = {
  session_id: "abc",
  user_id: 2,
  username: "sam",
  item: makeItem({ id: 7, title: "Heat", duration_ms: 6_000_000 }),
  method: "transcode",
  position_ms: 60_000,
  state: "playing",
  started_at: 1_700_000_000,
  updated_at: 1_700_000_000,
  transcode: null,
};

function serve(stop: () => unknown = () => undefined) {
  let activity: Activity = { scans: [], sessions: [playing] };
  return mockApi({
    "GET /api/activity": () => activity,
    "DELETE /api/sessions/abc": () => {
      const res = stop();
      if (!(res instanceof Response)) activity = { scans: [], sessions: [] };
      return res;
    },
  });
}

describe("DashboardSettings stopping playback", () => {
  it("stops a session, with a message for the viewer", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderWithProviders(<DashboardSettings />);

    await user.click(await screen.findByRole("button", { name: "Stop Heat for sam" }));
    const dialog = screen.getByRole("dialog", { name: "Stop playback" });
    expect(dialog).toHaveTextContent("Heat stops for sam right away.");
    await user.type(within(dialog).getByLabelText("Message"), "Server going down for updates");
    await user.click(within(dialog).getByRole("button", { name: "Stop" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const stop = requests.find((r) => r.method === "DELETE" && r.path === "/api/sessions/abc");
    expect(stop?.body).toEqual({ message: "Server going down for updates" });
    expect(await screen.findByText("Playback stopped")).toBeInTheDocument();
    expect(await screen.findByText("Nobody is watching anything right now.")).toBeInTheDocument();
  });

  it("sends no message when none is written", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderWithProviders(<DashboardSettings />);
    await user.click(await screen.findByRole("button", { name: "Stop Heat for sam" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(requests.some((r) => r.method === "DELETE" && r.path === "/api/sessions/abc")).toBe(true));
    expect(requests.find((r) => r.method === "DELETE")?.body).toBeUndefined();
  });

  it("keeps the dialog and says why when it can't", async () => {
    const user = userEvent.setup();
    serve(() => apiError(403, "forbidden", "only administrators can stop other people's playback"));
    renderWithProviders(<DashboardSettings />);
    await user.click(await screen.findByRole("button", { name: "Stop Heat for sam" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Stop" }));
    expect(await screen.findByText("Only administrators can stop other people's playback")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
