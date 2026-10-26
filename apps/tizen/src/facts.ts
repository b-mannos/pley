/**
 * What a Samsung TV says of itself that decides what it plays. Samsung's
 * own capability calls answer only per codec (no profile, level or HDR
 * kind), so the report comes from the model year's specification (the
 * Tizen version tells the year) refined by these. Read once at startup;
 * tests build them by hand.
 */
export interface TizenFacts {
  /** 4.0, 5.0, 5.5, 6.0 … 10.0. */
  tizen: number;
  /** The model year the Tizen version belongs to. */
  year: number;
  chromium: number | null;
  model: string;
  firmware: string | null;
  uhd: boolean;
  uhd8k: boolean;
  /** webapis.avinfo.isHdrTvSupport(). */
  hdr: boolean;
  /** webapis.systeminfo's answers (Tizen 6.0 and later); `null` when it can't say. */
  av1: boolean | null;
  truehd: boolean | null;
  ac4: boolean | null;
  /** tizen.tvaudiocontrol.getOutputMode(): "PCM", "DOLBY", "DOLBY_DIGITAL_PLUS", "DTS", "AAC". */
  outputMode: string | null;
}

const YEARS: [number, number][] = [
  [10, 2026],
  [9, 2025],
  [8, 2024],
  [7, 2023],
  [6.5, 2022],
  [6, 2021],
  [5.5, 2020],
  [5, 2019],
  [4, 2018],
  [3, 2017],
  [2.4, 2016],
  [2.3, 2015],
];

/** The model year of a Tizen version (each year ships one; 2023 and later sets may be upgraded, see docs/TV_PLATFORMS.md). */
export function yearOfTizen(version: number): number {
  for (const [v, y] of YEARS) if (version >= v) return y;
  return 2015;
}

export function chromiumOf(userAgent: string): number | null {
  const m = /Chrome\/(\d+)/.exec(userAgent);
  return m ? Number(m[1]) : null;
}

function guard<T>(fn: () => T): T | null {
  try {
    const v = fn();
    return v === undefined ? null : v;
  } catch {
    return null;
  }
}

export function readFacts(): TizenFacts {
  const versionText = guard(() => String(tizen!.systeminfo.getCapability("http://tizen.org/feature/platform.version"))) ?? "4.0";
  const tizenVersion = parseFloat(versionText) || 4;
  const info = typeof webapis !== "undefined" ? webapis : undefined;
  const product = info?.productinfo;
  const codecs = info?.systeminfo;
  return {
    tizen: tizenVersion,
    year: yearOfTizen(tizenVersion),
    chromium: chromiumOf(navigator.userAgent),
    model: guard(() => product!.getRealModel()) ?? guard(() => product!.getModel()) ?? "Samsung TV",
    firmware: guard(() => product!.getFirmware()),
    uhd: guard(() => product!.isUdPanelSupported()) ?? false,
    uhd8k: guard(() => !!product!.is8KPanelSupported && product!.is8KPanelSupported()) ?? false,
    hdr: guard(() => info!.avinfo!.isHdrTvSupport()) ?? false,
    av1: codecs?.isSupportedVideoCodec ? guard(() => codecs.isSupportedVideoCodec!("AV1")) : null,
    truehd: codecs?.isSupportedAudioCodec ? guard(() => codecs.isSupportedAudioCodec!("TrueHD")) : null,
    ac4: codecs?.isSupportedAudioCodec ? guard(() => codecs.isSupportedAudioCodec!("AC4")) : null,
    outputMode: guard(() => tizen!.tvaudiocontrol!.getOutputMode()),
  };
}
