import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Eye, EyeOff, Save } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useSaveSettings, useSettings, useSystemInfo } from "@/api/hooks";
import type { HwAccel, Settings, ToneMapping, TranscodePreset } from "@/api/types";
import { useToast } from "@/components/Toast";
import { Button, Card, EmptyState, IconButton, PageSpinner, Select, TextInput, Toggle } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";

const HWACCEL: { value: HwAccel; label: string }[] = [
  { value: "auto", label: "Automatic (best available)" },
  { value: "nvenc", label: "NVIDIA NVENC" },
  { value: "qsv", label: "Intel Quick Sync (QSV)" },
  { value: "amf", label: "AMD AMF" },
  { value: "vaapi", label: "VAAPI (Linux)" },
  { value: "videotoolbox", label: "VideoToolbox (macOS)" },
  { value: "none", label: "Software only (libx264)" },
];

const TONEMAP: { value: ToneMapping; label: string }[] = [
  { value: "auto", label: "Automatic" },
  { value: "libplacebo", label: "libplacebo (GPU, best quality, Dolby Vision)" },
  { value: "opencl", label: "OpenCL (GPU)" },
  { value: "zscale", label: "zscale (CPU)" },
  { value: "none", label: "Disabled (HDR may look washed out)" },
];

const PRESETS: TranscodePreset[] = ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium"];

const LANGUAGES = [
  "en-US",
  "en-GB",
  "fr-FR",
  "de-DE",
  "es-ES",
  "es-MX",
  "it-IT",
  "pt-BR",
  "pt-PT",
  "nl-NL",
  "sv-SE",
  "da-DK",
  "fi-FI",
  "nb-NO",
  "pl-PL",
  "cs-CZ",
  "ru-RU",
  "uk-UA",
  "tr-TR",
  "ar-SA",
  "he-IL",
  "hi-IN",
  "ja-JP",
  "ko-KR",
  "zh-CN",
  "zh-TW",
];

function Group({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <Card className="space-y-5">
      <div>
        <h3 className="text-base font-semibold">{title}</h3>
        {description ? <p className="mt-0.5 text-sm text-muted">{description}</p> : null}
      </div>
      <div className="space-y-5">{children}</div>
    </Card>
  );
}

