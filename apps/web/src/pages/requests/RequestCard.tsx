import { useState, type ReactNode } from "react";
import { Link } from "react-router";
import { Ban, Check, Ellipsis, Hand, Play, RotateCw, Trash2, Undo2, UserMinus } from "lucide-react";
import { errorMessage } from "@/api/client";
import {
  requestProblem,
  useAskRequest,
  useApprovalOptions,
  useApproveRequest,
  useDeclineRequest,
  useDeleteRequest,
  useHandledRequest,
  useUnwatchRequest,
  type Approval,
  type ArrChoices,
  type MediaRequest,
  type RequestAction,
  type RequestSummary,
  type TitleKind,
} from "@/api/requests";
import { Artwork } from "@/components/Artwork";
import { Avatar } from "@/components/Avatar";
import { Menu, MenuItem } from "@/components/Menu";
import { ConfirmModal, Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, buttonClass, Field, IconButton, Select, Skeleton, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatAgo, formatBytes, formatTimestamp } from "@/lib/format";
import { useOnChange } from "@/lib/hooks";
import { actionLabel, actionQuestion, askedText, decidedText, kindLabel, problemText, quoted, seasonsLabel } from "@/lib/requests";
import { itemUrl, titleUrl } from "@/lib/urls";
import { StagePill } from "./shared";

/** Where an approved request of this kind goes. */
function service(request: Pick<MediaRequest, "kind">): "Radarr" | "Sonarr" {
  return request.kind === "movie" ? "Radarr" : "Sonarr";
}

function When({ ts }: { ts: number }) {
  return (
    <time dateTime={new Date(ts * 1000).toISOString()} title={formatTimestamp(ts)}>
      {formatAgo(ts)}
    </time>
  );
}

/** A failed answer, as a notice says it (with the limits, where asking went past one). */
export function failure(
  err: unknown,
  summary: Pick<RequestSummary, "manage"> & Partial<Pick<RequestSummary, "movies_limit" | "seasons_limit">>,
  kind: TitleKind,
): string {
  const problem = requestProblem(err);
  if (!problem) return errorMessage(err);
  const limits = { movies_limit: summary.movies_limit ?? null, seasons_limit: summary.seasons_limit ?? null };
  return problemText(problem, { manage: summary.manage, kind, limits });
}

function Checkbox({ checked }: { checked: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "flex size-4 shrink-0 items-center justify-center rounded border transition-colors peer-focus-visible:ring-2 peer-focus-visible:ring-accent",
        checked ? "border-accent bg-accent text-accent-fg" : "border-border-strong",
      )}
    >
      {checked ? <Check className="size-3" strokeWidth={3} /> : null}
    </span>
  );
}

/** A labelled checkbox that looks like the app's. */
export function CheckRow({
  id,
  checked,
  disabled,
  onChange,
  children,
  aside,
}: {
  id: string;
  checked: boolean;
  disabled?: boolean;
  onChange: () => void;
  children: ReactNode;
  aside?: ReactNode;
}) {
  return (
    <label
      htmlFor={id}
      className={cn("flex items-center gap-3 rounded px-2 py-1.5 text-sm", disabled ? "opacity-55" : "cursor-pointer hover:bg-white/5")}
    >
      <input id={id} type="checkbox" className="peer sr-only" checked={checked} disabled={disabled} onChange={onChange} />
      <Checkbox checked={checked} />
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {aside ? <span className="shrink-0 text-xs text-subtle">{aside}</span> : null}
    </label>
  );
}

/** The profile and folder a request is preselected with: anime's for anime, else the settings' own. */
function preselected(choices: ArrChoices, anime: boolean): { profile: number | null; folder: string | null } {
  return {
    profile: (anime ? choices.anime_quality_profile : null) ?? choices.quality_profile,
    folder: (anime ? choices.anime_root_folder : null) ?? choices.root_folder,
  };
}

/**
 * Approving with choices of its own: Radarr's or Sonarr's profile and
 * folder (the settings' ones preselected, anime's for anime), and for a
 * show which of the seasons asked for to keep. A service that didn't
 * answer, with nothing known of it, leaves only the settings' choices.
 */
