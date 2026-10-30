import { useState } from "react";
import { Link } from "react-router";
import { Ban, Check, Play, RotateCw, Trash2, Undo2 } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useApproveRequest, useDeclineRequest, useDeleteRequest, type MediaRequest } from "@/api/requests";
import { useAuth } from "@/auth";
import { Artwork } from "@/components/Artwork";
import { Avatar } from "@/components/Avatar";
import { ConfirmModal, Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, buttonClass, Field, IconButton, TextInput } from "@/components/ui";
import { formatRelative, formatTimestamp } from "@/lib/format";
import { useOnChange } from "@/lib/hooks";
import { itemUrl, titleUrl } from "@/lib/urls";
import { DownloadProgress, kindLabel, requestActions, RequestStatusBadge, seasonsLabel } from "./shared";

/** Where an approved request of this kind goes. */
function service(request: Pick<MediaRequest, "kind">): string {
  return request.kind === "movie" ? "Radarr" : "Sonarr";
}

function When({ ts }: { ts: number }) {
  return (
    <time dateTime={new Date(ts * 1000).toISOString()} title={formatTimestamp(ts)}>
      {formatRelative(ts)}
    </time>
  );
}

/** Under the request: what came of it, and why where there's a reason. */
function Outcome({ request }: { request: MediaRequest }) {
  const me = useAuth();
  const by = request.decided_by && request.decided_by.id !== request.requested_by.id ? ` by ${request.decided_by.name}` : "";
  switch (request.status) {
    case "declined":
      return (
        <p className="text-xs text-muted">
          Declined{by}
          {request.decided_at ? (
            <>
              {" "}
              <When ts={request.decided_at} />
            </>
          ) : null}
          {request.reason ? <span className="text-fg/85">: “{request.reason}”</span> : null}
        </p>
      );
    case "failed":
      return (
        <p className="text-xs text-danger">
          Couldn't go to {service(request)}
          {request.reason ? `: ${request.reason}` : ""}
          {me.is_admin ? null : <span className="text-muted"> · An administrator can try again.</span>}
        </p>
      );
    case "available":
      return request.available_at ? (
        <p className="text-xs text-muted">
          Ready to watch since <When ts={request.available_at} />
        </p>
      ) : null;
    case "approved": {
      const ready = request.seasons?.length ? request.available_seasons.filter((s) => request.seasons?.includes(s)) : [];
      return (
        <p className="text-xs text-muted">
          Approved{by}
          {request.decided_at ? (
            <>
              {" "}
              <When ts={request.decided_at} />
            </>
          ) : null}
          {ready.length ? <span className="text-fg/85"> · {seasonsLabel(ready)} ready to watch</span> : null}
        </p>
      );
    }
    default:
      return null;
  }
}

/** Declining a request, with a reason for the person who asked if there is one. */
function DeclineModal({ request, open, onClose }: { request: MediaRequest; open: boolean; onClose: () => void }) {
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
      { id: request.id, reason: reason.trim() || undefined },
      {
        onSuccess: (r) => {
          toast.success(`Declined “${r.title}”`);
          onClose();
        },
        onError: (e) => setError(errorMessage(e)),
      },
    );
  return (
    <Modal
      open={open}
      onClose={onClose}
      locked={decline.isPending}
      size="sm"
      title={`Decline “${request.title}”?`}
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
        <p className="text-sm text-muted">{request.requested_by.name} sees that it was declined, and your reason if you give one.</p>
        <Field label="Reason" htmlFor={`decline-${request.id}`} hint="Optional.">
          <TextInput
            id={`decline-${request.id}`}
            maxLength={500}
            autoComplete="off"
            placeholder="We have it on disc"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        </Field>
        {error ? <p className="text-sm text-danger">{error}</p> : null}
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}

/**
 * A request in a list: its title (unless the list is on the title's page),
 * who asked and when, what came of it, its download, and what this account
 * may do with it.
 */
