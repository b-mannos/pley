import { cn } from "@/lib/cn";

/** Modules of blank margin around a QR code, as scanners expect. */
const QUIET_ZONE = 4;

/**
 * A QR code the server drew: `path` covers its dark modules on a grid
 * `size` modules wide. Dark on white in either theme, which every scanner reads.
 */
export function QrCode({ size, path, label, className }: { size: number; path: string; label: string; className?: string }) {
  const span = size + QUIET_ZONE * 2;
  return (
    <svg
      role="img"
      aria-label={label}
      viewBox={`${-QUIET_ZONE} ${-QUIET_ZONE} ${span} ${span}`}
      shapeRendering="crispEdges"
      className={cn("rounded-lg", className)}
    >
      <rect x={-QUIET_ZONE} y={-QUIET_ZONE} width={span} height={span} fill="#fff" />
      <path d={path} fill="#000" />
    </svg>
  );
}