export function ApproveDialog({ request, open, onClose }: { request: MediaRequest; open: boolean; onClose: () => void }) {
  const options = useApprovalOptions(open);
  const approve = useApproveRequest();
  const toast = useToast();
  const choices = options.data ? (request.kind === "movie" ? options.data.radarr : options.data.sonarr) : undefined;
  const lists = !!choices && choices.quality_profiles.length > 0 && choices.root_folders.length > 0;
  const asked = request.seasons ?? [];
  const pick = choices ? preselected(choices, request.anime) : { profile: null, folder: null };
  // What was picked instead of the preselected ones (`null`: nothing).
  const [chosen, setChosen] = useState<{ profile: number | null; folder: string | null }>({ profile: null, folder: null });
  const profile = chosen.profile ?? pick.profile;
  const folder = chosen.folder ?? pick.folder;
  const [seasons, setSeasons] = useState<number[]>(asked);
  const [error, setError] = useState<string | null>(null);

  useOnChange(open, (o) => {
    if (!o) return;
    setChosen({ profile: null, folder: null });
    setSeasons(request.seasons ?? []);
    setError(null);
  });

  const submit = () => {
    if (asked.length && !seasons.length) return setError("Pick a season.");
    const body: Approval = {};
    if (lists) {
      if (profile !== null && profile !== pick.profile) body.quality_profile = profile;
      if (folder !== null && folder !== pick.folder) body.root_folder = folder;
    }
    if (asked.length && seasons.length < asked.length) body.seasons = [...seasons].sort((a, b) => a - b);
    setError(null);
    approve.mutate(
      { request, ...body },
      {
        onSuccess: (r) => {
          toast.success(decidedText(r.title, "approved", null));
          onClose();
        },
        onError: (e) => setError(failure(e, { manage: true }, request.kind)),
      },
    );
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      locked={approve.isPending}
      size="sm"
      title={`Approve ${quoted(request.title)}`}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={approve.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} loading={approve.isPending} data-autofocus>
            {lists ? "Approve" : "Approve with the settings' choices"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        {options.isPending ? (
          <div className="space-y-3" aria-busy>
            <Skeleton className="h-14" />
            <Skeleton className="h-14" />
          </div>
        ) : choices === null ? (
          <p className="text-sm text-muted">
            {service(request)} isn't connected: it stays approved until it is, or until someone adds it by hand.
          </p>
        ) : choices ? (
          <>
            {choices.error ? <p className="text-sm text-danger">{choices.error}</p> : null}
            {lists ? (
              <>
                <Field label="Quality profile" htmlFor={`approve-profile-${request.id}`}>
                  <Select
                    id={`approve-profile-${request.id}`}
                    className="w-full"
                    value={profile ?? ""}
                    onChange={(e) => setChosen((c) => ({ ...c, profile: Number(e.target.value) }))}
                  >
                    {profile === null ? <option value="">The settings' choice</option> : null}
                    {choices.quality_profiles.map((p) => (
                      <option key={p.id} value={p.id}>
                        {p.name}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="Folder" htmlFor={`approve-folder-${request.id}`}>
                  <Select
                    id={`approve-folder-${request.id}`}
                    className="w-full"
                    value={folder ?? ""}
                    onChange={(e) => setChosen((c) => ({ ...c, folder: e.target.value }))}
                  >
                    {folder === null ? <option value="">The settings' choice</option> : null}
                    {choices.root_folders.map((f) => (
                      <option key={f.path} value={f.path}>
                        {f.free_bytes !== null ? `${f.path} (${formatBytes(f.free_bytes)} free)` : f.path}
                      </option>
                    ))}
                  </Select>
                </Field>
              </>
            ) : null}
          </>
        ) : (
          // The options couldn't be had at all: the settings' choices still can.
          <p className="text-sm text-muted">Couldn't load {service(request)}'s profiles and folders.</p>
        )}
        {asked.length > 1 ? (
          <fieldset className="space-y-1">
            <legend className="mb-1 text-sm font-medium">Seasons</legend>
            <p className="pb-1 text-xs text-subtle">Those left out leave the request, and may be asked for again.</p>
            <div className="max-h-56 space-y-0.5 overflow-y-auto rounded-md border border-border p-1.5">
              {[...asked]
                .sort((a, b) => Number(a === 0) - Number(b === 0) || a - b)
                .map((n) => (
                  <CheckRow
                    key={n}
                    id={`approve-${request.id}-season-${n}`}
                    checked={seasons.includes(n)}
                    onChange={() => setSeasons((s) => (s.includes(n) ? s.filter((x) => x !== n) : [...s, n]))}
                  >
                    {n === 0 ? "Specials" : `Season ${n}`}
                  </CheckRow>
                ))}
            </div>
          </fieldset>
        ) : null}
        {error ? (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        ) : null}
      </div>
    </Modal>
  );
}

/** Declining, with a reason for the person who asked if there is one. */
export function DeclineDialog({ request, open, onClose }: { request: MediaRequest; open: boolean; onClose: () => void }) {
  const decline = useDeclineRequest();
  const toast = useToast();
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  useOnChange(open, (o) => {
    if (!o) return;
    setReason("");
    setError(null);
  });
  const submit = () =>
    decline.mutate(
      { request, reason: reason.trim() || undefined },
      {
        onSuccess: (r) => {
          toast.success(decidedText(r.title, "declined", null));
          onClose();
        },
        onError: (e) => setError(failure(e, { manage: true }, request.kind)),
      },
    );
  const who = request.requested_by?.name;
  return (
    <Modal
      open={open}
      onClose={onClose}
      locked={decline.isPending}
      size="sm"
      title={`Decline ${quoted(request.title)}?`}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={decline.isPending}>
            Cancel
          </Button>
          <Button variant="danger" onClick={submit} loading={decline.isPending}>
            Decline
          </Button>
        </>
      }
    >
      <form
        className="space-y-4"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <p className="text-sm text-muted">
          {who ? `${who} hears that it was declined, with your reason if you give one.` : "Whoever asked hears that it was declined, with your reason if you give one."}
          {request.watchers > 0 ? " Those who follow it hear it too, without the reason." : ""}
        </p>
        <Field label="Reason" htmlFor={`decline-${request.id}`} hint="Optional.">
          <TextInput
            id={`decline-${request.id}`}
            maxLength={500}
            autoComplete="off"
            placeholder="We have it on disc"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            data-autofocus
          />
        </Field>
        {error ? (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        ) : null}
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}

const ACTION_ICONS: Record<RequestAction, ReactNode> = {
  withdraw: <Undo2 className="size-4" />,
  unwatch: <UserMinus className="size-4" />,
  approve: <Check className="size-4" />,
  retry: <RotateCw className="size-3.5" />,
  decline: <Ban className="size-3.5" />,
  handled: <Hand className="size-4" />,
  delete: <Trash2 className="size-4" />,
  ask_again: <RotateCw className="size-3.5" />,
};

/** Actions that go in the card's menu rather than as buttons: those seldom wanted. */
const IN_MENU: RequestAction[] = ["handled", "delete"];

/**
 * What this session may do to a request, exactly as the server lists
 * them, in its order: approving with the settings' choices or with a
 * dialog, declining with a reason, withdrawing and deleting after asking.
 */
export function RequestActions({ request, summary }: { request: MediaRequest; summary: RequestSummary }) {
  const toast = useToast();
  const approve = useApproveRequest();
  const handled = useHandledRequest();
  const remove = useDeleteRequest();
  const unwatch = useUnwatchRequest();
  const ask = useAskRequest();
  const [dialog, setDialog] = useState<"approve" | "decline" | "withdraw" | "delete" | null>(null);
  const busy = approve.isPending || handled.isPending || remove.isPending || unwatch.isPending || ask.isPending;
  const fail = (e: unknown) => toast.error(failure(e, summary, request.kind));

  const run = (action: RequestAction) => {
    switch (action) {
      case "approve":
      case "retry":
        return approve.mutate({ request }, { onSuccess: (r) => toast.success(decidedText(r.title, "approved", null)), onError: fail });
      case "decline":
        return setDialog("decline");
      case "handled":
        return handled.mutate(request, { onSuccess: (r) => toast.success(`${quoted(r.title)} is marked as added by hand.`), onError: fail });
      case "withdraw":
        return setDialog("withdraw");
      case "delete":
        return setDialog("delete");
      case "unwatch":
        return unwatch.mutate(request, { onSuccess: () => toast.success(`You won't hear about ${quoted(request.title)} any more.`), onError: fail });
      case "ask_again":
        return ask.mutate(
          { kind: request.kind, tmdb_id: request.tmdb_id, ...(request.seasons ? { seasons: request.seasons } : {}) },
          { onSuccess: (asked) => toast.success(askedText(asked)), onError: fail },
        );
    }
  };

  const buttons = request.actions.filter((a) => !IN_MENU.includes(a));
  const menu = request.actions.filter((a) => IN_MENU.includes(a));
  const question = dialog === "withdraw" || dialog === "delete" ? actionQuestion(dialog, request) : null;

  return (
    <div className="flex flex-wrap items-center gap-2">
      {buttons.map((action) =>
        action === "approve" ? (
          <span key={action} className="contents">
            <Button variant="primary" size="sm" icon={ACTION_ICONS.approve} loading={approve.isPending} disabled={busy} onClick={() => run("approve")}>
              {actionLabel("approve", request)}
            </Button>
            <Button variant="outline" size="sm" disabled={busy} onClick={() => setDialog("approve")}>
              Approve…
            </Button>
          </span>
        ) : (
          <Button
            key={action}
            variant={action === "retry" || action === "ask_again" ? "outline" : action === "decline" ? "outline" : "ghost"}
            size="sm"
            icon={ACTION_ICONS[action]}
            loading={(action === "retry" && approve.isPending) || (action === "ask_again" && ask.isPending) || (action === "unwatch" && unwatch.isPending)}
            disabled={busy}
            onClick={() => run(action)}
          >
            {actionLabel(action, request)}
          </Button>
        ),
      )}
      {menu.length ? (
        <Menu
          trigger={({ open, toggle }) => (
            <IconButton size="sm" label={`More for ${request.title}`} aria-haspopup="menu" aria-expanded={open} onClick={toggle} disabled={busy}>
              <Ellipsis className="size-4" />
            </IconButton>
          )}
        >
          {(close) =>
            menu.map((action) => (
              <MenuItem
                key={action}
                icon={ACTION_ICONS[action]}
                danger={action === "delete"}
                onClick={() => {
                  close();
                  run(action);
                }}
              >
                {actionLabel(action, request)}
              </MenuItem>
            ))
          }
        </Menu>
      ) : null}

      {request.actions.includes("approve") ? <ApproveDialog request={request} open={dialog === "approve"} onClose={() => setDialog(null)} /> : null}
      {request.actions.includes("decline") ? <DeclineDialog request={request} open={dialog === "decline"} onClose={() => setDialog(null)} /> : null}
      <ConfirmModal
        open={question !== null}
        onClose={() => setDialog(null)}
        title={question ?? ""}
        danger
        confirmLabel={dialog === "delete" ? "Delete" : actionLabel("withdraw", request)}
        loading={remove.isPending}
        message={
          dialog === "delete" ? (
            <>
              It goes for everyone who asked for <strong className="text-fg">{request.title}</strong> or follows it.
            </>
          ) : request.status === "declined" ? (
            "It may be asked for again."
          ) : (
            <>
              <strong className="text-fg">{request.title}</strong> won't be asked for any more
              {request.watchers > 0 ? ", unless those who follow it keep it." : "."}
            </>
          )
        }
        onConfirm={() =>
          remove.mutate(request, {
            onSuccess: () => setDialog(null),
            onError: (e) => {
              setDialog(null);
              fail(e);
            },
          })
        }
      />
    </div>
  );
}

/** Who asked, for managers (or "Asked for by someone else"), and when. */
function Asker({ request, manage }: { request: MediaRequest; manage: boolean }) {
  const by = request.requested_by;
  return (
    <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-xs text-subtle">
      {request.mine ? (
        <span>
          You asked <When ts={request.created_at} />
        </span>
      ) : by ? (
        <>
          <Avatar user={by} size="xs" />
          <span>
            <span className="text-muted">{by.name}</span> asked <When ts={request.created_at} />
          </span>
        </>
      ) : (
        <span>
          Asked for by someone else <When ts={request.created_at} />
        </span>
      )}
      {request.watching && !request.mine ? <span>· You follow it</span> : null}
      {manage && request.watchers > 0 ? <span>· {request.watchers === 1 ? "1 follows it" : `${request.watchers} follow it`}</span> : null}
    </div>
  );
}

/** What there is to know beyond its stage: why it was declined, the managers' note, and where it was sent. */
function Notes({ request, manage }: { request: MediaRequest; manage: boolean }) {
  const lines: ReactNode[] = [];
  if (request.status === "declined") {
    lines.push(
      <span key="d">
        Declined{request.decided_by && manage ? ` by ${request.decided_by.name}` : ""}
        {request.decided_at ? (
          <>
            {" "}
            <When ts={request.decided_at} />
          </>
        ) : null}
        {request.reason ? <span className="text-fg/85">: {quoted(request.reason)}</span> : null}
      </span>,
    );
  }
  if (manage && request.note) lines.push(<span key="n">{request.note}</span>);
  if (manage && request.sent_to) {
    const where = [request.sent_to.service === "radarr" ? "Radarr" : "Sonarr", request.sent_to.quality_profile, request.sent_to.root_folder].filter(Boolean);
    lines.push(<span key="s">Sent to {where.join(" · ")}</span>);
  }
  if (!lines.length) return null;
  return (
    <div className="space-y-0.5 text-xs text-muted">
      {lines.map((line, i) => (
        <p key={i}>{line}</p>
      ))}
    </div>
  );
}

/** Ready: Watch where this account can open it, or who can help where it can't. */
function Ready({ request, manage }: { request: MediaRequest; manage: boolean }) {
  if (request.stage !== "available" && request.stage !== "partial") return null;
  if (request.item_id !== null) {
    return (
      <Link to={itemUrl(request.item_id)} className={buttonClass("secondary", "sm")}>
        <Play className="size-3.5 fill-current" />
        Watch
      </Link>
    );
  }
  return manage ? null : <p className="text-xs text-muted">Ready, in a library you can't open: ask an administrator</p>;
}

/**
 * A request in a list: its poster and title (unless the list is on the
 * title's page), its seasons, a stage pill with a download's progress, who
 * asked and when, the reason or note, and what this session may do.
 * `selected` puts a checkbox before it, for deciding on several at once.
 */
export function RequestCard({
  request,
  summary,
  showTitle = true,
  selected,
  onSelect,
}: {
  request: MediaRequest;
  summary: RequestSummary;
  showTitle?: boolean;
  selected?: boolean;
  onSelect?: (selected: boolean) => void;
}) {
  const manage = summary.manage;
  const seasons = seasonsLabel(request.seasons);
  const to = titleUrl(request.kind, request.tmdb_id);
  return (
    <li className={cn("flex gap-3 px-4 py-3.5", request.unseen && "bg-accent/[0.04]")}>
      {onSelect ? (
        <label className="flex shrink-0 cursor-pointer items-start pt-1">
          <input
            type="checkbox"
            className="peer sr-only"
            checked={!!selected}
            onChange={(e) => onSelect(e.target.checked)}
            aria-label={`Select ${request.title}`}
          />
          <Checkbox checked={!!selected} />
        </label>
      ) : null}
      {showTitle ? (
        <Link to={to} className="shrink-0" tabIndex={-1} aria-hidden>
          <Artwork
            candidates={request.poster_url ? [request.poster_url] : []}
            width={56}
            alt=""
            title=""
            bare
            className="aspect-[2/3] w-14 rounded ring-1 ring-white/5"
          />
        </Link>
      ) : null}
      <div className="flex min-w-0 flex-1 flex-col gap-3 md:flex-row md:items-start">
        <div className="min-w-0 flex-1 space-y-1.5">
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
            {showTitle ? (
              <Link to={to} className="truncate font-medium transition-colors hover:text-accent">
                {request.title}
              </Link>
            ) : (
              <span className="font-medium">{seasons || kindLabel(request.kind)}</span>
            )}
            {showTitle && request.year ? <span className="text-sm text-subtle">{request.year}</span> : null}
            {showTitle ? (
              <span className="text-xs text-subtle">
                {kindLabel(request.kind)}
                {seasons ? ` · ${seasons}` : ""}
              </span>
            ) : null}
            {request.unseen ? <span className="sr-only">, new</span> : null}
          </div>
          <StagePill request={request} manage={manage} />
          <Asker request={request} manage={manage} />
          <Notes request={request} manage={manage} />
        </div>
        <div className="flex shrink-0 flex-wrap items-center gap-2 md:max-w-[55%] md:justify-end">
          <Ready request={request} manage={manage} />
          <RequestActions request={request} summary={summary} />
        </div>
      </div>
    </li>
  );
}
