import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import type { AppState, RequestNotice } from "../../state";
import { useUi } from "../context";
import { Art } from "../kit/art";
import { TextButton } from "../kit/buttons";
import { G } from "../kit/focusable";
import { Icon } from "../kit/icons";

/** Untouched this long, a notice goes by itself. */
export const NOTICE_SHOWN_MS = 10_000;
/** A notice waits for the remote to rest this long, so it never takes a press meant for the page. */
export const NOTICE_IDLE_MS = 1_500;
/** OK this soon after a notice took focus was meant for what had it before: it does nothing. */
const OK_GUARD_MS = 600;

export interface NoticeApi {
  /** The pointer moved onto the page: an open notice closes, and true when one did. */
  pointedAway(el: HTMLElement): boolean;
}

/**
 * Ready-to-watch news over the pages, one at a time (TVs pop up nothing
 * else): a card at the bottom right with [Open] and [Close]. It shows once
 * the remote has rested and nothing else holds focus (`free`): never over
 * a playing video, a panel, a dialog, the open rail or a field that types.
 * It gives focus back to where it was when it closes: by Close, Back, Up or
 * Down, the pointer moving onto the page, or after 10 s untouched.
 */
export function RequestNotices(p: { state: AppState; free: () => boolean; onOpen: (n: RequestNotice) => void; noticeRef: (api: NoticeApi) => void }) {
  const ui = useUi();
  const queue = p.state.notices.value;
  const [shown, setShown] = useState<RequestNotice | null>(null);
  const free = useRef(p.free);
  free.current = p.free;

  // The first in the queue, once the remote has rested and nothing else holds focus.
  const next = queue[0] ?? null;
  useEffect(() => {
    if (shown || !next) return;
    const t = setInterval(() => {
      if (Date.now() - ui.input.lastKeyAt < NOTICE_IDLE_MS || !free.current()) return;
      clearInterval(t);
      setShown(next);
    }, 250);
    return () => clearInterval(t);
  }, [shown?.id ?? null, next?.id ?? null]);

  // Gone from the queue meanwhile (another profile): it goes.
  const gone = !!shown && !queue.some((n) => n.id === shown.id);
  useEffect(() => {
    if (gone) setShown(null);
  }, [gone]);

  if (!shown || gone) {
    p.noticeRef({ pointedAway: () => false });
    return null;
  }
  return (
    <NoticeCard
      key={shown.id}
      notice={shown}
      noticeRef={p.noticeRef}
      onDone={(open) => {
        p.state.dropNotice(shown.id);
        setShown(null);
        if (open) p.onOpen(shown);
      }}
    />
  );
}

function NoticeCard(p: { notice: RequestNotice; onDone: (open: boolean) => void; noticeRef: (api: NoticeApi) => void }) {
  const ui = useUi();
  const card = useRef<HTMLDivElement>(null);
  const shownAt = useRef(Date.now());
  const touchedAt = useRef(Date.now());
  const closed = useRef(false);

  const close = (restore: boolean, open = false) => {
    if (closed.current) return;
    closed.current = true;
    if (card.current) ui.focus.release(card.current, restore);
    // What had focus went meanwhile (its row was drawn again): the page's first takes it, so focus is never lost.
    if (restore && !(ui.focus.current && ui.focus.current.isConnected)) ui.focus.move("down");
    p.onDone(open);
  };
  p.noticeRef({
    pointedAway: (el) => {
      if (closed.current || !card.current || card.current.contains(el)) return false;
      close(false);
      return true;
    },
  });

  // Focus comes here, and goes back when it closes; Back closes it.
  useLayoutEffect(() => {
    const el = card.current;
    if (!el) return;
    ui.focus.trap(el);
    ui.focus.focus(el.querySelector<HTMLElement>("[data-f]"));
    shownAt.current = Date.now();
    touchedAt.current = shownAt.current;
    const off = ui.input.push((key, e) => {
      if (key !== "back") return false;
      if (e.type === "down") close(true);
      return true;
    });
    return () => {
      off();
      // Closed some other way (the page went): the trap goes with it.
      if (!closed.current) ui.focus.release(el, false);
    };
  }, []);

  // Untouched, it goes by itself.
  useEffect(() => {
    let t: ReturnType<typeof setTimeout>;
    const wait = (ms: number) => {
      t = setTimeout(() => {
        const left = touchedAt.current + NOTICE_SHOWN_MS - Date.now();
        if (left > 0) wait(left);
        else close(true);
      }, ms);
    };
    wait(NOTICE_SHOWN_MS);
    return () => clearTimeout(t);
  }, []);

  const touched = () => (touchedAt.current = Date.now());
  const guarded = () => Date.now() - shownAt.current < OK_GUARD_MS;
  return (
    <div ref={card} class="request-notice" role="alertdialog" aria-label={p.notice.text}>
      <div class="request-notice-head">
        {p.notice.poster && (
          <div class="request-notice-art">
            <Art candidates={[p.notice.poster]} width={40} title="" class="tile-clip" />
          </div>
        )}
        <div class="request-notice-text">
          {/* The poster says which title it is; without one, a tick says it's good news. */}
          {!p.notice.poster && <Icon name="circle-check" size={20} color="var(--success)" />}
          <div class="t-label lines-3 c-fg">{p.notice.text}</div>
        </div>
      </div>
      <G
        kind="row"
        class="request-notice-buttons"
        onEdge={(dir) => {
          if (dir === "up" || dir === "down") {
            close(true);
            return true;
          }
          touched();
          return true;
        }}
      >
        <TextButton
          label="Open"
          onFocus={touched}
          onSelect={() => {
            if (!guarded()) close(false, true);
          }}
        />
        <TextButton
          label="Close"
          onFocus={touched}
          onSelect={() => {
            if (!guarded()) close(true);
          }}
        />
      </G>
    </div>
  );
}
