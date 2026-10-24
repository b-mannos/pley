import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ArrowUpDown, ChevronDown, ChevronUp, GripVertical, Layers, Pencil, Play, Shuffle, Trash2, X } from "lucide-react";
import { errorMessage, isApiError } from "@/api/client";
import { useLibraries } from "@/api/hooks";
import { useCollection, useDeleteCollection, useRemoveFromCollection, useReorderCollection, type CollectionDetail } from "@/api/lists";
import { useAuth } from "@/auth";
import { Artwork } from "@/components/Artwork";
import { CardSkeleton, MediaCard } from "@/components/cards";
import { collectionNoun } from "@/components/lists/CollectionCard";
import { moved, useDragReorder } from "@/components/lists/reorder";
import { ConfirmModal } from "@/components/Modal";
import { useToast } from "@/components/Toast";
import { Badge, Button, EmptyState, IconButton, SectionTitle, Skeleton } from "@/components/ui";
import { cn } from "@/lib/cn";
import { pluralize } from "@/lib/format";
import { describeSmartFilter } from "@/lib/smartFilter";
import { useDocumentTitle, useElementWidth } from "@/lib/hooks";
import { posterCandidates } from "@/lib/images";
import { usePlayItem } from "@/lib/usePlayItem";
import { useVideoQueue } from "@/lib/videoQueue";
import { NotFoundPage } from "../NotFoundPage";
import { MetaLine, Summary } from "../item/shared";
import { CollectionFormModal } from "./CollectionFormModal";
import { collectionUrl, itemUrl, libraryUrl } from "@/lib/urls";
import { idParam } from "@/lib/ids";

const SORT_NOTE = { release: "By release date", title: "By title", custom: "Custom order" } as const;

export function CollectionPage() {
  const { id } = useParams();
  const collectionId = idParam(id);
  const collection = useCollection(collectionId);
  useDocumentTitle(collection.data?.title);

  if (!Number.isFinite(collectionId) || collectionId <= 0) return <NotFoundPage />;
  if (collection.isPending) return <CollectionSkeleton />;
  if (collection.isError) {
    if (isApiError(collection.error, 404)) return <NotFoundPage />;
    return (
      <EmptyState
        className="min-h-[60vh]"
        title="Couldn't load this collection"
        description={errorMessage(collection.error)}
        action={<Button onClick={() => void collection.refetch()}>Retry</Button>}
      />
    );
  }
  return <CollectionView key={collection.data.id} collection={collection.data} />;
}

