import { useEffect, useState } from "react";
import { FolderOpen, Plus, Trash2 } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useCreateLibrary, useUpdateLibrary } from "@/api/hooks";
import type { Library, LibraryKind } from "@/api/types";
import { FolderPicker } from "@/components/FolderPicker";
import { Modal } from "@/components/Modal";
import { LibraryIcon } from "@/components/Sidebar";
import { useToast } from "@/components/Toast";
import { Button, Field, IconButton, TextInput } from "@/components/ui";
import { cn } from "@/lib/cn";

const KINDS: { kind: LibraryKind; label: string; hint: string; defaultName: string }[] = [
  { kind: "movies", label: "Movies", hint: "Movie Name (Year)/Movie Name (Year).mkv", defaultName: "Movies" },
  { kind: "shows", label: "TV Shows", hint: "Show Name/Season 01/Show Name - S01E01.mkv", defaultName: "TV Shows" },
  { kind: "music", label: "Music", hint: "Artist/Album/01 - Track.flac", defaultName: "Music" },
];

export function LibraryFormModal({ open, onClose, library }: { open: boolean; onClose: () => void; library?: Library | null }) {
  const editing = !!library;
  const toast = useToast();
  const create = useCreateLibrary();
  const update = useUpdateLibrary();
  const [name, setName] = useState("");
  const [kind, setKind] = useState<LibraryKind>("movies");
  const [paths, setPaths] = useState<string[]>([]);
  const [manual, setManual] = useState("");
  const [picker, setPicker] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setName(library?.name ?? "");
    setKind(library?.kind ?? "movies");
    setPaths(library?.paths ?? []);
    setManual("");
    setError(null);
  }, [open, library]);

  const kindInfo = KINDS.find((k) => k.kind === kind)!;
  const busy = create.isPending || update.isPending;

  const addPath = (p: string) => {
    const trimmed = p.trim();
    if (!trimmed) return;
    setPaths((list) => (list.includes(trimmed) ? list : [...list, trimmed]));
  };

  const submit = () => {
    setError(null);
    const finalPaths = manual.trim() ? Array.from(new Set([...paths, manual.trim()])) : paths;
    if (finalPaths.length === 0) {
      setError("Add at least one folder.");
      return;
    }
    const finalName = name.trim() || kindInfo.defaultName;
    if (editing && library) {
      update.mutate(
        { id: library.id, name: finalName, paths: finalPaths },
        {
          onSuccess: () => {
            toast.success("Library updated");
            onClose();
          },
          onError: (e) => setError(errorMessage(e)),
        },
      );
    } else {
      create.mutate(
        { name: finalName, kind, paths: finalPaths },
        {
          onSuccess: (lib) => {
            toast.success(`“${lib.name}” added — scanning now`);
            onClose();
          },
          onError: (e) => setError(errorMessage(e)),
        },
      );
    }
  };

  return (
    <>
      <Modal
        open={open && !picker}
        onClose={onClose}
        locked={busy}
        size="lg"
        title={editing ? `Edit ${library?.name}` : "Add library"}
        footer={
          <>
            <Button variant="ghost" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submit} loading={busy}>
              {editing ? "Save changes" : "Add library"}
            </Button>
          </>
        }
      >
        <div className="space-y-5">
          {!editing ? (
            <div>
              <div className="mb-2 text-sm font-medium">Type</div>
              <div className="grid grid-cols-3 gap-2">
                {KINDS.map((k) => (
                  <button
                    key={k.kind}
                    type="button"
                    onClick={() => setKind(k.kind)}
                    aria-pressed={kind === k.kind}
                    className={cn(
                      "flex flex-col items-center gap-2 rounded-lg border px-3 py-4 text-sm font-medium transition-colors",
                      kind === k.kind
                        ? "border-accent bg-accent/10 text-fg"
                        : "border-border bg-surface-2 text-muted hover:border-border-strong hover:text-fg",
                    )}
                  >
                    <LibraryIcon kind={k.kind} className={cn("size-6", kind === k.kind && "text-accent")} />
                    {k.label}
                  </button>
                ))}
              </div>
            </div>
          ) : null}

          <Field label="Name" htmlFor="lib-name">
            <TextInput id="lib-name" value={name} placeholder={kindInfo.defaultName} onChange={(e) => setName(e.target.value)} />
          </Field>

          <div>
            <div className="mb-2 flex items-center justify-between">
              <span className="text-sm font-medium">Folders</span>
              <Button size="sm" icon={<FolderOpen className="size-4" />} onClick={() => setPicker(true)}>
                Browse…
              </Button>
            </div>
            {paths.length ? (
              <ul className="mb-2 divide-y divide-white/[0.05] rounded-md border border-border">
                {paths.map((p) => (
                  <li key={p} className="flex items-center gap-2 px-3 py-1.5">
                    <span className="min-w-0 flex-1 truncate font-mono text-xs" title={p}>
                      {p}
                    </span>
                    <IconButton label={`Remove ${p}`} size="sm" onClick={() => setPaths((list) => list.filter((x) => x !== p))}>
                      <Trash2 className="size-4" />
                    </IconButton>
                  </li>
                ))}
              </ul>
            ) : null}
            <div className="flex gap-2">
              <TextInput
                value={manual}
                onChange={(e) => setManual(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addPath(manual);
                    setManual("");
                  }
                }}
                placeholder="…or type a folder path"
                className="font-mono text-xs"
                aria-label="Folder path"
              />
              <Button
                icon={<Plus className="size-4" />}
                onClick={() => {
                  addPath(manual);
                  setManual("");
                }}
                disabled={!manual.trim()}
              >
                Add
              </Button>
            </div>
            <p className="mt-2 text-xs text-subtle">
              Recommended layout: <span className="font-mono">{kindInfo.hint}</span>
            </p>
          </div>

          {error ? <p className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">{error}</p> : null}
        </div>
      </Modal>
      <FolderPicker
        open={open && picker}
        onClose={() => setPicker(false)}
        initialPath={paths[paths.length - 1] ?? null}
        onSelect={(p) => {
          addPath(p);
          setPicker(false);
        }}
      />
    </>
  );
}
