import { useState, type FormEvent } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";
import { errorMessage, isApiError } from "@/api/client";
import { useLogin, useMe, useServer } from "@/api/hooks";
import { Splash } from "@/auth";
import { Button, Field, TextInput } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { AuthLayout } from "./AuthLayout";

function safeNext(next: string | null): string {
  if (!next || !next.startsWith("/") || next.startsWith("//") || next.startsWith("/login")) return "/";
  return next;
}

export function LoginPage() {
  useDocumentTitle("Sign in");
  const [params] = useSearchParams();
  const next = safeNext(params.get("next"));
  const navigate = useNavigate();
  const server = useServer();
  const me = useMe(server.isSuccess && !server.data.setup_required);
  const login = useLogin();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  if (server.data?.setup_required) return <Navigate to="/setup" replace />;
  if (me.data) return <Navigate to={next} replace />;
  if (server.isPending || (me.isPending && me.fetchStatus === "fetching")) return <Splash />;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    login.mutate(
      { username: username.trim(), password },
      { onSuccess: () => navigate(next, { replace: true }) },
    );
  };

  const error = login.error
    ? isApiError(login.error, 401)
      ? "Wrong username or password."
      : errorMessage(login.error)
    : null;

  return (
    <AuthLayout title="Sign in" subtitle={server.data ? `to ${server.data.name}` : undefined}>
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
        <Button type="submit" variant="primary" size="lg" className="w-full" loading={login.isPending}>
          Sign in
        </Button>
      </form>
    </AuthLayout>
  );
}
