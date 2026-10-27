import { useState, type FormEvent } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { TicketX } from "lucide-react";
import { errorMessage, isApiError } from "@/api/client";
import { useAcceptInvite, useInviteCheck, useLogout, useMe } from "@/api/hooks";
import { Splash } from "@/auth";
import { PasswordInput } from "@/components/PasswordInput";
import { Button, buttonClass, Field, TextInput } from "@/components/ui";
import { formatRelative } from "@/lib/format";
import { useDocumentTitle } from "@/lib/hooks";
import { MIN_PASSWORD_LENGTH, passwordProblem } from "@/lib/passwords";
import { AuthLayout } from "./AuthLayout";

/** `/invite/:code`: create an account with an invitation link. */
export function InvitePage() {
  useDocumentTitle("Invitation");
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const check = useInviteCheck(code);
  const me = useMe();
  const logout = useLogout();
  const accept = useAcceptInvite(code);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  if (check.isPending) return <Splash />;
  if (check.isError) {
    const gone = isApiError(check.error, 404);
    return (
      <AuthLayout title={gone ? "This invitation can't be used" : "Something went wrong"}>
        <div className="space-y-5">
          <div className="flex items-start gap-3 text-sm text-muted">
            <TicketX className="mt-0.5 size-5 shrink-0 text-subtle" />
            <p>
              {gone
                ? "The link is wrong, has expired, or has already been used. Ask the person who sent it for a new one."
                : errorMessage(check.error)}
            </p>
          </div>
          <Link to="/login" className={buttonClass("primary", "md", "w-full")}>
            Go to sign in
          </Link>
        </div>
      </AuthLayout>
    );
  }

  const server = check.data.server_name;

  if (me.data && !accept.isSuccess) {
    return (
      <AuthLayout title={`Join ${server}`} subtitle={`You're signed in as ${me.data.name}.`}>
        <div className="space-y-3">
          <p className="text-sm text-muted">To create a new account with this invitation, sign out first.</p>
          <Button
            variant="primary"
            className="w-full"
            loading={logout.isPending}
            onClick={() => logout.mutate(undefined, { onSettled: () => void me.refetch() })}
          >
            Sign out and continue
          </Button>
          <Button variant="ghost" className="w-full" onClick={() => void navigate("/", { replace: true })}>
            Stay signed in
          </Button>
        </div>
      </AuthLayout>
    );
  }

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setLocalError(null);
    if (!username.trim()) return setLocalError("Choose a username.");
    const problem = passwordProblem(password, username);
    if (problem) return setLocalError(problem);
    accept.mutate(
      { username: username.trim(), password, display_name: displayName.trim() || undefined },
      { onSuccess: () => void navigate("/", { replace: true }) },
    );
  };

  const error = localError ?? (accept.error ? errorMessage(accept.error) : null);

  return (
    <AuthLayout
      title={`You're invited to ${server}`}
      subtitle={
        check.data.expires_at
          ? `Create your account to start watching. This invitation expires ${formatRelative(check.data.expires_at)}.`
          : "Create your account to start watching."
      }
    >
      <form onSubmit={onSubmit} className="space-y-4">
        <Field label="Username" htmlFor="invite-username" hint="Letters, digits and _ - . @">
          <TextInput
            id="invite-username"
            autoComplete="username"
            autoFocus
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
        </Field>
        <Field label="Your name" htmlFor="invite-name" hint="Optional: how pley greets you.">
          <TextInput id="invite-name" autoComplete="name" value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
        </Field>
        <Field label="Password" htmlFor="invite-password" hint={`At least ${MIN_PASSWORD_LENGTH} characters.`}>
          <PasswordInput id="invite-password" autoComplete="new-password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        </Field>
        {error ? (
          <p role="alert" className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">
            {error}
          </p>
        ) : null}
        <Button type="submit" variant="primary" size="lg" className="w-full" loading={accept.isPending}>
          Create account
        </Button>
        <p className="text-center text-xs text-subtle">
          Already have an account?{" "}
          <Link to="/login" className="font-medium text-accent hover:text-accent-hover">
            Sign in
          </Link>
        </p>
      </form>
    </AuthLayout>
  );
}
