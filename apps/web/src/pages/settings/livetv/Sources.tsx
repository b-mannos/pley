import { useState } from "react";
import { CalendarClock, CircleAlert, FileText, KeyRound, LoaderCircle, Pencil, Plus, Radio, RefreshCw, Satellite, Square, Trash2, Users } from "lucide-react";
import { errorMessage } from "@/api/client";
import {
  useDeleteSource,
  useLiveSources,
  useRefreshSource,
  useStopTuner,
  useTuners,
  useUpdateSource,
  type LiveSource,
  type TunerStatus,
} from "@/api/livetv";
import { ConfirmModal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Badge, Button, Card, CardTitle, EmptyState, IconButton, PageSpinner, Toggle } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatAgo, pluralize } from "@/lib/format";
import { accountLine, expiresSoon, formatFor, formatKbps, planLine } from "./common";
import { SourceModal } from "./SourceModal";

export function Sources() {
  const sources = useLiveSources();
  const [editing, setEditing] = useState<LiveSource | null>(null);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<LiveSource | null>(null);
  const remove = useDeleteSource();
  const toast = useToast();

  if (sources.isPending) return <PageSpinner />;
  if (sources.isError) {
    return <EmptyState title="Couldn't load the sources" description={errorMessage(sources.error)} action={<Button onClick={() => void sources.refetch()}>Retry</Button>} />;
  }
  const list = sources.data;
  return (
    <div className="space-y-5">
      <Connections sources={list} />

      <div className="flex items-center justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold">Sources</h3>
          <p className="text-sm text-muted">Where channels come from. Channels and guides are read again on their own.</p>
        </div>
        {list.length ? (
          <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => setAdding(true)}>
            Add source
          </Button>
        ) : null}
      </div>

      {list.length === 0 ? (
        <Card className="py-2">
          <EmptyState
            icon={<Satellite />}
            title="No IPTV source yet"
            description="Add your provider's M3U playlist or Xtream Codes account. Its channels and guide show in Live TV, for everyone you let watch."
            action={
              <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => setAdding(true)}>
                Add a source
              </Button>
            }
          />
        </Card>
      ) : (
        <ul className="space-y-3">
          {list.map((s) => (
            <li key={s.id}>
              <SourceCard source={s} onEdit={() => setEditing(s)} onDelete={() => setDeleting(s)} />
            </li>
          ))}
        </ul>
      )}

      <SourceModal open={adding || editing !== null} source={editing} onClose={() => (setAdding(false), setEditing(null))} />
      <ConfirmModal
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        title={`Remove ${deleting?.name ?? "this source"}?`}
        message="Its channels, their guide and everyone's favourites of them go too. Anyone watching one stops."
        confirmLabel="Remove"
        danger
        loading={remove.isPending}
        onConfirm={() =>
          deleting &&
          remove.mutate(deleting.id, {
            onSuccess: () => {
              toast.success(`${deleting.name} removed`);
              setDeleting(null);
            },
            onError: (e) => toast.error(errorMessage(e)),
          })
        }
      />
    </div>
  );
}

function SourceCard({ source: s, onEdit, onDelete }: { source: LiveSource; onEdit: () => void; onDelete: () => void }) {
  const refresh = useRefreshSource();
  const update = useUpdateSource();
  const toast = useToast();
  const soon = expiresSoon(s.account);
  const busy = s.refreshing || refresh.isPending;
  const visible = s.channel_count - s.hidden_count;
  const guided = s.channel_count ? Math.round((s.guided_count / s.channel_count) * 100) : 0;
  const Kind = s.kind === "xtream" ? KeyRound : FileText;
  return (
    <Card className={cn("p-4 transition-opacity sm:p-5", !s.enabled && "opacity-60")}>
      <div className="flex items-start gap-4">
        <div className="hidden size-11 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-accent sm:flex">
          <Kind className="size-5" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="truncate font-semibold">{s.name}</h4>
            <Badge>{s.kind === "xtream" ? "Xtream Codes" : "M3U"}</Badge>
            {!s.enabled ? <Badge>Off</Badge> : null}
            {busy ? (
              <Badge tone="info">
                <LoaderCircle className="size-3 animate-spin" /> Reading
              </Badge>
            ) : null}
          </div>
          <div className="mt-0.5 truncate font-mono text-xs text-subtle" title={s.url}>
            {s.url}
          </div>

          <dl className="mt-3 grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-4">
            <Stat label="Channels" value={s.channels_at ? visible.toLocaleString() : "—"} note={s.hidden_count ? `${s.hidden_count} hidden` : undefined} />
            <Stat label="In the guide" value={s.guide_at ? `${guided}%` : "—"} note={s.guide_at ? `${s.guided_count.toLocaleString()} channels` : "no guide yet"} />
            <Stat
              label="Connections"
              value={s.max_connections === 0 ? `${s.connections_in_use} in use` : `${s.connections_in_use} of ${s.max_connections}`}
              note={s.max_connections === 0 ? "no limit" : s.connections_in_use >= s.max_connections ? "all in use" : "in use"}
              meter={s.max_connections ? s.connections_in_use / s.max_connections : undefined}
            />
            <Stat label="Read" value={s.channels_at ? formatAgo(s.channels_at) : "never"} note={s.guide_at ? `guide ${formatAgo(s.guide_at)}` : undefined} />
          </dl>

          {s.account ? (
            <div className={cn("mt-3 flex items-center gap-1.5 text-xs", soon !== null ? "text-danger" : "text-muted")}>
              <CalendarClock className="size-3.5" />
              {accountLine(s.account)}
              {soon !== null ? <strong className="ml-1">{soon <= 0 ? "· renew it" : `· ${pluralize(soon, "day")} left`}</strong> : null}
            </div>
          ) : null}
          {s.channels_error ? <Problem>{s.channels_error}</Problem> : null}
          {s.guide_error ? <Problem>Guide: {s.guide_error}</Problem> : null}
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2 sm:flex-row sm:items-center">
          <Toggle
            checked={s.enabled}
            label={s.enabled ? `Turn ${s.name} off` : `Turn ${s.name} on`}
            onChange={(enabled) =>
              update.mutate(
                {
                  id: s.id,
                  input: {
                    name: s.name,
                    kind: s.kind,
                    url: s.url,
                    username: s.username,
                    guide_url: s.guide_url,
                    user_agent: s.user_agent,
                    max_connections: s.max_connections,
                    enabled,
                  },
                },
                { onError: (e) => toast.error(errorMessage(e)) },
              )
            }
          />
          <div className="flex">
            <IconButton
              label="Read again now"
              size="sm"
              disabled={busy || !s.enabled}
              onClick={() => refresh.mutate(s.id, { onError: (e) => toast.error(errorMessage(e)) })}
            >
              <RefreshCw className={cn("size-4", busy && "animate-spin")} />
            </IconButton>
            <IconButton label="Edit" size="sm" onClick={onEdit}>
              <Pencil className="size-4" />
            </IconButton>
            <IconButton label="Remove" size="sm" onClick={onDelete} className="hover:text-danger">
              <Trash2 className="size-4" />
            </IconButton>
          </div>
        </div>
      </div>
    </Card>
  );
}

