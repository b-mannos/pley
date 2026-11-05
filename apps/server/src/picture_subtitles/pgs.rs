//! PGS subtitles made of the server's Matroska file of the track, for
//! players that can't read that file: a `.sup` file (the Blu-ray's own
//! format, which Kodi reads next to a stream), or the pictures themselves,
//! placed and timed ([`PgsPictures`]).
//!
//! A Matroska block of a PGS track is a display set's segments one after
//! the other (type, 16-bit size, data); a `.sup` file has the same segments,
//! each after a header of its own: `PG`, its presentation and decoding
//! times (90 kHz).

use std::collections::HashMap;

use super::mks::{self, Track};
use super::{MAX_SIDE, Picture, PictureCue, end_ms, so_far, union, ycbcr_to_rgb};

/// The `.sup` file of the PGS track in the Matroska file `mks`, with an
/// empty display set at 0 first: a reader that takes a file's first time
/// for its start (FFmpeg does) starts it where the video does. `None` for
/// anything else.
pub fn from_matroska(mks: &[u8]) -> Option<Vec<u8>> {
    let track = mks::read(mks).filter(is_pgs)?;
    let mut out = Vec::with_capacity(track.blocks.iter().map(|b| b.data.len() + 64).sum());
    let mut first = true;
    for block in &track.blocks {
        // Nanoseconds to 90 kHz ticks: 90 a millisecond.
        let pts = (i128::from(block.ticks) * i128::from(track.scale_ns) * 90 / 1_000_000).clamp(0, 0xFFFF_FFFF) as u32;
        let data = block.data;
        let mut at = 0;
        while at + 3 <= data.len() {
            let len = usize::from(u16::from_be_bytes([data[at + 1], data[at + 2]]));
            let Some(segment) = data.get(at..at + 3 + len) else { break };
            // The presentation composition of the first display set, empty, at 0.
            if first && segment[0] == PCS && len >= 11 {
                first = false;
                if pts > 0 {
                    let mut empty = segment[..3 + 11].to_vec();
                    empty[1..3].copy_from_slice(&11u16.to_be_bytes());
                    // Composition 0, an epoch start, no palette update, no object.
                    empty[8..10].copy_from_slice(&[0, 0]);
                    empty[10] = 0x80;
                    empty[11] = 0;
                    empty[13] = 0;
                    push(&mut out, 0, &empty);
                    push(&mut out, 0, &[END, 0, 0]);
                }
            }
            push(&mut out, pts, segment);
            at += 3 + len;
        }
    }
    Some(out)
}

fn is_pgs(track: &Track) -> bool {
    track.codec_id.starts_with(b"S_HDMV/PGS")
}

// ---------------------------------------------------------------------------
// Pictures, for players that can't decode PGS

/// Segment types: palette definition, object definition.
const PDS: u8 = 0x14;
const ODS: u8 = 0x15;
/// A composition's states that hold everything they need: an acquisition
/// point, an epoch start.
const FRESH_STATES: u8 = 0xC0;
/// A composition object's flag: cropping follows its place.
const CROPPED: u8 = 0x80;
/// The most objects a composition places.
const MAX_PLACED: u8 = 2;

/// A PGS track's pictures, for a player that shows them without decoding
/// PGS: when each shows and where ([`PgsPictures::cues`]), and each picture
/// ([`PgsPictures::picture`]). An epoch start or acquisition point holds
/// everything it needs; other display sets reuse the palettes and objects
/// defined since, as the apps' decoders read them
/// (packages/picture-subtitles/src/pgs.ts).
pub struct PgsPictures<'a> {
    /// The plane the pictures are placed on: the first composition's.
    pub width: u32,
    pub height: u32,
    /// In order of time.
    sets: Vec<DisplaySet<'a>>,
}

struct DisplaySet<'a> {
    time_ms: i64,
    /// Its block's own duration (ms), when it says.
    duration_ms: Option<i64>,
    composition: Option<Composition>,
    /// Palettes defined: id, and entries (index, Y, Cr, Cb, alpha).
    palettes: Vec<(u8, Vec<[u8; 5]>)>,
    objects: Vec<ObjectPart<'a>>,
}

struct Composition {
    width: u32,
    height: u32,
    state: u8,
    palette: u8,
    placed: Vec<Placed>,
}

