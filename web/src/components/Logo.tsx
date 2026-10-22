import { cn } from "@/lib/cn";

export function LogoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" className={cn("shrink-0", className)} aria-hidden>
      <rect width="64" height="64" rx="14" fill="var(--color-surface-3)" />
      <path
        d="M24 17.5v29a2.5 2.5 0 0 0 3.8 2.1l23.2-14.5a2.5 2.5 0 0 0 0-4.2L27.8 15.4a2.5 2.5 0 0 0-3.8 2.1z"
        fill="var(--color-accent)"
      />
    </svg>
  );
}

export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn("inline-flex h-8 items-center gap-2.5", className)}>
      <LogoMark className="h-full w-auto" />
      <span className="text-xl font-extrabold tracking-tight text-fg">
        pley
      </span>
    </span>
  );
}
