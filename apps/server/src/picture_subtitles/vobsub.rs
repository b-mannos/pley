//! DVD subtitles (VobSub, `S_VOBSUB`) as pictures. Each block is one
//! subpicture unit (SPU): two interlaced fields of 2-bit run-length pixels,
//! then control sequences that say when it stops, which 4 of the 16 palette
//! colours its pixels take, their alpha, and where it goes. The palette and
//! the plane's size come from the track's private data, the `.idx` header
//! (`size: 720x480`, `palette: 000000, ffffff, …`). Read as the apps' decoder
//! reads them (packages/picture-subtitles/src/vobsub.ts).

use super::mks::{Sample, Track};
use super::{MAX_PIXELS, MAX_SIDE, Picture, PictureCue, end_ms, on_plane, so_far};

/// An SPU's bytes at most: its size is a 16-bit field.
const MAX_SPU_BYTES: usize = 0xFFFF;
/// The control sequences read of an SPU: a real one has two or three.
const MAX_SEQUENCES: usize = 64;
/// The plane when the header doesn't say: a PAL DVD's.
const DEFAULT_PLANE: (u32, u32) = (720, 576);

/// A VobSub track's pictures: each block shows from its time until the next
/// block's, or until its stop command when that is sooner.
pub struct VobSubPictures<'a> {
    /// The plane the pictures are placed on: the header's `size`.
    pub width: u32,
    pub height: u32,
    /// The header's 16 colours.
    palette: [[u8; 3]; 16],
    /// In order of time.
    samples: Vec<Sample<'a>>,
}

/// What an SPU's control sequences say.
#[derive(Debug, PartialEq, Eq)]
struct Spu {
    /// Its corners, both included: nothing shows when the second is before
    /// the first.
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    /// Palette entries for pixel values 0-3.
    colors: [u8; 4],
    /// Alpha 0-15 for pixel values 0-3.
    alpha: [u8; 4],
    /// Where each field's lines start.
    top: usize,
    bottom: usize,
    start_ms: i64,
    /// From the stop command; `None` without one.
    stop_ms: Option<i64>,
}

impl Spu {
    /// How long the SPU says it shows, when it does.
    fn own_ms(&self) -> Option<i64> {
        self.stop_ms.filter(|&stop| stop > self.start_ms).map(|stop| stop - self.start_ms)
    }

    /// Its picture's box (x, y, width, height); `None` when it shows
    /// nothing, or more than a frame's pixels.
    fn area(&self) -> Option<(u32, u32, u32, u32)> {
        let (width, height) = (self.x2 - self.x1 + 1, self.y2 - self.y1 + 1);
        if width <= 0 || height <= 0 || width as u64 * height as u64 > MAX_PIXELS {
            return None;
        }
        Some((self.x1 as u32, self.y1 as u32, width as u32, height as u32))
    }
}

impl<'a> VobSubPictures<'a> {
    /// The SPUs of a VobSub track.
    pub(super) fn of(track: &Track<'a>) -> Self {
        let (width, height, palette) = parse_idx(track.codec_private);
        Self { width, height, palette, samples: track.samples() }
    }

    /// When each picture shows and where, in order of time, cut to the
    /// plane. An SPU that shows nothing clears the picture before.
    pub fn cues(&self) -> Vec<PictureCue> {
        let mut cues = Vec::new();
        for (i, sample) in self.samples.iter().enumerate() {
            let Some(spu) = parse_spu(sample.data) else { continue };
            let Some((x, y, width, height)) = spu.area().and_then(|area| on_plane(area, self.plane())) else {
                continue;
            };
            let end_ms = self.end_of(i, &spu);
            if end_ms > sample.time_ms {
                cues.push(PictureCue { start_ms: sample.time_ms, end_ms, x, y, width, height });
            }
        }
        cues
    }

    /// [`VobSubPictures::cues`] of a track copied only so far
    /// ([`super::PictureTrack::cues_so_far`]).
    pub fn cues_so_far(&self) -> Vec<PictureCue> {
        let last = self.samples.last().map(|last| {
            let own = last.duration_ms.or_else(|| parse_spu(last.data).and_then(|spu| spu.own_ms()));
            (last.time_ms, own.is_some_and(|own| own > 0))
        });
        so_far(self.cues(), last)
    }

