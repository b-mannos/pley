import { useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { Play, RotateCcw } from "lucide-react";
import type { ItemDetail, MediaFile } from "@/api/types";
import { ListButtons, PartOfCollections } from "@/components/lists/ListButtons";
import { StarRating } from "@/components/StarRating";
import { Button, Select } from "@/components/ui";
import { codecLabel, formatBytes, formatDate, formatDuration, resolutionLabel, seasonTitle, streamLabel } from "@/lib/format";
import { saveStreamChoice } from "@/lib/progress";
import { usePlayItem } from "@/lib/usePlayItem";
import { CastRow, CrewRows } from "./Credits";
import { MediaInfo } from "./MediaInfo";
import { MoreLikeThis } from "./MoreLikeThis";
import {
  ContentRating,
  Genres,
  ItemArt,
  ItemPageShell,
  MetaLine,
  QualityBadges,
  qualityBadgeLabels,
  RatingBadge,
  RefreshButton,
  ResumeProgress,
  Summary,
  WatchedButton,
} from "./shared";

function versionLabel(f: MediaFile, i: number): string {
  const parts = [resolutionLabel(f.width, f.height), codecLabel(f.video_codec), f.container?.toUpperCase(), formatBytes(f.size)].filter(Boolean);
  return parts.length ? parts.join(" · ") : `Version ${i + 1}`;
}

function LabeledSelect({ label, children, value, onChange }: { label: string; children: ReactNode; value: string; onChange: (v: string) => void }) {
  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-[11px] font-semibold uppercase tracking-wider text-subtle">{label}</span>
      <Select value={value} onChange={(e) => onChange(e.target.value)} className="max-w-[20rem] bg-surface-2/80 backdrop-blur">
        {children}
      </Select>
    </label>
  );
}

