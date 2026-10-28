import { useState } from "react";
import { CircleAlert, FolderOpen } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSaveTranscodeDir, useSystemInfo } from "@/api/hooks";
import type { HwAccel, Settings, ToneMapping, TranscodeDir, TranscodePreset } from "@/api/types";
import { FolderPicker } from "@/components/FolderPicker";
import { useToast } from "@/components/Toast";
import { Button, Select, TextInput, Toggle } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";
import { Group, Link, Note, Row, SettingsForm, TestButton, useSettingsForm } from "./SettingsForm";
import {
  accelChoices,
  accelInUse,
  accelName,
  accelReason,
  capitalize,
  knownTranscoder,
  tonemapChoices,
  tonemapFallback,
  tonemapReason,
  transcoderOs,
} from "./transcodeChoices";

/** The settings this page edits. */
const KEYS = [
  "hwaccel",
  "hardware_decoding",
  "tonemapping",
  "transcode_preset",
  "max_transcodes",
  "throttle_transcodes",
] as const satisfies readonly (keyof Settings)[];

const PRESETS: { value: TranscodePreset; label: string }[] = [
  { value: "auto", label: "Automatic (adapts to load)" },
  { value: "ultrafast", label: "ultrafast" },
  { value: "superfast", label: "superfast" },
  { value: "veryfast", label: "veryfast" },
  { value: "faster", label: "faster" },
  { value: "fast", label: "fast" },
  { value: "medium", label: "medium" },
];

/** FFmpeg builds with libplacebo for the transcoder's system (every one's, when it runs elsewhere). */
function DolbyVisionBuilds({ os }: { os: string | null }) {
  const gyan = <Link href="https://www.gyan.dev/ffmpeg/builds/">gyan.dev's full build</Link>;
  const jellyfin = <Link href="https://github.com/jellyfin/jellyfin-ffmpeg/releases">jellyfin-ffmpeg</Link>;
  if (os === "windows") return <>{gyan} or {jellyfin}</>;
  if (os === "linux") return jellyfin;
  if (os === "macos") return <>Homebrew's ffmpeg-full</>;
  return (
    <>
      {gyan} on Windows, {jellyfin} on Windows and Linux, or Homebrew's ffmpeg-full on a Mac
    </>
  );
}

/**
 * Where the transcoder works. Saved in pley.toml for pley's next start: the
 * transcoder can't move what it is making while it runs.
 */
function TranscodeFolder({ dir }: { dir: TranscodeDir }) {
  const save = useSaveTranscodeDir();
  const toast = useToast();
  const [picking, setPicking] = useState(false);
  // The folder pley's next start tries: the one saved, even if it couldn't be used this time.
  const chosen = dir.next ?? dir.problem?.folder ?? dir.in_use;
  const choose = (path: string | null) =>
    save.mutate(path, {
      onSuccess: (saved) => toast.success(saved.next ? "Saved: pley uses it from its next start" : "Saved"),
      onError: (e) => toast.error(errorMessage(e)),
    });

  return (
    <Row
      label="Transcode folder"
      hint="Holds stream segments, converted downloads and caches. Use a fast local disk with free space. If it can't be used, pley falls back to the data folder."
    >
      <div className="space-y-2">
        <div className="break-all font-mono text-xs text-fg/80">{dir.in_use}</div>
        {dir.problem ? (
          <p className="flex items-start gap-1.5 text-xs text-danger">
            <CircleAlert className="mt-px size-3.5 shrink-0" />
            <span>
              <span className="break-all font-mono">{dir.problem.folder}</span> couldn't be used ({dir.problem.reason}), so the
              default folder is. pley tries it again when it next starts.
            </span>
          </p>
        ) : null}
        {dir.next ? (
          <p className="text-xs text-accent">
            From pley's next start: <span className="break-all font-mono">{dir.next}</span>
          </p>
        ) : null}
        {dir.fixed ? (
          <p className="text-xs text-subtle">Chosen on pley's command line or in its environment (PLEY_TRANSCODE_DIR).</p>
        ) : (
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              icon={<FolderOpen className="size-4" />}
              loading={save.isPending}
              onClick={() => setPicking(true)}
            >
              Change…
            </Button>
            {chosen !== dir.default ? (
              <Button size="sm" variant="ghost" disabled={save.isPending} onClick={() => choose(null)}>
                Use the default
              </Button>
            ) : null}
          </div>
        )}
      </div>
      <FolderPicker
        open={picking}
        onClose={() => setPicking(false)}
        initialPath={chosen}
        onSelect={(path) => {
          setPicking(false);
          choose(path);
        }}
      />
    </Row>
  );
}

