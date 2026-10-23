import { useState, type ReactNode } from "react";
import { Check, Eye, EyeOff, Star } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSetWatched } from "@/api/hooks";
import type { ItemDetail, ItemSummary } from "@/api/types";
import { Artwork } from "@/components/Artwork";
import { ASPECT_CLASS, type CardAspect } from "@/components/cards";
import { useToast } from "@/components/Toast";
import { Badge, Button, IconButton, ProgressBar } from "@/components/ui";
import { cn } from "@/lib/cn";
import { dynamicRangeBadge, formatRemaining, progressFraction } from "@/lib/format";
import { backdropCandidates, posterCandidates, thumbCandidates } from "@/lib/images";

/** Full-bleed backdrop behind the item header. */
export function Backdrop({ item }: { item: ItemSummary }) {
  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 h-[78vh] max-h-[760px] overflow-hidden" aria-hidden>
      <Artwork
        candidates={backdropCandidates(item)}
        width={1920}
        alt=""
        title={item.grandparent_title ?? item.parent_title ?? item.title}
        bare
        eager
        className="size-full opacity-70"
        imgClassName="object-cover object-[center_20%]"
      />
      <div className="absolute inset-0 bg-linear-to-t from-bg via-bg/75 to-bg/10" />
      <div className="absolute inset-0 bg-linear-to-r from-bg/95 via-bg/50 to-transparent" />
    </div>
  );
}

export function ItemArt({
  item,
  aspect,
  width,
  className,
}: {
  item: ItemSummary;
  aspect: CardAspect;
  width: number;
  className?: string;
}) {
  const candidates = aspect === "thumb" ? thumbCandidates(item) : posterCandidates(item);
  return (
    <Artwork
      candidates={candidates}
      width={width}
      alt={item.title}
      title={item.title}
      eager
      className={cn(
        ASPECT_CLASS[aspect],
        "w-full shadow-2xl shadow-black/60 ring-1 ring-white/10",
        item.kind === "artist" ? "rounded-full" : "rounded-lg",
        className,
      )}
    />
  );
}

/** Page wrapper that layers content over the backdrop. */
export function ItemPageShell({ item, children }: { item: ItemSummary; children: ReactNode }) {
  return (
    <div className="relative min-h-[calc(100vh-4rem)] animate-fade-in">
      <Backdrop item={item} />
      <div className="relative px-4 pb-16 pt-6 sm:px-8 sm:pt-[10vh] lg:pt-[14vh]">{children}</div>
    </div>
  );
}

export function MetaLine({ parts, className }: { parts: (ReactNode | null | undefined | false)[]; className?: string }) {
  const visible = parts.filter((p) => p !== null && p !== undefined && p !== false && p !== "");
  if (!visible.length) return null;
  return (
    <div className={cn("flex flex-wrap items-center gap-x-2.5 gap-y-1.5 text-sm text-muted", className)}>
      {visible.map((p, i) => (
        <span key={i} className="flex items-center gap-2.5">
          {i > 0 ? <span className="text-subtle">·</span> : null}
          {p}
        </span>
      ))}
    </div>
  );
}

export function RatingBadge({ rating }: { rating: number | null }) {
  if (rating == null || rating <= 0) return null;
  return (
    <span className="inline-flex items-center gap-1 text-fg">
      <Star className="size-3.5 fill-accent text-accent" />
      {rating.toFixed(1)}
    </span>
  );
}

export function ContentRating({ value }: { value: string | null }) {
  if (!value) return null;
  return <span className="rounded border border-white/25 px-1.5 py-px text-[11px] font-semibold text-fg/90">{value}</span>;
}

export function Genres({ genres }: { genres: string[] }) {
  if (!genres.length) return null;
  return (
    <div className="flex flex-wrap gap-1.5">
      {genres.map((g) => (
        <span key={g} className="rounded-full bg-white/[0.07] px-2.5 py-0.5 text-xs text-fg/85">
          {g}
        </span>
      ))}
    </div>
  );
}

export function Summary({ text, className }: { text?: string | null; className?: string }) {
  const [expanded, setExpanded] = useState(false);
  if (!text) return null;
  const long = text.length > 360;
  return (
    <div className={cn("max-w-3xl", className)}>
      <p className={cn("whitespace-pre-line text-[15px] leading-relaxed text-fg/85", !expanded && long && "line-clamp-4")}>{text}</p>
      {long ? (
        <button type="button" onClick={() => setExpanded((e) => !e)} className="mt-1 text-sm font-medium text-accent hover:underline">
          {expanded ? "Show less" : "Read more"}
        </button>
      ) : null}
    </div>
  );
}

export function ResumeProgress({ item }: { item: ItemSummary }) {
  const fraction = item.user_state.watched ? 0 : progressFraction(item);
  if (fraction <= 0) return null;
  return (
    <div className="flex max-w-sm items-center gap-3">
      <ProgressBar value={fraction} className="h-1.5 flex-1" />
      <span className="shrink-0 text-xs text-muted">{formatRemaining(item.user_state.position_ms, item.duration_ms)}</span>
    </div>
  );
}

/** Mark watched / unwatched toggle. For containers "watched" means every episode is watched. */
export function WatchedButton({ item, compact }: { item: ItemSummary; compact?: boolean }) {
  const setWatched = useSetWatched();
  const toast = useToast();
  const isContainer = item.kind === "show" || item.kind === "season";
  const watched = isContainer
    ? item.leaf_count != null && item.leaf_count > 0 && (item.viewed_leaf_count ?? 0) >= item.leaf_count
    : item.user_state.watched;
  const label = watched ? "Mark as unwatched" : "Mark as watched";
  const onClick = () =>
    setWatched.mutate(
      { id: item.id, watched: !watched },
      { onError: (e) => toast.error(errorMessage(e)) },
    );

  if (compact) {
    return (
      <IconButton label={label} size="sm" onClick={onClick} disabled={setWatched.isPending} active={watched}>
        {watched ? <Check className="size-4" strokeWidth={3} /> : <Check className="size-4 opacity-60" />}
      </IconButton>
    );
  }
  return (
    <Button
      variant="secondary"
      size="lg"
      onClick={onClick}
      loading={setWatched.isPending}
      icon={watched ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
      title={label}
    >
      <span className="hidden sm:inline">{watched ? "Unwatched" : "Watched"}</span>
    </Button>
  );
}

/** Short quality labels for the first file: resolution, dynamic range, surround. */
export function qualityBadgeLabels(item: ItemDetail): string[] {
  const file = item.files[0];
  if (!file) return [];
  const video = file.streams.find((s) => s.kind === "video");
  const audio = file.streams.find((s) => s.kind === "audio" && s.default) ?? file.streams.find((s) => s.kind === "audio");
  const badges: string[] = [];
  const h = file.height ?? video?.height ?? 0;
  const w = file.width ?? video?.width ?? 0;
  if (w >= 3800 || h >= 2100) badges.push("4K");
  else if (w >= 1800 || h >= 1000) badges.push("1080p");
  else if (w >= 1200 || h >= 700) badges.push("720p");
  else if (h > 0) badges.push("SD");
  const range = dynamicRangeBadge(video);
  if (range) badges.push(range);
  if (audio?.channels && audio.channels >= 6) badges.push(audio.channels >= 8 ? "7.1" : "5.1");
  return badges;
}

export function QualityBadges({ item }: { item: ItemDetail }) {
  const badges = qualityBadgeLabels(item);
  if (!badges.length) return null;
  return (
    <span className="inline-flex gap-1">
      {badges.map((b) => (
        <Badge key={b}>{b}</Badge>
      ))}
    </span>
  );
}
