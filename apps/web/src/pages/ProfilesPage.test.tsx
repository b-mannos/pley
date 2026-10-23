import { Route, Routes } from "react-router";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { User } from "@/api/types";
import { apiError, type FakeRequest, LocationDisplay, mockApi, renderWithProviders, testUser } from "@/test/render";
import { challenge, ProfilesPage } from "./ProfilesPage";

const me: User = { ...testUser, id: 1, name: "Sam", is_admin: false };
const kid: User = { ...testUser, id: 2, name: "Kid", is_admin: false };
const locked: User = { ...testUser, id: 3, name: "Alex", is_admin: false, has_pin: true };
const admin: User = { ...testUser, id: 4, name: "Owner", is_admin: true };

function serve() {
  return mockApi({
    "GET /api/auth/profiles": () => [me, kid, locked, admin],
    "POST /api/auth/switch": (req) => {
      const body = req.body as { user_id: number; pin?: string; password?: string };
      if (body.user_id === locked.id && body.pin !== "2468") return apiError(403, "forbidden", "wrong PIN");
      if (body.user_id === admin.id && body.password !== "secret") return apiError(403, "forbidden", "wrong password");
      return { user: [kid, locked, admin].find((u) => u.id === body.user_id), token: "t" };
    },
  });
}

/** The last profile switch: queries refetched after it may come later. */
function lastSwitch(requests: FakeRequest[]) {
  return requests.filter((r) => r.method === "POST" && r.path === "/api/auth/switch").at(-1);
}

function renderProfiles() {
  return renderWithProviders(
    <Routes>
      <Route path="/profiles" element={<ProfilesPage />} />
      <Route path="*" element={<LocationDisplay />} />
    </Routes>,
    { route: "/profiles?next=%2Flibrary%2F2", user: me },
  );
}

describe("ProfilesPage", () => {
  it("switches to a profile without a PIN", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderProfiles();

    await user.click(await screen.findByRole("button", { name: "Kid" }));
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
    expect(lastSwitch(requests)?.body).toEqual({ user_id: kid.id });
  });

  it("goes on as the current profile without asking the server", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderProfiles();

    await user.click(await screen.findByRole("button", { name: "Sam" }));
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
    expect(requests.filter((r) => r.method === "POST")).toHaveLength(0);
  });

  it("asks for the PIN of a locked profile and starts over after a wrong one", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderProfiles();

    await user.click(await screen.findByRole("button", { name: "Alex" }));
    expect(screen.getByText("Enter the profile PIN")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Switch" })).toBeDisabled();

    await user.keyboard("1235");
    await user.keyboard("{Backspace}");
    expect(screen.getByLabelText("3 digits entered")).toBeInTheDocument();
    await user.keyboard("4{Enter}");
    expect(await screen.findByText("Wrong PIN")).toHaveAttribute("role", "alert");
    expect(screen.getByLabelText("0 digits entered")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "2" }));
    await user.click(screen.getByRole("button", { name: "4" }));
    await user.click(screen.getByRole("button", { name: "6" }));
    await user.click(screen.getByRole("button", { name: "8" }));
    await user.click(screen.getByRole("button", { name: "Switch" }));

    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
    expect(requests.filter((r) => r.method === "POST").map((r) => r.body)).toEqual([
      { user_id: locked.id, pin: "1234" },
      { user_id: locked.id, pin: "2468" },
    ]);
  });

  it("goes back to the profiles from the PIN pad", async () => {
    const user = userEvent.setup();
    serve();
    renderProfiles();

    await user.click(await screen.findByRole("button", { name: "Alex" }));
    await user.keyboard("{Escape}");
    expect(screen.getByRole("heading", { name: "Who's watching?" })).toBeInTheDocument();
  });

  it("asks for the password of an administrator", async () => {
    const user = userEvent.setup();
    const requests = serve();
    renderProfiles();

    await user.click(await screen.findByRole("button", { name: "Owner" }));
    await user.type(screen.getByLabelText("Password"), "guess");
    await user.click(screen.getByRole("button", { name: "Switch" }));
    expect(await screen.findByText("Wrong password")).toHaveAttribute("role", "alert");

    await user.clear(screen.getByLabelText("Password"));
    await user.type(screen.getByLabelText("Password"), "secret");
    await user.click(screen.getByRole("button", { name: "Switch" }));
    await waitFor(() => expect(lastSwitch(requests)?.body).toEqual({ user_id: admin.id, password: "secret" }));
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
  });

  it("can skip choosing", async () => {
    const user = userEvent.setup();
    serve();
    renderProfiles();
    await user.click(await screen.findByRole("button", { name: "Continue as Sam" }));
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
  });
});

describe("challenge", () => {
  const child: User = { ...testUser, id: 10, is_admin: false, managed: true, has_password: false, max_age: 12 };
  const teen: User = { ...child, id: 11, max_age: 16 };
  const parent: User = { ...testUser, id: 12, is_admin: false, max_age: null };

  it("asks for nothing to switch down, or between equals", () => {
    expect(challenge(child, parent)).toBe("none");
    expect(challenge({ ...child, id: 13 }, child)).toBe("none");
  });

  it("asks for a password, or a PIN, to switch to a profile that can see more", () => {
    expect(challenge(parent, child)).toBe("password");
    expect(challenge({ ...teen, has_pin: true }, child)).toBe("pin");
    // A managed profile has no password: it needs a PIN first.
    expect(challenge(teen, child)).toBe("locked");
  });

  it("counts libraries too", () => {
    const one: User = { ...parent, id: 14, all_libraries: false, library_ids: [1] };
    const both: User = { ...parent, id: 15, all_libraries: false, library_ids: [1, 2] };
    expect(challenge(both, one)).toBe("password");
    expect(challenge(one, both)).toBe("none");
  });
});
