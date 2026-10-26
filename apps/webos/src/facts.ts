import { hasLuna, luna, type LunaAnswer } from "./luna";

/**
 * What the TV says of itself that decides what it plays: the webOS
 * version (by its web engine, which LG never updates within a version),
 * the panel, the HDR formats it shows and where its sound goes. Read once
 * at startup from LG's own services; tests build these by hand.
 */
export interface WebosFacts {
  /** 4, 5, 6, 22, 23, 24, 25, 26. */
  webos: number;
  chromium: number | null;
  model: string;
  firmware: string | null;
  /** The pipeline's resolution: 3840×2160 on a UHD set (the web layer stays 1920×1080). */
  screen: { width: number; height: number } | null;
  uhd: boolean;
  uhd8k: boolean;
  oled: boolean;
  /** "tv.model.supportHDR". */
  hdr10: boolean;
  /** "tv.config.supportDolbyHDRContents". */
  dolbyVision: boolean;
  /** Where the sound goes: "tv_speaker", "external_arc", "external_optical", "bt_soundbar"… */
  soundOutput: string | null;
  /** The digital output setting: "auto", "passThrough" or "pcm". */
  digitalOutput: string | null;
  /** The receiver reports Atmos (eARC/ARC), or the TV decodes it to its own speakers. */
  atmos: boolean;
  /** "tv.model.edidType": what the HDMI sink takes ("TrueHD", "TrueHD+dts"). */
  edid: string | null;
  /** "tv.hw.ddrSize" ("1.5G", "3G"). */
  memory: string | null;
}

/** The webOS version of a web engine: LG ships one Chromium per version and never updates it. */
export function webosOfChromium(chromium: number | null): number | null {
  if (chromium === null) return null;
  if (chromium >= 132) return 26;
  if (chromium >= 120) return 25;
  if (chromium >= 108) return 24;
  if (chromium >= 94) return 23;
  if (chromium >= 87) return 22;
  if (chromium >= 79) return 6;
  if (chromium >= 68) return 5;
  if (chromium >= 53) return 4;
  if (chromium >= 38) return 3;
  return null;
}

/** The webOS version of a platform version ("4.4.3" is webOS 4, "10.2.0" is webOS 25). */
export function webosOfPlatform(version: string | null | undefined): number | null {
  const m = /^(\d+)\./.exec(version ?? "");
  if (!m) return null;
  const major = Number(m[1]);
  if (major >= 7 && major <= 11) return major + 15;
  return major >= 3 && major <= 6 ? major : null;
}

export function chromiumOf(userAgent: string): number | null {
  const m = /Chrome\/(\d+)/.exec(userAgent);
  return m ? Number(m[1]) : null;
}

const CONFIGS = [
  "tv.model.modelname",
  "tv.nyx.firmwareVersion",
  "tv.nyx.platformVersion",
  "tv.hw.panelResolution",
  "tv.hw.displayType",
  "tv.hw.ddrSize",
  "tv.model.supportHDR",
  "tv.config.supportDolbyHDRContents",
  "tv.config.supportDolbyTVATMOS",
  "tv.model.soundModeType",
  "tv.model.edidType",
];

function str(v: unknown): string | null {
  return typeof v === "string" && v ? v : typeof v === "number" ? String(v) : null;
}

function bool(v: unknown): boolean {
  return v === true || v === "true" || v === "TRUE" || v === 1;
}

/**
 * Reads the facts from the TV: PalmSystem's device info at once, then
 * the config and sound services. Anything that doesn't answer is left at
 * what is safe (no Dolby Vision, sound decoded by the TV).
 */
export async function readFacts(): Promise<WebosFacts> {
  const ua = navigator.userAgent;
  const chromium = chromiumOf(ua);
  let device: Record<string, unknown> = {};
  try {
    const raw = window.PalmSystem?.deviceInfo ?? window.webOSSystem?.deviceInfo;
    if (raw) device = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    // Not on a TV.
  }
  let configs: Record<string, unknown> = {};
  let sound: Record<string, unknown> = {};
  let arc: LunaAnswer = {};
  if (hasLuna()) {
    const [c, s, a] = await Promise.all([
      luna("luna://com.webos.service.config/getConfigs", { configNames: CONFIGS }).catch(() => ({}) as LunaAnswer),
      luna("luna://com.webos.settingsservice/getSystemSettings", { category: "sound", keys: ["soundOutput", "soundOutputDigital"] }).catch(
        () => ({}) as LunaAnswer,
      ),
      luna("luna://com.webos.service.arccontroller/getARCState", {}).catch(() => ({}) as LunaAnswer),
    ]);
    configs = (c.configs as Record<string, unknown>) ?? {};
    sound = (s.settings as Record<string, unknown>) ?? {};
    arc = a;
  }
  const platform = str(configs["tv.nyx.platformVersion"]) ?? str(device.platformVersion);
  const webos = webosOfChromium(chromium) ?? webosOfPlatform(platform) ?? 4;
  const panel = (str(configs["tv.hw.panelResolution"]) ?? "").toUpperCase();
  const width = Number(device.screenWidth) || 0;
  const height = Number(device.screenHeight) || 0;
  const uhd = panel === "UD" || panel === "8K" || width >= 3840 || device.UHD === "true";
  const soundOutput = str(sound.soundOutput);
  const digital = str(sound.soundOutputDigital);
  const arcAtmos = arc.arcProfile === "ATMOS" || arc.earcATMOS === true;
  const tvAtmos = bool(configs["tv.config.supportDolbyTVATMOS"]) || str(configs["tv.model.soundModeType"]) === "Dolby Atmos";
  const toReceiver = !!soundOutput && soundOutput.indexOf("external_arc") === 0 && soundOutput !== "external_arc_sound_alive";
  return {
    webos,
    chromium,
    model: str(configs["tv.model.modelname"]) ?? str(device.modelName) ?? "LG TV",
    firmware: str(configs["tv.nyx.firmwareVersion"]) ?? str(device.platformVersion),
    screen: width && height ? { width, height } : uhd ? { width: 3840, height: 2160 } : null,
    uhd,
    uhd8k: panel === "8K",
    oled: str(configs["tv.hw.displayType"]) === "OLED",
    hdr10: bool(configs["tv.model.supportHDR"]),
    dolbyVision: bool(configs["tv.config.supportDolbyHDRContents"]),
    soundOutput,
    digitalOutput: digital,
    atmos: toReceiver ? arcAtmos && (digital === "auto" || digital === "passThrough") : tvAtmos,
    edid: str(configs["tv.model.edidType"]),
    memory: str(configs["tv.hw.ddrSize"]),
  };
}

/** Facts for a desktop browser (development): an LG of the newest kind, decoding what Chromium decodes. */
export function browserFacts(): WebosFacts {
  return {
    webos: webosOfChromium(chromiumOf(navigator.userAgent)) ?? 25,
    chromium: chromiumOf(navigator.userAgent),
    model: "Browser",
    firmware: null,
    screen: { width: 1920, height: 1080 },
    uhd: false,
    uhd8k: false,
    oled: false,
    hdr10: false,
    dolbyVision: false,
    soundOutput: "tv_speaker",
    digitalOutput: null,
    atmos: false,
    edid: null,
    memory: null,
  };
}
