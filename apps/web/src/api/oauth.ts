import { api } from "./client";

/** An app that asks to use an account, as the server checked it. */
export interface OAuthClient {
  /** The client ID: the address of the app's description. */
  id: string;
  /** The ASCII host of that address. */
  host: string;
  /** What the app calls itself. pley vouches for it only for its own apps. */
  name: string | null;
  /** One of pley's own apps (built into the server). */
  first_party: boolean;
}

/** What an app may ask for. */
export type OAuthScope = "library" | "play" | "playlists" | "profiles";

/** `POST /api/oauth/requests`: a request to show for consent. */
export interface ConsentRequest {
  status: "consent";
  id: string;
  expires_at: number;
  client: OAuthClient;
  /** Where the browser goes back to: a website's host, or an app's scheme. */
  redirect: { kind: "app" | "loopback" | "web"; to: string };
  scopes: (OAuthScope | (string & {}))[];
}

/** `POST /api/oauth/requests`: a mistake that goes back to the app, but only on a click. */
export interface RequestError {
  status: "error";
  error: string;
  message: string;
  back_url: string;
  client: OAuthClient;
}

export type RequestAnswer = ConsentRequest | RequestError;

/** `POST /api/oauth/requests/{id}`: where the browser goes next. */
export interface Decision {
  continue_url: string;
}

/** Hands the authorize page's query (without "?") to the server, which checks it. */
export function startRequest(query: string) {
  return api.post<RequestAnswer>("/api/oauth/requests", { query });
}

/** Allows the app, or not. */
export function decide(id: string, allow: boolean) {
  return api.post<Decision>(`/api/oauth/requests/${encodeURIComponent(id)}`, { allow });
}
