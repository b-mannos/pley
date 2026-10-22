import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { keys } from "./hooks";
import type { Activity, Id, Library, ServerEvent } from "./types";

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
    const pendingLibraries = new Set<Id>();

    // The server neither replays events nor queues them for slow clients:
    // after a reconnect anything may have changed (e.g. a scan finished while
    // the server restarted), so refetch what events would have updated.
    const resync = () => {
      qc.invalidateQueries({ queryKey: keys.libraries });
      qc.invalidateQueries({ queryKey: keys.activity });
      qc.invalidateQueries({ queryKey: ["hubs"] });
      qc.invalidateQueries({ queryKey: ["library"] });
      qc.invalidateQueries({ queryKey: ["item"] });
      qc.invalidateQueries({ queryKey: ["search"] });
    };

    // Metadata refreshes can emit many `library_changed` events in a burst;
    // batch them so we refetch at most every ~1.5 s.
    const flush = () => {
      flushTimer = undefined;
      for (const id of pendingLibraries) qc.invalidateQueries({ queryKey: keys.library(id) });
      pendingLibraries.clear();
      qc.invalidateQueries({ queryKey: keys.libraries });
      qc.invalidateQueries({ queryKey: ["hubs"] });
      qc.invalidateQueries({ queryKey: ["item"] });
      qc.invalidateQueries({ queryKey: ["search"] });
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
          qc.invalidateQueries({ queryKey: keys.activity });
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
          qc.invalidateQueries({ queryKey: keys.activity });
          scheduleLibrary(ev.library_id);
          break;
        case "library_changed":
          scheduleLibrary(ev.library_id);
          break;
        case "libraries_changed":
          qc.invalidateQueries({ queryKey: keys.libraries });
          qc.invalidateQueries({ queryKey: ["hubs"] });
          break;
        case "sessions_changed":
          qc.invalidateQueries({ queryKey: keys.activity });
          break;
      }
    };

    const connect = () => {
      if (closed) return;
      es = new EventSource("/api/events");
      es.onopen = () => {
        failures = 0;
        // Also fires when the browser reconnects on its own.
        if (connectedBefore) resync();
        connectedBefore = true;
      };
      es.onmessage = (msg) => {
        try {
          handle(JSON.parse(msg.data) as ServerEvent);
        } catch {
          // ignore malformed events
        }
      };
      es.onerror = () => {
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
    };
  }, [enabled, qc]);
}
