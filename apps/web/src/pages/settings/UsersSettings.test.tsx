import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Library, User } from "@/api/types";
import { mockApi, renderWithProviders, testUser } from "@/test/render";
import { UsersSettings } from "./UsersSettings";

const admin: User = { ...testUser, id: 1, username: "admin", name: "Admin", is_admin: true };
const kid: User = {
  ...testUser,
  id: 2,
  username: "kid",
  name: "Kiddo",
  display_name: "Kiddo",
  is_admin: false,
  managed: true,
  has_pin: true,
  all_libraries: false,
  library_ids: [2],
  max_age: 7,
};
const ann: User = { ...testUser, id: 3, username: "ann", name: "Ann", is_admin: false, two_step: true };
const libraries: Library[] = [
  { id: 1, name: "Movies", kind: "movies", paths: [], item_count: 0, created_at: 0, scanned_at: null, scanning: false },
  { id: 2, name: "Cartoons", kind: "shows", paths: [], item_count: 0, created_at: 0, scanned_at: null, scanning: false },
];

function serve() {
  return mockApi({
    "GET /api/users": () => [admin, kid, ann],
    "GET /api/libraries": () => libraries,
    "GET /api/users/1/devices": () => [],
    "GET /api/users/2/devices": () => [{ id: "d1", name: "Safari on iPhone", user_agent: null, ip: "192.0.2.4", created_at: 0, last_seen_at: 0, current: false }],
    "POST /api/users": (req) => ({ ...testUser, id: 3, name: (req.body as { username: string }).username }),
    "PATCH /api/users/2": () => ({ ...kid, name: "Kiddo Two" }),
    "DELETE /api/users/2": () => undefined,
    "GET /api/users/3/devices": () => [],
    "DELETE /api/users/3/two-step": () => undefined,
  });
}

/** Renders the page as the administrator; returns the requests it makes. */
function renderUsers() {
  const requests = serve();
  renderWithProviders(<UsersSettings />, { user: admin });
  return requests;
}

async function openAddDialog() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "Add user" }));
  return { user, dialog: screen.getByRole("dialog", { name: "Add user" }) };
}

