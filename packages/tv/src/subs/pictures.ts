/**
 * What picture subtitles become once decoded: bitmaps placed on the
 * subtitle plane (1920×1080 for a Blu-ray's PGS, 720×576 for a DVD's
 * VobSub), drawn over the whole screen at the same fractions.
 */

export interface SubtitleBitmap {
  x: number;
  y: number;
  width: number;
  height: number;
  /** RGBA, `width × height × 4` bytes, not premultiplied. */
  rgba: Uint8ClampedArray;
}

export interface PictureFrame {
  /** The plane the bitmaps are placed on. */
  planeWidth: number;
  planeHeight: number;
  /** Nothing to show: the screen clears. */
  bitmaps: SubtitleBitmap[];
}

/** A decoder for one track's samples, which may need earlier samples to decode a later one. */
export interface PictureDecoder {
  /** The frame sample `index` shows; `null` when it can't be decoded. */
  decode(index: number): PictureFrame | null;
  /**
   * How long sample `index` shows when the sample itself says (VobSub's
   * stop command), else `null`: until the next sample.
   */
  ownDurationMs(index: number): number | null;
}

/** Limited-range YCbCr to RGB: BT.709 for HD planes, BT.601 for SD (as FFmpeg's decoders). */
export function ycbcrToRgb(y: number, cb: number, cr: number, hd: boolean): [number, number, number] {
  const yy = 1.164383 * (y - 16);
  const u = cb - 128;
  const v = cr - 128;
  let r: number;
  let g: number;
  let b: number;
  if (hd) {
    r = yy + 1.792741 * v;
    g = yy - 0.213249 * u - 0.532909 * v;
    b = yy + 2.112402 * u;
  } else {
    r = yy + 1.596027 * v;
    g = yy - 0.391762 * u - 0.812968 * v;
    b = yy + 2.017232 * u;
  }
  const c = (x: number) => (x < 0 ? 0 : x > 255 ? 255 : Math.round(x));
  return [c(r), c(g), c(b)];
}