    /// The picture that shows from `start_ms` (a cue's start), or `None`.
    pub fn picture(&self, start_ms: i64) -> Option<Picture> {
        let data = self.samples.iter().rfind(|s| s.time_ms == start_ms)?.data;
        let spu = parse_spu(data)?;
        let area = spu.area()?;
        let (_, _, width, height) = area;
        let (_, _, shown_width, shown_height) = on_plane(area, self.plane())?;
        let size = usize::from(u16::from_be_bytes([data[0], data[1]]));
        let ctrl = usize::from(u16::from_be_bytes([data[2], data[3]]));
        // The pixels end where the control sequences start.
        let end = [ctrl, size, MAX_SPU_BYTES].into_iter().find(|&n| n > 0).unwrap_or(MAX_SPU_BYTES).min(data.len());
        let mut pixels = vec![0u8; width as usize * height as usize];
        decode_field(data, spu.top, end, width as usize, height as usize, 0, &mut pixels);
        decode_field(data, spu.bottom, end, width as usize, height as usize, 1, &mut pixels);
        let colors = [0, 1, 2, 3].map(|v| {
            let [r, g, b] = self.palette[usize::from(spu.colors[v])];
            [r, g, b, spu.alpha[v] * 17]
        });
        let mut rgba = Vec::with_capacity(shown_width as usize * shown_height as usize * 4);
        for row in pixels.chunks_exact(width as usize).take(shown_height as usize) {
            for &value in &row[..shown_width as usize] {
                rgba.extend_from_slice(&colors[usize::from(value)]);
            }
        }
        Some(Picture { width: shown_width, height: shown_height, rgba })
    }