/// An object placed on the plane, and the part of it shown (x, y, width,
/// height) when cropped.
struct Placed {
    id: u16,
    x: u32,
    y: u32,
    crop: Option<(u32, u32, u32, u32)>,
}

/// An object definition, or a part of one: the first part says its size.
struct ObjectPart<'a> {
    id: u16,
    first: bool,
    last: bool,
    width: u32,
    height: u32,
    data: &'a [u8],
}

/// An object: its size, and its run-length data in the parts it came in.
#[derive(Default)]
struct Object<'a> {
    width: u32,
    height: u32,
    parts: Vec<&'a [u8]>,
}

/// An object as a composition shows it: the box it takes on the plane (x,
/// y, width, height), and where in the object that box starts (cropped).
struct Shown<'e, 'a> {
    area: (u32, u32, u32, u32),
    from: (u32, u32),
    object: &'e Object<'a>,
}

/// The palettes and objects defined so far in an epoch.
#[derive(Default)]
struct Epoch<'a> {
    palettes: HashMap<u8, HashMap<u8, [u8; 4]>>,
    objects: HashMap<u16, Object<'a>>,
    /// Objects whose last part hasn't come yet.
    pending: HashMap<u16, Object<'a>>,
}

impl<'a> Epoch<'a> {
    /// Takes in what `set` defines: everything anew from an epoch start or
    /// acquisition point.
    fn take(&mut self, set: &DisplaySet<'a>) {
        if set.composition.as_ref().is_some_and(|c| c.state & FRESH_STATES != 0) {
            *self = Epoch::default();
        }
        for (id, entries) in &set.palettes {
            let palette = self.palettes.entry(*id).or_default();
            for &[index, y, cr, cb, a] in entries {
                palette.insert(index, [y, cr, cb, a]);
            }
        }
        for part in &set.objects {
            if part.first {
                self.pending.insert(part.id, Object { width: part.width, height: part.height, parts: Vec::new() });
            }
            let Some(pending) = self.pending.get_mut(&part.id) else { continue };
            pending.parts.push(part.data);
            if part.last
                && let Some(whole) = self.pending.remove(&part.id)
            {
                self.objects.insert(part.id, whole);
            }
        }
    }

    /// The objects `composition` places that are defined, as it shows them:
    /// within bounds, and cut to its plane, as players draw them.
    fn shown(&self, composition: &Composition) -> Vec<Shown<'_, 'a>> {
        // A plane of no size cuts nothing but what is past the largest.
        let (plane_width, plane_height) = match (composition.width, composition.height) {
            (w, h) if w > 0 && h > 0 => (w.min(MAX_SIDE), h.min(MAX_SIDE)),
            _ => (MAX_SIDE, MAX_SIDE),
        };
        let mut shown = Vec::new();
        for placed in &composition.placed {
            let Some(object) = self.objects.get(&placed.id) else { continue };
            if object.width > MAX_SIDE || object.height > MAX_SIDE {
                continue;
            }
            let (cx, cy, cw, ch) = placed.crop.unwrap_or((0, 0, object.width, object.height));
            let w = cw.min(object.width.saturating_sub(cx)).min(plane_width.saturating_sub(placed.x));
            let h = ch.min(object.height.saturating_sub(cy)).min(plane_height.saturating_sub(placed.y));
            if w > 0 && h > 0 {
                shown.push(Shown { area: (placed.x, placed.y, w, h), from: (cx, cy), object });
            }
        }
        shown
    }
}

impl<'a> PgsPictures<'a> {
    /// The display sets of a PGS track.
    pub(super) fn of(track: &Track<'a>) -> Self {
        let sets: Vec<DisplaySet> =
            track.samples().into_iter().map(|s| display_set(s.data, s.time_ms, s.duration_ms)).collect();
        let plane = sets.iter().find_map(|s| s.composition.as_ref().map(|c| (c.width, c.height)));
        let (width, height) = plane.filter(|&(w, h)| w > 0 && h > 0).unwrap_or((1920, 1080));
        Self { width, height, sets }
    }

