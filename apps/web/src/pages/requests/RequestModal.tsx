import { useState } from "react";
import { Check } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useCreateRequest, type DiscoverDetail, type DiscoverSeason, type RequestSummary } from "@/api/requests";
import { useAuth } from "@/auth";
import { Artwork } from "@/components/Artwork";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button } from "@/components/ui";
import { cn } from "@/lib/cn";
import { formatDate, pluralize } from "@/lib/format";
import { useOnChange } from "@/lib/hooks";
import { allowanceText, kindLabel } from "./shared";

/** A season a library has every aired episode of. */
export function wholeInLibrary(season: DiscoverSeason): boolean {
  return season.aired > 0 && season.in_library >= season.aired;
}

/** Seasons that may still be asked for: not whole in a library, not asked for already. */
export function askableSeasons(seasons: DiscoverSeason[] | null): DiscoverSeason[] {
  return (seasons ?? []).filter((s) => !s.requested && !wholeInLibrary(s));
}

/** What "All missing seasons" picks: those that may be asked for and have begun, specials left out (as the server's default). */
export function missingSeasons(seasons: DiscoverSeason[] | null): number[] {
  return askableSeasons(seasons)
    .filter((s) => s.number > 0 && s.aired > 0)
    .map((s) => s.number);
}

export function seasonName(season: DiscoverSeason): string {
  return season.name || (season.number === 0 ? "Specials" : `Season ${season.number}`);
}

/** "2026-11-04" is still to come. */
function toCome(date: string | null): date is string {
  return !!date && new Date(`${date}T00:00:00`).getTime() > Date.now();
}

/** How a season stands: "In library", "Requested", "4 of 10 episodes in library", "Airs Nov 4, 2026". */
export function seasonState(season: DiscoverSeason): string {
  if (season.requested) return "Requested";
  if (wholeInLibrary(season)) return "In library";
  if (season.aired === 0) return toCome(season.air_date) ? `Airs ${formatDate(season.air_date)}` : "Not aired yet";
  if (season.in_library > 0) return `${season.in_library} of ${pluralize(season.aired, "episode")} in library`;
  return pluralize(season.episodes, "episode");
}

/** Specials after the numbered seasons. */
function pickerOrder(seasons: DiscoverSeason[] | null): DiscoverSeason[] {
  const all = seasons ?? [];
  return [...all.filter((s) => s.number > 0), ...all.filter((s) => s.number === 0)];
}

function Checkbox({ checked }: { checked: boolean }) {
  return (
    <span
      className={cn(
        "flex size-4 shrink-0 items-center justify-center rounded border transition-colors peer-focus-visible:ring-2 peer-focus-visible:ring-accent",
        checked ? "border-accent bg-accent text-accent-fg" : "border-border-strong",
      )}
    >
      {checked ? <Check className="size-3" strokeWidth={3} /> : null}
    </span>
  );
}

/**
 * Asking for a movie, or for a show's seasons: those a library lacks, all
 * of them picked at first. Says whether an administrator decides, and how
 * much more the account may ask for.
 */