export function VideoItemView({ item }: { item: ItemDetail }) {
  const play = usePlayItem();
  const isEpisode = item.kind === "episode";

  const [fileId, setFileId] = useState<number | null>(item.files[0]?.id ?? null);
  const file = item.files.find((f) => f.id === fileId) ?? item.files[0];
  const audioStreams = useMemo(() => file?.streams.filter((s) => s.kind === "audio") ?? [], [file]);
  const subStreams = useMemo(() => file?.streams.filter((s) => s.kind === "subtitle") ?? [], [file]);
  const defaultAudio = audioStreams.find((s) => s.default) ?? audioStreams[0];

  // "auto" → the server's pick for this user (remembered choice for the show or movie);
  // per-file selections reset when the version changes.
  const qc = useQueryClient();
  const [audio, setAudio] = useState<{ file: number | null; index: number | null | "auto" }>({ file: null, index: "auto" });
  const [sub, setSub] = useState<{ file: number | null; index: number | null | "auto" }>({ file: null, index: "auto" });
  const audioChoice = audio.file === (file?.id ?? null) ? audio.index : "auto";
  const audioIndex = audioChoice === "auto" ? (file?.selected_audio ?? null) : audioChoice;
  const subChoice = sub.file === (file?.id ?? null) ? sub.index : "auto";
  const subIndex = subChoice === "auto" ? (file?.selected_subtitle ?? null) : subChoice;

  /** Remember a choice made here for the whole show (or this movie). */
  const remember = (nextAudio: number | null, nextSub: number | null) => {
    if (!file) return;
    void saveStreamChoice(item.id, { file_id: file.id, audio_index: nextAudio, subtitle_index: nextSub }).then(() =>
      qc.invalidateQueries({ queryKey: ["item"], predicate: (q) => q.queryKey[1] !== item.id }),
    );
  };

  const resumable = !item.user_state.watched && item.user_state.position_ms > 0;

  const onPlay = (fromStart: boolean) =>
    void play(item, {
      fromStart,
      fileId: item.files.length > 1 ? file?.id : undefined,
      audio: audioIndex != null && audioIndex !== defaultAudio?.index ? audioIndex : undefined,
      sub: subIndex ?? undefined,
    });

  const art = isEpisode ? (
    <div className="w-full max-w-md md:w-[22rem] lg:w-[26rem]">
      <ItemArt item={item} aspect="thumb" width={416} />
    </div>
  ) : (
    <div className="hidden w-52 shrink-0 sm:block md:w-60 lg:w-64">
      <ItemArt item={item} aspect="poster" width={256} />
    </div>
  );

  return (
    <ItemPageShell item={item}>
      <div className={isEpisode ? "flex flex-col gap-6 lg:flex-row lg:gap-10" : "flex flex-col gap-6 sm:flex-row sm:gap-10"}>
        <div className="shrink-0">{art}</div>
        <div className="min-w-0 flex-1 space-y-4">
          {isEpisode ? (
            <div className="space-y-1">
              {item.grandparent_id ? (
                <Link
                  to={`/item/${item.grandparent_id}`}
                  className="text-lg font-semibold text-fg/90 transition-colors hover:text-accent"
                >
                  {item.grandparent_title}
                </Link>
              ) : null}
              <div className="text-sm text-muted">
                {item.parent_id ? (
                  <Link to={`/item/${item.parent_id}`} className="hover:text-fg">
                    {item.parent_index != null ? seasonTitle({ title: "", index: item.parent_index }) : (item.parent_title ?? "Season")}
                  </Link>
                ) : null}
                {item.index != null ? <span> · Episode {item.index}</span> : null}
              </div>
            </div>
          ) : null}

          <h1 className="text-3xl font-bold leading-tight tracking-tight text-balance sm:text-4xl">{item.title}</h1>

          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <MetaLine
              parts={[
                isEpisode ? formatDate(item.release_date) : item.year,
                formatDuration(item.duration_ms),
                item.content_rating ? <ContentRating key="cr" value={item.content_rating} /> : null,
                item.rating != null && item.rating > 0 ? <RatingBadge key="r" rating={item.rating} /> : null,
                qualityBadgeLabels(item).length ? <QualityBadges key="q" item={item} /> : null,
              ]}
            />
            <StarRating item={item} />
          </div>

          <Genres genres={item.genres} />
          <PartOfCollections item={item} />

          <div className="flex flex-wrap items-center gap-2.5 pt-1">
            {item.files.length ? (
              <Button variant="primary" size="lg" icon={<Play className="size-5 fill-current" />} onClick={() => onPlay(false)}>
                {resumable ? "Resume" : "Play"}
              </Button>
            ) : (
              <Button variant="secondary" size="lg" disabled>
                Unavailable
              </Button>
            )}
            {resumable ? (
              <Button variant="secondary" size="lg" icon={<RotateCcw className="size-4" />} onClick={() => onPlay(true)}>
                <span className="hidden sm:inline">From start</span>
              </Button>
            ) : null}
            <WatchedButton item={item} />
            <ListButtons item={item} />
            <RefreshButton item={item} />
          </div>

          <ResumeProgress item={item} />

          {item.tagline ? <p className="text-base italic text-fg/75">{item.tagline}</p> : null}
          <Summary text={item.summary} />

          {file && (item.files.length > 1 || audioStreams.length > 1 || subStreams.length > 0) ? (
            <div className="flex flex-wrap gap-3 pt-2">
              {item.files.length > 1 ? (
                <LabeledSelect label="Version" value={String(file.id)} onChange={(v) => setFileId(Number(v))}>
                  {item.files.map((f, i) => (
                    <option key={f.id} value={f.id}>
                      {versionLabel(f, i)}
                    </option>
                  ))}
                </LabeledSelect>
              ) : null}
              {audioStreams.length > 1 ? (
                <LabeledSelect
                  label="Audio"
                  value={String(audioIndex ?? defaultAudio?.index ?? "")}
                  onChange={(v) => {
                    setAudio({ file: file.id, index: Number(v) });
                    remember(Number(v), subIndex);
                  }}
                >
                  {audioStreams.map((s) => (
                    <option key={s.index} value={s.index}>
                      {streamLabel(s)}
                    </option>
                  ))}
                </LabeledSelect>
              ) : null}
              {subStreams.length > 0 ? (
                <LabeledSelect
                  label="Subtitles"
                  value={subIndex == null ? "off" : String(subIndex)}
                  onChange={(v) => {
                    const index = v === "off" ? null : Number(v);
                    setSub({ file: file.id, index });
                    remember(audioIndex, index);
                  }}
                >
                  <option value="off">Off</option>
                  {subStreams.map((s) => (
                    <option key={s.index} value={s.index}>
                      {streamLabel(s)}
                    </option>
                  ))}
                </LabeledSelect>
              ) : null}
            </div>
          ) : null}

          <dl className="grid max-w-3xl grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 pt-2 text-sm">
            <CrewRows crew={item.crew} />
            {item.original_title && item.original_title !== item.title ? (
              <>
                <dt className="text-subtle">Original title</dt>
                <dd className="text-fg/85">{item.original_title}</dd>
              </>
            ) : null}
            {item.release_date && !isEpisode ? (
              <>
                <dt className="text-subtle">Released</dt>
                <dd className="text-fg/85">{formatDate(item.release_date)}</dd>
              </>
            ) : null}
            {item.studio ? (
              <>
                <dt className="text-subtle">{isEpisode ? "Network" : "Studio"}</dt>
                <dd className="text-fg/85">{item.studio}</dd>
              </>
            ) : null}
            {item.user_state.play_count > 0 ? (
              <>
                <dt className="text-subtle">Plays</dt>
                <dd className="text-fg/85">{item.user_state.play_count}</dd>
              </>
            ) : null}
          </dl>

          <MediaInfo files={item.files} />
        </div>
      </div>
      <CastRow title={isEpisode ? "Guest Stars" : "Cast"} credits={item.cast} />
      {isEpisode ? null : <MoreLikeThis item={item} />}
    </ItemPageShell>
  );
}
