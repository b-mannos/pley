import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { useLocation, useNavigate } from "react-router";
import { Check, ShieldCheck, TriangleAlert } from "lucide-react";
import { errorMessage, isApiError } from "@/api/client";
import { useLogout, useProfiles, useServer } from "@/api/hooks";
import { decide, startRequest, type ConsentRequest, type OAuthClient, type RequestAnswer, type RequestError } from "@/api/oauth";
import { useOptionalAudioPlayer } from "@/audio/AudioPlayer";
import { Splash, useAuth } from "@/auth";
import { Avatar } from "@/components/Avatar";
import { Badge, Button, Spinner } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { AuthLayout } from "./AuthLayout";

/** What each scope lets an app do, for the profile it acts as. */
const SCOPE_WORDS: Record<string, (profile: string) => string> = {
  library: (p) => `See ${p}'s libraries, playlists and watch history.`,
  play: (p) => `Play titles, and keep ${p}'s place, watched marks, ratings and track choices.`,
  playlists: (p) => `Make, change and delete ${p}'s playlists.`,
  profiles: () => "Switch to the household's other profiles. Profiles with a PIN still ask for it.",
};

/** How long the page must have been seen before Allow can be pressed. */
const STEADY_MS = 1000;
/** How long before a link back to the app shows, in case the browser didn't go. */
const FALLBACK_MS = 3000;

/** pley's own apps by the name pley gave them; others by their address, which is all pley can vouch for. */
function appName(client: OAuthClient): string {
  return client.first_party && client.name ? client.name : client.host;
}

function subscribeVisibility(onChange: () => void) {
  document.addEventListener("visibilitychange", onChange);
  return () => document.removeEventListener("visibilitychange", onChange);
}

function useVisible(): boolean {
  return useSyncExternalStore(subscribeVisibility, () => document.visibilityState === "visible");
}

/**
 * Whether the page has been visible and focused for `ms` without a break.
 * Leaving the window or tab starts it over: a click meant for another page
 * (a double click whose second half lands here) can't allow an app.
 */
function useSteadyAttention(ms: number): boolean {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let timer: number | undefined;
    const start = () => {
      if (document.visibilityState === "visible" && document.hasFocus()) timer = window.setTimeout(() => setReady(true), ms);
    };
    const restart = () => {
      window.clearTimeout(timer);
      timer = undefined;
      setReady(false);
      start();
    };
    start();
    window.addEventListener("focus", restart);
    window.addEventListener("blur", restart);
    document.addEventListener("visibilitychange", restart);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", restart);
      window.removeEventListener("blur", restart);
      document.removeEventListener("visibilitychange", restart);
    };
  }, [ms]);
  return ready;
}

/**
 * The authorization endpoint: an app sent the browser here to ask for the
 * account. The server checks the request; this page only shows what it
 * answers, as text, and never goes anywhere without a click.
 */
export function AuthorizePage() {
  useDocumentTitle("Allow an app");
  const { search } = useLocation();
  const query = search.replace(/^\?/, "");
  // One request per page (React runs effects twice in development).
  const started = useRef<{ query: string; answer: Promise<RequestAnswer> } | null>(null);
  const [result, setResult] = useState<{ query: string; answer?: RequestAnswer; error?: unknown } | null>(null);

  useEffect(() => {
    if (started.current?.query !== query) started.current = { query, answer: startRequest(query) };
    let live = true;
    started.current.answer.then(
      (answer) => live && setResult({ query, answer }),
      (error: unknown) => live && setResult({ query, error }),
    );
    return () => {
      live = false;
    };
  }, [query]);

  const current = result?.query === query ? result : null;
  // 401: the app goes to sign-in and comes back.
  if (!current || isApiError(current.error, 401)) return <Splash />;
  if (!current.answer) {
    return (
      <AuthLayout title={isApiError(current.error, 400) ? "This app can't sign in here" : "Something went wrong"}>
        <p className="text-sm text-muted">{errorMessage(current.error)}</p>
      </AuthLayout>
    );
  }
  if (current.answer.status === "error") return <RequestErrorScreen request={current.answer} />;
  return <Consent request={current.answer} />;
}

/** A mistake the app made: it hears about it only if the person goes back. */
function RequestErrorScreen({ request }: { request: RequestError }) {
  return (
    <AuthLayout title="This request can't be allowed">
      <div className="space-y-5">
        <p className="text-sm text-muted">{errorMessage(new Error(request.message))}</p>
        <Button variant="primary" className="w-full" onClick={() => window.location.assign(request.back_url)}>
          Back to the app
        </Button>
      </div>
    </AuthLayout>
  );
}

