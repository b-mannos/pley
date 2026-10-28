import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useLocation } from "react-router";
import { useQueryClient } from "@tanstack/react-query";
import { CircleAlert, CircleCheck, Eye, EyeOff, FolderOpen, Info, Save } from "lucide-react";
import { appKeys } from "@/api/apps";
import { errorMessage } from "@/api/client";
import { useSaveSettings, useSaveTranscodeDir, useSettings, useSystemInfo, useTestService } from "@/api/hooks";
import type { HwAccel, ServiceTest, Settings, ToneMapping, TranscodeDir, TranscodePreset, TvMetadata } from "@/api/types";
import { FolderPicker } from "@/components/FolderPicker";
import { useToast } from "@/components/Toast";
import { Button, Card, EmptyState, IconButton, PageSpinner, Select, TextInput, Toggle } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle, useOnChange } from "@/lib/hooks";
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

const PRESETS: { value: TranscodePreset; label: string }[] = [
  { value: "auto", label: "Automatic (adapts to load)" },
  { value: "ultrafast", label: "ultrafast" },
  { value: "superfast", label: "superfast" },
  { value: "veryfast", label: "veryfast" },
  { value: "faster", label: "faster" },
  { value: "fast", label: "fast" },
  { value: "medium", label: "medium" },
];

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

const TV_SOURCES: Record<Exclude<TvMetadata, "auto">, string> = { tmdb: "TMDB", tvdb: "TheTVDB", tvmaze: "TVmaze" };

/** The TV source "Automatic" stands for with these keys (the server decides the same way). */
function automaticTvSource(form: Settings): Exclude<TvMetadata, "auto"> {
  if (form.tmdb_api_key.trim()) return "tmdb";
  if (form.tvdb_api_key.trim()) return "tvdb";
  return "tvmaze";
}

function Link({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noreferrer" className="text-accent hover:underline">
      {children}
    </a>
  );
}

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

