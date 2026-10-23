import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router";
import { Layers, ListVideo } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useCreateCollection, useCreatePlaylist } from "@/api/lists";
import type { Id } from "@/api/types";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, Field, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";
import { describeSmartFilter } from "@/lib/smartFilter";

type Target = "playlist" | "collection";

const TARGETS: { value: Target; label: string; hint: string; icon: typeof Layers }[] = [
  { value: "playlist", label: "Smart playlist", hint: "Yours: plays what these filters find.", icon: ListVideo },
  { value: "collection", label: "Smart collection", hint: "Everyone's: shown with the library's collections.", icon: Layers },
];

/** Save a library's filters as a smart playlist, or (administrators) a smart collection. */
export function SaveFiltersModal({
  onClose,
  libraryId,
  filter,
  allowCollection,
}: {
  onClose: () => void;
  libraryId: Id;
  /** The library's filters in Plex's syntax (`lib/smartFilter`). */
  filter: string;
  allowCollection: boolean;
}) {
  const navigate = useNavigate();
  const toast = useToast();
  const playlist = useCreatePlaylist();
  const collection = useCreateCollection();
  const [title, setTitle] = useState("");
  const [target, setTarget] = useState<Target>("playlist");
  const busy = playlist.isPending || collection.isPending;
  const onError = (err: unknown) => toast.error(errorMessage(err));

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const name = title.trim();
    if (!name) return;
    if (target === "collection") {
      collection.mutate(
        { libraryId, title: name, smart_filter: filter },
        { onSuccess: (c) => (onClose(), void navigate(`/collection/${c.id}`)), onError },
      );
    } else {
      playlist.mutate(
        { title: name, library_id: libraryId, smart_filter: filter },
        { onSuccess: (p) => (onClose(), void navigate(`/playlists/${p.id}`)), onError },
      );
    }
  };

  return (
    <Modal
      open
      onClose={onClose}
      title="Save these filters"
      size="sm"
      locked={busy}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" form="save-filters" loading={busy} disabled={!title.trim()}>
            Save
          </Button>
        </>
      }
    >
      <form id="save-filters" onSubmit={onSubmit} className="space-y-4">
        <p className="text-sm text-muted">{describeSmartFilter(filter).join(" · ")}. It keeps up with the library by itself.</p>
        <Field label="Title" htmlFor="save-filters-title">
          <TextInput id="save-filters-title" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} required />
        </Field>
        {allowCollection ? (
          <div className="space-y-1.5">
            <span className="block text-sm font-medium text-fg">Save as</span>
            <div className="grid gap-2" role="radiogroup" aria-label="Save as">
              {TARGETS.map(({ value, label, hint, icon: Icon }) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={target === value}
                  onClick={() => setTarget(value)}
                  className={cn(
                    "flex items-start gap-3 rounded-md border px-3 py-2.5 text-left transition-colors",
                    target === value ? "border-accent bg-accent/10" : "border-border hover:border-border-strong",
                  )}
                >
                  <Icon className="mt-0.5 size-4 shrink-0 text-accent" />
                  <span>
                    <span className="block text-sm font-medium text-fg">{label}</span>
                    <span className="block text-xs text-muted">{hint}</span>
                  </span>
                </button>
              ))}
            </div>
          </div>
        ) : null}
      </form>
    </Modal>
  );
}
