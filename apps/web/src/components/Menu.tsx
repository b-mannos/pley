import { useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/hooks";

/** Small dropdown menu anchored to a trigger. */
export function Menu({
  trigger,
  children,
  align = "right",
  className,
}: {
  trigger: (props: { open: boolean; toggle: () => void }) => ReactNode;
  children: (close: () => void) => ReactNode;
  align?: "left" | "right";
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useDismiss(ref, open, () => setOpen(false));
  return (
    <div ref={ref} className="relative">
      {trigger({ open, toggle: () => setOpen((o) => !o) })}
      {open ? (
        <div
          role="menu"
          className={cn(
            "absolute top-full z-40 mt-2 min-w-48 animate-pop-in overflow-hidden rounded-lg border border-border bg-surface-2 py-1 shadow-xl shadow-black/50",
            align === "right" ? "right-0" : "left-0",
            className,
          )}
        >
          {children(() => setOpen(false))}
        </div>
      ) : null}
    </div>
  );
}

export function MenuItem({
  children,
  onClick,
  icon,
  danger,
  disabled,
}: {
  children: ReactNode;
  onClick: () => void;
  icon?: ReactNode;
  danger?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-2.5 px-3.5 py-2 text-left text-sm transition-colors hover:bg-white/5 disabled:opacity-50",
        danger ? "text-danger" : "text-fg",
      )}
    >
      {icon ? <span className="text-muted [&>svg]:size-4">{icon}</span> : null}
      {children}
    </button>
  );
}

export function MenuSeparator() {
  return <div className="my-1 h-px bg-border" />;
}