function Row({ label, hint, children, htmlFor }: { label: string; hint?: ReactNode; children: ReactNode; htmlFor?: string }) {
  return (
    <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] sm:items-center sm:gap-6">
      <div>
        <label htmlFor={htmlFor} className="text-sm font-medium">
          {label}
        </label>
        {hint ? <div className="mt-0.5 text-xs text-subtle">{hint}</div> : null}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

export function ServerSettings() {
  useDocumentTitle("Server settings");
  const settings = useSettings();
  const system = useSystemInfo();
  const save = useSaveSettings();
  const toast = useToast();
  const [form, setForm] = useState<Settings | null>(null);
  const [showKey, setShowKey] = useState(false);

  useEffect(() => {
    if (settings.data) setForm(settings.data);
  }, [settings.data]);

  const dirty = useMemo(() => {
    if (!form || !settings.data) return false;
    return (Object.keys(form) as (keyof Settings)[]).some((k) => form[k] !== settings.data[k]);
  }, [form, settings.data]);

  if (settings.isPending || (!form && !settings.isError)) return <PageSpinner />;
  if (settings.isError || !form) return <EmptyState title="Couldn't load settings" description={errorMessage(settings.error)} />;

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => setForm((f) => (f ? { ...f, [key]: value } : f));
  const tc = system.data?.transcoder;

  const onSave = () => {
    if (!form || !settings.data) return;
    const changes: Partial<Settings> = {};
    for (const k of Object.keys(form) as (keyof Settings)[]) {
      if (form[k] !== settings.data[k]) (changes as Record<string, unknown>)[k] = form[k];
    }
    save.mutate(changes, {
      onSuccess: () => toast.success("Settings saved"),
      onError: (e) => toast.error(errorMessage(e)),
    });
  };

  return (
    <form
      className="space-y-5 pb-20"
      onSubmit={(e) => {
        e.preventDefault();
        onSave();
      }}
    >
      <div>
        <h2 className="text-lg font-semibold">Server</h2>
        <p className="text-sm text-muted">Changes apply immediately — no restart needed.</p>
      </div>

      <Group title="General">
        <Row label="Server name" hint="Shown on the sign-in screen." htmlFor="server-name">
          <TextInput id="server-name" value={form.server_name} onChange={(e) => set("server_name", e.target.value)} />
        </Row>
      </Group>

      <Group title="Metadata" description="Posters, summaries, ratings and episode titles.">
        <Row
          label="TMDB API key"
          htmlFor="tmdb-key"
          hint={
            <>
              Free key from{" "}
              <a href="https://www.themoviedb.org/settings/api" target="_blank" rel="noreferrer" className="text-accent hover:underline">
                themoviedb.org/settings/api
              </a>
              . Leave empty to use file names and local artwork only.
            </>
          }
        >
          <div className="flex gap-2">
            <TextInput
              id="tmdb-key"
              type={showKey ? "text" : "password"}
              autoComplete="off"
              spellCheck={false}
              value={form.tmdb_api_key}
              onChange={(e) => set("tmdb_api_key", e.target.value)}
              placeholder="Not set"
              className="font-mono text-xs"
            />
            <IconButton label={showKey ? "Hide key" : "Show key"} onClick={() => setShowKey((s) => !s)} className="rounded-md bg-surface-2">
              {showKey ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
            </IconButton>
          </div>
        </Row>
        <Row label="Metadata language" htmlFor="meta-lang">
          <Select id="meta-lang" value={form.metadata_language} onChange={(e) => set("metadata_language", e.target.value)} className="w-full">
            {(LANGUAGES.includes(form.metadata_language) ? LANGUAGES : [form.metadata_language, ...LANGUAGES]).map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </Select>
        </Row>
      </Group>

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
        <Row
          label="Hardware acceleration"
          htmlFor="hwaccel"
          hint={tc && tc.encoders.length ? `Detected encoders: ${tc.encoders.join(", ")}` : "Encoder used for video transcodes."}
        >
          <Select id="hwaccel" value={form.hwaccel} onChange={(e) => set("hwaccel", e.target.value as HwAccel)} className="w-full">
            {HWACCEL.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </Select>
        </Row>
        <Row label="Hardware decoding" hint="Decode on the GPU when possible (falls back to CPU automatically).">
          <Toggle checked={form.hardware_decoding} onChange={(v) => set("hardware_decoding", v)} label="Hardware decoding" />
        </Row>
        <Row
          label="HDR tone mapping"
          htmlFor="tonemap"
          hint={
            tc && tc.tonemapping.length
              ? `Available: ${tc.tonemapping.join(", ")}${tc.dolby_vision ? " · Dolby Vision supported" : ""}`
              : "Convert HDR10, HLG and Dolby Vision to SDR when transcoding."
          }
        >
          <Select id="tonemap" value={form.tonemapping} onChange={(e) => set("tonemapping", e.target.value as ToneMapping)} className="w-full">
            {TONEMAP.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </Select>
        </Row>
        <Row label="Software encoder preset" htmlFor="preset" hint="Faster presets use less CPU but produce larger files (libx264 only).">
          <Select
            id="preset"
            value={form.transcode_preset}
            onChange={(e) => set("transcode_preset", e.target.value as TranscodePreset)}
            className="w-full"
          >
            {PRESETS.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </Select>
        </Row>
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
        </Row>
        <Row label="Throttle transcodes" hint="Limit ffmpeg to ~2× realtime once a buffer is built (saves CPU/GPU and disk).">
          <Toggle checked={form.throttle_transcodes} onChange={(v) => set("throttle_transcodes", v)} label="Throttle transcodes" />
        </Row>
      </Group>

      <Group title="Library scanning">
        <Row label="Watch folders for changes" hint="Pick up new, changed and deleted files automatically.">
          <Toggle checked={form.watch_libraries} onChange={(v) => set("watch_libraries", v)} label="Watch folders" />
        </Row>
        <Row
          label="Detect intros and credits"
          hint="Compare the audio of episodes in the background to offer Skip Intro and Skip Credits. Chapters named Intro or Credits are always used."
        >
          <Toggle
            checked={form.detect_markers}
            onChange={(v) => set("detect_markers", v)}
            label="Detect intros and credits"
          />
        </Row>
        <Row
          label="Seek previews"
          hint="Make the thumbnails shown above the seek bar, in the background, the first time a movie or episode is opened."
        >
          <Toggle checked={form.seek_previews} onChange={(v) => set("seek_previews", v)} label="Seek previews" />
        </Row>
        <Row label="Periodic scan interval" htmlFor="scan-interval" hint="Minutes between full scans. 0 disables periodic scans.">
          <div className="flex items-center gap-2">
            <TextInput
              id="scan-interval"
              type="number"
              min={0}
              value={form.scan_interval_minutes}
              onChange={(e) => set("scan_interval_minutes", Math.max(0, Number(e.target.value) || 0))}
              className="w-28"
            />
            <span className="text-sm text-muted">minutes</span>
          </div>
        </Row>
      </Group>

      <div className="sticky bottom-4 z-10 flex justify-end">
        <div className="flex items-center gap-3 rounded-lg border border-border bg-surface-2/95 px-3 py-2 shadow-xl backdrop-blur">
          {dirty ? <span className="text-xs text-muted">Unsaved changes</span> : null}
          <Button variant="ghost" size="sm" disabled={!dirty || save.isPending} onClick={() => settings.data && setForm(settings.data)}>
            Reset
          </Button>
          <Button type="submit" variant="primary" size="sm" icon={<Save className="size-4" />} loading={save.isPending} disabled={!dirty}>
            Save
          </Button>
        </div>
      </div>
    </form>
  );
}