function CollectionView({ collection: c }: { collection: CollectionDetail }) {
  const user = useAuth();
  const navigate = useNavigate();
  const toast = useToast();
  const libs = useLibraries();
  const lib = libs.data?.find((l) => l.id === c.library_id);
  const noun = collectionNoun(lib?.kind);
  const startQueue = useVideoQueue();
  const play = usePlayItem();
  const remove = useDeleteCollection();
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [managing, setManaging] = useState(false);
  const returnTo = collectionUrl(c.id);
  const movies = c.items.some((i) => i.kind === "movie");

  const onPlay = (shuffle: boolean) => {
    if (movies) return startQueue(c.items, { shuffle, returnTo });
    const pick = shuffle ? c.items[Math.floor(Math.random() * c.items.length)] : c.items[0];
    if (pick) void play(pick);
  };

  return (
    <div className="relative min-h-[calc(100vh-4rem)] animate-fade-in">
      <div className="pointer-events-none absolute inset-x-0 top-0 h-[78vh] max-h-[760px] overflow-hidden" aria-hidden>
        <Artwork
          candidates={[c.backdrop_url]}
          width={1920}
          alt=""
          title={c.title}
          bare
          eager
          className="size-full opacity-70"
          imgClassName="object-cover object-[center_20%]"
        />
        <div className="absolute inset-0 bg-linear-to-t from-bg via-bg/75 to-bg/10" />
        <div className="absolute inset-0 bg-linear-to-r from-bg/95 via-bg/50 to-transparent" />
      </div>

      <div className="relative px-4 pb-16 pt-6 sm:px-8 sm:pt-[10vh] lg:pt-[14vh]">
        <div className="flex flex-col gap-6 sm:flex-row sm:gap-10">
          <div className="hidden w-52 shrink-0 sm:block md:w-60 lg:w-64">
            <Artwork
              candidates={[c.poster_url]}
              width={256}
              alt={c.title}
              title={c.title}
              eager
              className="aspect-[2/3] w-full rounded-lg shadow-2xl shadow-black/60 ring-1 ring-white/10"
            />
          </div>
          <div className="min-w-0 flex-1 space-y-4">
            <div className="flex items-center gap-2 text-sm font-medium text-muted">
              <Layers className="size-3.5" /> Collection
              {c.automatic ? <Badge>TMDB</Badge> : null}
              {c.smart_filter ? <Badge tone="accent">Smart</Badge> : null}
            </div>
            <h1 className="text-3xl font-bold leading-tight tracking-tight text-balance sm:text-4xl">{c.title}</h1>
            <MetaLine
              parts={[
                pluralize(c.item_count, noun[0], noun[1]),
                lib ? (
                  <Link key="lib" to={libraryUrl(lib.id, "tab=collections")} className="transition-colors hover:text-fg">
                    {lib.name}
                  </Link>
                ) : null,
                // A smart collection shows what its filter finds, in its order.
                ...(c.smart_filter ? describeSmartFilter(c.smart_filter) : [SORT_NOTE[c.sort]]),
              ]}
            />
            <div className="flex flex-wrap items-center gap-2.5 pt-1">
              <Button
                variant="primary"
                size="lg"
                icon={<Play className="size-5 fill-current" />}
                onClick={() => onPlay(false)}
                disabled={!c.items.length}
              >
                Play
              </Button>
              <Button size="lg" icon={<Shuffle className="size-4" />} onClick={() => onPlay(true)} disabled={c.items.length < 2}>
                Shuffle
              </Button>
              {user.is_admin ? (
                <>
                  <IconButton label="Edit collection" size="lg" className="bg-surface-3/80 hover:bg-border" onClick={() => setEditing(true)}>
                    <Pencil className="size-5" />
                  </IconButton>
                  {c.smart_filter ? null : (
                    <IconButton
                      label={managing ? "Done reordering" : "Reorder and remove"}
                      size="lg"
                      active={managing}
                      className="bg-surface-3/80 hover:bg-border"
                      onClick={() => setManaging((m) => !m)}
                      disabled={!c.items.length}
                    >
                      <ArrowUpDown className="size-5" />
                    </IconButton>
                  )}
                  <IconButton label="Delete collection" size="lg" className="bg-surface-3/80 hover:bg-border hover:text-danger!" onClick={() => setDeleting(true)}>
                    <Trash2 className="size-5" />
                  </IconButton>
                </>
              ) : null}
            </div>
            <Summary text={c.summary} />
          </div>
        </div>

        <section className="mt-12">
          <SectionTitle
            action={
              managing ? (
                <Button size="sm" variant="primary" onClick={() => setManaging(false)}>
                  Done
                </Button>
              ) : undefined
            }
          >
            {noun[1].charAt(0).toUpperCase() + noun[1].slice(1)}
          </SectionTitle>
          {c.items.length === 0 ? (
            <p className="text-sm text-muted">
              Nothing in this collection yet. Add {noun[1]} from their page, or by right-clicking a poster.
            </p>
          ) : managing ? (
            <ManageItems collection={c} />
          ) : (
            <ItemsGrid collection={c} />
          )}
        </section>
      </div>

      {user.is_admin ? (
        <>
          <CollectionFormModal
            key={editing ? `edit${c.updated_at}` : "closed"}
            open={editing}
            onClose={() => setEditing(false)}
            libraryId={c.library_id}
            collection={c}
          />
          <ConfirmModal
            open={deleting}
            onClose={() => setDeleting(false)}
            title="Delete collection?"
            message={
              c.automatic
                ? `"${c.title}" is made from TMDB: it comes back the next time these movies' metadata is refreshed.`
                : `"${c.title}" will be deleted. The ${noun[1]} in it stay in the library.`
            }
            confirmLabel="Delete"
            danger
            loading={remove.isPending}
            onConfirm={() =>
              remove.mutate(c, {
                onSuccess: () => {
                  toast.success("Collection deleted");
                  void navigate(libraryUrl(c.library_id, "tab=collections"), { replace: true });
                },
                onError: (err) => toast.error(errorMessage(err)),
              })
            }
          />
        </>
      ) : null}
    </div>
  );
}

