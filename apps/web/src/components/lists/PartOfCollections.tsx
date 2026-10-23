import { Link } from "react-router";
import { Layers } from "lucide-react";
import { collectable, useItemCollections } from "@/api/lists";
import type { ItemSummary } from "@/api/types";

/** "Part of …" links to the collections a movie or show belongs to. */
export function PartOfCollections({ item }: { item: ItemSummary }) {
  const collections = useItemCollections(item.id, collectable(item));
  const list = collections.data ?? [];
  if (!list.length) return null;
  return (
    <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-sm text-muted">
      <Layers className="size-4 text-subtle" aria-hidden />
      <span>Part of</span>
      {list.map((c, i) => (
        <span key={c.id}>
          <Link to={`/collection/${c.id}`} className="font-medium text-fg transition-colors hover:text-accent">
            {c.title}
          </Link>
          {i < list.length - 1 ? "," : null}
        </span>
      ))}
    </div>
  );
}
