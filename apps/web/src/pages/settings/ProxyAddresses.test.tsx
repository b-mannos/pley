import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { mockApi, renderWithProviders } from "@/test/render";
import { droppedPort, ProxyAddresses } from "./ProxyAddresses";

describe("droppedPort", () => {
  it("is a proxy leaving the page's port out", () => {
    expect(droppedPort("https://tokyo.example.com:9999", "https://tokyo.example.com")).toBe(true);
    expect(droppedPort("https://tokyo.example.com", "https://tokyo.example.com")).toBe(false);
    expect(droppedPort("https://tokyo.example.com:9999", "https://tokyo.example.com:9999")).toBe(false);
    expect(droppedPort("https://tokyo.example.com:9999", "http://tokyo.example.com")).toBe(false);
    expect(droppedPort("https://tokyo.example.com:9999", "https://osaka.example.com")).toBe(false);
    expect(droppedPort("nonsense", "https://tokyo.example.com")).toBe(false);
  });
});

describe("ProxyAddresses", () => {
  it("notices the port a proxy left out, and adds the page's address", async () => {
    const user = userEvent.setup();
    const here = window.location.origin;
    let saved: string[] = [];
    const requests = mockApi({
      "GET /api/settings": () => ({ proxy_addresses: saved }),
      "PUT /api/settings": (req) => {
        saved = (req.body as { proxy_addresses: string[] }).proxy_addresses;
        return { proxy_addresses: saved };
      },
      "GET /.well-known/oauth-authorization-server": () => ({
        issuer: saved.includes(here) ? here : here.replace(/:\d+$/, ""),
      }),
    });
    renderWithProviders(<ProxyAddresses />);
    expect(await screen.findByRole("alert")).toHaveTextContent("apps can't sign in at this address");
    await user.click(screen.getByRole("button", { name: `Add ${here}` }));
    await waitFor(() => expect(requests.find((r) => r.method === "PUT")?.body).toEqual({ proxy_addresses: [here] }));
    expect(await screen.findByText(here)).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: `Remove ${here}` }));
    await waitFor(() => expect(requests.filter((r) => r.method === "PUT").at(-1)?.body).toEqual({ proxy_addresses: [] }));
  });

  it("shows nothing for a server from before the setting", async () => {
    mockApi({
      "GET /api/settings": () => ({ server_name: "Den" }),
      "GET /.well-known/oauth-authorization-server": () => ({ issuer: "http://localhost" }),
    });
    const { container } = renderWithProviders(<ProxyAddresses />);
    await waitFor(() => expect(container).toBeEmptyDOMElement());
  });
});
