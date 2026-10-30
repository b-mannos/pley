import { useEffect } from "react";
import { useNavigate } from "react-router";
import { onServerEvent } from "@/api/events";
import { itemUrl } from "@/lib/urls";
import { useToast } from "./Toast";

/**
 * Says so when something the user asked for is in a library now, with a
 * way to it. (The toasts show outside the router: a button that navigates,
 * not a link.)
 */
export function useRequestAlerts() {
  const toast = useToast();
  const navigate = useNavigate();
  useEffect(
    () =>
      onServerEvent((ev) => {
        if (ev.type !== "request_available") return;
        const itemId = ev.item_id;
        toast.success(
          <span>
            <strong className="font-semibold">{ev.title}</strong>, which you asked for, is ready to watch.
            {itemId !== null ? (
              <button type="button" onClick={() => void navigate(itemUrl(itemId))} className="ml-2 font-medium text-accent hover:underline">
                Open
              </button>
            ) : null}
          </span>,
          { duration: 10_000 },
        );
      }),
    [toast, navigate],
  );
}
