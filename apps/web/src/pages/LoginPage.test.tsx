import { Route, Routes } from "react-router";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { User } from "@/api/types";
import { apiError, LocationDisplay, mockApi, renderWithProviders, testUser } from "@/test/render";
import { LoginPage } from "./LoginPage";

interface Server {
  setupRequired?: boolean;
  signedIn?: boolean;
  profiles?: User[];
}

function serve({ setupRequired = false, signedIn = false, profiles = [testUser] }: Server = {}) {
  return mockApi({
    "GET /api/server": () => ({ name: "Living Room", version: "1.0.0", setup_required: setupRequired }),
    "GET /api/auth/me": () => (signedIn ? testUser : apiError(401, "unauthorized", "not signed in")),
    "POST /api/auth/login": (req) => {
      const { password } = req.body as { password: string };
      if (password === "too many") return apiError(429, "too_many_requests", "too many attempts, wait a minute");
      return password === "open-sesame" ? { user: testUser, token: "t" } : apiError(401, "unauthorized", "invalid credentials");
    },
    "GET /api/auth/profiles": () => profiles,
  });
}

function renderLogin(route = "/login?next=%2Flibrary%2F2") {
  return renderWithProviders(
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/setup" element={<p>Setup</p>} />
      <Route path="*" element={<LocationDisplay />} />
    </Routes>,
    { route },
  );
}

async function signIn(password: string) {
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText("Username"), "  admin ");
  await user.type(screen.getByLabelText("Password"), password);
  await user.click(screen.getByRole("button", { name: "Sign in" }));
}

describe("LoginPage", () => {
  it("signs in and goes where the user was headed", async () => {
    const requests = serve();
    renderLogin();

    expect(await screen.findByText("to Living Room")).toBeInTheDocument();
    await signIn("open-sesame");

    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
    expect(requests.find((r) => r.method === "POST")?.body).toEqual({ username: "admin", password: "open-sesame" });
  });

  it("lets a household pick a profile first", async () => {
    serve({ profiles: [testUser, { ...testUser, id: 2, name: "Kid" }] });
    renderLogin();

    await signIn("open-sesame");
    expect(await screen.findByTestId("location")).toHaveTextContent("/profiles?next=%2Flibrary%2F2");
  });

  it("says when the password is wrong", async () => {
    serve();
    renderLogin();

    await signIn("nope");
    expect(await screen.findByRole("alert")).toHaveTextContent("Wrong username or password.");
    expect(screen.getByRole("button", { name: "Sign in" })).toBeEnabled();
  });

  it("passes on the server's explanation of other refusals", async () => {
    serve();
    renderLogin();

    await signIn("too many");
    expect(await screen.findByRole("alert")).toHaveTextContent("Too many attempts, wait a minute");
  });

  it("skips the form when already signed in", async () => {
    serve({ signedIn: true });
    renderLogin();
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
  });

  it("sends a new server to setup", async () => {
    serve({ setupRequired: true });
    renderLogin();
    expect(await screen.findByText("Setup")).toBeInTheDocument();
  });

  it("ignores redirects to other sites", async () => {
    serve();
    renderLogin("/login?next=%2F%5Cevil.example");

    await signIn("open-sesame");
    expect((await screen.findByTestId("location")).textContent).toBe("/");
  });
});
