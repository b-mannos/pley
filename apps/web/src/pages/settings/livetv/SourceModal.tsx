import { useState } from "react";
import { ChevronDown, CircleCheck, FileText, KeyRound, TriangleAlert } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useAddSource, useTestSource, useUpdateSource, type LiveSource, type SourceInput, type SourceKind, type SourceTest } from "@/api/livetv";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, Field, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";
import { accountLine } from "./common";

function blank(kind: SourceKind): SourceInput {
  return { name: "", kind, url: "", username: "", password: "", guide_url: "", user_agent: "", max_connections: 1, enabled: true };
}

function fromSource(s: LiveSource): SourceInput {
  return {
    name: s.name,
    kind: s.kind,
    url: s.url,
    username: s.username,
    guide_url: s.guide_url,
    user_agent: s.user_agent,
    max_connections: s.max_connections,
    enabled: s.enabled,
  };
}

/** Adds a source, or changes one (`source`). */
export function SourceModal({ source, open, onClose }: { source: LiveSource | null; open: boolean; onClose: () => void }) {
  return open ? <SourceForm key={source?.id ?? "new"} source={source} onClose={onClose} /> : null;
}

function SourceForm({ source, onClose }: { source: LiveSource | null; onClose: () => void }) {
  const toast = useToast();
  const add = useAddSource();
  const update = useUpdateSource();
  const test = useTestSource();
  const [form, setForm] = useState<SourceInput>(() => (source ? fromSource(source) : blank("m3u")));
  const [advanced, setAdvanced] = useState(!!source?.user_agent);
  const [tested, setTested] = useState<{ form: string; result: SourceTest } | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const set = <K extends keyof SourceInput>(key: K, value: SourceInput[K]) => {
    setForm((f) => ({ ...f, [key]: value }));
    setTestError(null);
  };
  const xtream = form.kind === "xtream";
  const saving = add.isPending || update.isPending;
  const body = (): SourceInput => {
    const out = { ...form, name: form.name.trim() || defaultName(form) };
    // An empty password field keeps the one saved.
    if (source && !form.password) delete out.password;
    return out;
  };
  const key = JSON.stringify(form);
  const result = tested?.form === key ? tested.result : null;
  const complete = form.url.trim() !== "" && (!xtream || form.username.trim() !== "");

  const runTest = () => {
    setTestError(null);
    test.mutate(
      { ...body(), source_id: source?.id },
      {
        onSuccess: (r) => {
          setTested({ form: key, result: r });
          // An Xtream account says how many connections it has.
          if (r.account?.max_connections && !source) set("max_connections", r.account.max_connections);
        },
        onError: (e) => setTestError(errorMessage(e)),
      },
    );
  };
  const save = () => {
    const done = {
      onSuccess: () => {
        toast.success(source ? "Source saved" : "Source added: its channels are being read");
        onClose();
      },
      onError: (e: unknown) => toast.error(errorMessage(e)),
    };
    if (source) update.mutate({ id: source.id, input: body() }, done);
    else add.mutate(body(), done);
  };

  return (
    <Modal
      open
      onClose={onClose}
      locked={saving}
      size="lg"
      title={source ? `Edit ${source.name}` : "Add an IPTV source"}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={runTest} loading={test.isPending} disabled={!complete || saving}>
            Test
          </Button>
          <Button variant="primary" onClick={save} loading={saving} disabled={!complete}>
            {source ? "Save" : "Add source"}
          </Button>
        </>
      }
    >
      <form
        className="space-y-5"
        onSubmit={(e) => {
          e.preventDefault();
          if (complete) save();
        }}
      >
        {!source ? (
          <div className="grid gap-3 sm:grid-cols-2" role="radiogroup" aria-label="Kind of source">
            <KindCard
              active={form.kind === "m3u"}
              onClick={() => set("kind", "m3u")}
              icon={<FileText className="size-5" />}
              title="M3U playlist"
              text="A playlist's address: what most providers give, and HDHomeRun tuners too."
            />
            <KindCard
              active={xtream}
              onClick={() => set("kind", "xtream")}
              icon={<KeyRound className="size-5" />}
              title="Xtream Codes"
              text="A server with a user name and password: channels, groups and guide come with it."
            />
          </div>
        ) : null}

        <Field label="Name" htmlFor="live-name" hint="What it is called in the lists. Leave it empty for the provider's address.">
          <TextInput id="live-name" value={form.name} onChange={(e) => set("name", e.target.value)} placeholder={defaultName(form) || "My provider"} />
        </Field>

        {xtream ? (
          <>
            <Field label="Server" htmlFor="live-url" hint="With its port: http://provider.example:8080">
              <TextInput
                id="live-url"
                value={form.url}
                onChange={(e) => set("url", e.target.value)}
                placeholder="http://provider.example:8080"
                inputMode="url"
                autoComplete="off"
                spellCheck={false}
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="User name" htmlFor="live-user">
                <TextInput id="live-user" value={form.username} onChange={(e) => set("username", e.target.value)} autoComplete="off" spellCheck={false} />
              </Field>
              <Field label="Password" htmlFor="live-pass" hint={source?.password_set ? "Saved. Type a new one to change it." : undefined}>
                <TextInput
                  id="live-pass"
                  type="password"
                  value={form.password ?? ""}
                  onChange={(e) => set("password", e.target.value)}
                  placeholder={source?.password_set ? "••••••••" : ""}
                  autoComplete="new-password"
                />
              </Field>
            </div>
          </>
        ) : (
          <Field label="Playlist address" htmlFor="live-url" hint="Its http:// or https:// address (an .m3u or .m3u8, or get.php).">
            <TextInput
              id="live-url"
              value={form.url}
              onChange={(e) => set("url", e.target.value)}
              placeholder="https://provider.example/playlist.m3u"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
        )}

        <div className="grid gap-4 sm:grid-cols-[1fr_9rem]">
          <Field
            label="Guide (XMLTV)"
            htmlFor="live-guide"
            hint={xtream ? "Leave it empty for the provider's own guide." : "Leave it empty for the one the playlist names, if it names one."}
          >
            <TextInput
              id="live-guide"
              value={form.guide_url}
              onChange={(e) => set("guide_url", e.target.value)}
              placeholder="https://provider.example/guide.xml.gz"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
          <Field label="Connections" htmlFor="live-conn" hint="0: no limit">
            <TextInput
              id="live-conn"
              type="number"
              min={0}
              max={100}
              value={form.max_connections}
              onChange={(e) => set("max_connections", Math.max(0, Math.min(100, Number(e.target.value) || 0)))}
            />
          </Field>
        </div>
        <p className="-mt-2 text-xs text-subtle">
          <strong className="font-medium text-muted">Connections</strong> is how many channels the provider lets the
          account play at once. Everyone watching the same channel shares one; a channel nobody watches gives its
          connection back after a few seconds.
        </p>

        <div>
          <button
            type="button"
            onClick={() => setAdvanced((a) => !a)}
            className="flex items-center gap-1 text-sm font-medium text-muted hover:text-fg"
            aria-expanded={advanced}
          >
            <ChevronDown className={cn("size-4 transition-transform", advanced && "rotate-180")} /> Advanced
          </button>
          {advanced ? (
            <div className="mt-3">
              <Field label="User agent" htmlFor="live-ua" hint="Some providers play only to the app they name (VLC, say). Empty: Pley's own.">
                <TextInput id="live-ua" value={form.user_agent} onChange={(e) => set("user_agent", e.target.value)} placeholder="VLC/3.0.20 LibVLC/3.0.20" spellCheck={false} />
              </Field>
            </div>
          ) : null}
        </div>

        {result ? (
          <div className="flex items-start gap-3 rounded-lg border border-success/30 bg-success/[0.08] p-3 text-sm animate-fade-in">
            <CircleCheck className="mt-0.5 size-5 shrink-0 text-success" />
            <div className="min-w-0">
              <div className="font-medium">
                Found {pluralize(result.channels, "channel")} in {pluralize(result.groups, "group")}
              </div>
              {result.account ? <div className="mt-0.5 text-muted">{accountLine(result.account)}</div> : null}
              <div className="mt-0.5 truncate text-muted">
                {result.guide_url ? (
                  <>
                    Guide: <span className="font-mono text-xs">{result.guide_url}</span>
                  </>
                ) : (
                  "No guide: add one's address for what's on."
                )}
              </div>
            </div>
          </div>
        ) : testError ? (
          <div className="flex items-start gap-3 rounded-lg border border-danger/30 bg-danger/[0.08] p-3 text-sm animate-fade-in" role="alert">
            <TriangleAlert className="mt-0.5 size-5 shrink-0 text-danger" />
            <div className="min-w-0 break-words">{testError}</div>
          </div>
        ) : null}
        <button type="submit" hidden />
      </form>
    </Modal>
  );
}

function defaultName(form: SourceInput): string {
  try {
    return new URL(form.url.trim()).hostname;
  } catch {
    return "";
  }
}

function KindCard({
  active,
  onClick,
  icon,
  title,
  text,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  title: string;
  text: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={active}
      onClick={onClick}
      className={cn(
        "flex items-start gap-3 rounded-lg border p-4 text-left transition-colors",
        active ? "border-accent bg-accent/[0.08]" : "border-border hover:border-border-strong hover:bg-white/[0.02]",
      )}
    >
      <span className={cn("mt-0.5", active ? "text-accent" : "text-subtle")}>{icon}</span>
      <span>
        <span className="block text-sm font-semibold">{title}</span>
        <span className="mt-0.5 block text-xs text-muted">{text}</span>
      </span>
    </button>
  );
}