function Consent({ request }: { request: ConsentRequest }) {
  const user = useAuth();
  const server = useServer();
  const profiles = useProfiles(user.home);
  const logout = useLogout();
  const audio = useOptionalAudioPlayer();
  const navigate = useNavigate();
  const location = useLocation();
  const here = location.pathname + location.search;
  const ready = useSteadyAttention(STEADY_MS);
  const sent = useRef(false);
  /** The button pressed, while its answer is on its way. */
  const [deciding, setDeciding] = useState<"allow" | "deny" | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [continueUrl, setContinueUrl] = useState<string | null>(null);

  const { client, redirect } = request;
  const app = appName(client);

  const answer = (allow: boolean) => {
    if (sent.current || (allow && !ready)) return;
    sent.current = true;
    setDeciding(allow ? "allow" : "deny");
    setError(null);
    decide(request.id, allow).then(
      ({ continue_url }) => {
        window.location.assign(continue_url);
        setContinueUrl(continue_url);
      },
      (err: unknown) => {
        // 404: the request is over; anything else may be tried again.
        sent.current = isApiError(err, 404);
        setDeciding(null);
        setError(err);
      },
    );
  };

  if (continueUrl) return <Returning app={app} url={continueUrl} />;

  const over = isApiError(error, 404);
  const canSwitch = (profiles.data?.length ?? 0) > 1;

  return (
    <AuthLayout title={`${app} wants to use your pley account`} subtitle={server.data ? `on ${server.data.name}` : undefined}>
      <div className="space-y-6 text-sm">
        {client.first_party ? (
          <Badge tone="accent">
            <ShieldCheck className="size-3.5" aria-hidden /> pley's own app
          </Badge>
        ) : (
          <div className="flex gap-2.5 rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-fg/90">
            <TriangleAlert className="mt-0.5 size-4 shrink-0 text-danger" aria-hidden />
            <div className="space-y-1">
              {client.name ? <p>It calls itself “{client.name}” (pley can't check that name)</p> : null}
              <p>This app isn't one of pley's own</p>
            </div>
          </div>
        )}

        <div className="flex items-center gap-3 rounded-lg bg-surface-2 px-3 py-2.5">
          <Avatar user={user} size="md" />
          <div className="min-w-0 flex-1">
            <div className="truncate font-semibold">{user.name}</div>
            <div className="flex flex-wrap gap-x-3 text-xs">
              {canSwitch ? (
                <button
                  type="button"
                  className="font-medium text-accent hover:text-accent-hover"
                  onClick={() => void navigate(`/profiles?next=${encodeURIComponent(here)}`)}
                >
                  Use another profile
                </button>
              ) : null}
              <button
                type="button"
                className="text-muted hover:text-fg"
                disabled={logout.isPending}
                onClick={() => {
                  audio?.close();
                  logout.mutate(undefined, {
                    onSettled: () => void navigate(`/login?next=${encodeURIComponent(here)}`, { replace: true }),
                  });
                }}
              >
                Not you? Sign out
              </button>
            </div>
          </div>
        </div>

        <div>
          <p className="font-medium">It will be able to:</p>
          <ul className="mt-2 space-y-1.5">
            {request.scopes.map((scope) => (
              <li key={scope} className="flex gap-2">
                <Check className="mt-0.5 size-4 shrink-0 text-success" aria-hidden />
                <span>{SCOPE_WORDS[scope]?.(user.name) ?? scope}</span>
              </li>
            ))}
          </ul>
          <p className="mt-3 text-muted">
            It can't change your password, sign-in settings or devices, and it never gets administrator rights.
          </p>
          {user.is_admin ? <p className="mt-1 text-muted">Your administrator rights stay in pley's web app.</p> : null}
        </div>

        <div>
          <p className="font-medium">It goes back to:</p>
          <p className="mt-1">
            {redirect.kind === "web"
              ? `the website ${redirect.to}`
              : redirect.kind === "loopback"
                ? "an app on this computer"
                : "an app on this device"}
          </p>
          {redirect.kind !== "web" ? (
            <p className="mt-1 text-danger">Only allow this if you just started signing in to the app.</p>
          ) : null}
        </div>

        {over ? (
          <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-danger">
            This request is over: start again from the app
          </p>
        ) : (
          <>
            {error ? (
              <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-danger">
                {errorMessage(error)}
              </p>
            ) : null}
            <div className="flex gap-2">
              <Button
                variant="outline"
                className="flex-1"
                autoFocus
                loading={deciding === "deny"}
                disabled={deciding !== null}
                onClick={() => answer(false)}
              >
                Don't allow
              </Button>
              <Button
                variant="primary"
                className="flex-1"
                loading={deciding === "allow"}
                disabled={!ready || deciding !== null}
                onClick={() => answer(true)}
              >
                Allow
              </Button>
            </div>
          </>
        )}
      </div>
    </AuthLayout>
  );
}

/** The browser is on its way back to the app; a link in case it didn't go. */
function Returning({ app, url }: { app: string; url: string }) {
  const visible = useVisible();
  const [late, setLate] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setLate(true), FALLBACK_MS);
    return () => window.clearTimeout(timer);
  }, []);
  return (
    <AuthLayout title={`Returning to ${app}…`}>
      {late && visible ? (
        <a href={url} className="text-sm font-medium text-accent hover:text-accent-hover">
          Open {app}
        </a>
      ) : (
        <Spinner />
      )}
    </AuthLayout>
  );
}