    fn plane(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// When sample `i` stops showing: at the next one, or after its block's
    /// own duration, else its stop command's, when that is sooner.
    fn end_of(&self, i: usize, spu: &Spu) -> i64 {
        let sample = &self.samples[i];
        let own = sample.duration_ms.or_else(|| spu.own_ms());
        end_ms(sample.time_ms, self.samples.get(i + 1).map(|s| s.time_ms), own)
    }
}

/// The plane's size and the 16 colours of a VobSub header (its first 64
/// KiB, a KiB or two in practice): a PAL DVD's plane, and a DVD's default
/// greys, for what it doesn't say.
fn parse_idx(header: Option<&[u8]>) -> (u32, u32, [[u8; 3]; 16]) {
    let text: String = header.unwrap_or_default().iter().map(|&b| char::from(b.to_ascii_lowercase())).collect();
    let (mut width, mut height) = DEFAULT_PLANE;
    for (at, _) in text.match_indices("size:") {
        if let Some((w, h)) = size(&text[at + "size:".len()..]) {
            if (1..=MAX_SIDE).contains(&w) && (1..=MAX_SIDE).contains(&h) {
                (width, height) = (w, h);
            }
            break;
        }
    }
    let mut colors = Vec::with_capacity(16);
    if let Some(at) = text.find("palette:") {
        let rest = text[at + "palette:".len()..].trim_start();
        let list = &rest
            [..rest.find(|c: char| !(c.is_ascii_hexdigit() || c == ',' || c.is_whitespace())).unwrap_or(rest.len())];
        for entry in list.split(',').take(16) {
            // Its leading hex digits, as the apps read it (`parseInt`).
            let digits = entry.trim();
            let digits = &digits[..digits.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(digits.len())];
            if !digits.is_empty() {
                let v = digits.chars().fold(0u32, |v, c| v.wrapping_mul(16).wrapping_add(c.to_digit(16).unwrap_or(0)));
                colors.push([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
            }
        }
    }
    let mut palette = [[0u8; 3]; 16];
    for (i, entry) in palette.iter_mut().enumerate() {
        *entry = colors.get(i).copied().unwrap_or(if i % 2 == 1 { [255; 3] } else { [0; 3] });
    }
    (width, height, palette)
}

/// `720x480` at the start of `text`, spaces allowed around its parts.
fn size(text: &str) -> Option<(u32, u32)> {
    fn number(s: &str) -> Option<(u32, &str)> {
        let s = s.trim_start();
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        (end > 0).then(|| (s[..end].parse::<u32>().unwrap_or(u32::MAX), &s[end..]))
    }
    let (width, rest) = number(text)?;
    let (height, _) = number(rest.trim_start().strip_prefix('x')?)?;
    Some((width, height))
}

/// Reads the control sequences of an SPU; `None` when it is cut short.
fn parse_spu(data: &[u8]) -> Option<Spu> {
    if data.len() < 4 {
        return None;
    }
    let size = usize::from(u16::from_be_bytes([data[0], data[1]]));
    let ctrl = usize::from(u16::from_be_bytes([data[2], data[3]]));
    let end = if size == 0 { MAX_SPU_BYTES } else { size }.min(data.len());
    let mut spu = Spu {
        x1: 0,
        y1: 0,
        x2: -1,
        y2: -1,
        colors: [0, 1, 2, 3],
        alpha: [0, 15, 15, 15],
        top: 0,
        bottom: 0,
        start_ms: 0,
        stop_ms: None,
    };
    let mut seq = ctrl;
    let mut seen = Vec::new();
    while seq + 4 <= end && !seen.contains(&seq) && seen.len() < MAX_SEQUENCES {
        seen.push(seq);
        // Delays count in 1024 ticks of a 90 kHz clock, rounded to the
        // nearest millisecond.
        let delay = i64::from(u16::from_be_bytes([data[seq], data[seq + 1]]));
        let delay_ms = (delay * 1024 * 2 + 90) / 180;
        let next = usize::from(u16::from_be_bytes([data[seq + 2], data[seq + 3]]));
        let mut at = seq + 4;
        while at < end {
            let command = data[at];
            at += 1;
            // What follows the command, up to the SPU's end.
            let args = &data[at..end];
            match command {
                // Forced start, start.
                0x00 | 0x01 => spu.start_ms = delay_ms,
                0x02 => spu.stop_ms = Some(delay_ms),
                0x03 => {
                    let b = args.get(..2)?;
                    spu.colors = [b[1] & 0x0F, b[1] >> 4, b[0] & 0x0F, b[0] >> 4];
                    at += 2;
                }
                0x04 => {
                    let b = args.get(..2)?;
                    spu.alpha = [b[1] & 0x0F, b[1] >> 4, b[0] & 0x0F, b[0] >> 4];
                    at += 2;
                }
                0x05 => {
                    let b = args.get(..6)?;
                    spu.x1 = (i32::from(b[0]) << 4) | i32::from(b[1] >> 4);
                    spu.x2 = (i32::from(b[1] & 0x0F) << 8) | i32::from(b[2]);
                    spu.y1 = (i32::from(b[3]) << 4) | i32::from(b[4] >> 4);
                    spu.y2 = (i32::from(b[4] & 0x0F) << 8) | i32::from(b[5]);
                    at += 6;
                }
                0x06 => {
                    let b = args.get(..4)?;
                    spu.top = usize::from(u16::from_be_bytes([b[0], b[1]]));
                    spu.bottom = usize::from(u16::from_be_bytes([b[2], b[3]]));
                    at += 4;
                }
                // The end of the sequence; an unknown command too, as its
                // length isn't known.
                _ => break,
            }
        }
        if next == seq {
            break;
        }
        seq = next;
    }
    Some(spu)
}

/// One field's lines (every other line of the picture, from line `first`)
/// into `out`, from its run-length codes at `offset` up to `end`.
fn decode_field(data: &[u8], offset: usize, end: usize, width: usize, height: usize, first: usize, out: &mut [u8]) {
    let mut nibble = offset * 2;
    let limit = end * 2;
    let read = |nibble: &mut usize| -> u16 {
        if *nibble >= limit {
            return 0;
        }
        let byte = data[*nibble / 2];
        let v = if *nibble % 2 == 1 { byte & 0x0F } else { byte >> 4 };
        *nibble += 1;
        u16::from(v)
    };
    for y in (first..height).step_by(2) {
        let mut x = 0;
        while x < width {
            // A code of 1 to 4 nibbles, longer as it starts with more zeros:
            // its count, then its colour in the last 2 bits.
            let mut v = read(&mut nibble);
            if v < 0x4 {
                v = (v << 4) | read(&mut nibble);
                if v < 0x10 {
                    v = (v << 4) | read(&mut nibble);
                    if v < 0x40 {
                        v = (v << 4) | read(&mut nibble);
                    }
                }
            }
            let color = (v & 3) as u8;
            // A count of 0: to the end of the line.
            let count = match usize::from(v >> 2) {
                0 => width - x,
                count => count,
            };
            let stop = width.min(x + count);
            out[y * width + x..y * width + stop].fill(color);
            x = stop;
            if nibble >= limit {
                break;
            }
        }
        // Lines start on a byte.
        nibble += nibble % 2;
    }
}

#[cfg(test)]
mod tests {
    use super::super::PictureTrack;
    use super::super::mks::testing::{lasting, mks_of};
    use super::*;

    const IDX: &[u8] =
        b"# VobSub index file, v7\nsize: 720x480\npalette: 000000, ffffff, 808080, ff0000, 00ff00, 0000ff, \
        000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000, 000000\n";

    /// An SPU of a 4×2 picture at the corners `area` says (10, 400 to 13,
    /// 401 by default): its top line in colour 1 (white), its bottom line in
    /// colour 2 (grey), shown for 2 s.
    fn spu(area: [u8; 6]) -> Vec<u8> {
        // 4-nibble codes: 0001 00cc, 4 pixels of colour cc.
        let pixels = [0x11, 0x00, 0x12, 0x00];
        let ctrl_at = 4 + pixels.len() as u16;
        let stop_at = ctrl_at + 4 + 1 + 3 + 3 + 7 + 5 + 1;
        let mut ctrl = vec![0, 0];
        ctrl.extend(stop_at.to_be_bytes());
        ctrl.extend([0x01, 0x03, 0x32, 0x10, 0x04, 0xFF, 0xF0, 0x05]);
        ctrl.extend(area);
        ctrl.extend([0x06, 0, 4, 0, 6, 0xFF]);
        // 175 ticks of 1024 at 90 kHz: 1991 ms.
        let stop = [&175u16.to_be_bytes()[..], &stop_at.to_be_bytes(), &[0x02, 0xFF]].concat();
        let all = [&pixels[..], &ctrl, &stop].concat();
        [&(4 + all.len() as u16).to_be_bytes()[..], &ctrl_at.to_be_bytes(), &all].concat()
    }

    const AREA: [u8; 6] = [0x00, 0xA0, 0x0D, 0x19, 0x01, 0x91];

    fn read(mks: &[u8]) -> VobSubPictures<'_> {
        match PictureTrack::read(mks) {
            Some(PictureTrack::VobSub(pictures)) => pictures,
            _ => panic!("not a VobSub track"),
        }
    }

    #[test]
    fn the_idx_header_gives_the_plane_and_palette() {
        let (width, height, palette) = parse_idx(Some(IDX));
        assert_eq!((width, height), (720, 480));
        assert_eq!((palette[1], palette[3], palette[15]), ([255, 255, 255], [255, 0, 0], [0, 0, 0]));
        // Upper case, and spaces around the size's parts.
        assert_eq!(parse_idx(Some(b"SIZE: 1920 x 1080\nPALETTE: FFFFFF")).0, 1920);
        // Nothing said: a PAL DVD's plane and default greys.
        let (width, height, palette) = parse_idx(None);
        assert_eq!((width, height, palette[0], palette[1]), (720, 576, [0, 0, 0], [255, 255, 255]));
        // 16 colours at most, and a plane within bounds.
        let many = format!("palette: {}\nsize: 5000x5000\n", ["123456"; 20].join(", "));
        let (width, _, palette) = parse_idx(Some(many.as_bytes()));
        assert_eq!((width, palette[15]), (720, [0x12, 0x34, 0x56]));
    }

    #[test]
    fn control_sequences_say_where_in_what_colours_and_until_when() {
        let read = parse_spu(&spu(AREA)).unwrap();
        assert_eq!((read.x1, read.x2, read.y1, read.y2), (10, 13, 400, 401));
        assert_eq!((read.colors, read.alpha), ([0, 1, 2, 3], [0, 15, 15, 15]));
        assert_eq!((read.top, read.bottom), (4, 6));
        assert_eq!((read.start_ms, read.stop_ms, read.own_ms()), (0, Some(1_991), Some(1_991)));
        // Cut short in a command (its colours): nothing to read.
        assert_eq!(parse_spu(&spu(AREA)[..15]), None);
        assert_eq!(parse_spu(&[0, 4]), None);
    }

    #[test]
    fn control_sequences_are_read_64_at_most() {
        // `n` sequences chained one to the next; the last says when it stops.
        let chained = |n: usize| {
            let mut out = vec![0, 0, 0, 4];
            for _ in 0..n - 1 {
                let next = (out.len() + 5) as u16;
                out.extend([0, 0]);
                out.extend(next.to_be_bytes());
                out.push(0xFF);
            }
            let here = out.len() as u16;
            out.extend(175u16.to_be_bytes());
            out.extend(here.to_be_bytes());
            out.extend([0x02, 0xFF]);
            let size = out.len() as u16;
            out[..2].copy_from_slice(&size.to_be_bytes());
            out
        };
        assert_eq!(parse_spu(&chained(64)).unwrap().stop_ms, Some(1_991));
        assert_eq!(parse_spu(&chained(65)).unwrap().stop_ms, None);
    }

    #[test]
    fn interlaced_fields_decode_in_the_palettes_colours() {
        let file = mks_of(b"S_VOBSUB", Some(IDX), &[(1_000, &[(0, spu(AREA))])], false);
        let pictures = read(&file);
        assert_eq!((pictures.width, pictures.height), (720, 480));
        // Its stop command ends it.
        let cue = PictureCue { start_ms: 1_000, end_ms: 2_991, x: 10, y: 400, width: 4, height: 2 };
        assert_eq!(pictures.cues(), [cue]);
        let picture = pictures.picture(1_000).unwrap();
        assert_eq!((picture.width, picture.height), (4, 2));
        // The top field's line white, the bottom's grey, all opaque.
        assert_eq!(&picture.rgba[..16], [[255, 255, 255, 255]; 4].concat());
        assert_eq!(&picture.rgba[16..], [[128, 128, 128, 255]; 4].concat());
        assert_eq!(pictures.picture(1_500), None);
    }

    #[test]
    fn run_lengths_decode_as_dvds_write_them() {
        // Codes of 1 nibble (3 px of 1), 2 (5 px of 2), 3 (17 px of 3), then
        // 4: to the end of the line, in colour 1.
        let data = [0xD1, 0x60, 0x47, 0x00, 0x01];
        let mut out = vec![0u8; 40];
        decode_field(&data, 0, data.len(), 40, 1, 0, &mut out);
        let expected = [vec![1; 3], vec![2; 5], vec![3; 17], vec![1; 15]].concat();
        assert_eq!(out, expected);
    }

    #[test]
    fn pictures_are_cut_to_the_plane_and_a_frame_at_most() {
        // x 700..719, y 470..489: 10 of its 20 lines on a 720×480 plane.
        let file = mks_of(b"S_VOBSUB", Some(IDX), &[(0, &[(0, spu([0x2B, 0xC2, 0xCF, 0x1D, 0x61, 0xE9]))])], false);
        let pictures = read(&file);
        let cue = pictures.cues()[0];
        assert_eq!((cue.x, cue.y, cue.width, cue.height), (700, 470, 20, 10));
        let picture = pictures.picture(0).unwrap();
        assert_eq!((picture.width, picture.height, picture.rgba.len()), (20, 10, 20 * 10 * 4));
        // x 0..3839, y 0..2159: a 4K screen's worth, just so; a line more is
        // too much.
        let area = |file: &[u8]| read(file).cues().first().map(|c| (c.width, c.height));
        let big =
            mks_of(b"S_VOBSUB", Some(b"size: 3840x2160"), &[(0, &[(0, spu([0, 0x0E, 0xFF, 0, 0x08, 0x6F]))])], false);
        assert_eq!(area(&big), Some((3840, 2160)));
        let bigger =
            mks_of(b"S_VOBSUB", Some(b"size: 3840x2160"), &[(0, &[(0, spu([0, 0x0E, 0xFF, 0, 0x08, 0x70]))])], false);
        assert_eq!(area(&bigger), None);
    }

    #[test]
    fn a_picture_shows_until_the_next_its_block_or_its_stop_command_says() {
        // An SPU without a stop command: until the next, which shows nothing
        // (its corners the wrong way round), or 10 s for the last.
        let mut endless = spu(AREA);
        let stop_at = endless.len() - 6;
        endless[stop_at + 4] = 0xFF;
        let clear = spu([0, 0xA0, 0x05, 0x19, 0x01, 0x91]);
        let file = mks_of(
            b"S_VOBSUB",
            Some(IDX),
            &[(1_000, &[(0, endless.clone()), (3_000, clear)]), (9_000, &[(0, endless.clone())])],
            false,
        );
        let pictures = read(&file);
        let times = |cues: Vec<PictureCue>| cues.iter().map(|c| (c.start_ms, c.end_ms)).collect::<Vec<_>>();
        assert_eq!(times(pictures.cues()), [(1_000, 4_000), (9_000, 19_000)]);
        // Copied so far: what ends the last may be copied later.
        assert_eq!(times(pictures.cues_so_far()), [(1_000, 4_000)]);
        // A block's own duration comes first.
        let file = [mks_of(b"S_VOBSUB", Some(IDX), &[], false), lasting(1_000, 500, &spu(AREA))].concat();
        assert_eq!(times(read(&file).cues()), [(1_000, 1_500)]);
        assert_eq!(times(read(&file).cues_so_far()), [(1_000, 1_500)]);
    }
}