function ItemsGrid({ collection }: { collection: CollectionDetail }) {
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const gap = width > 0 && width < 640 ? 12 : 18;
  const cols = width > 0 ? Math.max(2, Math.floor((width + gap) / (160 + gap))) : 6;
  const colWidth = width > 0 ? (width - gap * (cols - 1)) / cols : 160;
  return (
    <div
      ref={ref}
      className="grid"
      style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, columnGap: gap, rowGap: gap < 18 ? 18 : 26 }}
    >
      {collection.items.map((item) => (
        <MediaCard key={item.id} item={item} aspect="poster" width={colWidth} />
      ))}
    </div>
  );
}

/** Administrators: drag (or move up / down) to reorder, remove items. */
function ManageItems({ collection: c }: { collection: CollectionDetail }) {
  const reorder = useReorderCollection();
  const remove = useRemoveFromCollection();
  const toast = useToast();
  const move = (from: number, to: number) => {
    if (to < 0 || to >= c.items.length || from === to) return;
    reorder.mutate(
      { collection: c, itemIds: moved(c.items.map((i) => i.id), from, to) },
      { onError: (err) => toast.error(errorMessage(err)) },
    );
  };
  const { dragging, rowProps, dropMark } = useDragReorder(c.items.map((i) => i.id), move);

  return (
    <div className="max-w-3xl">
      {c.sort !== "custom" ? <p className="mb-2 text-xs text-muted">Moving an item switches this collection to a custom order.</p> : null}
      <ol className="-mx-2 space-y-0.5">
        {c.items.map((item, i) => {
          const mark = dropMark(i);
          return (
            <li
              key={item.id}
              {...rowProps(i)}
              className={cn(
                "group flex items-center gap-3 rounded-md border-y-2 border-transparent px-2 py-1.5 transition-colors hover:bg-white/[0.04]",
                dragging === i && "opacity-40",
                mark === "before" && "border-t-accent",
                mark === "after" && "border-b-accent",
              )}
            >
              <GripVertical className="size-4 shrink-0 cursor-grab text-subtle" aria-hidden />
              <span className="w-6 shrink-0 text-right text-sm tabular-nums text-subtle">{i + 1}</span>
              <Artwork candidates={posterCandidates(item)} width={40} alt="" title={item.title} className="aspect-[2/3] w-10 shrink-0 rounded" />
              <div className="min-w-0 flex-1">
                <Link to={itemUrl(item.id)} draggable={false} className="block truncate text-sm font-medium text-fg hover:text-accent">
                  {item.title}
                </Link>
                <div className="text-xs text-muted">{item.year ?? ""}</div>
              </div>
              <IconButton label="Move up" size="sm" disabled={i === 0} onClick={() => move(i, i - 1)}>
                <ChevronUp className="size-4" />
              </IconButton>
              <IconButton label="Move down" size="sm" disabled={i === c.items.length - 1} onClick={() => move(i, i + 1)}>
                <ChevronDown className="size-4" />
              </IconButton>
              <IconButton
                label="Remove from collection"
                size="sm"
                className="hover:text-danger!"
                disabled={remove.isPending}
                onClick={() =>
                  remove.mutate(
                    { collection: c, itemId: item.id },
                    {
                      onSuccess: () => toast.success(`Removed ${item.title}`),
                      onError: (err) => toast.error(errorMessage(err)),
                    },
                  )
                }
              >
                <X className="size-4" />
              </IconButton>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function CollectionSkeleton() {
  return (
    <div className="px-4 pb-16 pt-6 sm:px-8 sm:pt-[10vh] lg:pt-[14vh]">
      <div className="flex flex-col gap-8 sm:flex-row sm:gap-10">
        <Skeleton className="hidden aspect-[2/3] w-60 shrink-0 rounded-lg sm:block" />
        <div className="flex-1 space-y-4">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-10 w-2/3" />
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="h-12 w-48" />
        </div>
      </div>
      <div className="mt-12 grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-x-[18px] gap-y-[26px]">
        {Array.from({ length: 6 }, (_, i) => (
          <CardSkeleton key={i} aspect="poster" />
        ))}
      </div>
    </div>
  );
}
