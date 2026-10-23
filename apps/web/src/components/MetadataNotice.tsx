import type { ReactNode } from "react";
import { Link } from "react-router";
import { Info, X } from "lucide-react";
import { useSettings } from "@/api/hooks";
import { useAuth } from "@/auth";
import { cn } from "@/lib/cn";
import { useLocalStorage } from "@/lib/hooks";
import { IconButton } from "./ui";

/** Where the TMDB key is entered. */
export const TMDB_KEY_SETTING = "/settings/server#tmdb-key";

/**
 * For administrators: whether movie details are missing because no TMDB key
 * is set (`undefined` while unknown, and always for everyone else).
 */
export function useNeedsTmdbKey(): boolean | undefined {
  const user = useAuth();
  const settings = useSettings(user.is_admin);
  if (!user.is_admin || !settings.data) return undefined;
  return settings.data.tmdb_api_key.trim() === "";
}

function Notice({ children, onDismiss, className }: { children: ReactNode; onDismiss?: () => void; className?: string }) {
  return (
    <div
      role="note"
      className={cn("flex items-start gap-3 rounded-lg border border-accent/25 bg-accent/[0.07] px-4 py-3 text-sm text-fg/90", className)}
    >
      <Info className="mt-0.5 size-4 shrink-0 text-accent" aria-hidden />
      <div className="min-w-0 flex-1 leading-relaxed">{children}</div>
      {onDismiss ? (
        <IconButton label="Dismiss" size="sm" className="-my-1 -mr-2 size-7" onClick={onDismiss}>
          <X className="size-4" />
        </IconButton>
      ) : null}
    </div>
  );
}

/** Movie libraries without a TMDB key: why there are no posters, and where to fix it. */
export function TmdbKeyNotice({ className }: { className?: string }) {
  const needsKey = useNeedsTmdbKey();
  const [dismissed, setDismissed] = useLocalStorage("pley.notice.tmdbKey", false);
  if (!needsKey || dismissed) return null;
  return (
    <Notice className={className} onDismiss={() => setDismissed(true)}>
      Movie posters, summaries and cast come from TMDB, which needs an API key.{" "}
      <Link to={TMDB_KEY_SETTING} className="font-medium text-accent hover:underline">
        Add a free key in Settings
      </Link>{" "}
      to fill them in.
    </Notice>
  );
}

/** An administrator's view of a movie pley couldn't identify. */
export function UnmatchedNotice({ onFixMatch, className }: { onFixMatch: () => void; className?: string }) {
  const needsKey = useNeedsTmdbKey();
  if (needsKey === undefined) return null;
  return (
    <Notice className={className}>
      {needsKey ? (
        <>
          No details yet: movie details come from TMDB, which needs an API key.{" "}
          <Link to={TMDB_KEY_SETTING} className="font-medium text-accent hover:underline">
            Add a free key in Settings
          </Link>
          .
        </>
      ) : (
        <>
          pley couldn't find this movie on TMDB.{" "}
          <button type="button" onClick={onFixMatch} className="font-medium text-accent hover:underline">
            Fix match…
          </button>
        </>
      )}
    </Notice>
  );
}
