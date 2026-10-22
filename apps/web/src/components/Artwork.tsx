import { memo, useState } from "react";
import { cn } from "@/lib/cn";
import { placeholderGradient, sizedImage } from "@/lib/images";

export interface ArtworkProps {
  /** Unsized image urls in priority order (the first that loads wins). */
  candidates: string[];
  /** Rendered CSS width used to pick the image bucket. */
  width: number;
  alt: string;
  /** Text shown on the placeholder tile. */
  title: string;
  className?: string;
  imgClassName?: string;
  eager?: boolean;
  /** Hide the title on the placeholder (e.g. for backdrops). */
  bare?: boolean;
}

/**
 * Lazy-loaded image with fade-in, fallback chain and a gradient placeholder.
 * The parent controls the box size (aspect ratio) through `className`.
 */
export const Artwork = memo(function Artwork({
  candidates,
  width,
  alt,
  title,
  className,
  imgClassName,
  eager,
  bare,
}: ArtworkProps) {
  const key = candidates.join("|");
  const [state, setState] = useState({ key, index: 0, loaded: false });
  const current = state.key === key ? state : { key, index: 0, loaded: false };
  const url = candidates[current.index];
  const src = url ? sizedImage(url, width) : null;
  const failed = !src;

  return (
    <div
      className={cn("relative overflow-hidden bg-surface-2", className)}
      style={!current.loaded ? { backgroundImage: placeholderGradient(title || alt) } : undefined}
    >
      {failed && !bare ? (
        <div className="absolute inset-0 flex items-center justify-center p-3 text-center">
          <span className="line-clamp-4 text-sm font-semibold leading-snug text-white/70 [text-wrap:balance]">
            {title}
          </span>
        </div>
      ) : null}
      {src ? (
        <img
          key={src}
          src={src}
          alt={alt}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          draggable={false}
          onLoad={() => setState({ key, index: current.index, loaded: true })}
          onError={() => setState({ key, index: current.index + 1, loaded: false })}
          className={cn(
            "absolute inset-0 size-full object-cover transition-opacity duration-300",
            current.loaded ? "opacity-100" : "opacity-0",
            imgClassName,
          )}
        />
      ) : null}
    </div>
  );
});
