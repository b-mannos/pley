import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { ArrowLeft, Delete, Lock } from "lucide-react";
import { errorMessage, isApiError } from "@/api/client";
import { useProfiles, useSwitchProfile } from "@/api/hooks";
import type { User } from "@/api/types";
import { useOptionalAudioPlayer } from "@/audio/AudioPlayer";
import { useAuth } from "@/auth";
import { Avatar } from "@/components/Avatar";
import { Logo } from "@/components/Logo";
import { Button, Spinner, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle } from "@/lib/hooks";

/**
 * Where to go after signing in (`?next=`): a path on this site, but not the
 * sign-in or profile screens. URL parsing decides what is another site: it
 * reads "/\evil.com" and "/\t/evil.com" as "//evil.com".
 */
export function safeNext(next: string | null): string {
  if (!next?.startsWith("/")) return "/";
  let url: URL;
  try {
    url = new URL(next, window.location.origin);
  } catch {
    return "/";
  }
  if (url.origin !== window.location.origin) return "/";
  const path = `${url.pathname}${url.search}${url.hash}`;
  return path.startsWith("/login") || path.startsWith("/profiles") ? "/" : path;
}

/** What switching to `target` asks for. */
function challenge(target: User, me: User): "none" | "pin" | "password" {
  if (target.id === me.id) return "none";
  if (target.has_pin) return "pin";
  if (target.is_admin && !me.is_admin) return "password";
  return "none";
}

/** The server explains 403 (wrong PIN or password) and 429 (too many tries). */
function switchError(err: unknown): string {
  return isApiError(err, 0) ? "Can't reach the server" : errorMessage(err);
}

const PAD = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "back", "0", "ok"] as const;

function PinPad({
  target,
  busy,
  error,
  onSubmit,
  onCancel,
}: {
  target: User;
  busy: boolean;
  error: string | null;
  /** Called with the PIN once it has at least 4 digits. */
  onSubmit: (pin: string) => void;
  onCancel: () => void;
}) {
  const [pin, setPinState] = useState("");
  // Keys can arrive faster than renders (quick typing, password managers):
  // the ref always holds what was typed so far.
  const pinRef = useRef("");
  const setPin = useCallback((next: string) => {
    pinRef.current = next;
    setPinState(next);
  }, []);

  const press = useCallback(
    (key: (typeof PAD)[number]) => {
      if (busy) return;
      const current = pinRef.current;
      if (key === "back") setPin(current.slice(0, -1));
      else if (key === "ok") {
        if (current.length >= 4) onSubmit(current);
      } else if (current.length < 8) setPin(current + key);
    },
    [busy, onSubmit, setPin],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (/^\d$/.test(e.key)) press(e.key as (typeof PAD)[number]);
      else if (e.key === "Backspace") press("back");
      else if (e.key === "Enter") press("ok");
      else if (e.key === "Escape") onCancel();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [press, onCancel]);

  return (
    <div className="flex flex-col items-center">
      <Avatar user={target} size="lg" />
      <div className="mt-3 text-lg font-semibold">{target.name}</div>
      <div className="mt-1 flex items-center gap-1.5 text-sm text-muted">
        <Lock className="size-3.5" /> Enter the profile PIN
      </div>
      <div className="mt-5 flex h-4 items-center gap-3" aria-label={`${pin.length} digits entered`}>
        {Array.from({ length: Math.max(4, pin.length) }, (_, i) => (
          <span
            key={i}
            className={cn("size-3 rounded-full border transition-colors", i < pin.length ? "border-accent bg-accent" : "border-border-strong")}
          />
        ))}
      </div>
      <p role="alert" className="mt-3 min-h-5 text-sm text-danger">
        {error}
      </p>
      <div className="mt-2 grid grid-cols-3 gap-3">
        {PAD.map((key) => (
          <button
            key={key}
            type="button"
            disabled={busy || (key === "ok" && pin.length < 4)}
            onClick={() => press(key)}
            aria-label={key === "back" ? "Delete digit" : key === "ok" ? "Switch" : key}
            className={cn(
              "flex size-16 items-center justify-center rounded-full text-xl font-semibold transition-colors disabled:opacity-40",
              key === "ok" ? "bg-accent text-accent-fg hover:bg-accent-hover" : "bg-surface-2 text-fg hover:bg-surface-3",
            )}
          >
            {key === "back" ? <Delete className="size-5" /> : key === "ok" ? busy ? <Spinner className="size-5 text-accent-fg" /> : "OK" : key}
          </button>
        ))}
      </div>
      <Button variant="ghost" className="mt-6" icon={<ArrowLeft className="size-4" />} onClick={onCancel}>
        Back to profiles
      </Button>
    </div>
  );
}

