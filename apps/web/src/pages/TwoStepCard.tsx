import { useState, type FormEvent, type ReactNode } from "react";
import { Copy, Download, ShieldCheck } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useDisableTwoStep, useEnableTwoStep, useNewRecoveryCodes, useServer, useStartTwoStep, useTwoStep } from "@/api/hooks";
import type { TwoStepSetup, TwoStepStatus } from "@/api/types";
import { useAuth } from "@/auth";
import { QrCode } from "@/components/QrCode";
import { useToast } from "@/components/Toast";
import { Button, Card, CardTitle, Field, IconButton, TextInput } from "@/components/ui";
import { copyText } from "@/lib/clipboard";
import { cn } from "@/lib/cn";
import { formatDay, pluralize } from "@/lib/format";
import { appShell } from "@/lib/shell";
import { codeKind, downloadText, groupSecret, recoveryCodesText } from "@/lib/twoStep";

/** Where the card is: showing the status, asking for the password, scanning, turning off, or showing new codes. */
type Stage =
  | { kind: "status" }
  | { kind: "password"; next: "setup" | "codes" }
  | { kind: "scan"; setup: TwoStepSetup }
  | { kind: "off" }
  | { kind: "codes"; codes: string[]; turnedOn: boolean };

/** Recovery codes left at which the card suggests making new ones. */
const FEW_CODES = 3;

/** Two-step sign-in: a code from an authenticator app after the password. */
export function TwoStepCard() {
  const user = useAuth();
  const usable = !user.managed && user.has_password;
  const status = useTwoStep(usable);
  const [stage, setStage] = useState<Stage>({ kind: "status" });
  if (!usable) return null;
  const done = () => setStage({ kind: "status" });

  return (
    <Card>
      <CardTitle icon={<ShieldCheck />}>Two-step sign-in</CardTitle>
      {stage.kind === "password" ? (
        <PasswordStep
          next={stage.next}
          onCancel={done}
          onSetup={(setup) => setStage({ kind: "scan", setup })}
          onCodes={(codes) => setStage({ kind: "codes", codes, turnedOn: false })}
        />
      ) : stage.kind === "scan" ? (
        <ScanStep setup={stage.setup} onCancel={done} onOn={(codes) => setStage({ kind: "codes", codes, turnedOn: true })} />
      ) : stage.kind === "off" ? (
        <TurnOff onCancel={done} onOff={done} />
      ) : stage.kind === "codes" ? (
        <RecoveryCodes codes={stage.codes} turnedOn={stage.turnedOn} onDone={done} />
      ) : status.isPending ? (
        <p className="text-sm text-subtle">Loading…</p>
      ) : status.isError ? (
        <p className="text-sm text-danger">{errorMessage(status.error)}</p>
      ) : (
        <Status
          status={status.data}
          onSetUp={() => setStage({ kind: "password", next: "setup" })}
          onNewCodes={() => setStage({ kind: "password", next: "codes" })}
          onTurnOff={() => setStage({ kind: "off" })}
        />
      )}
    </Card>
  );
}

function Status({
  status,
  onSetUp,
  onNewCodes,
  onTurnOff,
}: {
  status: TwoStepStatus;
  onSetUp: () => void;
  onNewCodes: () => void;
  onTurnOff: () => void;
}) {
  if (!status.enabled) {
    return (
      <>
        <p className="text-sm text-muted">
          After your password, pley asks for a code from an authenticator app on your phone, so a password alone can't
          sign in to your account.
        </p>
        <div className="mt-4 flex justify-end">
          <Button variant="primary" onClick={onSetUp}>
            Set up
          </Button>
        </div>
      </>
    );
  }
  const left = status.recovery_codes_left;
  return (
    <>
      <p className="text-sm text-muted">
        On since {formatDay(status.since)}: signing in takes your password and a code from your authenticator app.
      </p>
      <p className={cn("mt-2 text-sm", left <= FEW_CODES ? "text-danger" : "text-subtle")}>
        {left === 0
          ? "No recovery codes left: make new ones, so a lost phone doesn't lock you out."
          : left <= FEW_CODES
            ? `Only ${pluralize(left, "recovery code")} left: make new ones before they run out.`
            : `${pluralize(left, "recovery code")} left, for when your phone isn't at hand.`}
      </p>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        <Button variant="ghost" onClick={onTurnOff}>
          Turn off
        </Button>
        <Button variant="outline" onClick={onNewCodes}>
          New recovery codes
        </Button>
      </div>
    </>
  );
}