describe("UsersSettings", () => {
  it("lists users with what sets them apart", async () => {
    renderUsers();
    const rows = await screen.findAllByRole("listitem");
    expect(rows[0]).toHaveTextContent("Admin");
    expect(rows[0]).toHaveTextContent("(you)");
    expect(rows[1]).toHaveTextContent("@kid");
    for (const badge of ["Managed", "Home", "Cartoons", "Up to 7", "PIN"]) expect(rows[1]).toHaveTextContent(badge);
  });

  it("turns two-step sign-in off for someone who lost their phone", async () => {
    const user = userEvent.setup();
    const requests = renderUsers();
    const rows = await screen.findAllByRole("listitem");
    expect(rows[2]).toHaveTextContent("2FA");
    await user.click(screen.getByRole("button", { name: /Ann/ }));
    const dialog = screen.getByRole("dialog", { name: "Edit Ann" });
    await user.click(within(dialog).getByRole("button", { name: "Turn off" }));
    expect(requests.some((r) => r.method === "DELETE")).toBe(false);
    await user.click(within(dialog).getByRole("button", { name: "Turn off" }));
    expect(await screen.findByText("Two-step sign-in is off for Ann")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "DELETE")?.path).toBe("/api/users/3/two-step");
  });

  it("adds an account after checking the form", async () => {
    const requests = renderUsers();
    const { user, dialog } = await openAddDialog();
    const add = within(dialog).getByRole("button", { name: "Add user" });

    await user.click(add);
    expect(within(dialog).getByText("Enter a username.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Username"), " guest ");
    await user.type(within(dialog).getByLabelText("Password"), "abc");
    await user.click(add);
    expect(within(dialog).getByText("Use at least 8 characters for the password.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Password"), "-defgh");
    await user.type(within(dialog).getByLabelText("Profile PIN"), "1a2");
    await user.click(add);
    expect(within(dialog).getByText("The PIN must be 4 to 8 digits.")).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Profile PIN"), "34");

    await user.click(add);
    expect(await screen.findByText("Added guest")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "POST")?.body).toEqual({
      username: "guest",
      password: "abc-defgh",
      is_admin: false,
      managed: false,
      home: true,
      all_libraries: true,
      library_ids: [],
      max_age: null,
      pin: "1234",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("adds a managed profile without a password, limited to some libraries", async () => {
    const requests = renderUsers();
    const { user, dialog } = await openAddDialog();

    await user.click(within(dialog).getByRole("button", { name: /Managed profile/ }));
    expect(within(dialog).queryByLabelText("Password")).not.toBeInTheDocument();
    expect(within(dialog).getByRole("switch", { name: "Administrator" })).toBeDisabled();
    await user.type(within(dialog).getByLabelText("Username"), "little");
    await user.click(within(dialog).getByRole("button", { name: "Only some" }));
    await user.click(within(dialog).getByRole("button", { name: "Add user" }));
    expect(within(dialog).getByText("Pick at least one library, or allow all of them.")).toBeInTheDocument();

    await user.click(within(dialog).getByLabelText(/Cartoons/));
    await user.click(within(dialog).getByRole("button", { name: "Add user" }));
    await waitFor(() => expect(requests.some((r) => r.method === "POST")).toBe(true));
    expect(requests.find((r) => r.method === "POST")?.body).toMatchObject({
      username: "little",
      managed: true,
      is_admin: false,
      all_libraries: false,
      library_ids: [2],
    });
    expect(requests.find((r) => r.method === "POST")?.body).not.toHaveProperty("password");
  });

  it("saves only what changed", async () => {
    const user = userEvent.setup();
    const requests = renderUsers();
    await user.click(await screen.findByRole("button", { name: /Kiddo/ }));
    const dialog = screen.getByRole("dialog", { name: "Edit Kiddo" });
    expect(await within(dialog).findByText("Safari on iPhone")).toBeInTheDocument();

    const name = within(dialog).getByLabelText("Display name");
    await user.clear(name);
    await user.type(name, "Kiddo Two");
    await user.click(within(dialog).getByRole("button", { name: "Remove PIN" }));
    await user.click(within(dialog).getByRole("button", { name: "Save" }));

    expect(await screen.findByText("Saved Kiddo Two")).toBeInTheDocument();
    expect(requests.find((r) => r.method === "PATCH")?.body).toEqual({ display_name: "Kiddo Two", pin: null });
  });

  it("closes without a request when nothing changed", async () => {
    const user = userEvent.setup();
    const requests = renderUsers();
    await user.click(await screen.findByRole("button", { name: /Kiddo/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Save" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(requests.filter((r) => r.method === "PATCH")).toHaveLength(0);
  });

  it("keeps administrators from locking themselves out", async () => {
    const user = userEvent.setup();
    renderUsers();
    await user.click(await screen.findByRole("button", { name: /Admin/ }));
    const dialog = screen.getByRole("dialog", { name: "Edit Admin" });

    expect(within(dialog).getByRole("switch", { name: "Administrator" })).toBeDisabled();
    expect(within(dialog).getByRole("switch", { name: "Disabled" })).toBeDisabled();
    expect(within(dialog).queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
  });

  it("deletes a user after confirming", async () => {
    const user = userEvent.setup();
    const requests = renderUsers();
    await user.click(await screen.findByRole("button", { name: /Kiddo/ }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));

    const confirm = screen.getByRole("dialog", { name: "Delete user?" });
    expect(confirm).toHaveTextContent("Kiddo will lose access");
    await user.click(within(confirm).getByRole("button", { name: "Delete" }));

    expect(await screen.findByText("Deleted Kiddo")).toBeInTheDocument();
    expect(requests.some((r) => r.method === "DELETE" && r.path === "/api/users/2")).toBe(true);
  });
});
