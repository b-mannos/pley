import { Route, Routes } from "react-router";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { apiError, LocationDisplay, mockApi, renderWithProviders, testUser } from "@/test/render";
import { RequireAdmin, RequireAuth, useAuth } from "./auth";

beforeEach(() => {
  // No live updates in tests.
  vi.stubGlobal("EventSource", undefined);
});

function Greeting() {
  const user = useAuth();
  return <p>Hello {user.name}</p>;
}

function renderApp(route = "/library/2?sort=year") {
  return renderWithProviders(
    <Routes>
      <Route path="/login" element={<LocationDisplay />} />
      <Route path="/setup" element={<p>Setup</p>} />
      <Route element={<RequireAuth />}>
        <Route path="/library/:id" element={<Greeting />} />
        <Route path="/" element={<Greeting />} />
      </Route>
    </Routes>,
    { route },
  );
}

const server = { name: "pley", version: "1.0.0", setup_required: false };

describe("RequireAuth", () => {
  it("shows the page to a signed-in user", async () => {
    mockApi({ "GET /api/server": () => server, "GET /api/auth/me": () => ({ ...testUser, name: "Sam" }) });
    renderApp();
    expect(await screen.findByText("Hello Sam")).toBeInTheDocument();
  });

  it("sends everyone else to sign in, and back afterwards", async () => {
    mockApi({ "GET /api/server": () => server, "GET /api/auth/me": () => apiError(401, "unauthorized", "not signed in") });
    renderApp();
    expect(await screen.findByTestId("location")).toHaveTextContent("/login?next=%2Flibrary%2F2%3Fsort%3Dyear");
  });

  it("doesn't bother with the home page as a destination", async () => {
    mockApi({ "GET /api/server": () => server, "GET /api/auth/me": () => apiError(401, "unauthorized", "not signed in") });
    renderApp("/");
    expect((await screen.findByTestId("location")).textContent).toBe("/login");
  });

  it("sends a new server to setup", async () => {
    mockApi({ "GET /api/server": () => ({ ...server, setup_required: true }) });
    renderApp();
    expect(await screen.findByText("Setup")).toBeInTheDocument();
  });

  it("offers a retry when the server is unreachable", async () => {
    let up = false;
    mockApi({
      "GET /api/server": () => (up ? server : apiError(503, "unavailable", "starting up")),
      "GET /api/auth/me": () => testUser,
    });
    renderApp();

    expect(await screen.findByText("Can't reach the pley server")).toBeInTheDocument();
    up = true;
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Hello Admin")).toBeInTheDocument();
  });

  it("explains other failures", async () => {
    mockApi({ "GET /api/server": () => server, "GET /api/auth/me": () => apiError(500, "internal", "database is locked") });
    renderApp();
    expect(await screen.findByText("Database is locked")).toBeInTheDocument();
  });
});

describe("RequireAdmin", () => {
  it("keeps settings to admins", () => {
    renderWithProviders(<RequireAdmin>Server settings</RequireAdmin>, { user: { ...testUser, is_admin: false } });
    expect(screen.getByText("Admins only")).toBeInTheDocument();
    expect(screen.queryByText("Server settings")).not.toBeInTheDocument();
  });

  it("shows them to admins", () => {
    renderWithProviders(<RequireAdmin>Server settings</RequireAdmin>);
    expect(screen.getByText("Server settings")).toBeInTheDocument();
  });
});

describe("useAuth", () => {
  it("must be used under RequireAuth", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<Greeting />)).toThrow(/inside <RequireAuth>/);
  });
});
