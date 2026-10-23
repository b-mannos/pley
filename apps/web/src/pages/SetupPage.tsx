import { useState, type FormEvent } from "react";
import { Navigate, useNavigate } from "react-router";
import { errorMessage } from "@/api/client";
import { useServer, useSetup } from "@/api/hooks";
import { ErrorScreen, Splash } from "@/auth";
import { Button, Field, TextInput } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { AuthLayout } from "./AuthLayout";

export function SetupPage() {
  useDocumentTitle("Welcome");
  const navigate = useNavigate();
  const server = useServer();
  const setup = useSetup();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  if (server.isPending) return <Splash />;
  if (server.isError) {
    return <ErrorScreen title="Can't reach the pley server" message={errorMessage(server.error)} onRetry={() => void server.refetch()} />;
  }
  if (!server.data.setup_required && !setup.isSuccess) return <Navigate to="/login" replace />;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setLocalError(null);
    if (password.length < 4) {
      setLocalError("Use at least 4 characters for the password.");
      return;
    }
    if (password !== confirm) {
      setLocalError("Passwords don't match.");
      return;
    }
    setup.mutate(
      { username: username.trim(), password },
      { onSuccess: () => void navigate("/settings/libraries?add=1", { replace: true }) },
    );
  };

  const error = localError ?? (setup.error ? errorMessage(setup.error) : null);

  return (
    <AuthLayout
      title="Welcome to pley"
      subtitle="Create the administrator account for this server. You'll add your media libraries next."
    >
      <form onSubmit={onSubmit} className="space-y-4">
        <Field label="Username" htmlFor="setup-username">
          <TextInput
            id="setup-username"
            autoComplete="username"
            autoFocus
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
        </Field>
        <Field label="Password" htmlFor="setup-password">
          <TextInput
            id="setup-password"
            type="password"
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </Field>
        <Field label="Confirm password" htmlFor="setup-confirm">
          <TextInput
            id="setup-confirm"
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            required
          />
        </Field>
        {error ? (
          <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">
            {error}
          </p>
        ) : null}
        <Button type="submit" variant="primary" size="lg" className="w-full" loading={setup.isPending}>
          Create account
        </Button>
      </form>
    </AuthLayout>
  );
}
