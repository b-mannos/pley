import { useState, type FormEvent } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { errorMessage, isApiError } from "@/api/client";
import { fetchProfiles, useLogin, useMe, useServer } from "@/api/hooks";
import { Splash } from "@/auth";
import { Button, buttonClass, Field, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle } from "@/lib/hooks";
import { appShell, SWITCH_SERVER_PATH } from "@/lib/shell";
import { ssoNotice, ssoStartUrl } from "@/lib/sso";
import { AuthLayout } from "./AuthLayout";
import { safeNext } from "./ProfilesPage";

export function LoginPage() {
  useDocumentTitle("Sign in");
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const navigate = useNavigate();
  const qc = useQueryClient();
  const server = useServer();
  const me = useMe(!server.data?.setup_required);
  const login = useLogin();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [signingIn, setSigningIn] = useState(false);

  if (server.data?.setup_required) return <Navigate to="/setup" replace />;
  if (me.data && !signingIn) return <Navigate to={next} replace />;
  if (server.isPending || (me.isPending && me.fetchStatus === "fetching")) return <Splash />;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setSigningIn(true);
    login.mutate(
      { username: username.trim(), password },
      {
        // A household with several profiles picks one first.
        onSuccess: () => {
          void fetchProfiles(qc)
            .catch(() => [])
            .then((profiles) =>
              navigate(profiles.length > 1 ? `/profiles?next=${encodeURIComponent(next)}` : next, { replace: true }),
            );
        },
        onError: () => setSigningIn(false),
      },
    );
  };

  // 403: disabled account, 429: too many attempts; the server explains both.
  const error = login.error
    ? isApiError(login.error, 401)
      ? "Wrong username or password."
      : errorMessage(login.error)
    : null;
  const sso = server.data?.sso ?? null;
  // Back from the provider without being signed in.
  const notice = ssoNotice(params.get("sso"), sso?.name ?? "the provider");

  return (
    <AuthLayout title="Sign in" subtitle={server.data ? `to ${server.data.name}` : undefined}>
      <form onSubmit={onSubmit} className="space-y-4">
        {notice ? (
          <p
            role="alert"
            className={cn(
              "rounded-md border px-3 py-2 text-sm",
              notice.tone === "success" ? "border-success/40 bg-success/10 text-success" : "border-danger/40 bg-danger/10 text-danger",
            )}
          >
            {notice.text}
          </p>
        ) : null}
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
        {sso ? (
          <>
            <div className="flex items-center gap-3 text-xs text-subtle" aria-hidden>
              <span className="h-px flex-1 bg-border" />
              or
              <span className="h-px flex-1 bg-border" />
            </div>
            <a href={ssoStartUrl(sso.start, next)} className={buttonClass("outline", "lg", "w-full")}>
              Sign in with {sso.name}
            </a>
          </>
        ) : null}
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
