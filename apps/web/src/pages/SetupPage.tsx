import { useState, type FormEvent } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router";
import { errorMessage } from "@/api/client";
import { useServer, useSetup } from "@/api/hooks";
import { ErrorScreen, Splash } from "@/auth";
import { PasswordInput } from "@/components/PasswordInput";
import { Button, Field, TextInput } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { MIN_PASSWORD_LENGTH, passwordProblem } from "@/lib/passwords";
import { AuthLayout } from "./AuthLayout";

export function SetupPage() {
  useDocumentTitle("Welcome");
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const server = useServer();
  const setup = useSetup();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  // Links printed by the server carry the code.
  const [code, setCode] = useState(params.get("code") ?? "");
  const [localError, setLocalError] = useState<string | null>(null);

  if (server.isPending) return <Splash />;
  if (server.isError) {
    return <ErrorScreen title="Can't reach the pley server" message={errorMessage(server.error)} onRetry={() => void server.refetch()} />;
  }
  if (!server.data.setup_required && !setup.isSuccess) return <Navigate to="/login" replace />;
  const needsCode = server.data.setup_code_required === true;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const problem = passwordProblem(password, username);
    setLocalError(problem);
    if (problem) return;
    setup.mutate(
      { username: username.trim(), password, ...(needsCode ? { setup_code: code.trim() } : {}) },
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
        <Field
          label="Password"
          htmlFor="setup-password"
          hint={`At least ${MIN_PASSWORD_LENGTH} characters. A few words that don't go together are easy to remember and hard to guess.`}
        >
          <PasswordInput id="setup-password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        </Field>
        {needsCode ? (
          <Field
            label="Setup code"
            htmlFor="setup-code"
            hint="You're not on the server's network, so pley asks for the code it printed in its log when it started (with Docker: docker logs pley)."
          >
            <TextInput
              id="setup-code"
              autoComplete="one-time-code"
              autoCapitalize="characters"
              spellCheck={false}
              placeholder="ABCD-EFGH"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              className="font-mono uppercase tracking-wider"
              required
            />
          </Field>
        ) : null}
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
