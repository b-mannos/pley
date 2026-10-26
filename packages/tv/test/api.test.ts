import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Api, unreachable, unreachableReason } from "../src/api/client";
import type { Tokens } from "../src/auth/tokens";
import { NetworkError, TIMED_OUT, type HttpRequest, type HttpResponse } from "../src/net/transport";

const tokens = { accessToken: () => Promise.resolve("token") } as unknown as Tokens;

/** A transport whose answers the test decides: `null` never answers, so the request times out (as XHR does) unless cancelled first. */
function transport(answer: (req: HttpRequest) => Promise<HttpResponse> | null) {
  const asked: string[] = [];
  const http = (req: HttpRequest): Promise<HttpResponse> => {
    asked.push(`${req.method} ${req.url.replace(/^https?:\/\/[^/]+/, "")}`);
    const a = answer(req);
    if (a) return a;
    return new Promise((_resolve, reject) => {
      const timer = setTimeout(() => reject(new NetworkError(TIMED_OUT)), req.timeoutMs ?? 20_000);
      req.cancel?.onCancel(() => {
        clearTimeout(timer);
        reject(new NetworkError("cancelled", true));
      });
    });
  };
  return { http, asked };
}

const ok = (body: unknown): Promise<HttpResponse> => Promise.resolve({ status: 200, header: () => "application/json", body });

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("a server on the home network", () => {
  it("gives up after 3 s plus a quick question nobody answers, saying there was no answer in time", async () => {
    const t = transport(() => null);
    const api = new Api("http://192.168.1.20:7539", tokens, t.http);
    let failed: unknown = null;
    void api.home().catch((e: unknown) => (failed = e));
    await vi.advanceTimersByTimeAsync(3_000);
    expect(t.asked).toEqual(["GET /api/hubs/home", "GET /api/server"]);
    expect(failed).toBeNull();
    await vi.advanceTimersByTimeAsync(3_000);
    // 6 s, not the request's 20.
    expect(unreachable(failed)).toBe(true);
    expect(unreachableReason(failed)).toBe("No answer in time.");
  });

  it("waits as long as it takes for a server that answers the question (a conversion starting)", async () => {
    let answerPlay: ((r: HttpResponse) => void) | null = null;
    const t = transport((req) => {
      if (req.url.endsWith("/api/server")) return ok({ name: "Home" });
      return new Promise<HttpResponse>((resolve) => (answerPlay = resolve));
    });
    const api = new Api("http://nas.local:7539", tokens, t.http);
    const result = api.home();
    await vi.advanceTimersByTimeAsync(10_000);
    answerPlay!({ status: 200, header: () => null, body: { hubs: [] } });
    await expect(result).resolves.toEqual({ hubs: [] });
  });

  it("gives up with no answer in time when the question fails", async () => {
    const t = transport((req) => (req.url.endsWith("/api/server") ? Promise.reject(new NetworkError(TIMED_OUT)) : null));
    const api = new Api("http://192.168.1.20:7539", tokens, t.http);
    const result = api.home().then(
      () => null,
      (e: unknown) => e,
    );
    await vi.advanceTimersByTimeAsync(3_100);
    const e = await result;
    expect(unreachable(e)).toBe(true);
    expect(unreachableReason(e)).toBe("No answer in time.");
  });

  it("leaves servers elsewhere (and Tailscale ones) to their own timeouts", async () => {
    for (const server of ["https://pley.example.com", "http://100.101.102.103:7539", "http://nas.tail1234.ts.net:7539"]) {
      const t = transport(() => null);
      const api = new Api(server, tokens, t.http);
      void api.home().catch(() => undefined);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(t.asked).toEqual(["GET /api/hubs/home"]);
    }
  });
});

describe("no answer", () => {
  it("is a failed connection, a timeout or a gateway with nothing behind it, never a cancelled request", () => {
    expect(unreachable(new NetworkError("Can't reach the server"))).toBe(true);
    expect(unreachableReason(new NetworkError("Can't reach the server"))).toBe("Nothing answers there.");
    expect(unreachable(new NetworkError("cancelled", true))).toBe(false);
  });
});
