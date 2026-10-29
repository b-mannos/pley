import { useState } from "react";
import { Link } from "react-router";
import { ListVideo, Save, SlidersHorizontal, Tv2, Waypoints } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useLiveSettings, useLiveSources, useSaveLiveSettings, type LiveSettings } from "@/api/livetv";
import { useToast } from "@/components/Toast";
import { Button, buttonClass, EmptyState, PageSpinner, Select, Toggle } from "@/components/ui";
import { cn } from "@/lib/cn";
import { useDocumentTitle, useLocalStorage } from "@/lib/hooks";
import { Lineup } from "./livetv/Lineup";
import { Sources } from "./livetv/Sources";
import { Group, Note, Row } from "./SettingsForm";

type Tab = "sources" | "channels" | "options";

const TABS: { key: Tab; label: string; icon: typeof Tv2 }[] = [
  { key: "sources", label: "Sources", icon: Waypoints },
  { key: "channels", label: "Channels", icon: ListVideo },
  { key: "options", label: "Options", icon: SlidersHorizontal },
];

/** Settings → Live TV: where channels come from, which show, and how they play. */
export function LiveTvSettings() {
  useDocumentTitle("Live TV settings");
  const [tab, setTab] = useLocalStorage<Tab>("pley.settings.live.tab", "sources");
  const sources = useLiveSources();
  const hasSources = (sources.data?.length ?? 0) > 0;
  return (
    <div className="space-y-5 pb-20">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Live TV</h2>
          <p className="text-sm text-muted">Your IPTV channels and their guide, watched through this server.</p>
        </div>
        {hasSources ? (
          <Link to="/live" className={buttonClass("outline", "sm")}>
            <Tv2 className="size-4" /> Open Live TV
          </Link>
        ) : null}
      </div>
      <div role="tablist" aria-label="Live TV settings" className="flex gap-1 border-b border-border">
        {TABS.map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            role="tab"
            aria-selected={tab === key}
            onClick={() => setTab(key)}
            disabled={key !== "sources" && key !== "options" && !hasSources}
            className={cn(
              "-mb-px flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium transition-colors disabled:opacity-40",
              tab === key ? "border-accent text-fg" : "border-transparent text-muted hover:text-fg",
            )}
          >
            <Icon className={cn("size-4", tab === key ? "text-accent" : "text-subtle")} />
            {label}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab === "sources" || (tab === "channels" && !hasSources) ? <Sources /> : tab === "channels" ? <Lineup /> : <Options />}
      </div>
    </div>
  );
}

const HEIGHTS: [string, number | null][] = [
  ["As the channel sends it", null],
  ["1080p", 1080],
  ["720p", 720],
  ["480p", 480],
];

function Options() {
  const settings = useLiveSettings();
  if (settings.isPending) return <PageSpinner />;
  if (settings.isError) {
    return <EmptyState title="Couldn't load the settings" description={errorMessage(settings.error)} action={<Button onClick={() => void settings.refetch()}>Retry</Button>} />;
  }
  return <OptionsForm key={JSON.stringify(settings.data)} saved={settings.data} />;
}

function OptionsForm({ saved }: { saved: LiveSettings }) {
  const save = useSaveLiveSettings();
  const toast = useToast();
  const [form, setForm] = useState(saved);
  const set = <K extends keyof LiveSettings>(key: K, value: LiveSettings[K]) => setForm((f) => ({ ...f, [key]: value }));
  const dirty = JSON.stringify(form) !== JSON.stringify(saved);
  return (
    <form
      className="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(form, { onSuccess: () => toast.success("Settings saved"), onError: (err) => toast.error(errorMessage(err)) });
      }}
    >
      <Group title="Who watches" description="Live TV shows in the sidebar for the accounts it is open to.">
        <Row label="Open to" htmlFor="live-access">
          <Select id="live-access" value={form.access} onChange={(e) => set("access", e.target.value as LiveSettings["access"])} className="w-full">
            <option value="everyone">Everyone</option>
            <option value="admins">Administrators only</option>
          </Select>
        </Row>
        <Row label="Profiles with an age limit" hint="Channels carry no ratings, so the limit can't hold them to it.">
          <Toggle checked={form.age_limited} onChange={(v) => set("age_limited", v)} label="Profiles with an age limit watch Live TV" disabled={form.access === "admins"} />
        </Row>
      </Group>

      <Group title="Playing" description="Every player of a channel watches the same stream, made once by the transcoder.">
        <Row
          label="Deinterlace"
          hint="Interlaced channels (1080i, 576i) made smooth and progressive: a transcode, on the graphics card where the transcoder has one."
        >
          <Toggle checked={form.deinterlace} onChange={(v) => set("deinterlace", v)} label="Deinterlace interlaced channels" />
        </Row>
        <Row label="Transcoded channels" hint="The most lines a channel that has to be transcoded has." htmlFor="live-height">
          <Select
            id="live-height"
            value={String(form.max_height ?? "")}
            onChange={(e) => set("max_height", e.target.value ? Number(e.target.value) : null)}
            className="w-full"
          >
            {HEIGHTS.map(([label, h]) => (
              <option key={label} value={h ?? ""}>
                {label}
              </option>
            ))}
          </Select>
          <Note>H.264 channels are sent as they are, whatever their size.</Note>
        </Row>
        <Row label="Pause and rewind" hint="How far back a channel can be paused and gone back over." htmlFor="live-timeshift">
          <Select id="live-timeshift" value={form.timeshift_minutes} onChange={(e) => set("timeshift_minutes", Number(e.target.value))} className="w-full">
            {[1, 2, 5, 10, 15, 30, 60].map((m) => (
              <option key={m} value={m}>
                {m === 60 ? "1 hour" : `${m} minutes`}
              </option>
            ))}
          </Select>
        </Row>
      </Group>

      <Group title="Channels and guide">
        <Row label="Read again every" htmlFor="live-refresh">
          <Select id="live-refresh" value={form.refresh_hours} onChange={(e) => set("refresh_hours", Number(e.target.value))} className="w-full">
            {[3, 6, 12, 24, 48, 168].map((h) => (
              <option key={h} value={h}>
                {h === 168 ? "week" : h === 24 ? "day" : h === 48 ? "2 days" : `${h} hours`}
              </option>
            ))}
          </Select>
        </Row>
        <Row label="Guide kept ahead" htmlFor="live-days">
          <Select id="live-days" value={form.guide_days} onChange={(e) => set("guide_days", Number(e.target.value))} className="w-full">
            {[1, 2, 3, 5, 7, 10, 14].map((d) => (
              <option key={d} value={d}>
                {d === 1 ? "1 day" : `${d} days`}
              </option>
            ))}
          </Select>
        </Row>
      </Group>

      <div className={cn("flex justify-end", (dirty || save.isPending) && "sticky bottom-4 z-10")}>
        <div className="flex items-center gap-3 rounded-lg border border-border bg-surface-2/95 px-3 py-2 shadow-xl backdrop-blur">
          {dirty ? <span className="text-xs text-muted">Unsaved changes</span> : null}
          <Button variant="ghost" size="sm" disabled={!dirty || save.isPending} onClick={() => setForm(saved)}>
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
