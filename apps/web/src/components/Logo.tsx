import { useId } from "react";
import { cn } from "@/lib/cn";

/** The pley mark: a lowercase "p" whose bowl holds a play triangle (cut out). */
export function LogoMark({ className, tile = true }: { className?: string; tile?: boolean }) {
  const gradient = useId();
  return (
    <svg viewBox="0 0 64 64" className={cn("shrink-0", className)} aria-hidden>
      <defs>
        <linearGradient id={gradient} x1="16" y1="12" x2="48" y2="54" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="var(--color-brand-light)" />
          <stop offset="0.5" stopColor="var(--color-accent)" />
          <stop offset="1" stopColor="var(--color-brand-deep)" />
        </linearGradient>
      </defs>
      {tile ? <rect width="64" height="64" rx="15" fill="var(--color-brand-tile)" /> : null}
      <path
        fill={`url(#${gradient})`}
        fillRule="evenodd"
        d="M17 19a5 5 0 0 1 5-5h13a14 14 0 0 1 0 28h-8v8a5 5 0 0 1-10 0zM30 21v14l12-7z"
      />
    </svg>
  );
}

export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn("inline-flex h-8 items-center gap-2.5", className)}>
      <LogoMark className="h-full w-auto" />
      <span className="text-xl font-extrabold tracking-tight text-fg">Pley</span>
    </span>
  );
}