    /// When each picture shows and where, in order of time: from its
    /// display set's time until the next display set's (or its block's own
    /// duration), as players show them. Display sets that place nothing
    /// clear the picture before.
    pub fn cues(&self) -> Vec<PictureCue> {
        let mut cues = Vec::new();
        let mut epoch = Epoch::default();
        for (i, set) in self.sets.iter().enumerate() {
            epoch.take(set);
            let Some(composition) = &set.composition else { continue };
            // Without its palette, nothing to draw ([`PgsPictures::picture`]).
            if !epoch.palettes.contains_key(&composition.palette) {
                continue;
            }
            let Some((x, y, width, height)) = union(epoch.shown(composition).iter().map(|s| s.area)) else {
                continue;
            };
            let end_ms = self.end_of(i);
            if end_ms > set.time_ms {
                cues.push(PictureCue { start_ms: set.time_ms, end_ms, x, y, width, height });
            }
        }
        cues
    }

    /// [`PgsPictures::cues`] of a track copied only so far
    /// ([`super::PictureTrack::cues_so_far`]).
    pub fn cues_so_far(&self) -> Vec<PictureCue> {
        let last = self.sets.last().map(|last| (last.time_ms, last.duration_ms.is_some_and(|own| own > 0)));
        so_far(self.cues(), last)
    }

    /// The picture that shows from `start_ms` (a cue's start), or `None`.
    pub fn picture(&self, start_ms: i64) -> Option<Picture> {
        let at = self.sets.iter().rposition(|s| s.time_ms == start_ms)?;
        let composition = self.sets[at].composition.as_ref()?;
        // Palettes and objects as they stand there: from the last epoch
        // start or acquisition point.
        let from = self.sets[..=at]
            .iter()
            .rposition(|s| s.composition.as_ref().is_some_and(|c| c.state & FRESH_STATES != 0))
            .unwrap_or(0);
        let mut epoch = Epoch::default();
        for set in &self.sets[from..=at] {
            epoch.take(set);
        }
        let palette = epoch.palettes.get(&composition.palette)?;
        let hd = composition.height > 576;
        let mut colors = [[0u8; 4]; 256];
        for (&index, &[y, cr, cb, a]) in palette {
            let [r, g, b] = ycbcr_to_rgb(y, cb, cr, hd);
            colors[usize::from(index)] = [r, g, b, a];
        }
        let shown = epoch.shown(composition);
        let (ux, uy, width, height) = union(shown.iter().map(|s| s.area))?;
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        for Shown { area: (x, y, w, h), from: (cx, cy), object } in shown {
            let indexes = decode_rle(&object.parts.concat(), object.width, object.height);
            for row in 0..h {
                let src = ((cy + row) * object.width + cx) as usize;
                let dst = (((y - uy + row) * width + (x - ux)) * 4) as usize;
                for col in 0..w as usize {
                    let color = colors[usize::from(indexes[src + col])];
                    rgba[dst + col * 4..dst + col * 4 + 4].copy_from_slice(&color);
                }
            }
        }
        Some(Picture { width, height, rgba })
    }

    /// When display set `i` stops showing: at the next one, or after its
    /// block's own duration when that is sooner.
    fn end_of(&self, i: usize) -> i64 {
        end_ms(self.sets[i].time_ms, self.sets.get(i + 1).map(|s| s.time_ms), self.sets[i].duration_ms)
    }
}

/// A display set's segments (a `.sup` file's `PG` headers skipped, which
/// some muxers keep).
fn display_set(data: &[u8], time_ms: i64, duration_ms: Option<i64>) -> DisplaySet<'_> {
    let mut set = DisplaySet { time_ms, duration_ms, composition: None, palettes: Vec::new(), objects: Vec::new() };
    let mut at = 0;
    while at + 3 <= data.len() {
        if data[at..].starts_with(b"PG") {
            at += 10;
            if at + 3 > data.len() {
                break;
            }
        }
        let kind = data[at];
        let size = usize::from(u16::from_be_bytes([data[at + 1], data[at + 2]]));
        let body = &data[at + 3..data.len().min(at + 3 + size)];
        at += 3 + size;
        match kind {
            PCS => set.composition = composition(body),
            PDS if body.len() >= 2 => {
                let entries = body[2..].as_chunks::<5>().0.to_vec();
                set.palettes.push((body[0], entries));
            }
            ODS if body.len() >= 4 => {
                let id = u16::from_be_bytes([body[0], body[1]]);
                let (first, last) = (body[3] & 0x80 != 0, body[3] & 0x40 != 0);
                if first && body.len() >= 11 {
                    let width = u32::from(u16::from_be_bytes([body[7], body[8]]));
                    let height = u32::from(u16::from_be_bytes([body[9], body[10]]));
                    set.objects.push(ObjectPart { id, first, last, width, height, data: &body[11..] });
                } else if !first {
                    set.objects.push(ObjectPart { id, first, last, width: 0, height: 0, data: &body[4..] });
                }
            }
            END => break,
            _ => {}
        }
    }
    set
}

