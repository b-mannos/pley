import { useMemo } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { CHANNEL_PAGE, liveQueries, type Channel, type LiveCategory } from "@/api/livetv";

/** A list of channels whose pages come as they are needed. */
export interface ChannelList {
  /** Channels in the whole list; undefined until its first page is in. */
  total: number | undefined;
  /** The channel at `index`, once its page is in. */
  at: (index: number) => Channel | undefined;
  /** The pages in, by page number. */
  pages: ReadonlyMap<number, Channel[]>;
  isError: boolean;
  error: unknown;
  refetch: () => void;
}

/**
 * A category's channels that match `search`, a page at a time: the first
 * page (it says how many there are) and those covering indexes `from` to
 * `to`, so a lineup of tens of thousands costs what's on screen.
 */
export function useChannelList(category: LiveCategory, search: string, from: number, to: number): ChannelList {
  const first = useQuery(liveQueries.page(category, search, 0));
  const total = first.data?.total;
  const lastPage = total === undefined ? 0 : Math.max(0, Math.ceil(total / CHANNEL_PAGE) - 1);
  const fromPage = Math.min(lastPage, Math.max(0, Math.floor(from / CHANNEL_PAGE)));
  const toPage = Math.min(lastPage, Math.max(fromPage, Math.floor(to / CHANNEL_PAGE)));
  const wanted: number[] = [];
  for (let p = Math.max(1, fromPage); p <= toPage; p++) wanted.push(p);
  const rest = useQueries({ queries: wanted.map((p) => liveQueries.page(category, search, p)) });

  const firstData = first.data;
  // The pages asked for and when each came: the list changes with them only.
  const version = `${wanted.join()}|${rest.map((r) => r.dataUpdatedAt).join()}`;
  const pages = useMemo(() => {
    const map = new Map<number, Channel[]>();
    if (firstData) map.set(0, firstData.channels);
    rest.forEach((r, i) => {
      if (r.data) map.set(wanted[i]!, r.data.channels);
    });
    return map;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `version` stands for the pages' data
  }, [firstData, version]);

  return {
    total,
    at: (index) => pages.get(Math.floor(index / CHANNEL_PAGE))?.[index % CHANNEL_PAGE],
    pages,
    isError: first.isError,
    error: first.error,
    refetch: () => void first.refetch(),
  };
}
