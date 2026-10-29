import { Link } from "react-router";
import { Clapperboard, FolderPlus } from "lucide-react";
import { errorMessage } from "@/api/client";
import { useHomeHubs, useLibraries } from "@/api/hooks";
import { useAuth } from "@/auth";
import { HubRow, HubRowSkeleton } from "@/components/HubRow";
import { Button, buttonClass, EmptyState } from "@/components/ui";
import { useDocumentTitle } from "@/lib/hooks";

export function HomePage() {
  useDocumentTitle("Home");
  const user = useAuth();
  const libs = useLibraries();
  const hubs = useHomeHubs();

  if (libs.data && libs.data.length === 0) {
    return (
      <EmptyState
        className="min-h-[70vh]"
        icon={<FolderPlus />}
        title="No libraries yet"
        description={
          user.is_admin
            ? "Point Pley at the folders where your movies, shows and music live. It will scan them and build your library."
            : "Ask the server owner to add a library."
        }
        action={
          user.is_admin ? (
            <Link to="/settings/libraries?add=1" className={buttonClass("primary", "lg")}>
              Add a library
            </Link>
          ) : undefined
        }
      />
    );
  }

  if (hubs.isPending) {
    return (
      <div className="space-y-9 py-7 sm:space-y-10 sm:py-8">
        <HubRowSkeleton aspect="thumb" />
        <HubRowSkeleton />
        <HubRowSkeleton />
      </div>
    );
  }

  if (hubs.isError) {
    return (
      <EmptyState
        className="min-h-[60vh]"
        title="Couldn't load your home screen"
        description={errorMessage(hubs.error)}
        action={<Button onClick={() => void hubs.refetch()}>Retry</Button>}
      />
    );
  }

  const visible = hubs.data.filter((h) => h.items.length > 0);
  if (visible.length === 0) {
    const scanning = libs.data?.some((l) => l.scanning);
    return (
      <EmptyState
        className="min-h-[70vh]"
        icon={<Clapperboard />}
        title={scanning ? "Scanning your libraries…" : "Nothing here yet"}
        description={
          scanning
            ? "New items will show up here as soon as they're found."
            : "Your libraries are empty. Add media to the library folders and run a scan."
        }
        action={
          user.is_admin && !scanning ? (
            <Link to="/settings/libraries" className={buttonClass()}>
              Manage libraries
            </Link>
          ) : undefined
        }
      />
    );
  }

  return (
    <div className="space-y-9 py-7 sm:space-y-10 sm:py-8">
      {visible.map((hub) => (
        <HubRow key={hub.key} hub={hub} />
      ))}
    </div>
  );
}