function Problem({ children }: { children: ReactNode }) {
  return children ? <p className="text-sm text-danger">{children}</p> : null;
}

/** Changing how the account signs in takes its password: a session alone isn't enough. */
function PasswordStep({
  next,
  onCancel,
  onSetup,
  onCodes,
}: {
  next: "setup" | "codes";
  onCancel: () => void;
  onSetup: (setup: TwoStepSetup) => void;
  onCodes: (codes: string[]) => void;
}) {
  const start = useStartTwoStep();
  const codes = useNewRecoveryCodes();
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const busy = start.isPending || codes.isPending;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    const onError = (err: unknown) => setError(errorMessage(err));
    if (next === "setup") start.mutate(password, { onSuccess: onSetup, onError });
    else codes.mutate(password, { onSuccess: (res) => onCodes(res.recovery_codes), onError });
  };

  return (
    <form onSubmit={onSubmit} className="space-y-3">
      <p className="text-sm text-muted">
        {next === "setup"
          ? "You'll need an authenticator app on your phone, such as Google Authenticator, Microsoft Authenticator, 1Password or Bitwarden."
          : "New recovery codes replace the ones you have: those stop working."}
      </p>
      <Field label="Your password" htmlFor="two-step-password">
        <TextInput
          id="two-step-password"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
        />
      </Field>
      <Problem>{error}</Problem>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" loading={busy} disabled={!password}>
          {next === "setup" ? "Continue" : "Make new codes"}
        </Button>
      </div>
    </form>
  );
}

/** The QR code (or the key, or a link on phones) and the first code from the app, which turns it on. */
function ScanStep({ setup, onCancel, onOn }: { setup: TwoStepSetup; onCancel: () => void; onOn: (codes: string[]) => void }) {
  const enable = useEnableTwoStep();
  const toast = useToast();
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  // On a phone the code can't be scanned with the phone: a link opens the app instead.
  const touch = typeof window !== "undefined" && !!window.matchMedia?.("(pointer: coarse)").matches;

  const confirm = (value: string) => {
    setError(null);
    enable.mutate(value, {
      onSuccess: (res) => {
        toast.success("Two-step sign-in is on");
        onOn(res.recovery_codes);
      },
      onError: (err) => setError(errorMessage(err)),
    });
  };

  const type = (value: string) => {
    const digits = value.replace(/\D/g, "").slice(0, 6);
    setCode(digits);
    if (digits.length === 6 && !enable.isPending) confirm(digits);
  };

  return (
    <div className="space-y-4">
      <div className="grid gap-5 sm:grid-cols-[auto_1fr] sm:items-start">
        <QrCode
          size={setup.qr.size}
          path={setup.qr.path}
          label="QR code that adds pley to an authenticator app"
          className="mx-auto size-44 shrink-0 sm:mx-0"
        />
        <div className="min-w-0 space-y-3 text-sm">
          <ol className="list-decimal space-y-1.5 pl-5 text-muted">
            <li>Scan this QR code with your authenticator app.</li>
            <li>Enter the 6-digit code the app shows for pley.</li>
          </ol>
          {touch ? (
            <a href={setup.uri} className="inline-block font-medium text-accent hover:text-accent-hover">
              Open in an authenticator app on this phone
            </a>
          ) : null}
          <div className="rounded-md border border-border bg-surface-2 px-3 py-2">
            <div className="text-xs text-subtle">Can't scan it? Enter this key in the app instead:</div>
            <div className="mt-1 flex items-center gap-1">
              <code className="min-w-0 flex-1 break-all font-mono text-sm text-fg">{groupSecret(setup.secret)}</code>
              <IconButton
                label="Copy the key"
                size="sm"
                onClick={() =>
                  void copyText(setup.secret).then((ok) => (ok ? toast.success("Key copied") : toast.error("Couldn't copy the key")))
                }
              >
                <Copy className="size-4" />
              </IconButton>
            </div>
          </div>
        </div>
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (code.length === 6) confirm(code);
        }}
        className="space-y-3"
      >
        <Field label="Code from the app" htmlFor="two-step-code">
          <TextInput
            id="two-step-code"
            inputMode="numeric"
            autoComplete="one-time-code"
            autoFocus
            maxLength={6}
            value={code}
            onChange={(e) => type(e.target.value)}
            className="max-w-40 tracking-[0.3em] tabular-nums"
          />
        </Field>
        <Problem>{error}</Problem>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel} disabled={enable.isPending}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={enable.isPending} disabled={code.length !== 6}>
            Turn on
          </Button>
        </div>
      </form>
    </div>
  );
}

