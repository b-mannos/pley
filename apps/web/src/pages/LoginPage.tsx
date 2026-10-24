import { useState, type FormEvent } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { errorMessage, isApiError } from "@/api/client";
import { fetchProfiles, useLogin, useMe, useServer } from "@/api/hooks";
import { Splash } from "@/auth";
import { Button, Field, TextInput } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { appShell, SWITCH_SERVER_PATH } from "@/lib/shell";
import { codeKind, isTwoStepError } from "@/lib/twoStep";
import { AuthLayout } from "./AuthLayout";
import { safeNext } from "./ProfilesPage";

export function LoginPage() {
  useDocumentTitle("Sign in");
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  /** Signing in to allow an app (the OAuth authorize page). */
  const forApp = next === "/oauth/authorize" || next.startsWith("/oauth/authorize?");
  const navigate = useNavigate();
  const qc = useQueryClient();
  const server = useServer();
  const me = useMe(!server.data?.setup_required);
  const login = useLogin();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [signingIn, setSigningIn] = useState(false);
  /** The account has two-step sign-in: a code comes after the password. */
  const [step, setStep] = useState<"password" | "code">("password");
  const [code, setCode] = useState("");
  /** A recovery code instead of one from the app. */
  const [recovery, setRecovery] = useState(false);

  if (server.data?.setup_required) return <Navigate to="/setup" replace />;
  if (me.data && !signingIn) return <Navigate to={next} replace />;
  if (server.isPending || (me.isPending && me.fetchStatus === "fetching")) return <Splash />;

  const signIn = (withCode?: string) => {
    setSigningIn(true);
    login.mutate(
      { username: username.trim(), password, ...(withCode ? { code: withCode } : {}) },
      {
        // A household with several profiles picks one first.
        onSuccess: () => {
          void fetchProfiles(qc)
            .catch(() => [])
            .then((profiles) =>
              navigate(profiles.length > 1 ? `/profiles?next=${encodeURIComponent(next)}` : next, { replace: true }),
            );
        },
        onError: (err) => {
          setSigningIn(false);
          if (!withCode && isTwoStepError(err)) setStep("code");
        },
      },
    );
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    signIn();
  };

  const onSubmitCode = (e: FormEvent) => {
    e.preventDefault();
    if (codeKind(code) && !login.isPending) signIn(code.trim());
  };

  const typeCode = (value: string) => {
    if (recovery) return setCode(value);
    const digits = value.replace(/\D/g, "").slice(0, 6);
    setCode(digits);
    // A whole code from the app goes right away.
    if (digits.length === 6 && !login.isPending) signIn(digits);
  };

  const chooseRecovery = (on: boolean) => {
    setRecovery(on);
    setCode("");
    login.reset();
  };

  const back = () => {
    setStep("password");
    setPassword("");
    chooseRecovery(false);
  };

  // 403: disabled account, 429: too many attempts; the server explains both,
  // and a wrong code. Asking for the code isn't an error.
  const error = !login.error
    ? null
    : isTwoStepError(login.error)
      ? login.variables?.code
        ? errorMessage(login.error)
        : null
      : isApiError(login.error, 401)
        ? "Wrong username or password."
        : errorMessage(login.error);

  if (step === "code") {
    return (
      <AuthLayout
        title="Two-step sign-in"
        subtitle={
          recovery
            ? "Enter one of the recovery codes you saved when you turned it on. Each works once."
            : "Enter the 6-digit code from your authenticator app."
        }
      >
        <form onSubmit={onSubmitCode} className="space-y-4">
          <Field label={recovery ? "Recovery code" : "Code"} htmlFor="code">
            <TextInput
              key={recovery ? "recovery" : "app"}
              id="code"
              autoFocus
              autoComplete={recovery ? "off" : "one-time-code"}
              inputMode={recovery ? "text" : "numeric"}
              autoCapitalize="off"
              spellCheck={false}
              maxLength={recovery ? 24 : 6}
              placeholder={recovery ? "xxxx-xxxx-xxxx-xxxx" : undefined}
              value={code}
              onChange={(e) => typeCode(e.target.value)}
              className={
                recovery ? "font-mono" : "indent-[0.4em] text-center text-2xl font-semibold tracking-[0.4em] tabular-nums"
              }
              required
            />
          </Field>
          {error ? (
            <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">
              {error}
            </p>
          ) : null}
          <Button
            type="submit"
            variant="primary"
            size="lg"
            className="w-full"
            loading={login.isPending || (signingIn && !login.isError)}
            disabled={!codeKind(code)}
          >
            Sign in
          </Button>
          <div className="flex items-center justify-between gap-3 text-sm">
            <button type="button" className="font-medium text-accent hover:text-accent-hover" onClick={() => chooseRecovery(!recovery)}>
              {recovery ? "Use your authenticator app" : "Use a recovery code"}
            </button>
            <button type="button" className="text-muted hover:text-fg" onClick={back}>
              Back
            </button>
          </div>
        </form>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      title="Sign in"
      subtitle={server.data ? (forApp ? `to let an app use ${server.data.name}` : `to ${server.data.name}`) : undefined}
    >
      <form onSubmit={onSubmit} className="space-y-4">
        <Field label="Username" htmlFor="username">
          <TextInput
            id="username"
            autoComplete="username"
            autoFocus
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
        </Field>
        <Field label="Password" htmlFor="password">
          <TextInput
            id="password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </Field>
        {error ? (
          <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">
            {error}
          </p>
        ) : null}
        <Button type="submit" variant="primary" size="lg" className="w-full" loading={login.isPending || (signingIn && !login.isError)}>
          Sign in
        </Button>
        {appShell() ? (
          <p className="text-center text-xs text-subtle">
            <a href={SWITCH_SERVER_PATH} className="font-medium text-accent hover:text-accent-hover">
              Use another server
            </a>
          </p>
        ) : null}
      </form>
    </AuthLayout>
  );
}
