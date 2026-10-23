import { useState, type FormEvent } from "react";
import { errorMessage } from "@/api/client";
import { useCreateCollection, useUpdateCollection, type Collection, type CollectionDetail, type CollectionSort } from "@/api/lists";
import type { Id } from "@/api/types";
import { Modal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Button, Field, Select, TextInput } from "@/components/ui";

const SORT_LABELS: Record<CollectionSort, string> = {
  release: "Release date",
  title: "Title",
  custom: "Custom",
};

/** Create a collection in a library, or edit one (administrators). */
export function CollectionFormModal({
  open,
  onClose,
  libraryId,
  collection,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  libraryId: Id;
  /** The collection to edit; a new one is created without it. */
  collection?: Collection;
  onCreated?: (created: CollectionDetail) => void;
}) {
  const create = useCreateCollection();
  const update = useUpdateCollection();
  const toast = useToast();
  const [title, setTitle] = useState(collection?.title ?? "");
  const [summary, setSummary] = useState(collection?.summary ?? "");
  const [sort, setSort] = useState<CollectionSort>(collection?.sort ?? "release");
  const busy = create.isPending || update.isPending;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    const name = title.trim();
    const text = summary.trim() || null;
    if (!name) return;
    if (collection) {
      // Only what changed: an edited title or summary is kept over TMDB's.
      update.mutate(
        {
          id: collection.id,
          title: name !== collection.title ? name : undefined,
          summary: text !== collection.summary ? text : undefined,
          sort: sort !== collection.sort ? sort : undefined,
        },
        {
          onSuccess: () => {
            toast.success("Collection saved");
            onClose();
          },
          onError: (err) => toast.error(errorMessage(err)),
        },
      );
    } else {
      create.mutate(
        { libraryId, title: name, summary: text },
        {
          onSuccess: (created) => {
            onClose();
            onCreated?.(created);
          },
          onError: (err) => toast.error(errorMessage(err)),
        },
      );
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={collection ? "Edit collection" : "New collection"}
      locked={busy}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" form="collection-form" loading={busy} disabled={!title.trim()}>
            {collection ? "Save" : "Create"}
          </Button>
        </>
      }
    >
      <form id="collection-form" onSubmit={onSubmit} className="space-y-4">
        <Field label="Title" htmlFor="collection-title">
          <TextInput id="collection-title" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={200} required />
        </Field>
        <Field label="Summary" htmlFor="collection-summary">
          <textarea
            id="collection-summary"
            rows={4}
            value={summary}
            onChange={(e) => setSummary(e.target.value)}
            className="w-full resize-y rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-fg placeholder:text-subtle transition-colors focus:border-accent focus:outline-none"
          />
        </Field>
        {collection && !collection.smart_filter ? (
          <Field
            label="Order"
            htmlFor="collection-sort"
            hint={collection.automatic ? "A title or summary changed here is kept when TMDB metadata is refreshed." : undefined}
          >
            <Select id="collection-sort" value={sort} onChange={(e) => setSort(e.target.value as CollectionSort)}>
              {(Object.keys(SORT_LABELS) as CollectionSort[]).map((s) => (
                <option key={s} value={s}>
                  {SORT_LABELS[s]}
                </option>
              ))}
            </Select>
          </Field>
        ) : null}
      </form>
    </Modal>
  );
}
