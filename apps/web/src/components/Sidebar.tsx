import { useEffect, type ReactNode } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { Film, House, List, ListMusic, ListVideo, LoaderCircle, Music, Plus, Settings, Tv, X } from "lucide-react";
import { useLibraries } from "@/api/hooks";
import { usePlaylists } from "@/api/lists";
import type { LibraryKind } from "@/api/types";
import { useAuth } from "@/auth";
import { cn } from "@/lib/cn";
import { Logo } from "./Logo";
import { IconButton, Skeleton } from "./ui";
import { libraryUrl, playlistUrl } from "@/lib/urls";

export function LibraryIcon({ kind, className }: { kind: LibraryKind; className?: string }) {
  const cls = cn("size-[18px] shrink-0", className);
  if (kind === "movies") return <Film className={cls} />;
  if (kind === "shows") return <Tv className={cls} />;
  return <Music className={cls} />;
}

function NavItem({ to, icon, children, end, trailing }: { to: string; icon: ReactNode; children: ReactNode; end?: boolean; trailing?: ReactNode }) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        cn(
          "group relative flex h-10 items-center gap-3 rounded-md px-3 text-sm font-medium transition-colors duration-150",
          isActive ? "bg-white/[0.07] text-fg" : "text-muted hover:bg-white/[0.04] hover:text-fg",
        )
      }
    >
      {({ isActive }) => (
        <>
          {isActive ? <span className="absolute inset-y-2 left-0 w-[3px] rounded-r bg-accent" /> : null}
          <span className={cn("transition-colors", isActive ? "text-accent" : "text-subtle group-hover:text-muted")}>{icon}</span>
          <span className="min-w-0 flex-1 truncate">{children}</span>
          {trailing}
        </>
      )}
    </NavLink>
  );
}

function SidebarContent({ onClose }: { onClose?: () => void }) {
  const user = useAuth();
  const libs = useLibraries();

  return (
    <nav className="flex h-full flex-col overflow-y-auto px-3 pb-4 pt-4" aria-label="Main">
      <div className="mb-6 flex items-center justify-between px-2">
        <Link to="/" aria-label="pley home">
          <Logo />
        </Link>
        {onClose ? (
          <IconButton label="Close menu" size="sm" onClick={onClose}>
            <X className="size-4" />
          </IconButton>
        ) : null}
      </div>

      <div className="space-y-0.5">
        <NavItem to="/" end icon={<House className="size-[18px]" />}>
          Home
        </NavItem>
      </div>

      <div className="mb-1.5 mt-6 px-3 text-xs font-medium text-subtle">Libraries</div>
      <div className="space-y-0.5">
        {libs.isPending
          ? Array.from({ length: 3 }, (_, i) => <Skeleton key={i} className="mx-3 my-2.5 h-5" />)
          : libs.data?.map((lib) => (
              <NavItem
                key={lib.id}
                to={libraryUrl(lib.id)}
                icon={<LibraryIcon kind={lib.kind} />}
                trailing={
                  lib.scanning ? (
                    <LoaderCircle className="size-3.5 animate-spin text-accent" aria-label="Scanning" />
                  ) : null
                }
              >
                {lib.name}
              </NavItem>
            ))}
        {libs.data && libs.data.length === 0 ? (
          user.is_admin ? (
            <Link
              to="/settings/libraries?add=1"
              className="flex h-10 items-center gap-3 rounded-md px-3 text-sm text-muted transition-colors hover:bg-white/[0.04] hover:text-fg"
            >
              <Plus className="size-[18px] text-subtle" />
              Add library
            </Link>
          ) : (
            <p className="px-3 py-2 text-xs text-subtle">No libraries yet.</p>
          )
        ) : null}
      </div>

      <PlaylistsSection />

      <div className="flex-1" />

      {user.is_admin ? (
        <div className="mt-6 space-y-0.5 border-t border-border pt-3">
          <NavItem to="/settings" icon={<Settings className="size-[18px]" />}>
            Settings
          </NavItem>
        </div>
      ) : null}
    </nav>
  );
}

/** The user's playlists (the first few) and a link to all of them. */
function PlaylistsSection() {
  const playlists = usePlaylists();
  const shown = playlists.data?.slice(0, 8) ?? [];
  return (
    <>
      <div className="mb-1.5 mt-6 px-3 text-xs font-medium text-subtle">Playlists</div>
      <div className="space-y-0.5">
        {shown.map((p) => (
          <NavItem
            key={p.id}
            to={playlistUrl(p.id)}
            icon={p.kind === "audio" ? <ListMusic className="size-[18px]" /> : <ListVideo className="size-[18px]" />}
          >
            {p.title}
          </NavItem>
        ))}
        <NavItem to="/playlists" end icon={<List className="size-[18px]" />}>
          {shown.length ? "All playlists" : "Your playlists"}
        </NavItem>
      </div>
    </>
  );
}

export function Sidebar({ open, onClose }: { open: boolean; onClose: () => void }) {
  const location = useLocation();

  // Close the drawer after navigating.
  useEffect(() => {
    onClose();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return (
    <>
      <aside className="fixed inset-y-0 left-0 z-30 hidden w-60 border-r border-border bg-surface lg:block">
        <SidebarContent />
      </aside>
      {open ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          <div className="absolute inset-0 animate-fade-in bg-black/60" onClick={onClose} />
          <aside className="absolute inset-y-0 left-0 w-72 max-w-[85vw] animate-pop-in border-r border-border bg-surface shadow-2xl">
            <SidebarContent onClose={onClose} />
          </aside>
        </div>
      ) : null}
    </>
  );
}
