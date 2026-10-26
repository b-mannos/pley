import {
  ArrowDownWideNarrow,
  ArrowLeft,
  ArrowRight,
  ArrowUpNarrowWide,
  AudioLines,
  BookmarkCheck,
  BookmarkPlus,
  Captions,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronsRight,
  CircleAlert,
  CircleCheck,
  Clapperboard,
  Ellipsis,
  EyeOff,
  Film,
  Gauge,
  History,
  House,
  Info,
  Layers,
  ListMusic,
  ListPlus,
  ListVideo,
  LogOut,
  Moon,
  Music,
  Pause,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
  RotateCw,
  Search,
  Server,
  Settings,
  Shuffle,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Star,
  Tv,
  UserRound,
  Users,
  X,
  type IconNode,
} from "lucide";

const ICONS = {
  "arrow-down-wide-narrow": ArrowDownWideNarrow,
  "arrow-left": ArrowLeft,
  "arrow-right": ArrowRight,
  "arrow-up-narrow-wide": ArrowUpNarrowWide,
  "audio-lines": AudioLines,
  "bookmark-check": BookmarkCheck,
  "bookmark-plus": BookmarkPlus,
  captions: Captions,
  check: Check,
  "chevron-left": ChevronLeft,
  "chevron-right": ChevronRight,
  "chevrons-right": ChevronsRight,
  "circle-alert": CircleAlert,
  "circle-check": CircleCheck,
  clapperboard: Clapperboard,
  ellipsis: Ellipsis,
  "eye-off": EyeOff,
  film: Film,
  gauge: Gauge,
  history: History,
  house: House,
  info: Info,
  layers: Layers,
  "list-music": ListMusic,
  "list-plus": ListPlus,
  "list-video": ListVideo,
  "log-out": LogOut,
  moon: Moon,
  music: Music,
  pause: Pause,
  play: Play,
  plus: Plus,
  "refresh-cw": RefreshCw,
  "rotate-ccw": RotateCcw,
  "rotate-cw": RotateCw,
  search: Search,
  server: Server,
  settings: Settings,
  shuffle: Shuffle,
  "skip-back": SkipBack,
  "skip-forward": SkipForward,
  "sliders-horizontal": SlidersHorizontal,
  star: Star,
  tv: Tv,
  "user-round": UserRound,
  users: Users,
  x: X,
} satisfies Record<string, IconNode>;

export type IconName = keyof typeof ICONS;

/** Drawn filled as well as stroked (the Android app's PleyIcons). */
const FILLED: IconName[] = ["play", "pause", "skip-back", "skip-forward", "star"];

/**
 * A Lucide icon on its 24 grid, stroked with round caps and joins, the
 * stroke thinner as the icon grows: 2 + (24 − size)/16 units, within 1.5–2.5.
 */
export function Icon(props: { name: IconName; size?: number; class?: string; color?: string }) {
  const size = props.size ?? 20;
  const stroke = Math.max(1.5, Math.min(2.5, 2 + (24 - size) / 16));
  const node = ICONS[props.name];
  const filled = FILLED.indexOf(props.name) >= 0;
  return (
    <svg
      class={`icon ${props.class ?? ""}`}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-linejoin="round"
      style={props.color ? { color: props.color } : undefined}
      aria-hidden="true"
    >
      {node.map(([tag, attrs], i) => {
        const Tag = tag as "path";
        return <Tag key={i} {...(attrs as Record<string, string>)} />;
      })}
    </svg>
  );
}
