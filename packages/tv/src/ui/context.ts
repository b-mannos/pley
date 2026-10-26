import { createContext } from "preact";
import { useContext } from "preact/hooks";
import type { Api } from "../api/client";
import type { ImageLoader } from "./images";
import type { FocusManager } from "./focus";
import type { Input } from "./input";
import type { Navigator } from "./shell/navigator";

/** What every screen reaches: input and focus, navigation, the server, pictures and toasts. */
export interface Ui {
  input: Input;
  focus: FocusManager;
  nav: Navigator;
  api: Api;
  images: ImageLoader;
  /** Pixels per canvas px: 2 on a 1080p TV, 4/3 on 720p. */
  scale: number;
  /** "low" TVs swap backdrops at once and ask for smaller pictures. */
  tier: "low" | "mid" | "high";
  reducedMotion: boolean;
  toast(text: string, error?: boolean): void;
  /**
   * An element that may take focus as its page opens: the page's
   * remembered element at once, else the `initial` one when nothing in the
   * page (nor the rail) has focus.
   */
  claim(el: HTMLElement, fid: string | null, initial: boolean): void;
}

export const UiContext = createContext<Ui | null>(null);

export function useUi(): Ui {
  const ui = useContext(UiContext);
  if (!ui) throw new Error("no Ui");
  return ui;
}