function Stat({ label, value, note, meter }: { label: string; value: string; note?: string; meter?: number }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs text-subtle">{label}</dt>
      <dd className="font-semibold tabular-nums">{value}</dd>
      {meter !== undefined ? (
        <div className="mt-1 h-1 w-full max-w-24 overflow-hidden rounded-full bg-white/10">
          <div className={cn("h-full rounded-full", meter >= 1 ? "bg-danger" : "bg-accent")} style={{ width: `${Math.min(1, meter) * 100}%` }} />
        </div>
      ) : null}
      {note ? <dd className="truncate text-xs text-subtle">{note}</dd> : null}
    </div>
  );
}

function Problem({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-2 flex items-start gap-1.5 text-xs text-danger">
      <CircleAlert className="mt-px size-3.5 shrink-0" />
      <span className="break-words">{children}</span>
    </p>
  );
}

/** What is being watched now: each connection, who is on it, what it carries. */
function Connections({ sources }: { sources: LiveSource[] }) {
  const tuners = useTuners(sources.length > 0);
  const stop = useStopTuner();
  const toast = useToast();
  if (!sources.length) return null;
  const list = tuners.data ?? [];
  return (
    <Card>
      <CardTitle icon={<Radio />}>Watching now</CardTitle>
      {list.length === 0 ? (
        <p className="text-sm text-muted">Nobody is watching. Each channel being watched takes one of its source's connections.</p>
      ) : (
        <ul className="divide-y divide-border">
          {list.map((t) => (
            <TunerRow
              key={t.channel_id}
              tuner={t}
              stopping={stop.isPending && stop.variables === t.channel_id}
              onStop={() => stop.mutate(t.channel_id, { onError: (e) => toast.error(errorMessage(e)) })}
            />
          ))}
        </ul>
      )}
    </Card>
  );
}

function TunerRow({ tuner: t, stopping, onStop }: { tuner: TunerStatus; stopping: boolean; onStop: () => void }) {
  return (
    <li className="flex items-center gap-3 py-3 first:pt-0 last:pb-0">
      <span className={cn("size-2 shrink-0 rounded-full", t.error ? "bg-danger" : t.connected ? "bg-success animate-pulse" : "bg-white/30")} />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5">
          <span className="truncate font-medium">{t.channel_name}</span>
          <span className="text-xs text-subtle">{t.source_name}</span>
          {t.idle ? <Badge>Letting go</Badge> : null}
        </div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-muted">
          <span className="flex items-center gap-1">
            <Users className="size-3" />
            {t.viewers.length ? t.viewers.map((v) => v.username).join(", ") : "nobody"}
          </span>
          {t.kbps ? <span className="tabular-nums">{formatKbps(t.kbps)}</span> : null}
          <span>{formatFor(t.started_at)}</span>
          {t.plan ? <span className="truncate">{planLine(t.plan)}</span> : null}
        </div>
        {t.error ? <div className="mt-0.5 truncate text-xs text-danger">{t.error}</div> : null}
      </div>
      <Button size="sm" variant="ghost" icon={<Square className="size-3.5 fill-current" />} loading={stopping} onClick={onStop}>
        Stop
      </Button>
    </li>
  );
}
