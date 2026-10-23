import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { EllipsisVertical, FolderPlus, Pencil, Plus, RefreshCw, ScanSearch, Trash2 } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useActivity, useDeleteLibrary, useLibraries, useScanAll, useScanLibrary } from "@/api/hooks";
import type { Library, ScanStatus } from "@/api/types";
import { Menu, MenuItem, MenuSeparator } from "@/components/Menu";
import { ConfirmModal } from "@/components/Modal";
import { LibraryIcon } from "@/components/Sidebar";
import { useToast } from "@/components/Toast";
import { Button, EmptyState, IconButton, PageSpinner, ProgressBar } from "@/components/ui";
import { formatRelative, pluralize } from "@/lib/format";
import { useDocumentTitle } from "@/lib/hooks";
import { LibraryFormModal } from "./LibraryFormModal";
import { libraryUrl } from "@/lib/urls";

const KIND_NOUN: Record<Library["kind"], [string, string]> = {
  movies: ["movie", "movies"],
  shows: ["show", "shows"],
  music: ["artist", "artists"],
};

const PHASE_LABEL: Record<ScanStatus["phase"], string> = {
  queued: "Queued",
  walking: "Finding files",
  probing: "Analyzing media",
  metadata: "Fetching metadata",
  done: "Finished",
};

