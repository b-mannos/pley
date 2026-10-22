import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router";
import { KeyRound, LogOut, ShieldCheck, UserRound } from "lucide-react";
import { errorMessage, isApiError } from "@/api/client";
import { useChangePassword, useLogout } from "@/api/hooks";
import { useAudioPlayer } from "@/audio/AudioPlayer";
import { useAuth } from "@/auth";
import { useToast } from "@/components/Toast";
import { Badge, Button, Card, Field, TextInput } from "@/components/ui";
import { formatTimestamp } from "@/lib/format";
import { useDocumentTitle } from "@/lib/hooks";

export function AccountPage() {
  useDocumentTitle("Account");
  const user = useAuth();
  const navigate = useNavigate();
  const toast = useToast();
  const logout = useLogout();
  const audio = useAudioPlayer();
  const change = useChangePassword();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (next.length < 4) return setError("Use at least 4 characters for the new password.");
    if (next !== confirm) return setError("Passwords don't match.");
    change.mutate(
      { current_password: current, new_password: next },
      {
        onSuccess: () => {
          toast.success("Password changed");
          setCurrent("");
          setNext("");
          setConfirm("");
        },
        onError: (err) =>
          setError(isApiError(err, 401) || isApiError(err, 403) ? "Your current password is incorrect." : errorMessage(err)),
      },
    );
  };

  return (
    <div className="mx-auto max-w-2xl space-y-6 px-4 py-8 sm:px-8">
      <h1 className="text-2xl font-bold tracking-tight">Account</h1>

      <Card className="flex items-center gap-4">
        <div className="flex size-14 items-center justify-center rounded-full bg-surface-3 text-xl font-bold uppercase">
          {user.username.slice(0, 1)}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-lg font-semibold">{user.username}</span>
            {user.is_admin ? (
              <Badge tone="accent">
                <ShieldCheck className="size-3" /> Admin
              </Badge>
            ) : (
              <Badge>
                <UserRound className="size-3" /> User
              </Badge>
            )}
          </div>
          <div className="text-sm text-muted">Member since {formatTimestamp(user.created_at)}</div>
        </div>
        <Button
          variant="outline"
          icon={<LogOut className="size-4" />}
          loading={logout.isPending}
          onClick={() => {
            audio.close();
            logout.mutate(undefined, { onSettled: () => navigate("/login", { replace: true }) });
          }}
        >
          Sign out
        </Button>
      </Card>

      <Card>
        <h2 className="mb-4 flex items-center gap-2 text-base font-semibold">
          <KeyRound className="size-4 text-accent" /> Change password
        </h2>
        <form onSubmit={onSubmit} className="space-y-4">
          <Field label="Current password" htmlFor="pw-current">
            <TextInput
              id="pw-current"
              type="password"
              autoComplete="current-password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
              required
            />
          </Field>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="New password" htmlFor="pw-new">
              <TextInput id="pw-new" type="password" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} required />
            </Field>
            <Field label="Confirm new password" htmlFor="pw-confirm">
              <TextInput
                id="pw-confirm"
                type="password"
                autoComplete="new-password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                required
              />
            </Field>
          </div>
          {error ? <p className="text-sm text-danger">{error}</p> : null}
          <div className="flex justify-end">
            <Button type="submit" variant="primary" loading={change.isPending}>
              Update password
            </Button>
          </div>
        </form>
      </Card>
    </div>
  );
}