export function RequestModal({
  title,
  summary,
  open,
  onClose,
}: {
  title: DiscoverDetail;
  summary: RequestSummary | undefined;
  open: boolean;
  onClose: () => void;
}) {
  const me = useAuth();
  const create = useCreateRequest();
  const toast = useToast();
  const show = title.kind === "show";
  const seasons = pickerOrder(title.seasons);
  const askable = new Set(askableSeasons(title.seasons).map((s) => s.number));
  const missing = missingSeasons(title.seasons);
  const [picked, setPicked] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);

  useOnChange(open, (o) => {
    if (!o) return;
    setPicked(missingSeasons(title.seasons));
    setError(null);
  });

  const allMissing = missing.length > 0 && picked.length === missing.length && missing.every((n) => picked.includes(n));
  const left = show ? (summary?.seasons_left ?? null) : (summary?.movies_left ?? null);
  const over = left !== null && (show ? picked.length > left : left < 1);
  const allowance = summary ? allowanceText(summary) : null;
  const auto = summary?.access === "auto";

  const toggle = (n: number) => setPicked((p) => (p.includes(n) ? p.filter((x) => x !== n) : [...p, n]));

  const submit = () => {
    if (show && !picked.length) return setError("Pick a season.");
    setError(null);
    create.mutate(
      { kind: title.kind, tmdb_id: title.tmdb_id, ...(show ? { seasons: [...picked].sort((a, b) => a - b) } : {}) },
      {
        onSuccess: (r) => {
          if (r.status === "failed") {
            const where = r.kind === "movie" ? "Radarr" : "Sonarr";
            toast.error(
              me.is_admin
                ? `“${r.title}” couldn't go to ${where}${r.reason ? `: ${r.reason}` : ""}`
                : `Requested “${r.title}”, but it couldn't be sent to download yet: an administrator can try again.`,
            );
          } else if (r.status === "approved") {
            toast.success(`“${r.title}” is approved and on its way`);
          } else {
            toast.success(`Requested “${r.title}”`);
          }
          onClose();
        },
        onError: (e) => setError(errorMessage(e)),
      },
    );
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      locked={create.isPending}
      size={show ? "md" : "sm"}
      title={show ? "Request seasons" : "Request movie"}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={create.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} loading={create.isPending} disabled={over || (show && !picked.length)} data-autofocus>
            {show && picked.length ? `Request ${pluralize(picked.length, "season")}` : "Request"}
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        <div className="flex gap-4">
          <Artwork
            candidates={title.poster_url ? [title.poster_url] : []}
            width={80}
            alt=""
            title={title.title}
            className="aspect-[2/3] w-20 shrink-0 rounded-md ring-1 ring-white/10"
          />
          <div className="min-w-0 space-y-1">
            <div className="font-semibold leading-snug">{title.title}</div>
            <div className="text-sm text-muted">{[title.year, kindLabel(title.kind)].filter(Boolean).join(" · ")}</div>
            <p className="pt-1 text-sm text-muted">
              {auto
                ? "Approved as you ask, and sent on to download."
                : "An administrator decides on it. You'll hear when it's ready to watch."}
            </p>
          </div>
        </div>

        {show ? (
          <fieldset className="space-y-2">
            <legend className="mb-1.5 text-sm font-medium">Seasons</legend>
            <label
              htmlFor="request-all-missing"
              className={cn(
                "flex items-center gap-3 rounded-md bg-surface-2 px-3 py-2 text-sm font-medium",
                missing.length ? "cursor-pointer" : "opacity-60",
              )}
            >
              <input
                id="request-all-missing"
                type="checkbox"
                className="peer sr-only"
                checked={allMissing}
                disabled={!missing.length}
                onChange={() => setPicked(allMissing ? [] : missing)}
              />
              <Checkbox checked={allMissing} />
              <span className="flex-1">All missing seasons</span>
              <span className="text-xs font-normal text-subtle">{missing.length ? pluralize(missing.length, "season") : "None"}</span>
            </label>
            <div className="max-h-72 space-y-0.5 overflow-y-auto rounded-md border border-border p-1.5">
              {seasons.map((s) => {
                const enabled = askable.has(s.number);
                // Those a library has whole or a request asks for already are in, whatever is picked.
                const checked = enabled ? picked.includes(s.number) : true;
                const id = `request-season-${s.number}`;
                return (
                  <label
                    key={s.number}
                    htmlFor={id}
                    className={cn(
                      "flex items-center gap-3 rounded px-2 py-1.5 text-sm",
                      enabled ? "cursor-pointer hover:bg-white/5" : "opacity-55",
                    )}
                  >
                    <input id={id} type="checkbox" className="peer sr-only" checked={checked} disabled={!enabled} onChange={() => toggle(s.number)} />
                    <Checkbox checked={checked} />
                    <span className="min-w-0 flex-1 truncate">{seasonName(s)}</span>
                    <span className="shrink-0 text-xs text-subtle">{seasonState(s)}</span>
                  </label>
                );
              })}
            </div>
          </fieldset>
        ) : null}

        {allowance ? <p className={cn("text-xs", over ? "text-danger" : "text-subtle")}>{allowance}</p> : null}
        {error ? <p className="text-sm text-danger">{error}</p> : null}
      </div>
    </Modal>
  );
}
