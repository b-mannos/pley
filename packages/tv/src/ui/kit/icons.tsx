import {
  ArrowDownWideNarrow,
  ArrowLeft,
  ArrowRight,
  ArrowUpNarrowWide,
  AudioLines,
  BookmarkCheck,
  BookmarkPlus,
  Calendar,
  Captions,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronsRight,
  ChevronUp,
  CircleAlert,
  CircleCheck,
  CircleX,
  Clapperboard,
  Delete,
  Disc3,
  Ellipsis,
  EyeOff,
  Film,
  Funnel,
  Gauge,
  History,
  House,
  Info,
  Languages,
  Layers,
  List,
  ListMusic,
  ListOrdered,
  ListPlus,
  ListVideo,
  Lock,
  LogOut,
  Monitor,
  Moon,
  Music,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Repeat,
  Repeat1,
  RotateCcw,
  RotateCw,
  Save,
  Search,
  Server,
  ServerOff,
  Settings,
  Shuffle,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Star,
  Tag,
  Timer,
  Tv,
  UserRound,
  Users,
  WifiOff,
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
  calendar: Calendar,
  captions: Captions,
  check: Check,
  "chevron-down": ChevronDown,
  "chevron-left": ChevronLeft,
  "chevron-right": ChevronRight,
  "chevron-up": ChevronUp,
  "chevrons-right": ChevronsRight,
  "circle-alert": CircleAlert,
  "circle-check": CircleCheck,
  "circle-x": CircleX,
  clapperboard: Clapperboard,
  delete: Delete,
  "disc-3": Disc3,
  ellipsis: Ellipsis,
  "eye-off": EyeOff,
  film: Film,
  funnel: Funnel,
  gauge: Gauge,
  history: History,
  house: House,
  info: Info,
  languages: Languages,
  layers: Layers,
  list: List,
  "list-music": ListMusic,
  "list-ordered": ListOrdered,
  "list-plus": ListPlus,
  "list-video": ListVideo,
  lock: Lock,
  "log-out": LogOut,
  monitor: Monitor,
  moon: Moon,
  music: Music,
  pause: Pause,
  play: Play,
  plus: Plus,
  "refresh-cw": RefreshCw,
  repeat: Repeat,
  "repeat-1": Repeat1,
  "rotate-ccw": RotateCcw,
  "rotate-cw": RotateCw,
  save: Save,
  search: Search,
  server: Server,
  "server-off": ServerOff,
  settings: Settings,
  shuffle: Shuffle,
  "skip-back": SkipBack,
  "skip-forward": SkipForward,
  "sliders-horizontal": SlidersHorizontal,
  star: Star,
  tag: Tag,
  timer: Timer,
  tv: Tv,
  "user-round": UserRound,
  users: Users,
  "wifi-off": WifiOff,
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