function LibraryRow({ lib, scan, onEdit, onDelete }: { lib: Library; scan?: ScanStatus; onEdit: () => void; onDelete: () => void }) {
  const toast = useToast();
  const scanMut = useScanLibrary();
  const running = lib.scanning || (scan && scan.phase !== "done");
  const noun = KIND_NOUN[lib.kind];

  const startScan = (force: boolean) =>
    scanMut.mutate(
      { id: lib.id, force },
      {
        onSuccess: () => toast.success(force ? `Full rescan of “${lib.name}” started` : `Scanning “${lib.name}”`),
        onError: (e) => toast.error(errorMessage(e)),
      },
    );

  return (
    <div className="rounded-lg border border-border bg-surface p-4">
      <div className="flex items-start gap-4">
        <div className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-surface-3 text-accent">
          <LibraryIcon kind={lib.kind} className="size-5" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <Link to={libraryUrl(lib.id)} className="truncate text-base font-semibold hover:text-accent">
              {lib.name}
            </Link>
            <span className="text-xs text-muted">
              {pluralize(lib.item_count, noun[0], noun[1])} · {lib.scanned_at ? `scanned ${formatRelative(lib.scanned_at)}` : "never scanned"}
            </span>
          </div>
          <ul className="mt-1.5 space-y-0.5">
            {lib.paths.map((p) => (
              <li key={p} className="truncate font-mono text-xs text-subtle" title={p}>
                {p}
              </li>
            ))}
          </ul>
          {running ? (
            <div className="mt-3 max-w-md">
              <div className="mb-1 flex items-center justify-between text-xs">
                <span className="flex items-center gap-1.5 text-accent">
                  <RefreshCw className="size-3 animate-spin" />
                  {scan ? PHASE_LABEL[scan.phase] : "Scanning"}
                </span>
                {scan && scan.total > 0 ? (
                  <span className="tabular-nums text-muted">
                    {scan.processed.toLocaleString()} / {scan.total.toLocaleString()}
                  </span>
                ) : null}
              </div>
              <ProgressBar
                value={scan && scan.total > 0 ? scan.processed / scan.total : 0}
                className="h-1.5"
                barClassName={!scan || scan.total === 0 ? "w-1/3 animate-pulse" : undefined}
              />
            </div>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button size="sm" icon={<ScanSearch className="size-4" />} onClick={() => startScan(false)} loading={scanMut.isPending} disabled={!!running}>
            <span className="hidden sm:inline">Scan</span>
          </Button>
          <Menu
            trigger={({ toggle }) => (
              <IconButton label="More actions" size="sm" onClick={toggle}>
                <EllipsisVertical className="size-4" />
              </IconButton>
            )}
          >
            {(close) => (
              <>
                <MenuItem
                  icon={<RefreshCw />}
                  onClick={() => {
                    close();
                    startScan(true);
                  }}
                >
                  Force full rescan
                </MenuItem>
                <MenuItem
                  icon={<Pencil />}
                  onClick={() => {
                    close();
                    onEdit();
                  }}
                >
                  Edit
                </MenuItem>
                <MenuSeparator />
                <MenuItem
                  icon={<Trash2 />}
                  danger
                  onClick={() => {
                    close();
                    onDelete();
                  }}
                >
                  Delete
                </MenuItem>
              </>
            )}
          </Menu>
        </div>
      </div>
    </div>
  );
}

export function LibrariesSettings() {
  useDocumentTitle("Libraries");
  const libs = useLibraries();
  const activity = useActivity();
  const toast = useToast();
  const scanAll = useScanAll();
  const del = useDeleteLibrary();
  const [params, setParams] = useSearchParams();
  const adding = params.get("add") === "1";
  const [editing, setEditing] = useState<Library | null>(null);
  const [deleting, setDeleting] = useState<Library | null>(null);

  const openAdd = () => {
    const next = new URLSearchParams(params);
    next.set("add", "1");
    setParams(next, { replace: true });
  };
  const closeAdd = () => {
    const next = new URLSearchParams(params);
    next.delete("add");
    setParams(next, { replace: true });
  };

  if (libs.isPending) return <PageSpinner />;

  const scans = activity.data?.scans ?? [];

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Libraries</h2>
          <p className="text-sm text-muted">Folders pley watches for movies, shows and music.</p>
        </div>
        <div className="flex gap-2">
          {libs.data && libs.data.length > 0 ? (
            <Button
              icon={<RefreshCw className="size-4" />}
              loading={scanAll.isPending}
              onClick={() =>
                scanAll.mutate(undefined, {
                  onSuccess: () => toast.success("Scanning all libraries"),
                  onError: (e) => toast.error(errorMessage(e)),
                })
              }
            >
              Scan all
            </Button>
          ) : null}
          <Button variant="primary" icon={<Plus className="size-4" />} onClick={openAdd}>
            Add library
          </Button>
        </div>
      </div>

      {libs.isError ? (
        <EmptyState title="Couldn't load libraries" description={errorMessage(libs.error)} />
      ) : libs.data.length === 0 ? (
        <div className="rounded-lg border border-dashed border-border-strong">
          <EmptyState
            icon={<FolderPlus />}
            title="Add your first library"
            description="Choose a type and the folders that contain your media. pley scans them and keeps them up to date."
            action={
              <Button variant="primary" onClick={openAdd}>
                Add library
              </Button>
            }
          />
        </div>
      ) : (
        <div className="space-y-3">
          {libs.data.map((lib) => (
            <LibraryRow
              key={lib.id}
              lib={lib}
              scan={scans.find((s) => s.library_id === lib.id)}
              onEdit={() => setEditing(lib)}
              onDelete={() => setDeleting(lib)}
            />
          ))}
        </div>
      )}

      <LibraryFormModal open={adding} onClose={closeAdd} />
      <LibraryFormModal open={!!editing} onClose={() => setEditing(null)} library={editing} />
      <ConfirmModal
        open={!!deleting}
        onClose={() => setDeleting(null)}
        title="Delete library?"
        danger
        confirmLabel="Delete"
        loading={del.isPending}
        message={
          <>
            <strong className="text-fg">{deleting?.name}</strong> and its watch history will be removed from pley. Your media files
            are not touched.
          </>
        }
        onConfirm={() => {
          if (!deleting) return;
          del.mutate(deleting.id, {
            onSuccess: () => {
              toast.success(`Deleted “${deleting.name}”`);
              setDeleting(null);
            },
            onError: (e) => toast.error(errorMessage(e)),
          });
        }}
      />
    </div>
  );
}