export function TranscoderSettings() {
  useDocumentTitle("Transcoder settings");
  const draft = useSettingsForm(KEYS);
  const system = useSystemInfo();
  const saved = draft.settings.data;
  const set = draft.set;

  return (
    <SettingsForm draft={draft} title="Transcoder" description="Converts video a player can't play directly. Changes apply to new transcodes.">
      {(form) => {
        const tc = system.data?.transcoder;
        // What the transcoder's FFmpeg can do, once it says (nothing is greyed out before).
        const known = knownTranscoder(system.data);
        const os = transcoderOs(system.data);
        const accels = accelChoices(saved?.hwaccel ?? form.hwaccel, system.data);
        const accelProblem = accelReason(form.hwaccel, system.data);
        const encodesOn = accelInUse(form.hwaccel, system.data);
        const tonemaps = tonemapChoices(system.data);
        const tonemapProblem = tonemapReason(form.tonemapping, system.data);
        const noGpuDecoding = known?.hardware_decoding === false;
        const noThrottling = known?.throttling === false;

        return (
          <>
            <Group
              title="Transcoder"
              description={
                tc
                  ? tc.available
                    ? `pley-transcoder ${tc.version ?? ""} · ffmpeg ${tc.ffmpeg_version ?? "?"} · active encoder: ${tc.active_encoder ?? "none"}`
                    : `Transcoder unavailable${tc.error ? `: ${tc.error}` : ""}`
                  : undefined
              }
            >
              <Row label="Connection" hint={tc?.managed === false ? `External transcoder at ${tc.url ?? "?"}` : "Started by pley on this computer."}>
                <TestButton label="Test the transcoder" test={{ service: "transcoder" }} className="" />
              </Row>
              {/* An external transcoder keeps a folder of its own. */}
              {system.data?.transcode_dir && tc?.managed !== false ? <TranscodeFolder dir={system.data.transcode_dir} /> : null}
            </Group>

            <Group title="Video" description="How video is encoded, decoded and tone mapped.">
              <Row
                label="Hardware acceleration"
                htmlFor="hwaccel"
                hint={tc && tc.encoders.length ? `Detected encoders: ${tc.encoders.join(", ")}` : "Encoder used for video transcodes."}
              >
                <Select id="hwaccel" value={form.hwaccel} onChange={(e) => set("hwaccel", e.target.value as HwAccel)} className="w-full">
                  {accels.map((o) => (
                    <option key={o.value} value={o.value} disabled={o.reason !== null}>
                      {o.label}
                    </option>
                  ))}
                </Select>
                {accelProblem ? (
                  <Note notice>
                    {capitalize(accelProblem)}: transcodes are encoded in software instead
                    {known?.active_encoder && form.hwaccel === saved?.hwaccel ? ` (${known.active_encoder})` : ""}.
                  </Note>
                ) : null}
              </Row>
              <Row label="Hardware decoding" hint="Decode on the GPU when possible (falls back to CPU automatically).">
                <Toggle
                  checked={form.hardware_decoding}
                  disabled={noGpuDecoding}
                  onChange={(v) => set("hardware_decoding", v)}
                  label="Hardware decoding"
                />
                {noGpuDecoding ? <Note>No graphics card here that FFmpeg decodes on: videos are decoded on the CPU.</Note> : null}
              </Row>
              <Row
                label="Software encoder preset"
                htmlFor="preset"
                hint="Faster presets use less CPU but make larger files (libx264 and libx265). Automatic adjusts to the computer's load."
              >
                <Select
                  id="preset"
                  value={form.transcode_preset}
                  onChange={(e) => set("transcode_preset", e.target.value as TranscodePreset)}
                  className="w-full"
                >
                  {PRESETS.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </Select>
                {encodesOn ? <Note>{accelName(encodesOn)} encodes here: this applies only when software encodes instead.</Note> : null}
              </Row>
              <Row
                label="HDR tone mapping"
                htmlFor="tonemap"
                hint={
                  <>
                    {tc && tc.tonemapping.length
                      ? `Available: ${tc.tonemapping.join(", ")}${tc.dolby_vision ? " · Dolby Vision supported" : ""}`
                      : "Convert HDR10, HLG and Dolby Vision to SDR when transcoding."}
                    {tc?.available && !tc.dolby_vision ? (
                      <span className="mt-1 block">
                        Dolby Vision needs an FFmpeg with libplacebo, and a GPU it reaches through Vulkan: <DolbyVisionBuilds os={os} />. Point
                        pley at it with --ffmpeg and --ffprobe, or in pley.toml.
                      </span>
                    ) : null}
                  </>
                }
              >
                <Select id="tonemap" value={form.tonemapping} onChange={(e) => set("tonemapping", e.target.value as ToneMapping)} className="w-full">
                  {tonemaps.map((o) => (
                    <option key={o.value} value={o.value} disabled={o.reason !== null}>
                      {o.label}
                    </option>
                  ))}
                </Select>
                {tonemapProblem ? (
                  <Note notice>
                    {capitalize(tonemapProblem)}: {tonemapFallback(system.data)}
                  </Note>
                ) : null}
              </Row>
            </Group>

            <Group title="Load" description="How many transcodes run at once, and how far ahead of the player they work.">
              <Row label="Max simultaneous transcodes" htmlFor="max-transcodes">
                <TextInput
                  id="max-transcodes"
                  type="number"
                  min={1}
                  max={64}
                  value={form.max_transcodes}
                  onChange={(e) => set("max_transcodes", Math.max(1, Number(e.target.value) || 1))}
                  className="w-28"
                />
                {tc?.available && tc.max_jobs > 0 && form.max_transcodes > tc.max_jobs ? (
                  <Note>
                    The transcoder itself runs {tc.max_jobs} at most
                    {tc.managed ? <> (transcoder_max_jobs in pley.toml)</> : <> (its --max-jobs)</>}.
                  </Note>
                ) : null}
              </Row>
              <Row
                label="Throttle transcodes"
                hint="Once a buffer is built, convert at up to 2× real time and pause 90 s ahead of the player. Saves CPU, GPU and disk for other playbacks."
              >
                <Toggle
                  checked={form.throttle_transcodes}
                  disabled={noThrottling}
                  onChange={(v) => set("throttle_transcodes", v)}
                  label="Throttle transcodes"
                />
                {noThrottling ? <Note>This FFmpeg can't hold transcodes back: it needs FFmpeg 6.1 or newer.</Note> : null}
              </Row>
            </Group>
          </>
        );
      }}
    </SettingsForm>
  );
}
