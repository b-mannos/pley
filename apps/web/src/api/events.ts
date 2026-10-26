import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { connectionLost, serverAnswers, setConnectionLost } from "@/lib/connection";
import { keys } from "./hooks";
import type { Activity, Id, Library, ServerEvent } from "./types";

/** How long the stream may be down before the pages say the server is out of reach: a restart takes a moment. */
export const LOST_AFTER_MS = 5_000;

type Listener = (ev: ServerEvent) => void;
const listeners = new Set<Listener>();

/**
 * Calls `listener` with each event from the server's event stream (the one
 * {@link useServerEvents} keeps open) until the returned function is called.
 */
export function onServerEvent(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Keeps one `EventSource('/api/events')` open while logged in and turns server
 * events into React Query cache updates/invalidations.
 */
export function useServerEvents(enabled: boolean) {
  const qc = useQueryClient();

  useEffect(() => {
    if (!enabled || typeof EventSource === "undefined") return;

    let es: EventSource | null = null;
    let retryTimer: number | undefined;
    let flushTimer: number | undefined;
    let closed = false;
    let failures = 0;
    let connectedBefore = false;
    let lostTimer: number | undefined;
    const pendingLibraries = new Set<Id>();

    // Down a while, and the server doesn't answer at all: the pages say so
    // (a stream refused, a sign-in that ended, is something else).
    const dropped = () => {
      if (connectionLost() || lostTimer !== undefined) return;
      lostTimer = window.setTimeout(() => {
        lostTimer = undefined;
        void serverAnswers().then((answers) => {
          if (!closed && !answers && es?.readyState !== EventSource.OPEN) setConnectionLost(true);
        });
      }, LOST_AFTER_MS);
    };
    const back = () => {
      window.clearTimeout(lostTimer);
      lostTimer = undefined;
      setConnectionLost(false);
    };

    // The server neither replays events nor queues them for slow clients:
    // after a reconnect anything may have changed (e.g. a scan finished while
    // the server restarted), so refetch what events would have updated.
    const resync = () => {
      void qc.invalidateQueries({ queryKey: keys.libraries });
      void qc.invalidateQueries({ queryKey: keys.activity });
      void qc.invalidateQueries({ queryKey: ["hubs"] });
      void qc.invalidateQueries({ queryKey: ["library"] });
      void qc.invalidateQueries({ queryKey: ["item"] });
      void qc.invalidateQueries({ queryKey: ["search"] });
      void qc.invalidateQueries({ queryKey: ["history"] });
      void qc.invalidateQueries({ queryKey: keys.watchlist });
    };

    // Metadata refreshes can emit many `library_changed` events in a burst;
    // batch them so we refetch at most every ~1.5 s.
    const flush = () => {
      flushTimer = undefined;
      for (const id of pendingLibraries) void qc.invalidateQueries({ queryKey: keys.library(id) });
      pendingLibraries.clear();
      void qc.invalidateQueries({ queryKey: keys.libraries });
      void qc.invalidateQueries({ queryKey: ["hubs"] });
      void qc.invalidateQueries({ queryKey: ["item"] });
      void qc.invalidateQueries({ queryKey: ["search"] });
      // Finishing something is a library change too.
      void qc.invalidateQueries({ queryKey: ["history"] });
    };
    const scheduleLibrary = (id: Id) => {
      pendingLibraries.add(id);
      if (flushTimer === undefined) flushTimer = window.setTimeout(flush, 1500);
    };

    const setScanning = (id: Id, scanning: boolean) => {
      qc.setQueryData<Library[]>(keys.libraries, (old) =>
        old?.map((l) => (l.id === id && l.scanning !== scanning ? { ...l, scanning } : l)),
      );
    };

    const handle = (ev: ServerEvent) => {
      switch (ev.type) {
        case "scan_started":
          setScanning(ev.library_id, true);
          void qc.invalidateQueries({ queryKey: keys.activity });
          break;
        case "scan_progress": {
          setScanning(ev.library_id, ev.phase !== "done");
          qc.setQueryData<Activity>(keys.activity, (old) => {
            const base: Activity = old ?? { scans: [], sessions: [] };
            const existing = base.scans.find((s) => s.library_id === ev.library_id);
            if (existing) {
              return {
                ...base,
                scans: base.scans.map((s) =>
                  s.library_id === ev.library_id
                    ? { ...s, phase: ev.phase, processed: ev.processed, total: ev.total }
                    : s,
                ),
              };
            }
            const name = qc.getQueryData<Library[]>(keys.libraries)?.find((l) => l.id === ev.library_id)?.name ?? "";
            return {
              ...base,
              scans: [
                ...base.scans,
                {
                  library_id: ev.library_id,
                  library_name: name,
                  phase: ev.phase,
                  processed: ev.processed,
                  total: ev.total,
                  started_at: Math.floor(Date.now() / 1000),
                },
              ],
            };
          });
          break;
        }
        case "scan_finished":
          setScanning(ev.library_id, false);
          void qc.invalidateQueries({ queryKey: keys.activity });
          scheduleLibrary(ev.library_id);
          break;
        case "library_changed":
          scheduleLibrary(ev.library_id);
          break;
        case "libraries_changed":
          void qc.invalidateQueries({ queryKey: keys.libraries });
          void qc.invalidateQueries({ queryKey: ["hubs"] });
          break;
        case "sessions_changed":
          void qc.invalidateQueries({ queryKey: keys.activity });
          break;
        case "watchlist_changed":
          void qc.invalidateQueries({ queryKey: keys.watchlist });
          void qc.invalidateQueries({ queryKey: keys.homeHubs });
          void qc.invalidateQueries({ queryKey: ["item"] });
          break;
      }
      for (const listener of listeners) listener(ev);
    };

    const connect = () => {
      if (closed) return;
      es = new EventSource("/api/events");
      es.onopen = () => {
        failures = 0;
        back();
        // Also fires when the browser reconnects on its own.
        if (connectedBefore) resync();
        connectedBefore = true;
      };
      es.onmessage = (msg) => {
        try {
          handle(JSON.parse(String(msg.data)) as ServerEvent);
        } catch {
          // ignore malformed events
        }
      };
      es.onerror = () => {
        dropped();
        // While CONNECTING the browser retries on its own; once CLOSED
        // (e.g. a non-200 response) we have to reconnect manually.
        if (es && es.readyState === EventSource.CLOSED) {
          es.close();
          es = null;
          failures += 1;
          const delay = Math.min(30_000, 2_000 * failures);
          retryTimer = window.setTimeout(connect, delay);
        }
      };
    };

    connect();
    return () => {
      closed = true;
      es?.close();
      window.clearTimeout(retryTimer);
      window.clearTimeout(flushTimer);
      window.clearTimeout(lostTimer);
      setConnectionLost(false);
    };
  }, [enabled, qc]);
}
