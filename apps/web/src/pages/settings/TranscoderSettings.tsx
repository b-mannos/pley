import { useState } from "react";
import { CircleAlert, FolderOpen } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSaveTranscodeDir, useSystemInfo } from "@/api/hooks";
import type { HwAccel, Settings, ToneMapping, TranscodeDir, TranscodePreset } from "@/api/types";
import { FolderPicker } from "@/components/FolderPicker";
import { useToast } from "@/components/Toast";
import { Button, Select, TextInput, Toggle } from "@/components/ui";
import { formatBytes } from "@/lib/format";
import { useDocumentTitle } from "@/lib/hooks";
import { Group, Link, Note, Row, SettingsForm, TestButton, useSettingsForm } from "./SettingsForm";
import {
  type AccelFit,
  accelChoices,
  accelReason,
  capitalize,
  choiceLabel,
  decodingReason,
  fitToAccel,
  knownTranscoder,
  memoryReason,
  presetNote,
  tonemapAccelReason,
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
  "transcode_in_memory",
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
      onSuccess: (saved) => toast.success(saved.next ? "Saved: Pley uses it from its next start" : "Saved"),
      onError: (e) => toast.error(errorMessage(e)),
    });

  return (
    <Row
      label="Transcode folder"
      hint="Holds stream segments, converted downloads and caches. Use a fast local disk with free space. If it can't be used, Pley falls back to the data folder."
    >
      <div className="space-y-2">
        <div className="break-all font-mono text-xs text-fg/80">{dir.in_use}</div>
        {dir.problem ? (
          <p className="flex items-start gap-1.5 text-xs text-danger">
            <CircleAlert className="mt-px size-3.5 shrink-0" />
            <span>
              <span className="break-all font-mono">{dir.problem.folder}</span> couldn't be used ({dir.problem.reason}), so the
              default folder is. Pley tries it again when it next starts.
            </span>
          </p>
        ) : null}
        {dir.next ? (
          <p className="text-xs text-accent">
            From Pley's next start: <span className="break-all font-mono">{dir.next}</span>
          </p>
        ) : null}
        {dir.fixed ? (
          <p className="text-xs text-subtle">Chosen on Pley's command line or in its environment (PLEY_TRANSCODE_DIR).</p>
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
  // What changed with the last hardware acceleration chosen, until changed again.
  const [fitted, setFitted] = useState<AccelFit["notes"]>({});
  const chooseAccel = (form: Settings, accel: HwAccel) => {
    const fit = fitToAccel(form, accel, system.data);
    draft.setForm((f) => (f ? { ...f, hwaccel: accel, ...fit.changes } : f));
    setFitted(fit.notes);
  };

  return (
    <SettingsForm draft={draft} title="Transcoder" description="Converts video a player can't play directly. Changes apply to new transcodes.">
      {(form) => {
        const tc = system.data?.transcoder;
        // What the transcoder's FFmpeg can do, once it says (nothing is greyed out before).
        const known = knownTranscoder(system.data);
        const os = transcoderOs(system.data);
        const accels = accelChoices(saved?.hwaccel ?? form.hwaccel, system.data);
        const accelProblem = accelReason(form.hwaccel, system.data);
        const preset = presetNote(form.hwaccel, system.data);
        const tonemaps = tonemapChoices(system.data, form.hwaccel);
        const tonemapProblem = tonemapReason(form.tonemapping, system.data);
        // A combination saved before: the transcoder doesn't use Vulkan then.
        const tonemapClash = tonemapProblem ? null : tonemapAccelReason(form.tonemapping, form.hwaccel, system.data);
        const decodingProblem = decodingReason(form.hwaccel, system.data);
        // Vulkan decodes on the GPU: hardware decoding turns on with it, not off.
        const vulkanDecodes = form.hwaccel === "vulkan";
        // Shown while the changes wait to be saved.
        const fittedNote = (key: keyof AccelFit["notes"]) => (draft.dirty ? fitted[key] : undefined);
        const noThrottling = known?.throttling === false;
        const memoryProblem = memoryReason(system.data, form.throttle_transcodes);
        const memoryBudget = tc?.memory?.available ? tc.memory.budget_bytes : 0;

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
              <Row label="Connection" hint={tc?.managed === false ? `External transcoder at ${tc.url ?? "?"}` : "Started by Pley on this computer."}>
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
                <Select id="hwaccel" value={form.hwaccel} onChange={(e) => chooseAccel(form, e.target.value as HwAccel)} className="w-full">
                  {accels.map((o) => (
                    <option key={o.value} value={o.value} disabled={o.reason !== null}>
                      {choiceLabel(o)}
                    </option>
                  ))}
                </Select>
                {accelProblem ? (
                  <Note notice>
                    {capitalize(accelProblem)}: transcodes are encoded in software instead.
                  </Note>
                ) : null}
              </Row>
              <Row label="Hardware decoding" hint="Decode on the GPU when possible.">
                <Toggle
                  checked={form.hardware_decoding}
                  disabled={decodingProblem !== null || (vulkanDecodes && form.hardware_decoding)}
                  onChange={(v) => {
                    set("hardware_decoding", v);
                    setFitted((f) => ({ ...f, hardware_decoding: undefined }));
                  }}
                  label="Hardware decoding"
                />
                {decodingProblem ? <Note>{capitalize(decodingProblem)}: videos are decoded on the CPU.</Note> : null}
                {fittedNote("hardware_decoding") ? (
                  <Note>{fittedNote("hardware_decoding")}</Note>
                ) : vulkanDecodes ? (
                  form.hardware_decoding ? (
                    <Note>Always on with Vulkan, which decodes on the GPU.</Note>
                  ) : (
                    <Note notice>Vulkan needs it: transcodes are encoded in software instead.</Note>
                  )
                ) : null}
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
                {preset ? <Note>{preset}</Note> : null}
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
                        Pley at it with --ffmpeg and --ffprobe, or in pley.toml.
                      </span>
                    ) : null}
                  </>
                }
              >
                <Select
                  id="tonemap"
                  value={form.tonemapping}
                  onChange={(e) => {
                    set("tonemapping", e.target.value as ToneMapping);
                    setFitted((f) => ({ ...f, tonemapping: undefined }));
                  }}
                  className="w-full"
                >
                  {tonemaps.map((o) => (
                    <option key={o.value} value={o.value} disabled={o.reason !== null}>
                      {choiceLabel(o)}
                    </option>
                  ))}
                </Select>
                {tonemapProblem ? (
                  <Note notice>
                    {capitalize(tonemapProblem)}: {tonemapFallback(system.data)}
                  </Note>
                ) : tonemapClash ? (
                  <Note notice>
                    {capitalize(tonemapClash)}:{" "}
                    {form.hwaccel === "vulkan" ? "transcodes are encoded in software instead." : "transcodes tone map with another instead."}
                  </Note>
                ) : fittedNote("tonemapping") ? (
                  <Note>{fittedNote("tonemapping")}</Note>
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
                hint="Once a buffer is built, convert at real time. Saves CPU, GPU and disk for other playbacks."
              >
                <Toggle
                  checked={form.throttle_transcodes}
                  disabled={noThrottling}
                  onChange={(v) => set("throttle_transcodes", v)}
                  label="Throttle transcodes"
                />
                {noThrottling ? <Note>This FFmpeg can't hold transcodes back: it needs FFmpeg 6.1 or newer.</Note> : null}
              </Row>
              <Row
                label="Transcode in memory"
                hint={`Keeps segments in RAM instead of on disk.${memoryBudget ? ` Up to ${formatBytes(memoryBudget)}.` : ""}`}
              >
                <Toggle
                  checked={form.transcode_in_memory}
                  disabled={memoryProblem !== null}
                  onChange={(v) => set("transcode_in_memory", v)}
                  label="Transcode in memory"
                />
                {memoryProblem ? <Note>{capitalize(memoryProblem)}: segments go to disk.</Note> : null}
              </Row>
            </Group>
          </>
        );
      }}
    </SettingsForm>
  );
}