/// A presentation composition: the plane's size, its state, its palette and
/// the objects it places (as many as are whole). `None` when it is cut
/// short before them, or its plane is out of bounds.
fn composition(b: &[u8]) -> Option<Composition> {
    if b.len() < 11 {
        return None;
    }
    let width = u32::from(u16::from_be_bytes([b[0], b[1]]));
    let height = u32::from(u16::from_be_bytes([b[2], b[3]]));
    if width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    // Blu-ray places two objects at most, as FFmpeg reads them.
    let (state, palette, count) = (b[7], b[9], b[10].min(MAX_PLACED));
    let mut placed = Vec::with_capacity(usize::from(count));
    let mut at = 11;
    for _ in 0..count {
        let Some(object) = b.get(at..at + 8) else { break };
        let id = u16::from_be_bytes([object[0], object[1]]);
        let x = u32::from(u16::from_be_bytes([object[4], object[5]]));
        let y = u32::from(u16::from_be_bytes([object[6], object[7]]));
        at += 8;
        let mut crop = None;
        if object[3] & CROPPED != 0 {
            let Some(c) = b.get(at..at + 8) else { break };
            at += 8;
            let read = |i: usize| u32::from(u16::from_be_bytes([c[i], c[i + 1]]));
            crop = Some((read(0), read(2), read(4), read(6)));
        }
        placed.push(Placed { id, x, y, crop });
    }
    Some(Composition { width, height, state, palette, placed })
}

/// PGS run-length data as palette indexes, `width × height`, left to right
/// and top to bottom.
fn decode_rle(rle: &[u8], width: u32, height: u32) -> Vec<u8> {
    let (width, height) = (width.min(MAX_SIDE) as usize, height.min(MAX_SIDE) as usize);
    let mut out = vec![0u8; width * height];
    let (mut x, mut y, mut i) = (0usize, 0usize, 0usize);
    let mut next = || {
        let b = rle.get(i).copied();
        i += 1;
        b
    };
    while y < height {
        let Some(b) = next() else { break };
        let (color, count) = if b != 0 {
            (b, 1)
        } else {
            let flags = next().unwrap_or(0);
            if flags == 0 {
                // The end of a line.
                (x, y) = (0, y + 1);
                continue;
            }
            let count = if flags & 0x40 != 0 {
                (usize::from(flags & 0x3F) << 8) | usize::from(next().unwrap_or(0))
            } else {
                usize::from(flags & 0x3F)
            };
            let color = if flags & 0x80 != 0 { next().unwrap_or(0) } else { 0 };
            (color, count)
        };
        let end = width.min(x + count);
        if color != 0 {
            out[y * width + x..y * width + end].fill(color);
        }
        x = end;
    }
    out
}

/// Segment types: presentation composition, end of display set.
const PCS: u8 = 0x16;
const END: u8 = 0x80;

