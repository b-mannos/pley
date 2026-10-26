import { describe, expect, it } from "vitest";
import { tierOf, webosCaps } from "../src/caps";
import { chromiumOf, webosOfChromium, webosOfPlatform, type WebosFacts } from "../src/facts";
import { webosKey } from "../src/platform";

function tv(over: Partial<WebosFacts>): WebosFacts {
  return {
    webos: 25,
    chromium: 120,
    model: "OLED65C5PUA",
    firmware: "33.20.65",
    screen: { width: 3840, height: 2160 },
    uhd: true,
    uhd8k: false,
    oled: true,
    hdr10: true,
    dolbyVision: true,
    soundOutput: "tv_speaker",
    digitalOutput: "auto",
    atmos: false,
    edid: null,
    memory: "3G",
    ...over,
  };
}

/** Real sets, as their services describe them (docs/TV_PLATFORMS.md). */
const SETS: Record<string, WebosFacts> = {
  "2018 OLED C8, webOS 4, TV speakers": tv({ webos: 4, chromium: 53, model: "OLED55C8PUA", firmware: "04.40.10", memory: "2.5G" }),
  "2019 LCD UM7300, webOS 4.5, no Dolby Vision, soundbar on ARC": tv({
    webos: 4,
    chromium: 53,
    model: "43UM7300PUA",
    oled: false,
    dolbyVision: false,
    soundOutput: "external_arc",
    memory: "1.25G",
  }),
  "2020 OLED CX, webOS 5, Atmos receiver passing through": tv({
    webos: 5,
    chromium: 68,
    model: "OLED65CXPUA",
    soundOutput: "external_arc",
    digitalOutput: "passThrough",
    atmos: true,
    edid: "TrueHD",
  }),
  "2022 OLED C2 upgraded to webOS 25, DTS receiver": tv({ webos: 25, chromium: 120, model: "OLED55C2PUA", soundOutput: "external_arc", edid: "TrueHD+dts" }),
  "2023 OLED C3, webOS 23, DTS receiver": tv({ webos: 23, chromium: 94, model: "OLED65C3PUA", soundOutput: "external_arc", edid: "TrueHD+dts" }),
  "2026 OLED C6, webOS 26": tv({ webos: 26, chromium: 132, model: "OLED65C6PUA" }),
  "2021 HD set, webOS 6, PCM output": tv({
    webos: 6,
    chromium: 79,
    model: "32LM630BPUA",
    screen: { width: 1920, height: 1080 },
    uhd: false,
    oled: false,
    hdr10: false,
    dolbyVision: false,
    soundOutput: "external_optical",
    digitalOutput: "pcm",
    memory: "1G",
  }),
  "2020 8K NanoCell, webOS 5": tv({ webos: 5, chromium: 68, model: "75NANO99UNA", oled: false, uhd8k: true, screen: { width: 7680, height: 4320 } }),
};

describe("LG capabilities, per set", () => {
  for (const [name, facts] of Object.entries(SETS)) {
    it(name, () => {
      const r = webosCaps(facts);
      expect({ caps: r.caps, dvContainers: r.dvContainers }).toMatchSnapshot();
    });
  }
});

