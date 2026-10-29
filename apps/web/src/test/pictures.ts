/** A PGS track in a Matroska file, as the server makes them (`/api/files/{id}/subtitles/{index}.mks`). */

const u16 = (v: number) => [(v >> 8) & 0xff, v & 0xff];

function el(id: number, body: number[]): number[] {
  const idBytes: number[] = [];
  for (let v = id; v > 0; v = Math.floor(v / 256)) idBytes.unshift(v & 0xff);
  const n = body.length;
  return [...idBytes, 0x01, 0, 0, 0, (n >>> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff, ...body];
}

const seg = (type: number, body: number[]) => [type, ...u16(body.length), ...body];

/** A display set showing a white 4×2 object at (x, y) on a 1920×1080 plane, or clearing it. */
function displaySet(show: boolean, x = 100, y = 900): number[] {
  const pcs = [...u16(1920), ...u16(1080), 0x10, ...u16(1), show ? 0x80 : 0, 0, 0, show ? 1 : 0];
  if (!show) return [...seg(0x16, pcs), ...seg(0x80, [])];
  pcs.push(...u16(0), 0, 0, ...u16(x), ...u16(y));
  const rle = [1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0];
  const data = [...u16(4), ...u16(2), ...rle];
  return [
    ...seg(0x16, pcs),
    ...seg(0x17, [1, 0, ...u16(x), ...u16(y), ...u16(4), ...u16(2)]),
    ...seg(0x14, [0, 0, 1, 235, 128, 128, 255]),
    ...seg(0x15, [...u16(0), 0, 0xc0, 0, (data.length >> 8) & 0xff, data.length & 0xff, ...data]),
    ...seg(0x80, []),
  ];
}

/** Cues as [start ms, end ms]: each shows a white object, then clears. */
export function pgsMks(cues: [number, number][]): ArrayBuffer {
  const track = el(0xae, [...el(0xd7, [1]), ...el(0x83, [0x11]), ...el(0x86, Array.from("S_HDMV/PGS", (c) => c.charCodeAt(0)))]);
  const cluster = [...el(0xe7, [0])];
  for (const [start, end] of cues) {
    for (const [time, show] of [[start, true], [end, false]] as const) {
      cluster.push(...el(0xa3, [0x81, ...u16(time), 0x80, ...displaySet(show)]));
    }
  }
  const segment = el(0x18538067, [...el(0x1549a966, el(0x2ad7b1, [0x0f, 0x42, 0x40])), ...el(0x1654ae6b, track), ...el(0x1f43b675, cluster)]);
  return new Uint8Array([...el(0x1a45dfa3, el(0x4282, Array.from("matroska", (c) => c.charCodeAt(0)))), ...segment]).buffer;
}