fn push(out: &mut Vec<u8>, pts: u32, segment: &[u8]) {
    out.extend_from_slice(b"PG");
    out.extend_from_slice(&pts.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(segment);
}

#[cfg(test)]
mod tests {
    use super::super::mks::testing::{Cluster, lasting, mks_of};
    use super::*;

    /// A display set: a PCS of 1920×1080 with one object, an END.
    fn display_set(composition: u8) -> Vec<u8> {
        let pcs = [0x16, 0, 19, 0x07, 0x80, 0x04, 0x38, 0x10, 0, composition, 0x80, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        [&pcs[..], &[0x80, 0, 0]].concat()
    }

    fn mks(clusters: &[Cluster], unknown_cluster: bool) -> Vec<u8> {
        mks_of(b"S_HDMV/PGS", None, clusters, unknown_cluster)
    }

    /// The pictures of a PGS track's file.
    fn read(mks: &[u8]) -> Option<PgsPictures<'_>> {
        match super::super::PictureTrack::read(mks)? {
            super::super::PictureTrack::Pgs(pictures) => Some(pictures),
            _ => None,
        }
    }

    /// The `.sup` segments: (pts, type, size).
    fn segments(sup: &[u8]) -> Vec<(u32, u8, u16)> {
        let mut found = Vec::new();
        let mut at = 0;
        while at < sup.len() {
            assert_eq!(&sup[at..at + 2], b"PG");
            let pts = u32::from_be_bytes(sup[at + 2..at + 6].try_into().unwrap());
            let size = u16::from_be_bytes([sup[at + 11], sup[at + 12]]);
            found.push((pts, sup[at + 10], size));
            at += 13 + usize::from(size);
        }
        found
    }

    #[test]
    fn matroska_blocks_become_sup_segments() {
        let file =
            mks(&[(95_000, &[(0, display_set(1)), (1_600, display_set(2))]), (99_000, &[(0, display_set(3))])], false);
        let sup = from_matroska(&file).unwrap();
        assert_eq!(
            segments(&sup),
            [
                // Empty, at 0: the file starts where the video does.
                (0, 0x16, 11),
                (0, 0x80, 0),
                (95_000 * 90, 0x16, 19),
                (95_000 * 90, 0x80, 0),
                (96_600 * 90, 0x16, 19),
                (96_600 * 90, 0x80, 0),
                (99_000 * 90, 0x16, 19),
                (99_000 * 90, 0x80, 0),
            ]
        );
        // The empty one: the first's size, an epoch start, no object.
        // After the first header (PG, PTS, DTS: 10 bytes).
        assert_eq!(&sup[10..10 + 3 + 11], &[0x16, 0, 11, 0x07, 0x80, 0x04, 0x38, 0x10, 0, 0, 0x80, 0, 0, 0]);
        // A cluster still being written (its size unknown) counts too.
        let file = mks(&[(95_000, &[(0, display_set(1))]), (99_000, &[(0, display_set(3))])], true);
        assert_eq!(segments(&from_matroska(&file).unwrap()).len(), 6);
    }

    #[test]
    fn only_pgs_matroska_files_are_made_sup() {
        assert_eq!(from_matroska(b"not matroska"), None);
        let vobsub = mks_of(b"S_VOBSUB", None, &[(0, &[(0, vec![1, 2, 3])])], false);
        assert_eq!(from_matroska(&vobsub), None);
        // No block yet: an empty file.
        assert_eq!(from_matroska(&mks(&[], false)), Some(Vec::new()));
    }

    fn segment(kind: u8, body: &[u8]) -> Vec<u8> {
        [&[kind][..], &(body.len() as u16).to_be_bytes(), body].concat()
    }

    /// An object placed: id, flags, x, y, and crop (x, y, width, height).
    type Placement = (u16, u8, u16, u16, Option<[u16; 4]>);

    /// A composition of a 1920×1080 plane in `state` (0x80 an epoch start,
    /// 0 a normal one) with palette 0, placing `objects`.
    fn pcs(state: u8, objects: &[Placement]) -> Vec<u8> {
        let mut body = vec![0x07, 0x80, 0x04, 0x38, 0x10, 0, 1, state, 0, 0, objects.len() as u8];
        for &(id, flags, x, y, crop) in objects {
            body.extend(id.to_be_bytes());
            body.extend([0, flags]);
            body.extend(x.to_be_bytes());
            body.extend(y.to_be_bytes());
            for v in crop.into_iter().flatten() {
                body.extend(v.to_be_bytes());
            }
        }
        segment(PCS, &body)
    }

    /// Palette 0 with these entries: (index, Y, Cr, Cb, alpha).
    fn pds(entries: &[[u8; 5]]) -> Vec<u8> {
        segment(PDS, &[&[0u8, 0][..], &entries.concat()].concat())
    }

    /// Object `id`, `width`×`height`, every pixel of palette index `color`.
    fn ods(id: u16, width: u16, height: u16, color: u8) -> Vec<u8> {
        let mut rle = Vec::new();
        for _ in 0..height {
            rle.extend([0, 0xC0 | (width >> 8) as u8, width as u8, color, 0, 0]);
        }
        let mut body = id.to_be_bytes().to_vec();
        body.extend([0, 0xC0]);
        body.extend(((rle.len() + 4) as u32).to_be_bytes()[1..].iter());
        body.extend(width.to_be_bytes());
        body.extend(height.to_be_bytes());
        body.extend(rle);
        segment(ODS, &body)
    }

    fn set(parts: &[Vec<u8>]) -> Vec<u8> {
        [parts.concat(), segment(END, &[])].concat()
    }

    /// White and red, opaque, as Blu-rays' palettes say them (BT.709).
    const WHITE: [u8; 5] = [1, 235, 128, 128, 255];
    const RED: [u8; 5] = [2, 63, 240, 102, 255];

    #[test]
    fn pictures_show_where_and_while_their_display_sets_say() {
        let file = mks(
            &[
                (1_000, &[(0, set(&[pcs(0x80, &[(0, 0, 100, 900, None)]), pds(&[WHITE]), ods(0, 4, 2, 1)]))]),
                (3_000, &[(0, set(&[pcs(0, &[])]))]),
                // The object again, red now, elsewhere: a normal display set
                // reuses what its epoch defined.
                (5_000, &[(0, set(&[pcs(0, &[(0, 0, 200, 800, None)]), pds(&[[1, 63, 240, 102, 255]])]))]),
                (6_000, &[(0, set(&[pcs(0, &[])]))]),
            ],
            false,
        );
        let pictures = read(&file).unwrap();
        assert_eq!((pictures.width, pictures.height), (1920, 1080));
        let cue = |start_ms, end_ms, x, y| PictureCue { start_ms, end_ms, x, y, width: 4, height: 2 };
        assert_eq!(pictures.cues(), [cue(1_000, 3_000, 100, 900), cue(5_000, 6_000, 200, 800)]);
        let white = pictures.picture(1_000).unwrap();
        assert_eq!((white.width, white.height), (4, 2));
        assert!(white.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]), "{:?}", white.rgba);
        let red = pictures.picture(5_000).unwrap();
        assert!(red.rgba.chunks(4).all(|p| p[0] > 200 && p[1] < 30 && p[2] < 30 && p[3] == 255), "{:?}", red.rgba);
        // Clears, and times nothing starts at, have no picture.
        assert_eq!((pictures.picture(3_000), pictures.picture(4_000)), (None, None));
    }

    #[test]
    fn pictures_take_in_several_objects_cropped_or_forced() {
        let file = mks(
            &[(
                1_000,
                &[(
                    0,
                    set(&[
                        // A forced object (0x40, no crop to read), then one
                        // cropped (0x80) to its right half.
                        pcs(0x80, &[(0, 0x40, 100, 900, None), (1, 0x80, 600, 950, Some([2, 0, 2, 2]))]),
                        pds(&[WHITE, RED]),
                        ods(0, 4, 2, 1),
                        ods(1, 4, 2, 2),
                    ]),
                )],
            )],
            false,
        );
        let pictures = read(&file).unwrap();
        // Both in one picture, the box that holds them, until 10 s on (no
        // display set after).
        let cue = PictureCue { start_ms: 1_000, end_ms: 11_000, x: 100, y: 900, width: 502, height: 52 };
        assert_eq!(pictures.cues(), [cue]);
        let picture = pictures.picture(1_000).unwrap();
        let pixel = |x: usize, y: usize| &picture.rgba[(y * 502 + x) * 4..(y * 502 + x) * 4 + 4];
        assert_eq!((pixel(0, 0), pixel(3, 1)), (&[255, 255, 255, 255][..], &[255, 255, 255, 255][..]));
        assert_eq!(pixel(4, 0), [0, 0, 0, 0], "between them, nothing");
        assert_eq!(pixel(500, 50)[3], 255);
        assert_eq!(pixel(501, 51)[3], 255);
    }

    #[test]
    fn a_blocks_own_duration_ends_its_picture_sooner() {
        // A cluster at 1 s holding a BlockGroup that lasts 1.5 s.
        let data = set(&[pcs(0x80, &[(0, 0, 0, 0, None)]), pds(&[WHITE]), ods(0, 2, 2, 1)]);
        let file = [mks(&[], false), lasting(1_000, 1_500, &data)].concat();
        let pictures = read(&file).unwrap();
        assert_eq!(pictures.cues().iter().map(|c| (c.start_ms, c.end_ms)).collect::<Vec<_>>(), [(1_000, 2_500)]);
    }

    #[test]
    fn objects_out_of_bounds_or_undefined_show_nothing() {
        let file = mks(
            &[
                // An object placed off its plane, and one never defined.
                (
                    1_000,
                    &[(
                        0,
                        set(&[
                            pcs(0x80, &[(0, 0, 4000, 0, None), (7, 0, 0, 0, None)]),
                            pds(&[WHITE]),
                            ods(0, 200, 2, 1),
                        ]),
                    )],
                ),
            ],
            false,
        );
        let pictures = read(&file).unwrap();
        assert!(pictures.cues().is_empty());
        assert_eq!(pictures.picture(1_000), None);
        assert!(read(b"not matroska").is_none());
        // No palette: nothing to draw, so no cue either.
        let file = mks(&[(1_000, &[(0, set(&[pcs(0x80, &[(0, 0, 0, 0, None)]), ods(0, 2, 2, 1)]))])], false);
        let pictures = read(&file).unwrap();
        assert_eq!((pictures.cues(), pictures.picture(1_000)), (Vec::new(), None));
    }

    #[test]
    fn pictures_are_cut_to_their_plane() {
        // A 200×4 object placed 100 px from the plane's right edge and 2 from
        // its bottom: the part on the plane shows.
        let file = mks(
            &[(1_000, &[(0, set(&[pcs(0x80, &[(0, 0, 1820, 1078, None)]), pds(&[WHITE]), ods(0, 200, 4, 1)]))])],
            false,
        );
        let pictures = read(&file).unwrap();
        let cue = PictureCue { start_ms: 1_000, end_ms: 11_000, x: 1820, y: 1078, width: 100, height: 2 };
        assert_eq!(pictures.cues(), [cue]);
        let picture = pictures.picture(1_000).unwrap();
        assert_eq!((picture.width, picture.height, picture.rgba.len()), (100, 2, 100 * 2 * 4));
        assert!(picture.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
    }

    #[test]
    fn a_track_copied_so_far_leaves_out_a_last_picture_nothing_ends() {
        let shown = || set(&[pcs(0x80, &[(0, 0, 100, 900, None)]), pds(&[WHITE]), ods(0, 4, 2, 1)]);
        let file =
            mks(&[(1_000, &[(0, shown())]), (3_000, &[(0, set(&[pcs(0, &[])]))]), (5_000, &[(0, shown())])], false);
        let pictures = read(&file).unwrap();
        let times = |cues: Vec<PictureCue>| cues.iter().map(|c| (c.start_ms, c.end_ms)).collect::<Vec<_>>();
        // Whole, the last picture shows 10 s; copied so far, what ends it may
        // be copied later.
        assert_eq!(times(pictures.cues()), [(1_000, 3_000), (5_000, 15_000)]);
        assert_eq!(times(pictures.cues_so_far()), [(1_000, 3_000)]);
    }

    #[test]
    fn a_composition_places_two_objects_at_most() {
        let placed = [(0, 0, 0, 0, None), (1, 0, 10, 0, None), (2, 0, 1000, 0, None)];
        let file = mks(
            &[(
                1_000,
                &[(0, set(&[pcs(0x80, &placed), pds(&[WHITE]), ods(0, 2, 2, 1), ods(1, 2, 2, 1), ods(2, 2, 2, 1)]))],
            )],
            false,
        );
        let pictures = read(&file).unwrap();
        assert_eq!(pictures.cues()[0].width, 12, "the third object, far right, left out");
    }

    #[test]
    fn run_lengths_decode_as_pgs_writes_them() {
        // A pixel of 3; 2 transparent; 5 of 7 (short run); end of line; 70
        // of 9 (long run), cut at the width.
        let rle = [3, 0, 0x02, 0, 0x85, 7, 0, 0, 0, 0xC0, 70, 9, 0, 0];
        let out = decode_rle(&rle, 8, 2);
        assert_eq!(&out[..8], &[3, 0, 0, 7, 7, 7, 7, 7]);
        assert_eq!(&out[8..], &[9; 8]);
        // Cut short: what came, the rest transparent.
        assert_eq!(decode_rle(&[5, 5], 4, 1), [5, 5, 0, 0]);
    }
}