/** Turning it off takes the password and a code: neither alone can. */
function TurnOff({ onCancel, onOff }: { onCancel: () => void; onOff: () => void }) {
  const disable = useDisableTwoStep();
  const toast = useToast();
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    disable.mutate(
      { password, code: code.trim() },
      {
        onSuccess: () => {
          toast.success("Two-step sign-in is off");
          onOff();
        },
        onError: (err) => setError(errorMessage(err)),
      },
    );
  };

  return (
    <form onSubmit={onSubmit} className="space-y-3">
      <p className="text-sm text-muted">Turning it off takes your password and a code from the app, or a recovery code.</p>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Your password" htmlFor="two-step-off-password">
          <TextInput
            id="two-step-off-password"
            type="password"
            autoComplete="current-password"
            autoFocus
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
        </Field>
        <Field label="Code" htmlFor="two-step-off-code">
          <TextInput
            id="two-step-off-code"
            autoComplete="one-time-code"
            autoCapitalize="off"
            spellCheck={false}
            value={code}
            onChange={(e) => setCode(e.target.value)}
            required
          />
        </Field>
      </div>
      <Problem>{error}</Problem>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel} disabled={disable.isPending}>
          Cancel
        </Button>
        <Button type="submit" variant="danger" loading={disable.isPending} disabled={!password || !codeKind(code)}>
          Turn off
        </Button>
      </div>
    </form>
  );
}

/** New recovery codes, shown once. */
function RecoveryCodes({ codes, turnedOn, onDone }: { codes: string[]; turnedOn: boolean; onDone: () => void }) {
  const user = useAuth();
  const server = useServer();
  const toast = useToast();
  const text = recoveryCodesText(codes, user.username, server.data?.name);

  return (
    <div className="space-y-3">
      <p className="text-sm text-muted">
        Save these recovery codes somewhere safe, like a password manager. If you lose your phone, each one signs in
        once instead of a code from the app. They won't be shown again.
      </p>
      <ul
        aria-label="Recovery codes"
        className="grid gap-x-8 gap-y-1 rounded-md border border-border bg-surface-2 px-4 py-3 font-mono text-sm text-fg sm:grid-cols-2"
      >
        {codes.map((c) => (
          <li key={c}>{c}</li>
        ))}
      </ul>
      {turnedOn ? (
        <p className="text-xs text-subtle">Devices already signed in stay signed in: sign out the ones you don't know under Devices.</p>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          variant="ghost"
          icon={<Copy className="size-4" />}
          onClick={() => void copyText(text).then((ok) => (ok ? toast.success("Recovery codes copied") : toast.error("Couldn't copy them")))}
        >
          Copy
        </Button>
        {/* The apps' web views don't save files. */}
        {appShell() ? null : (
          <Button variant="ghost" icon={<Download className="size-4" />} onClick={() => downloadText("pley-recovery-codes.txt", text)}>
            Download
          </Button>
        )}
        <Button variant="primary" onClick={onDone}>
          I've saved them
        </Button>
      </div>
    </div>
  );
}
