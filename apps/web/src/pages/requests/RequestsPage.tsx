import { Link, useSearchParams } from "react-router";
import { Compass, Inbox } from "lucide-react";
import { isApiError } from "@/api/client";
import { mayRequest, requestsUnavailable, useRequests, useRequestSummary, type RequestStatus } from "@/api/requests";
import { useAuth } from "@/auth";
import { Button, buttonClass, EmptyState, Skeleton, Toggle } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle } from "@/lib/hooks";
import { discoverUrl } from "@/lib/urls";
import { RequestItem } from "./RequestItem";
import { LoadError, RequestsUnavailable, STATUS_LABELS } from "./shared";

const TABS: (RequestStatus | null)[] = [null, "pending", "approved", "available", "declined", "failed"];

function statusOf(value: string | null): RequestStatus | null {
  return TABS.find((t) => t === value) ?? null;
}

/** What a tab with nothing in it says. */
function emptyText(status: RequestStatus | null, everyone: boolean): string {
  const whose = everyone ? "" : " of yours";
  switch (status) {
    case "pending":
      return `No requests${whose} wait for a decision.`;
    case "approved":
      return `No requests${whose} are on their way.`;
    case "available":
      return `No requests${whose} are ready to watch yet.`;
    case "declined":
      return `No requests${whose} were declined.`;
    case "failed":
      return `No requests${whose} failed.`;
    default:
      return everyone ? "No one has asked for anything yet." : "You haven't asked for anything yet.";
  }
}

/**
 * Requests (`/requests`): your own, or everyone's for administrators (who
 * may keep to theirs), by status, newest first, with what may be done
 * with each.
 */
export function RequestsPage() {
  useDocumentTitle("Requests");
  const me = useAuth();
  const [params, setParams] = useSearchParams();
  const status = statusOf(params.get("status"));
  const mine = me.is_admin && params.get("mine") === "1";
  const everyone = me.is_admin && !mine;
  const summary = useRequestSummary();
  const requests = useRequests({ status, mine });
  const first = requests.data?.pages[0];
  const shown = requests.data?.pages.flatMap((p) => p.requests) ?? [];
  const counts = first?.counts;
  const total = counts ? Object.values(counts).reduce((n, c) => n + c, 0) : null;

  const setParam = (key: string, value: string | null) => {
    const p = new URLSearchParams(params);
    if (value === null) p.delete(key);
    else p.set(key, value);
    setParams(p, { replace: true });
  };

  if (requests.isError && !requests.data) {
    const why = requestsUnavailable(requests.error);
    if (why) return <RequestsUnavailable reason={why} />;
    // A server without requests.
    if (isApiError(requests.error, 404)) return <RequestsUnavailable reason="setup" />;
  }

  return (
    <div className="space-y-5 px-4 pb-16 pt-6 sm:px-8 sm:pt-8">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-2xl font-bold tracking-tight">{me.is_admin ? "Requests" : "Your requests"}</h1>
          <p className="mt-1 text-sm text-muted">
            {me.is_admin
              ? "What people asked for. Approving sends a title to Radarr or Sonarr where they're connected."
              : "What you asked for, and how it's going."}
          </p>
        </div>
        {mayRequest(summary.data) ? (
          <Link to={discoverUrl()} className={buttonClass("outline", "sm")}>
            <Compass className="size-4" />
            Discover
          </Link>
        ) : null}
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="scrollbar-none -mx-4 overflow-x-auto px-4 sm:mx-0 sm:px-0">
          <div className="flex w-max rounded-lg bg-surface-2 p-1" role="tablist" aria-label="Status">
            {TABS.map((t) => {
              const count = t === null ? total : (counts?.[t] ?? null);
              return (
                <button
                  key={t ?? "all"}
                  type="button"
                  role="tab"
                  aria-selected={status === t}
                  onClick={() => setParam("status", t)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                    status === t ? "bg-surface-3 text-fg shadow" : "text-muted hover:text-fg",
                  )}
                >
                  <span>{t === null ? "All" : STATUS_LABELS[t]}</span>{" "}
                  {count ? (
                    <span
                      className={cn(
                        "min-w-5 rounded-full px-1.5 text-center text-[11px] font-semibold tabular-nums",
                        t === "pending" && everyone ? "bg-accent text-accent-fg" : "bg-white/10 text-fg/80",
                      )}
                    >
                      {count}
                    </span>
                  ) : null}
                </button>
              );
            })}
          </div>
        </div>
        {me.is_admin ? (
          <label className="flex items-center gap-2.5 text-sm text-muted">
            <Toggle checked={mine} onChange={(v) => setParam("mine", v ? "1" : null)} label="Only mine" />
            Only mine
          </label>
        ) : null}
      </div>

      {requests.isPending ? (
        <div className="space-y-2">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-20" />
          ))}
        </div>
      ) : requests.isError && !requests.data ? (
        <LoadError title="Couldn't load the requests" error={requests.error} retrying={requests.isFetching} onRetry={() => void requests.refetch()} />
      ) : shown.length === 0 ? (
        <EmptyState
          icon={<Inbox />}
          title={emptyText(status, everyone)}
          action={
            mayRequest(summary.data) && status === null && !everyone ? (
              <Link to={discoverUrl()} className={buttonClass("primary")}>
                Find something to ask for
              </Link>
            ) : undefined
          }
        />
      ) : (
        <div>
          <ul className="divide-y divide-white/[0.05] overflow-hidden rounded-lg border border-border bg-surface">
            {shown.map((r) => (
              <RequestItem key={r.id} request={r} />
            ))}
          </ul>
          {requests.hasNextPage ? (
            <Button variant="ghost" size="sm" className="mt-3" loading={requests.isFetchingNextPage} onClick={() => void requests.fetchNextPage()}>
              Show more
            </Button>
          ) : null}
        </div>
      )}
    </div>
  );
}