function PasswordPrompt({
  target,
  busy,
  error,
  onSubmit,
  onCancel,
}: {
  target: User;
  busy: boolean;
  error: string | null;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}) {
  const [password, setPassword] = useState("");
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (password) onSubmit(password);
  };
  return (
    <form onSubmit={submit} className="flex w-full max-w-xs flex-col items-center">
      <Avatar user={target} size="lg" />
      <div className="mt-3 text-lg font-semibold">{target.name}</div>
      <p className="mt-1 text-center text-sm text-muted">This is an administrator profile. Enter its password to switch.</p>
      <TextInput
        autoFocus
        type="password"
        autoComplete="current-password"
        aria-label="Password"
        className="mt-5"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <p role="alert" className="mt-2 min-h-5 text-sm text-danger">
        {error}
      </p>
      <div className="mt-2 flex w-full gap-2">
        <Button variant="ghost" className="flex-1" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" className="flex-1" loading={busy} disabled={!password}>
          Switch
        </Button>
      </div>
    </form>
  );
}

/** "Who's watching?": pick a home profile for this device. */
export function ProfilesPage() {
  useDocumentTitle("Who's watching?");
  const me = useAuth();
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const navigate = useNavigate();
  const profiles = useProfiles();
  const switchProfile = useSwitchProfile();
  const audio = useOptionalAudioPlayer();
  const [target, setTarget] = useState<User | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [failures, setFailures] = useState(0);

  const doSwitch = (user: User, secret: { pin?: string; password?: string } = {}) => {
    setError(null);
    switchProfile.mutate(
      { user_id: user.id, ...secret },
      {
        onSuccess: () => {
          audio?.close();
          void navigate(next, { replace: true });
        },
        onError: (err) => {
          setError(switchError(err));
          setFailures((n) => n + 1);
        },
      },
    );
  };

  const choose = (user: User) => {
    const needs = challenge(user, me);
    if (user.id === me.id) return navigate(next, { replace: true });
    setError(null);
    if (needs === "none") doSwitch(user);
    else setTarget(user);
  };

  const list = profiles.data ?? [];
  const needs = target ? challenge(target, me) : "none";

  return (
    <div className="relative flex min-h-screen flex-col items-center justify-center overflow-hidden px-4 py-12">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 opacity-60"
        style={{
          background:
            "radial-gradient(900px 500px at 50% -10%, color-mix(in oklab, var(--color-accent) 16%, transparent), transparent 60%)",
        }}
      />
      <div className="relative flex w-full max-w-4xl animate-pop-in flex-col items-center">
        <Logo className="mb-10 h-9" />
        {target && needs === "pin" ? (
          <PinPad
            // Every failed attempt starts the pad over (clears the dots).
            key={failures}
            target={target}
            busy={switchProfile.isPending}
            error={error}
            onSubmit={(pin) => doSwitch(target, { pin })}
            onCancel={() => {
              setTarget(null);
              setError(null);
            }}
          />
        ) : target && needs === "password" ? (
          <PasswordPrompt
            target={target}
            busy={switchProfile.isPending}
            error={error}
            onSubmit={(password) => doSwitch(target, { password })}
            onCancel={() => {
              setTarget(null);
              setError(null);
            }}
          />
        ) : (
          <>
            <h1 className="text-3xl font-bold tracking-tight sm:text-4xl">Who's watching?</h1>
            {profiles.isPending ? (
              <Spinner className="mt-12 size-8" />
            ) : (
              <ul className="mt-10 flex flex-wrap justify-center gap-6 sm:gap-10">
                {list.map((p) => (
                  <li key={p.id}>
                    <button
                      type="button"
                      onClick={() => void choose(p)}
                      disabled={switchProfile.isPending}
                      className="group flex w-28 flex-col items-center gap-3 rounded-xl outline-none sm:w-32"
                    >
                      <span
                        className={cn(
                          "relative rounded-full ring-offset-4 ring-offset-bg transition group-hover:ring-4 group-hover:ring-accent group-focus-visible:ring-4 group-focus-visible:ring-accent",
                          p.id === me.id && "ring-2 ring-accent/60",
                        )}
                      >
                        <Avatar user={p} size="xl" />
                        {p.has_pin && p.id !== me.id ? (
                          <span className="absolute bottom-1 right-1 flex size-7 items-center justify-center rounded-full bg-bg text-muted shadow">
                            <Lock className="size-3.5" />
                          </span>
                        ) : null}
                      </span>
                      <span className="max-w-full truncate text-sm font-medium text-muted transition-colors group-hover:text-fg">
                        {p.name}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {error ? (
              <p role="alert" className="mt-6 text-sm text-danger">
                {error}
              </p>
            ) : null}
            <Button variant="ghost" className="mt-10" onClick={() => void navigate(next, { replace: true })}>
              Continue as {me.name}
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