export function RequestItem({ request, showTitle = true }: { request: MediaRequest; showTitle?: boolean }) {
  const me = useAuth();
  const can = requestActions(request, me);
  const approve = useApproveRequest();
  const remove = useDeleteRequest();
  const toast = useToast();
  const [declining, setDeclining] = useState(false);
  const [removing, setRemoving] = useState(false);
  const seasons = seasonsLabel(request.seasons);
  const to = titleUrl(request.kind, request.tmdb_id);
  const own = request.requested_by.id === me.id;
  // Administrators delete; the others withdraw their own.
  const withdrawing = can.withdraw;

  const onApprove = () =>
    approve.mutate(
      { id: request.id },
      {
        onSuccess: (r) => {
          if (r.status === "failed") toast.error(`“${r.title}” couldn't go to ${service(r)}${r.reason ? `: ${r.reason}` : ""}`);
          else toast.success(request.status === "failed" ? `Sent “${r.title}” to ${service(r)}` : `Approved “${r.title}”`);
        },
        onError: (e) => toast.error(errorMessage(e)),
      },
    );

  return (
    <li className="flex gap-3 px-4 py-3.5">
      {showTitle ? (
        <Link to={to} className="shrink-0" tabIndex={-1} aria-hidden>
          <Artwork
            candidates={request.poster_url ? [request.poster_url] : []}
            width={48}
            alt=""
            title=""
            bare
            className="aspect-[2/3] w-12 rounded ring-1 ring-white/5"
          />
        </Link>
      ) : null}
      <div className="flex min-w-0 flex-1 flex-col gap-3 sm:flex-row sm:items-start">
        <div className="min-w-0 flex-1 space-y-1.5">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            {showTitle ? (
              <Link to={to} className="truncate font-medium transition-colors hover:text-accent">
                {request.title}
              </Link>
            ) : (
              <span className="font-medium">{seasons || kindLabel(request.kind)}</span>
            )}
            {showTitle && request.year ? <span className="text-sm text-subtle">{request.year}</span> : null}
            <RequestStatusBadge status={request.status} />
          </div>
          <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-xs text-subtle">
            {showTitle ? (
              <span>
                {kindLabel(request.kind)}
                {seasons ? ` · ${seasons}` : ""} ·
              </span>
            ) : null}
            <Avatar user={request.requested_by} size="xs" />
            <span className="text-muted">{own ? "You" : request.requested_by.name}</span>
            <span>
              asked <When ts={request.created_at} />
            </span>
          </div>
          <Outcome request={request} />
          {request.download ? <DownloadProgress download={request.download} className="max-w-md pt-0.5" /> : null}
        </div>
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          {request.status === "available" && request.item_id !== null ? (
            <Link to={itemUrl(request.item_id)} className={buttonClass("secondary", "sm")}>
              <Play className="size-3.5 fill-current" />
              Watch
            </Link>
          ) : null}
          {can.approve ? (
            <Button variant="primary" size="sm" icon={<Check className="size-4" />} loading={approve.isPending} onClick={onApprove}>
              Approve
            </Button>
          ) : null}
          {can.retry ? (
            <Button variant="outline" size="sm" icon={<RotateCw className="size-3.5" />} loading={approve.isPending} onClick={onApprove}>
              Try again
            </Button>
          ) : null}
          {can.decline ? (
            <Button variant="outline" size="sm" icon={<Ban className="size-3.5" />} disabled={approve.isPending} onClick={() => setDeclining(true)}>
              Decline
            </Button>
          ) : null}
          {withdrawing ? (
            <Button variant="ghost" size="sm" icon={<Undo2 className="size-4" />} onClick={() => setRemoving(true)}>
              Withdraw
            </Button>
          ) : null}
          {can.remove ? (
            <IconButton size="sm" label="Delete request" onClick={() => setRemoving(true)}>
              <Trash2 className="size-4" />
            </IconButton>
          ) : null}
        </div>
      </div>

      {can.decline ? <DeclineModal request={request} open={declining} onClose={() => setDeclining(false)} /> : null}
      <ConfirmModal
        open={removing}
        onClose={() => setRemoving(false)}
        title={withdrawing ? "Withdraw your request?" : "Delete this request?"}
        danger
        confirmLabel={withdrawing ? "Withdraw" : "Delete"}
        loading={remove.isPending}
        message={
          withdrawing ? (
            <>
              <strong className="text-fg">{request.title}</strong> won't be asked for any more.
            </>
          ) : (
            <>
              The request for <strong className="text-fg">{request.title}</strong>
              {own ? "" : ` by ${request.requested_by.name}`} goes from the list.
              {request.status === "approved" || request.status === "available"
                ? ` What ${service(request)} has of it stays there.`
                : ""}
            </>
          )
        }
        onConfirm={() =>
          remove.mutate(request.id, {
            onSuccess: () => {
              toast.success(withdrawing ? `Withdrew “${request.title}”` : `Deleted the request for “${request.title}”`);
              setRemoving(false);
            },
            onError: (e) => toast.error(errorMessage(e)),
          })
        }
      />
    </li>
  );
}
