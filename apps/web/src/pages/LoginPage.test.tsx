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
      const { password, code } = req.body as { password: string; code?: string };
      if (password === "too many") return apiError(429, "too_many_requests", "too many attempts, wait a minute");
      if (password === "two-step") {
        if (!code) return apiError(401, "two_step", "enter the 6-digit code from your authenticator app");
        if (code !== "123456" && code !== "7k2m-q9xd-4hpt-3vwr") {
          return apiError(401, "two_step", "that code didn't work: enter the newest code from your authenticator app");
        }
        return { user: testUser };
      }
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

  it("says when signing in is for letting an app in", async () => {
    serve();
    const authorize = "/oauth/authorize?response_type=code&client_id=https%3A%2F%2Fapps.example.com%2Fpley.json&state=a%26b";
    renderLogin(`/login?next=${encodeURIComponent(authorize)}`);

    expect(await screen.findByText("to let an app use Living Room")).toBeInTheDocument();
    await signIn("open-sesame");
    expect(await screen.findByTestId("location")).toHaveTextContent(authorize);
  });

  it("says when signing in is for letting a TV in", async () => {
    serve();
    renderLogin(`/login?next=${encodeURIComponent("/link?code=BCDFGHJK")}`);

    expect(await screen.findByText("to let your TV use Living Room")).toBeInTheDocument();
    await signIn("open-sesame");
    expect(await screen.findByTestId("location")).toHaveTextContent("/link?code=BCDFGHJK");
  });

  it("lets a household pick a profile first", async () => {
    serve({ profiles: [testUser, { ...testUser, id: 2, name: "Kid" }] });
    renderLogin();

    await signIn("open-sesame");
    expect(await screen.findByTestId("location")).toHaveTextContent("/profiles?next=%2Flibrary%2F2");
  });

  it("asks for the code from the authenticator app, and signs in with it", async () => {
    const requests = serve();
    renderLogin();

    await signIn("two-step");
    expect(await screen.findByRole("heading", { name: "Two-step sign-in" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    const user = userEvent.setup();
    const code = screen.getByLabelText("Code");
    expect(code).toHaveAttribute("autocomplete", "one-time-code");

    // A wrong code says so; a whole one goes as soon as it's typed.
    await user.type(code, "999999");
    expect(await screen.findByRole("alert")).toHaveTextContent("That code didn't work");
    await user.clear(code);
    await user.type(code, "123 456");
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
    expect(requests.filter((r) => r.method === "POST").at(-1)?.body).toEqual({
      username: "admin",
      password: "two-step",
      code: "123456",
    });
  });

  it("takes a recovery code instead", async () => {
    serve();
    renderLogin();

    await signIn("two-step");
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Use a recovery code" }));
    const code = screen.getByLabelText("Recovery code");
    const submit = screen.getByRole("button", { name: "Sign in" });
    await user.type(code, "7k2m-q9xd");
    expect(submit).toBeDisabled();
    await user.type(code, "-4hpt-3vwr");
    await user.click(submit);
    expect(await screen.findByTestId("location")).toHaveTextContent("/library/2");
  });

  it("goes back to the password", async () => {
    serve();
    renderLogin();

    await signIn("two-step");
    await userEvent.setup().click(await screen.findByRole("button", { name: "Back" }));
    expect(await screen.findByLabelText("Password")).toHaveValue("");
    expect(screen.getByLabelText("Username")).toHaveValue("  admin ");
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
