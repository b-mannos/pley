import { useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import type { MediaFile, MediaStream } from "@/api/types";
import { cn } from "@/lib/cn";
import {
  channelsLabel,
  codecLabel,
  dynamicRangeLabel,
  formatBitrate,
  formatBytes,
  formatDuration,
  resolutionLabel,
} from "@/lib/format";
import { languageName } from "@/lib/languages";

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[7.5rem_1fr] gap-3 py-1 text-sm">
      <dt className="text-subtle">{label}</dt>
      <dd className="min-w-0 text-fg/90">{children}</dd>
    </div>
  );
}

function streamDetails(s: MediaStream): string[] {
  const parts: string[] = [];
  if (s.kind === "video") {
    const res = resolutionLabel(s.width, s.height);
    if (res) parts.push(res);
    if (s.width && s.height) parts.push(`${s.width}×${s.height}`);
    if (s.frame_rate) parts.push(`${Math.round(s.frame_rate * 100) / 100} fps`);
    if (s.bit_depth) parts.push(`${s.bit_depth}-bit`);
    const range = dynamicRangeLabel(s);
    if (range) parts.push(range);
    if (s.profile) parts.push(s.profile);
  } else if (s.kind === "audio") {
    const ch = channelsLabel(s.channels, s.channel_layout);
    if (ch) parts.push(ch);
    if (s.sample_rate) parts.push(`${(s.sample_rate / 1000).toFixed(1)} kHz`);
  } else {
    parts.push(s.text_based === false ? "Image-based" : "Text");
    if (s.external) parts.push("External");
  }
  if (s.bitrate) parts.push(formatBitrate(s.bitrate));
  if (s.forced) parts.push("Forced");
  if (s.default) parts.push("Default");
  return parts;
}

function StreamTable({ streams }: { streams: MediaStream[] }) {
  if (!streams.length) return null;
  return (
    <div className="mt-3 overflow-x-auto">
      <table className="w-full min-w-[520px] text-left text-sm">
        <thead>
          <tr className="text-xs text-subtle">
            <th className="py-1.5 pr-4 font-semibold">Type</th>
            <th className="py-1.5 pr-4 font-semibold">Codec</th>
            <th className="py-1.5 pr-4 font-semibold">Language</th>
            <th className="py-1.5 pr-4 font-semibold">Details</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-white/[0.05]">
          {streams.map((s) => (
            <tr key={`${s.kind}-${s.index}`} className="align-top">
              <td className="py-1.5 pr-4 capitalize text-muted">{s.kind}</td>
              <td className="py-1.5 pr-4 font-medium">{codecLabel(s.codec)}</td>
              <td className="py-1.5 pr-4 text-fg/85">
                {languageName(s.language) ?? "—"}
                {s.title ? <span className="block text-xs text-subtle">{s.title}</span> : null}
              </td>
              <td className="py-1.5 pr-4 text-fg/75">{streamDetails(s).join(" · ")}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function FileInfo({ file, title }: { file: MediaFile; title?: string }) {
  const order: Record<MediaFile["streams"][number]["kind"], number> = { video: 0, audio: 1, subtitle: 2 };
  const streams = [...file.streams].sort((a, b) => order[a.kind] - order[b.kind] || a.index - b.index);
  return (
    <div className="rounded-lg border border-border bg-surface/80 p-4 backdrop-blur">
      {title ? <h3 className="mb-2 text-sm font-semibold">{title}</h3> : null}
      <dl>
        <Row label="File">
          <span className="break-all font-mono text-xs text-fg/80">{file.path}</span>
        </Row>
        <Row label="Container">{file.container?.toUpperCase() ?? "—"}</Row>
        <Row label="Size">{formatBytes(file.size)}</Row>
        {file.duration_ms ? <Row label="Duration">{formatDuration(file.duration_ms)}</Row> : null}
        {file.bitrate ? <Row label="Bitrate">{formatBitrate(file.bitrate)}</Row> : null}
        {file.video_codec ? (
          <Row label="Video">
            {[
              codecLabel(file.video_codec),
              resolutionLabel(file.width, file.height),
              dynamicRangeLabel(file.streams.find((s) => s.kind === "video")),
            ]
              .filter(Boolean)
              .join(" · ")}
          </Row>
        ) : null}
        {file.audio_codec ? <Row label="Audio">{codecLabel(file.audio_codec)}</Row> : null}
      </dl>
      <StreamTable streams={streams} />
    </div>
  );
}

export function MediaInfo({ files }: { files: MediaFile[] }) {
  const [open, setOpen] = useState(false);
  if (!files.length) return null;
  return (
    <section className="mt-10 max-w-4xl">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex items-center gap-2 text-sm font-semibold text-muted transition-colors hover:text-fg"
      >
        <ChevronDown className={cn("size-4 transition-transform", open && "rotate-180")} />
        Media info
        {files.length > 1 ? <span className="text-subtle">({files.length} versions)</span> : null}
      </button>
      {open ? (
        <div className="mt-3 space-y-3">
          {files.map((f, i) => (
            <FileInfo key={f.id} file={f} title={f.edition ?? (files.length > 1 ? `Version ${i + 1}` : undefined)} />
          ))}
        </div>
      ) : null}
    </section>
  );
}