describe("LG capabilities, rules", () => {
  it("shows Dolby Vision only from MP4 and TS before webOS 25", () => {
    expect(webosCaps(tv({ webos: 24, chromium: 108 })).dvContainers).not.toContain("mkv");
    expect(webosCaps(tv({ webos: 25 })).dvContainers).toContain("mkv");
  });

  it("claims Dolby Vision only on sets that show it", () => {
    const no = webosCaps(tv({ dolbyVision: false })).caps;
    expect(no.hdr_types).not.toContain("dolby_vision");
    expect(no.dolby_vision_profiles).toEqual([]);
    const yes = webosCaps(tv({})).caps;
    expect(yes.dolby_vision_profiles.map((p) => p.profile)).toEqual([5, 8]);
  });

  it("never claims HDR10+, which no LG shows: HDR10+ files play their HDR10 base", () => {
    for (const facts of Object.values(SETS)) expect(webosCaps(facts).caps.hdr_types).not.toContain("hdr10plus");
  });

  it("sends Dolby to a receiver only when the digital output lets it", () => {
    expect(webosCaps(tv({ soundOutput: "external_arc", digitalOutput: "auto" })).caps.audio_passthrough).toEqual(["ac3", "eac3"]);
    expect(webosCaps(tv({ soundOutput: "external_arc", digitalOutput: "pcm" })).caps.audio_passthrough).toEqual([]);
    expect(webosCaps(tv({ soundOutput: "tv_speaker" })).caps.audio_passthrough).toEqual([]);
  });

  it("sends TrueHD and DTS only when Settings asks", () => {
    const off = webosCaps(tv({ soundOutput: "external_arc" })).caps;
    expect(off.audio_passthrough).not.toContain("truehd");
    const on = webosCaps(tv({ soundOutput: "external_arc" }), { truehd: true, dts: true }).caps;
    expect(on.audio_passthrough).toEqual(expect.arrayContaining(["truehd", "dts"]));
  });

  it("decodes DTS only on webOS 4 sets", () => {
    expect(webosCaps(tv({ webos: 4, chromium: 53 })).caps.audio_decode).toContain("dts");
    expect(webosCaps(tv({ webos: 5, chromium: 68 })).caps.audio_decode).not.toContain("dts");
    expect(webosCaps(tv({ webos: 25 })).caps.audio_decode).not.toContain("dts");
  });

  it("offers AV1 and VP9 Profile 2 from webOS 5", () => {
    expect(webosCaps(tv({ webos: 4, chromium: 53 })).caps.video_codecs).not.toContain("av1");
    expect(webosCaps(tv({ webos: 5, chromium: 68 })).caps.video_codecs).toContain("av1");
  });

  it("keeps fMP4 HLS for webOS 5 and later", () => {
    expect(webosCaps(tv({ webos: 4, chromium: 53 })).caps.hls.segments).toEqual(["ts"]);
    expect(webosCaps(tv({ webos: 6, chromium: 79 })).caps.hls.segments).toEqual(["ts", "fmp4"]);
  });

  it("rates 2018–2019 and small sets low", () => {
    expect(tierOf(tv({ webos: 4 }))).toBe("low");
    expect(tierOf(tv({ webos: 6, memory: "1G" }))).toBe("low");
    expect(tierOf(tv({ webos: 5, memory: "2G" }))).toBe("mid");
    expect(tierOf(tv({ webos: 23 }))).toBe("high");
  });
});

describe("LG facts", () => {
  it("tells the webOS version by its web engine", () => {
    expect([53, 68, 79, 87, 94, 108, 120, 132].map(webosOfChromium)).toEqual([4, 5, 6, 22, 23, 24, 25, 26]);
    expect(webosOfChromium(null)).toBeNull();
    expect(chromiumOf("Mozilla/5.0 (Web0S; Linux/SmartTV) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.128 Safari/537.36 WebAppManager")).toBe(94);
  });

  it("tells the webOS version by the platform version", () => {
    expect(webosOfPlatform("4.4.3")).toBe(4);
    expect(webosOfPlatform("8.2.0")).toBe(23);
    expect(webosOfPlatform("10.3.1")).toBe(25);
    expect(webosOfPlatform("11.1.0")).toBe(26);
    expect(webosOfPlatform(null)).toBeNull();
  });

  it("reads LG's remote", () => {
    const key = (keyCode: number) => webosKey({ keyCode } as KeyboardEvent);
    expect(key(461)).toBe("back");
    expect(key(13)).toBe("ok");
    expect(key(415)).toBe("play");
    expect(key(19)).toBe("pause");
    expect(key(1536)).toBeNull();
    expect(key(49)).toBe("digit1");
  });
});