/** A key or PIN: hidden unless asked for. */
function SecretInput({
  id,
  value,
  onChange,
  what,
  className,
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  what: string;
  className?: string;
}) {
  const [visible, setVisible] = useState(false);
  return (
    <div className="flex gap-2">
      <TextInput
        id={id}
        type={visible ? "text" : "password"}
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Not set"
        className={cn("font-mono text-xs", className)}
      />
      <IconButton label={`${visible ? "Hide" : "Show"} ${what}`} onClick={() => setVisible((s) => !s)} className="rounded-md bg-surface-2">
        {visible ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
      </IconButton>
    </div>
  );
}

/**
 * Tests a service with what the form holds (a key not saved yet) and says
 * how it went. Give it a `key` that changes with the key it tests, so an
 * answer about another key doesn't linger.
 */
export function TestButton({ test, label, className = "mt-2" }: { test: ServiceTest; label: string; className?: string }) {
  const check = useTestService();
  const result = check.data;
  return (
    <div className={cn("flex min-w-0 items-start gap-3", className)}>
      <Button size="sm" variant="outline" loading={check.isPending} onClick={() => check.mutate(test)} aria-label={label} className="shrink-0">
        Test
      </Button>
      <div role="status" className="min-w-0 pt-1 text-xs">
        {check.isError ? (
          <span className="text-danger">{errorMessage(check.error)}</span>
        ) : result ? (
          <span className={cn("flex items-start gap-1.5", result.ok ? "text-success" : "text-danger")}>
            {result.ok ? <CircleCheck className="mt-px size-3.5 shrink-0" /> : <CircleAlert className="mt-px size-3.5 shrink-0" />}
            <span>
              {result.message}
              {result.ok ? <span className="text-subtle"> {result.ms} ms</span> : null}
            </span>
          </span>
        ) : null}
      </div>
    </div>
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

/**
 * Under a choice: why it is greyed out here, or (`notice`) what happens
 * instead of a choice saved before that can't work here.
 */
function Note({ notice, children }: { notice?: boolean; children: ReactNode }) {
  return (
    <p className={cn("mt-1.5 flex items-start gap-1.5 text-xs", notice ? "text-fg/80" : "text-subtle")}>
      {notice ? <CircleAlert className="mt-px size-3.5 shrink-0 text-danger" /> : <Info className="mt-px size-3.5 shrink-0" />}
      <span>{children}</span>
    </p>
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
      hint="Stream segments, converted downloads and the transcoder's caches: a fast local disk with room to spare. If the folder can't be used when pley starts, the default one in the data folder is."
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

export function ServerSettings() {
  useDocumentTitle("Server settings");
  const settings = useSettings();
  const system = useSystemInfo();
  const save = useSaveSettings();
  const qc = useQueryClient();
  const toast = useToast();
  // The form starts from the saved settings, which are already there when
  // the page opens again (cached) or was opened by its route, and returns to
  // them after saving.
  const [form, setForm] = useState<Settings | null>(() => settings.data ?? null);

  // A link to one field (`#tmdb-key`) focuses it once the form is there.
  const { hash } = useLocation();
  const loaded = form !== null;
  useEffect(() => {
    if (loaded && hash) document.getElementById(decodeURIComponent(hash.slice(1)))?.focus();
  }, [loaded, hash]);

  useOnChange(settings.data, (data) => {
    if (data) setForm(data);
  });

  const dirty = useMemo(() => {
    if (!form || !settings.data) return false;
    return (Object.keys(form) as (keyof Settings)[]).some((k) => form[k] !== settings.data[k]);
  }, [form, settings.data]);

  if (settings.isPending || (!form && !settings.isError)) return <PageSpinner />;
  if (settings.isError || !form) {
    return <EmptyState title="Couldn't load settings" description={errorMessage(settings.error)} action={<Button onClick={() => void settings.refetch()}>Retry</Button>} />;
  }

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => setForm((f) => (f ? { ...f, [key]: value } : f));
  // A source can't stay chosen once its key is gone.
  const setKey = (key: "tmdb_api_key" | "tvdb_api_key", value: string) =>
    setForm((f) => {
      if (!f) return f;
      const source: TvMetadata = key === "tmdb_api_key" ? "tmdb" : "tvdb";
      const orphaned = !value.trim() && f.tv_metadata === source;
      return { ...f, [key]: value, ...(orphaned ? { tv_metadata: "auto" as const } : {}) };
    });
  const hasTmdb = form.tmdb_api_key.trim() !== "";
  const hasTvdb = form.tvdb_api_key.trim() !== "";
  const tc = system.data?.transcoder;
  // What the transcoder's FFmpeg can do, once it says (nothing is greyed out before).
  const known = knownTranscoder(system.data);
  const os = transcoderOs(system.data);
  const accels = accelChoices(settings.data?.hwaccel ?? form.hwaccel, system.data);
  const accelProblem = accelReason(form.hwaccel, system.data);
  const encodesOn = accelInUse(form.hwaccel, system.data);
  const tonemaps = tonemapChoices(system.data);
  const tonemapProblem = tonemapReason(form.tonemapping, system.data);
  const noGpuDecoding = known?.hardware_decoding === false;
  const noChromaprint = known?.chromaprint === false;
  const noThrottling = known?.throttling === false;

  const onSave = () => {
    if (!form || !settings.data) return;
    const changes: Partial<Settings> = {};
    for (const k of Object.keys(form) as (keyof Settings)[]) {
      if (form[k] !== settings.data[k]) (changes as Record<string, unknown>)[k] = form[k];
    }
    save.mutate(changes, {
      onSuccess: () => {
        toast.success("Settings saved");
        // Turning apps by other publishers off signed them out.
        if (changes.third_party_apps === false) {
          void qc.invalidateQueries({ queryKey: appKeys.mine });
          void qc.invalidateQueries({ queryKey: ["users"] });
        }
      },
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

      <Group title="Metadata" description="Posters, summaries, ratings, cast and episode titles.">
        <Row
          label="TMDB API key"
          htmlFor="tmdb-key"
          hint={
            <>
              Movies, collections and Fix Match use TMDB. Free key from{" "}
              <Link href="https://www.themoviedb.org/settings/api">themoviedb.org/settings/api</Link>. Without one, movies keep their file
              names and local artwork.
            </>
          }
        >
          <SecretInput id="tmdb-key" what="key" value={form.tmdb_api_key} onChange={(v) => setKey("tmdb_api_key", v)} />
          <TestButton key={form.tmdb_api_key} label="Test the TMDB key" test={{ service: "tmdb", api_key: form.tmdb_api_key }} />
        </Row>
        <Row
          label="TV shows"
          htmlFor="tv-source"
          hint={
            <>
              Automatic uses TMDB when it has a key, then TheTVDB when it has one, else <Link href="https://www.tvmaze.com">TVmaze</Link>,
              which needs none. Changing the source fetches every show again.
            </>
          }
        >
          <Select id="tv-source" value={form.tv_metadata} onChange={(e) => set("tv_metadata", e.target.value as TvMetadata)} className="w-full">
            <option value="auto">Automatic ({TV_SOURCES[automaticTvSource(form)]})</option>
            <option value="tmdb" disabled={!hasTmdb}>
              TMDB{hasTmdb ? "" : " (needs an API key)"}
            </option>
            <option value="tvdb" disabled={!hasTvdb}>
              TheTVDB{hasTvdb ? "" : " (needs an API key)"}
            </option>
            <option value="tvmaze">TVmaze (no key needed)</option>
          </Select>
          <TestButton label="Test TVmaze" test={{ service: "tvmaze" }} />
        </Row>
        <Row
          label="TheTVDB API key"
          htmlFor="tvdb-key"
          hint={
            <>
              Only needed to use TheTVDB for TV shows. Get one at <Link href="https://thetvdb.com/api-information">thetvdb.com/api-information</Link>.
            </>
          }
        >
          <SecretInput id="tvdb-key" what="key" value={form.tvdb_api_key} onChange={(v) => setKey("tvdb_api_key", v)} />
        </Row>
        <Row label="TheTVDB PIN" htmlFor="tvdb-pin" hint="User-supported keys also need your subscriber PIN. Leave empty for other keys.">
          <SecretInput id="tvdb-pin" what="PIN" value={form.tvdb_pin} onChange={(v) => set("tvdb_pin", v)} />
          <TestButton
            key={`${form.tvdb_api_key}
${form.tvdb_pin}`}
            label="Test the TheTVDB key"
            test={{ service: "tvdb", api_key: form.tvdb_api_key, pin: form.tvdb_pin }}
          />
        </Row>
        <Row
          label="Music metadata"
          hint={
            <>
              Genres and release dates from <Link href="https://musicbrainz.org">MusicBrainz</Link>, and covers from the Cover Art Archive for
              albums that have none. Tags in the files win. No key needed; the first look-up takes about a second per album.
            </>
          }
        >
          <Toggle checked={form.music_metadata} onChange={(v) => set("music_metadata", v)} label="Music metadata from MusicBrainz" />
          <TestButton label="Test MusicBrainz" test={{ service: "musicbrainz" }} />
        </Row>
        <Row label="Metadata language" htmlFor="meta-lang" hint="For TMDB and TheTVDB. TVmaze is in English.">
          <Select id="meta-lang" value={form.metadata_language} onChange={(e) => set("metadata_language", e.target.value)} className="w-full">
            {(LANGUAGES.includes(form.metadata_language) ? LANGUAGES : [form.metadata_language, ...LANGUAGES]).map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </Select>
        </Row>
        <p className="text-xs text-subtle">
          This product uses the TMDB API but is not endorsed or certified by TMDB. TV metadata from{" "}
          <Link href="https://thetvdb.com">TheTVDB</Link>. TV data from TVmaze is licensed{" "}
          <Link href="https://creativecommons.org/licenses/by-sa/4.0/">CC BY-SA</Link>; music data from MusicBrainz is public domain.
        </p>
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
        <Row label="Connection" hint={tc?.managed === false ? `External transcoder at ${tc.url ?? "?"}` : "Started by pley on this computer."}>
          <TestButton label="Test the transcoder" test={{ service: "transcoder" }} className="" />
        </Row>
        {/* An external transcoder keeps a folder of its own. */}
        {system.data?.transcode_dir && tc?.managed !== false ? <TranscodeFolder dir={system.data.transcode_dir} /> : null}
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
              {known?.active_encoder && form.hwaccel === settings.data?.hwaccel ? ` (${known.active_encoder})` : ""}.
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
        <Row
          label="Software encoder preset"
          htmlFor="preset"
          hint="Faster presets use less CPU for larger files (libx264 and libx265). Automatic starts as slow as this computer allows for the transcodes running, speeds up when a player is about to wait, and slows back down for better pictures once it is well ahead."
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
        <Row label="Throttle transcodes" hint="Limit ffmpeg to ~2× realtime once a buffer is built (saves CPU/GPU and disk).">
          <Toggle
            checked={form.throttle_transcodes}
            disabled={noThrottling}
            onChange={(v) => set("throttle_transcodes", v)}
            label="Throttle transcodes"
          />
          {noThrottling ? <Note>This FFmpeg can't hold transcodes back: it needs FFmpeg 6.1 or newer.</Note> : null}
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
            disabled={noChromaprint}
            onChange={(v) => set("detect_markers", v)}
            label="Detect intros and credits"
          />
          {noChromaprint ? <Note>This FFmpeg has no Chromaprint to compare audio with: only chapters find intros and credits.</Note> : null}
        </Row>
        <Row
          label="Seek previews"
          hint="Make the thumbnails shown above the seek bar, in the background, the first time a movie or episode is opened."
        >
          <Toggle checked={form.seek_previews} onChange={(v) => set("seek_previews", v)} label="Seek previews" />
        </Row>
        <Row
          label="Previews for whole libraries"
          hint="After each scan, make seek previews and chapter pictures for everything that lacks them, one file at a time, pausing while videos are being transcoded."
        >
          <Toggle
            checked={form.previews_in_background}
            disabled={!form.seek_previews}
            onChange={(v) => set("previews_in_background", v)}
            label="Previews for whole libraries"
          />
          {form.seek_previews ? null : <Note>Needs Seek previews.</Note>}
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

      <Group title="Apps">
        <Row
          label="Apps by other publishers"
          hint="pley's own apps can always sign in. Apps by other publishers are known by the address of a description they publish, which pley reads from the internet when someone allows one. Turning this off signs them out. Leave it off unless you need one."
        >
          {/* Off until a server that knows the setting sends it. */}
          <Toggle
            checked={form.third_party_apps ?? false}
            onChange={(v) => set("third_party_apps", v)}
            label="Apps by other publishers"
          />
        </Row>
      </Group>

      {/* Floats over the page only with something to save: at rest it would cover the rows on a phone. */}
      <div className={cn("flex justify-end", (dirty || save.isPending) && "sticky bottom-4 z-10")}>
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
