import { useState } from "react";
import type { User } from "@/api/types";
import { cn } from "@/lib/cn";

/** Colors offered for profiles; also the fallback picked from the name. */
export const AVATAR_COLORS = ["#38bdf8", "#a78bfa", "#f472b6", "#fb923c", "#facc15", "#4ade80", "#2dd4bf", "#f87171"];

const SIZES = {
  xs: "size-6 text-[11px]",
  sm: "size-8 text-xs",
  md: "size-9 text-sm",
  lg: "size-14 text-xl",
  xl: "size-28 text-4xl sm:size-32 sm:text-5xl",
} as const;

export type AvatarUser = Pick<User, "id" | "name" | "avatar_url" | "avatar_color">;

/** The profile's color, or a stable one derived from its name. */
export function avatarColor(user: Pick<User, "name" | "avatar_color">): string {
  if (user.avatar_color) return user.avatar_color;
  let hash = 0;
  for (const ch of user.name) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return AVATAR_COLORS[hash % AVATAR_COLORS.length];
}

/** Dark text on light colors, white on dark ones. */
function textOn(hex: string): string {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex);
  if (!m) return "#ffffff";
  const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join("") : m[1];
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 0.45 ? "#0b0d10" : "#ffffff";
}

/** Profile picture, or a colored circle with the initial. */
export function Avatar({
  user,
  size = "md",
  className,
}: {
  user: AvatarUser;
  size?: keyof typeof SIZES;
  className?: string;
}) {
  const [broken, setBroken] = useState<string | null>(null);
  const picture = user.avatar_url && broken !== user.avatar_url ? user.avatar_url : null;
  const color = avatarColor(user);
  return (
    <span
      aria-hidden
      className={cn(
        "relative inline-flex shrink-0 select-none items-center justify-center overflow-hidden rounded-full font-bold uppercase",
        SIZES[size],
        className,
      )}
      style={picture ? undefined : { backgroundColor: color, color: textOn(color) }}
    >
      {picture ? (
        <img
          src={picture}
          alt=""
          draggable={false}
          className="size-full object-cover"
          onError={() => setBroken(picture)}
        />
      ) : (
        (user.name.trim()[0] ?? "?")
      )}
    </span>
  );
}
