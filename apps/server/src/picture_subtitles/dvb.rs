//! Broadcast subtitles (DVB, ETSI EN 300 743, `S_DVBSUB`) as pictures. Each
//! block is a display set of segments. A page composition places regions on
//! the display; each region has a colour table (CLUT) and objects drawn into
//! it as 2, 4 or 8-bit run-length coded pixels, a field at a time. A page in
//! "normal case" only updates what an acquisition point or mode change
//! defined, so a picture is decoded from the last of those. Read as the
//! apps' decoder reads them (packages/picture-subtitles/src/dvb.ts).

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use super::mks::{Sample, Track};
use super::{MAX_PIXELS, MAX_SIDE, Picture, PictureCue, end_ms, on_plane, so_far, union, ycbcr_to_rgb};

/// Segment types: page composition, region composition, CLUT definition,
/// object data, display definition.
const PAGE: u8 = 0x10;
const REGION: u8 = 0x11;
const CLUT: u8 = 0x12;
const OBJECT: u8 = 0x13;
const DISPLAY: u8 = 0x14;

/// The plane when the track has no display definition: SD's.
const DEFAULT_PLANE: (u32, u32) = (720, 576);
/// The segments read of a display set: a real one has a handful.
const MAX_SEGMENTS: usize = 256;
/// The objects a region lists at most: a real one has a few, one for each
/// line of text.
const MAX_REGION_OBJECTS: usize = 64;
/// The regions a page places at most: a real one places a few.
const MAX_PLACED: usize = 32;
/// The most display sets a picture is decoded from, back to the last
/// acquisition point or mode change: without that, each picture could read
/// the whole track.
const MAX_EPOCH: usize = 4096;

/// The most work a picture takes to decode, in pixels filled or drawn, a
/// byte read costing as many pixels as it takes as long: eight 4K screens'
/// worth, where a real one takes a fraction of one. A crafted track could
/// fill a 4K region, or draw an object into thousands of places, in every
/// segment of every display set; a picture that would take more isn't
/// decoded.
const MAX_WORK: i64 = 8 * MAX_PIXELS as i64;
/// What reading a byte costs, next to filling a pixel: a segment's (a
/// colour table's entries) about 16 times as long, and a pixel string's,
/// walked a bit at a time, about 32. A segment costs 512 besides.
const READ_COST: i64 = 16;
const WALK_COST: i64 = 32;
const SEGMENT_COST: i64 = 512;
/// What drawing an object into a place costs, besides its bytes and pixels;
/// looking through a region for an object's places costs 32, and each place
/// it lists 32 more.
const DRAW_COST: i64 = 4096;
const SCAN_COST: i64 = 32;
/// The work the cues of a track take at most, besides [`MAX_WORK`], for
/// each byte of the track: they draw no pixels, so a real track's take a
/// fraction of it.
const CUES_WORK_PER_BYTE: i64 = 256;

/// The default map tables of 2-bit codes to 4 and 8 bits, and of 4-bit
/// codes to 8, for codes shallower than their region.
const MAP24: [u8; 4] = [0x0, 0x7, 0x8, 0xF];
const MAP28: [u8; 4] = [0x00, 0x77, 0x88, 0xFF];
const MAP48: [u8; 16] =
    [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

/// A DVB track's pictures: each display set shows from its time until the
/// next one's, or until its page's time-out when that is sooner.
pub struct DvbPictures<'a> {
    /// The plane the cues are placed on: the track's first display
    /// definition's. A picture is decoded on its own display set's plane
    /// (the last display definition since its acquisition point or mode
    /// change, else SD's), as the apps' decoder does, and its cue placed at
    /// the same fractions of this one.
    pub width: u32,
    pub height: u32,
    /// In order of time.
    samples: Vec<Sample<'a>>,
}

impl<'a> DvbPictures<'a> {
    /// The display sets of a DVB track.
    pub(super) fn of(track: &Track<'a>) -> Self {
        Self::new(track.samples())
    }

    fn new(samples: Vec<Sample<'a>>) -> Self {
        let display = samples.iter().flat_map(|s| segments(s.data)).find(|&(kind, b)| kind == DISPLAY && b.len() >= 5);
        let (width, height) = display.and_then(|(_, b)| display_plane(b)).unwrap_or(DEFAULT_PLANE);
        Self { width, height, samples }
    }

    /// When each picture shows and where, in order of time: the regions its
    /// page places that show something, cut to its plane, at the same
    /// fractions of the track's. Read in one pass that draws no pixels, only
    /// notes which values each region holds, within a budget of work for the
    /// track's size.
    pub fn cues(&self) -> Vec<PictureCue> {
        let bytes = self.samples.iter().map(|s| s.data.len() as i64).sum::<i64>();
        let budget = MAX_WORK.saturating_add(bytes.saturating_mul(CUES_WORK_PER_BYTE));
        let mut decoder = Decoder::new(false, budget);
        let mut cues = Vec::new();
        for (i, sample) in self.samples.iter().enumerate() {
            if page_head(sample.data).is_some_and(|(_, state)| state != 0) {
                decoder.start_epoch();
            }
            decoder.take(sample.data);
            if decoder.work <= 0 {
                break;
            }
            let plane = decoder.plane;
            let Some(area) = union(decoder.shown(plane).iter().map(|(area, _)| *area)) else {
                continue;
            };
            let (x, y, width, height) = onto(area, plane, self.plane());
            let end_ms = self.end_of(i);
            if end_ms > sample.time_ms {
                cues.push(PictureCue { start_ms: sample.time_ms, end_ms, x, y, width, height });
            }
        }
        cues
    }

    /// [`DvbPictures::cues`] of a track copied only so far
    /// ([`super::PictureTrack::cues_so_far`]).
    pub fn cues_so_far(&self) -> Vec<PictureCue> {
        let last = self.samples.last().map(|last| (last.time_ms, own_ms(last).is_some_and(|own| own > 0)));
        so_far(self.cues(), last)
    }

    /// The picture that shows from `start_ms` (a cue's start), or `None`:
    /// decoded from the last acquisition point or mode change, [`MAX_EPOCH`]
    /// display sets back at most, on its own plane. Regions placed over one
    /// another are drawn in the page's order, the later on top where it
    /// shows.
    pub fn picture(&self, start_ms: i64) -> Option<Picture> {
        let index = self.samples.iter().rposition(|s| s.time_ms == start_ms)?;
        let mut from = index;
        while from > 0
            && index - from < MAX_EPOCH
            && !page_head(self.samples[from].data).is_some_and(|(_, state)| state != 0)
        {
            from -= 1;
        }
        let mut decoder = Decoder::new(true, MAX_WORK);
        for sample in &self.samples[from..=index] {
            decoder.take(sample.data);
            if decoder.work <= 0 {
                return None;
            }
        }
        let shown = decoder.shown(decoder.plane);
        let (ux, uy, width, height) = union(shown.iter().map(|(area, _)| *area))?;
        let mut rgba = vec![0u8; width as usize * height as usize * 4];
        for &((x, y, w, h), region) in &shown {
            let colors = decoder.colors(region);
            for row in 0..h {
                let src = (row * region.width) as usize;
                let dst = (((y - uy + row) * width + (x - ux)) * 4) as usize;
                for col in 0..w as usize {
                    let color = colors.get(usize::from(region.pixels[src + col])).copied().unwrap_or_default();
                    if color[3] > 0 {
                        rgba[dst + col * 4..dst + col * 4 + 4].copy_from_slice(&color);
                    }
                }
            }
        }
        Some(Picture { width, height, rgba })
    }

    fn plane(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// When display set `i` stops showing: at the next one, or after its
    /// own duration when that is sooner.
    fn end_of(&self, i: usize) -> i64 {
        let sample = &self.samples[i];
        end_ms(sample.time_ms, self.samples.get(i + 1).map(|s| s.time_ms), own_ms(sample))
    }
}

/// How long a display set says it shows: its block's duration, else its
/// page's time-out.
fn own_ms(sample: &Sample) -> Option<i64> {
    sample
        .duration_ms
        .or_else(|| page_head(sample.data).map(|(timeout_s, _)| i64::from(timeout_s) * 1000).filter(|&ms| ms > 0))
}

/// The plane a display definition's body says (its width and height less
/// one), or `None` past [`MAX_SIDE`].
fn display_plane(b: &[u8]) -> Option<(u32, u32)> {
    let side = |hi: u8, lo: u8| u32::from(u16::from_be_bytes([hi, lo])) + 1;
    let (width, height) = (side(*b.get(1)?, *b.get(2)?), side(*b.get(3)?, *b.get(4)?));
    (width <= MAX_SIDE && height <= MAX_SIDE).then_some((width, height))
}

/// `area` of a plane of `from` at the same fractions of a plane of `to`, a
/// pixel at least and within it.
fn onto((x, y, width, height): (u32, u32, u32, u32), from: (u32, u32), to: (u32, u32)) -> (u32, u32, u32, u32) {
    if from == to {
        return (x, y, width, height);
    }
    // Both planes are MAX_SIDE at most: no product overflows.
    let scale = |v: u32, from: u32, to: u32| (v * to + from / 2) / from;
    let (x0, x1) = (scale(x, from.0, to.0), scale(x + width, from.0, to.0));
    let (y0, y1) = (scale(y, from.1, to.1), scale(y + height, from.1, to.1));
    let (width, height) = ((x1 - x0).max(1), (y1 - y0).max(1));
    (x0.min(to.0 - width), y0.min(to.1 - height), width, height)
}

/// A display set's segments (type, body), [`MAX_SEGMENTS`] at most.
fn segments(data: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    // data_identifier 0x20 and subtitle_stream_id 0x00 open a PES data field.
    let mut at = if data.starts_with(&[0x20, 0x00]) { 2 } else { 0 };
    std::iter::from_fn(move || {
        let head = data.get(at..at + 6).filter(|head| head[0] == 0x0F)?;
        let len = usize::from(u16::from_be_bytes([head[4], head[5]]));
        let body = &data[at + 6..data.len().min(at + 6 + len)];
        at += 6 + len;
        Some((head[1], body))
    })
    .take(MAX_SEGMENTS)
}

/// The time-out (seconds) and state of a display set's first page, read
/// without the rest of it: 0 normal case, 1 acquisition point, 2 mode
/// change.
fn page_head(data: &[u8]) -> Option<(u8, u8)> {
    let (_, b) = segments(data).find(|&(kind, _)| kind == PAGE)?;
    Some((b.first().copied().unwrap_or(0), b.get(1).map_or(0, |flags| (flags >> 2) & 3)))
}

/// A colour table: RGBA for 2, 4 and 8-bit pixels.
#[derive(Clone)]
struct Clut {
    two: [[u8; 4]; 4],
    four: [[u8; 4]; 16],
    eight: [[u8; 4]; 256],
}

impl Clut {
    /// The colours of a region of `depth` bits a pixel.
    fn table(&self, depth: u8) -> &[[u8; 4]] {
        match depth {
            2 => &self.two,
            4 => &self.four,
            _ => &self.eight,
        }
    }
}

/// The colour tables of ETSI EN 300 743 §10, for pixels a CLUT doesn't
/// define.
static DEFAULT_CLUT: LazyLock<Clut> = LazyLock::new(|| {
    let two = [[0, 0, 0, 0], [255, 255, 255, 255], [0, 0, 0, 255], [127, 127, 127, 255]];
    let mut clut = Clut { two, four: [[0; 4]; 16], eight: [[0; 4]; 256] };
    for (i, color) in clut.four.iter_mut().enumerate() {
        let v = if i < 8 { 255 } else { 127 };
        let on = |bit: usize| if i & bit != 0 { v } else { 0 };
        *color = [on(1), on(2), on(4), if i == 0 { 0 } else { 255 }];
    }
    for (i, color) in clut.eight.iter_mut().enumerate() {
        let on = |bit: usize, v: f64| if i & bit != 0 { v } else { 0.0 };
        let (r, g, b) = (on(0x01, 85.0), on(0x02, 85.0), on(0x04, 85.0));
        // Halves rounded to even, as the apps' tables are.
        let rgb = |r: f64, g: f64, b: f64| [r, g, b].map(|c| c.round_ties_even() as u8);
        let ([r, g, b], a) = match (i, i & 0x88) {
            (0..8, _) => (rgb(r * 3.0, g * 3.0, b * 3.0), if i == 0 { 0 } else { 64 }),
            (_, 0x00) => (rgb(r + on(0x10, 170.0), g + on(0x20, 170.0), b + on(0x40, 170.0)), 255),
            (_, 0x08) => (rgb(r + on(0x10, 170.0), g + on(0x20, 170.0), b + on(0x40, 170.0)), 127),
            (_, 0x80) => {
                let half = |c: f64, bit: usize| 127.0 + c / 2.0 + on(bit, 43.0);
                (rgb(half(r, 0x10), half(g, 0x20), half(b, 0x40)), 255)
            }
            _ => (rgb(r / 2.0 + on(0x10, 43.0), g / 2.0 + on(0x20, 43.0), b / 2.0 + on(0x40, 43.0)), 255),
        };
        *color = [r, g, b, a];
    }
    clut
});

/// A region of the page: its size, its pixels' depth and colour table, the
/// objects it lists (id, x, y), and what it holds.
struct Region {
    width: u32,
    height: u32,
    /// 2, 4 or 8 bits a pixel (0 for a depth the file doesn't define).
    depth: u8,
    clut: u8,
    background: u8,
    objects: Vec<(u16, u32, u32)>,
    /// Its pixels, `width × height`, when drawn ([`Decoder::draw`]).
    pixels: Vec<u8>,
    /// The values its pixels may have: those it was filled or drawn with
    /// since it was made or last filled. It shows something when one of
    /// them has a colour that isn't transparent.
    values: [bool; 256],
}

/// A region as its page shows it: the part of it on the plane (x, y, width,
/// height), and the region.
type Shown<'r> = ((u32, u32, u32, u32), &'r Region);

/// What a track's display sets define, taken in one after the other.
struct Decoder {
    /// Whether pixels are drawn, or only the values each region holds:
    /// enough to tell what shows and where.
    draw: bool,
    /// The plane the page is placed on: the last display definition's, else
    /// SD's.
    plane: (u32, u32),
    /// The regions the page places (id, x, y).
    page: Option<Vec<(u8, u32, u32)>>,
    regions: BTreeMap<u8, Region>,
    /// The pixels of the regions held, all together: a frame's at most.
    held: u64,
    cluts: HashMap<u8, Clut>,
    /// The work left: bytes read, pixels filled and drawn.
    work: i64,
}

impl Decoder {
    fn new(draw: bool, work: i64) -> Self {
        let (page, regions, cluts) = (None, BTreeMap::new(), HashMap::new());
        Self { draw, plane: DEFAULT_PLANE, page, regions, held: 0, cluts, work }
    }

    /// Lets go of everything defined, at an acquisition point or mode
    /// change: it defines everything anew.
    fn start_epoch(&mut self) {
        self.plane = DEFAULT_PLANE;
        self.page = None;
        self.regions.clear();
        self.held = 0;
        self.cluts.clear();
    }

    /// Takes in a display set's segments, as far as the work left goes.
    fn take(&mut self, data: &[u8]) {
        for (kind, b) in segments(data) {
            self.work -= SEGMENT_COST + b.len() as i64 * READ_COST;
            if self.work <= 0 {
                return;
            }
            match kind {
                PAGE => {
                    if b.get(1).is_some_and(|flags| (flags >> 2) & 3 != 0) {
                        self.regions.clear();
                        self.held = 0;
                    }
                    let place = |&[id, _, x1, x2, y1, y2]: &[u8; 6]| {
                        (id, u32::from(u16::from_be_bytes([x1, x2])), u32::from(u16::from_be_bytes([y1, y2])))
                    };
                    let places = b.get(2..).unwrap_or_default().as_chunks::<6>().0;
                    self.page = Some(places.iter().take(MAX_PLACED).map(place).collect());
                }
                REGION => self.region(b),
                CLUT => self.clut(b),
                OBJECT => self.object(b),
                DISPLAY => self.plane = display_plane(b).unwrap_or(self.plane),
                _ => {}
            }
        }
    }

    fn region(&mut self, b: &[u8]) {
        if b.len() < 10 {
            return;
        }
        let id = b[0];
        let fill = b[1] & 0x08 != 0;
        let width = u32::from(u16::from_be_bytes([b[2], b[3]]));
        let height = u32::from(u16::from_be_bytes([b[4], b[5]]));
        if width > MAX_SIDE || height > MAX_SIDE {
            return;
        }
        // Up to 256 regions are kept at once: together, no more than a
        // frame's pixels.
        let pixels = u64::from(width) * u64::from(height);
        let before = self.regions.get(&id).map_or(0, |r| u64::from(r.width) * u64::from(r.height));
        if self.held - before + pixels > MAX_PIXELS {
            return;
        }
        let depth = match (b[6] >> 2) & 7 {
            0 => 0,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 4,
        };
        let background = match depth {
            8 => b[8],
            4 => b[9] >> 4,
            _ => (b[9] >> 2) & 3,
        };
        let mut objects = Vec::new();
        let mut o = 10;
        while o + 6 <= b.len() && objects.len() < MAX_REGION_OBJECTS {
            let x = (u32::from(b[o + 2] & 0x0F) << 8) | u32::from(b[o + 3]);
            let y = (u32::from(b[o + 4] & 0x0F) << 8) | u32::from(b[o + 5]);
            objects.push((u16::from_be_bytes([b[o], b[o + 1]]), x, y));
            // Character objects say their colours in two bytes more.
            o += if matches!(b[o + 2] >> 6, 1 | 2) { 8 } else { 6 };
        }
        let same = self.regions.get(&id).is_some_and(|r| (r.width, r.height, r.depth) == (width, height, depth));
        if !same {
            if self.draw {
                self.work -= pixels as i64;
            }
            self.held = self.held - before + pixels;
            let mut values = [false; 256];
            values[0] = true;
            let pixels = if self.draw { vec![0; pixels as usize] } else { Vec::new() };
            let region = Region { width, height, depth, clut: 0, background: 0, objects: Vec::new(), pixels, values };
            self.regions.insert(id, region);
        }
        let Some(region) = self.regions.get_mut(&id) else { return };
        (region.clut, region.background, region.objects) = (b[7], background, objects);
        if fill {
            if self.draw {
                self.work -= pixels as i64;
                region.pixels.fill(background);
            }
            region.values = [false; 256];
            region.values[usize::from(background)] = true;
        }
    }

    fn clut(&mut self, b: &[u8]) {
        if b.len() < 2 {
            return;
        }
        // BT.709 colours on an HD plane, BT.601 on an SD one, as the plane
        // is when the table comes.
        let hd = self.plane.1 > 576;
        let clut = self.cluts.entry(b[0]).or_insert_with(|| DEFAULT_CLUT.clone());
        let mut at = 2;
        while at + 2 <= b.len() {
            let (entry, flags) = (usize::from(b[at]), b[at + 1]);
            let (y, cr, cb, t) = if flags & 0x01 != 0 {
                // Full range: 8 bits each.
                let Some(e) = b.get(at + 2..at + 6) else { break };
                at += 6;
                (e[0], e[1], e[2], e[3])
            } else {
                // Y in 6 bits, Cr and Cb in 4, T in 2.
                let Some(e) = b.get(at + 2..at + 4) else { break };
                at += 4;
                (e[0] & 0xFC, ((e[0] & 0x03) << 6) | ((e[1] >> 2) & 0x30), (e[1] << 2) & 0xF0, (e[1] & 0x03) * 0x55)
            };
            let [r, g, blue] = ycbcr_to_rgb(y, cb, cr, hd);
            // Y 0 is transparent whatever T says.
            let color = [r, g, blue, if y == 0 { 0 } else { 255 - t }];
            for (flag, table) in [(0x80, &mut clut.two[..]), (0x40, &mut clut.four[..]), (0x20, &mut clut.eight[..])] {
                if flags & flag != 0
                    && let Some(c) = table.get_mut(entry)
                {
                    *c = color;
                }
            }
        }
    }

    fn object(&mut self, b: &[u8]) {
        if b.len() < 7 {
            return;
        }
        let id = u16::from_be_bytes([b[0], b[1]]);
        // Coded as pixels: characters aren't drawn.
        if (b[2] >> 2) & 3 != 0 {
            return;
        }
        let top_len = usize::from(u16::from_be_bytes([b[3], b[4]]));
        let bottom_len = usize::from(u16::from_be_bytes([b[5], b[6]]));
        let field = |from: usize, len: usize| &b[from.min(b.len())..(from + len).min(b.len())];
        let top = field(7, top_len);
        // No bottom field: the top one is drawn on both.
        let bottom = if bottom_len > 0 { field(7 + top_len, bottom_len) } else { top };
        let draw = self.draw;
        for region in self.regions.values_mut() {
            self.work -= (1 + region.objects.len() as i64) * SCAN_COST;
            for k in 0..region.objects.len() {
                if self.work <= 0 {
                    break;
                }
                let (object, x, y) = region.objects[k];
                if object == id {
                    let drawn = draw_field(top, region, x, y, 0, draw) + draw_field(bottom, region, x, y, 1, draw);
                    self.work -= DRAW_COST + drawn;
                }
            }
        }
    }

    /// The colours of `region`'s pixels.
    fn colors(&self, region: &Region) -> &[[u8; 4]] {
        self.cluts.get(&region.clut).unwrap_or(&DEFAULT_CLUT).table(region.depth)
    }

    /// The regions the page places that show something, in its order, on a
    /// plane of `plane`. A page may place a region any number of times, a
    /// frame's pixels in all at most.
    fn shown(&self, plane: (u32, u32)) -> Vec<Shown<'_>> {
        let mut shown = Vec::new();
        let mut pixels_left = MAX_PIXELS as i64;
        for &(id, x, y) in self.page.iter().flatten() {
            let Some(region) = self.regions.get(&id) else { continue };
            if region.width == 0 || region.height == 0 {
                continue;
            }
            pixels_left -= i64::from(region.width) * i64::from(region.height);
            if pixels_left < 0 {
                break;
            }
            let colors = self.colors(region);
            let shows = (0..256).any(|v| region.values[v] && colors.get(v).is_some_and(|c| c[3] > 0));
            if shows && let Some(area) = on_plane((x, y, region.width, region.height), plane) {
                shown.push((area, region));
            }
        }
        shown
    }
}

/// Draws one field of an object's pixel data into `region` at (`ox`, `oy`),
/// every other line from `first`, up to the region's bottom (only noting
/// the values when not `draw`); the work it took, bytes walked and pixels
/// written.
fn draw_field(data: &[u8], region: &mut Region, ox: u32, oy: u32, first: u32, draw: bool) -> i64 {
    let (width, height) = (i64::from(region.width), i64::from(region.height));
    let depth_to = region.depth;
    let byte = |i: usize| data.get(i).copied().unwrap_or(0);
    let mut written = 0i64;
    let mut at = 0;
    let (mut x, mut y) = (i64::from(ox), i64::from(oy) + i64::from(first));
    // Map tables, for codes shallower than the region: the defaults until
    // the data gives its own.
    let (mut map24, mut map28, mut map48) = (MAP24, MAP28, MAP48);
    while at < data.len() && y < height {
        let kind = data[at];
        at += 1;
        let depth = match kind {
            // The end of a line.
            0xF0 => {
                (x, y) = (i64::from(ox), y + 2);
                continue;
            }
            0x20 => {
                map24 = [byte(at) >> 4, byte(at) & 0x0F, byte(at + 1) >> 4, byte(at + 1) & 0x0F];
                at += 2;
                continue;
            }
            0x21 => {
                map28 = [byte(at), byte(at + 1), byte(at + 2), byte(at + 3)];
                at += 4;
                continue;
            }
            0x22 => {
                for i in 0..8 {
                    (map48[i * 2], map48[i * 2 + 1]) = (byte(at + i) >> 4, byte(at + i) & 0x0F);
                }
                at += 8;
                continue;
            }
            0x10 => 2,
            0x11 => 4,
            0x12 => 8,
            _ => break,
        };
        let map = |p: u8| match (depth, depth_to) {
            (from, to) if from == to => p,
            (2, 4) => map24[usize::from(p)],
            (2, _) => map28[usize::from(p)],
            (4, 8) => map48[usize::from(p)],
            // Deeper codes than the region: their low bits.
            (_, to) => p & ((1u16 << to) - 1) as u8,
        };
        let mut bits = Bits { data, at, bit: 0 };
        pixel_string(&mut bits, depth, |count, pixel| {
            if y < height {
                let (start, stop) = (x.max(0), width.min(x + count));
                if stop > start {
                    let value = map(pixel);
                    region.values[usize::from(value)] = true;
                    if draw {
                        let row = (y * width) as usize;
                        region.pixels[row + start as usize..row + stop as usize].fill(value);
                        written += stop - start;
                    }
                }
            }
            x += count;
        });
        at = bits.align();
    }
    at as i64 * WALK_COST + written
}

/// The bits of a pixel string, read past its end as zeros.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    bit: u8,
}

impl Bits<'_> {
    fn done(&self) -> bool {
        self.at >= self.data.len()
    }

    fn read(&mut self, n: u32) -> u8 {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = self.data.get(self.at).copied().unwrap_or(0);
            v = (v << 1) | u32::from((byte >> (7 - self.bit)) & 1);
            self.bit += 1;
            if self.bit == 8 {
                (self.bit, self.at) = (0, self.at + 1);
            }
        }
        v as u8
    }

    /// The byte after the string: strings end on a byte.
    fn align(&mut self) -> usize {
        if self.bit != 0 {
            (self.bit, self.at) = (0, self.at + 1);
        }
        self.at
    }
}

/// The runs of one pixel string of `depth` bits a code (count, pixel),
/// until its end code. A run's count is read before its pixel.
fn pixel_string(bits: &mut Bits, depth: u8, mut emit: impl FnMut(i64, u8)) {
    while !bits.done() {
        match depth {
            2 => {
                let v = bits.read(2);
                if v != 0 {
                    emit(1, v);
                } else if bits.read(1) == 1 {
                    let count = 3 + i64::from(bits.read(3));
                    emit(count, bits.read(2));
                } else if bits.read(1) == 1 {
                    emit(1, 0);
                } else {
                    match bits.read(2) {
                        0 => return,
                        1 => emit(2, 0),
                        2 => {
                            let count = 12 + i64::from(bits.read(4));
                            emit(count, bits.read(2));
                        }
                        _ => {
                            let count = 29 + i64::from(bits.read(8));
                            emit(count, bits.read(2));
                        }
                    }
                }
            }
            4 => {
                let v = bits.read(4);
                if v != 0 {
                    emit(1, v);
                } else if bits.read(1) == 0 {
                    let count = bits.read(3);
                    if count == 0 {
                        return;
                    }
                    emit(2 + i64::from(count), 0);
                } else if bits.read(1) == 0 {
                    let count = 4 + i64::from(bits.read(2));
                    emit(count, bits.read(4));
                } else {
                    match bits.read(2) {
                        0 => emit(1, 0),
                        1 => emit(2, 0),
                        2 => {
                            let count = 9 + i64::from(bits.read(4));
                            emit(count, bits.read(4));
                        }
                        _ => {
                            let count = 25 + i64::from(bits.read(8));
                            emit(count, bits.read(4));
                        }
                    }
                }
            }
            _ => {
                let v = bits.read(8);
                if v != 0 {
                    emit(1, v);
                } else if bits.read(1) == 0 {
                    let count = bits.read(7);
                    if count == 0 {
                        return;
                    }
                    emit(i64::from(count), 0);
                } else {
                    let count = i64::from(bits.read(7));
                    emit(count, bits.read(8));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::PictureTrack;
    use super::super::mks::testing::{lasting, mks_of};
    use super::*;

    fn seg(kind: u8, body: &[u8]) -> Vec<u8> {
        [&[0x0F, kind, 0, 1][..], &(body.len() as u16).to_be_bytes(), body].concat()
    }

    /// A display set of `segs` and its end, in a PES data field.
    fn set(segs: &[Vec<u8>]) -> Vec<u8> {
        [&[0x20, 0x00][..], &segs.concat(), &seg(0x80, &[]), &[0xFF]].concat()
    }

    /// A page in `state` (0 normal case, 2 mode change) that times out after
    /// 5 s, placing regions (id, x, y).
    fn page(state: u8, regions: &[(u8, u16, u16)]) -> Vec<u8> {
        let mut body = vec![5, state << 2];
        for &(id, x, y) in regions {
            body.extend([id, 0]);
            body.extend(x.to_be_bytes());
            body.extend(y.to_be_bytes());
        }
        seg(PAGE, &body)
    }

    /// A mode change placing these regions.
    fn placing(regions: &[(u8, u16, u16)]) -> Vec<u8> {
        page(2, regions)
    }

    /// Region `id`, `width`×`height`, 4-bit pixels of CLUT 0, filled with
    /// entry `fill` when it says, listing objects (id, x, y).
    fn region(id: u8, width: u16, height: u16, fill: Option<u8>, objects: &[(u16, u16, u16)]) -> Vec<u8> {
        let mut body = vec![id, if fill.is_some() { 0x08 } else { 0 }];
        body.extend(width.to_be_bytes());
        body.extend(height.to_be_bytes());
        body.extend([2 << 2, 0, 0, fill.unwrap_or(0) << 4]);
        for &(object, x, y) in objects {
            body.extend(object.to_be_bytes());
            body.extend([(x >> 8) as u8, x as u8, (y >> 8) as u8, y as u8]);
        }
        seg(REGION, &body)
    }

    fn filled(id: u8, width: u16, height: u16) -> Vec<u8> {
        region(id, width, height, Some(1), &[])
    }

    /// CLUT 0 with entry 1 of its 4-bit table opaque red (Y 81, Cr 240, Cb
    /// 90, full range).
    fn red() -> Vec<u8> {
        seg(CLUT, &[0, 0, 1, 0x40 | 0x01, 81, 240, 90, 0])
    }

    /// Object `id`, its top field `line`, drawn on both fields.
    fn object(id: u16, line: &[u8]) -> Vec<u8> {
        seg(OBJECT, &[&id.to_be_bytes()[..], &[0], &(line.len() as u16).to_be_bytes(), &[0, 0], line].concat())
    }

    /// A 4-bit string of 4 pixels of entry 1, and its end.
    const LINE: [u8; 6] = [0x11, 0x11, 0x11, 0x00, 0x00, 0xF0];

    /// The sizes of the regions a display set of `segs` shows on the largest
    /// plane, decoded as a picture is; `None` when it isn't decoded.
    fn sizes(segs: &[Vec<u8>]) -> Option<Vec<(u32, u32)>> {
        let mut decoder = Decoder::new(true, MAX_WORK);
        decoder.take(&set(segs));
        (decoder.work > 0).then(|| decoder.shown((MAX_SIDE, MAX_SIDE)).iter().map(|((.., w, h), _)| (*w, *h)).collect())
    }

    /// The pictures of display sets at these times.
    fn pictures<'a>(sets: &[(i64, &'a [u8])]) -> DvbPictures<'a> {
        DvbPictures::new(sets.iter().map(|&(time_ms, data)| Sample { time_ms, duration_ms: None, data }).collect())
    }

    fn times(cues: Vec<PictureCue>) -> Vec<(i64, i64)> {
        cues.iter().map(|c| (c.start_ms, c.end_ms)).collect()
    }

    #[test]
    fn a_display_set_splits_into_segments() {
        let data = set(&[placing(&[(0, 60, 500)]), region(0, 4, 2, None, &[]), red(), object(0, &LINE)]);
        assert_eq!(segments(&data).map(|(kind, _)| kind).collect::<Vec<_>>(), [0x10, 0x11, 0x12, 0x13, 0x80]);
        assert_eq!(page_head(&data), Some((5, 2)));
    }

    #[test]
    fn objects_are_drawn_into_the_regions_the_page_places() {
        let data = set(&[placing(&[(0, 60, 500)]), region(0, 4, 2, None, &[(0, 0, 0)]), red(), object(0, &LINE)]);
        let file = mks_of(b"S_DVBSUB", Some(&[0, 1, 0, 1]), &[(1_000, &[(0, data)])], false);
        let Some(PictureTrack::Dvb(pictures)) = PictureTrack::read(&file) else { panic!("not a DVB track") };
        assert_eq!((pictures.width, pictures.height), (720, 576));
        // Until its page times out, 5 s on.
        let cue = PictureCue { start_ms: 1_000, end_ms: 6_000, x: 60, y: 500, width: 4, height: 2 };
        assert_eq!(pictures.cues(), [cue]);
        let picture = pictures.picture(1_000).unwrap();
        let [r, g, b] = ycbcr_to_rgb(81, 90, 240, false);
        // Both fields: the bottom one is the top one.
        assert_eq!(picture.rgba, [r, g, b, 255].repeat(8));
        assert_eq!(pictures.picture(2_000), None);
    }

    #[test]
    fn a_picture_shows_until_the_next_display_set_its_block_or_its_page_says() {
        // Shown; then filled with a transparent entry, which shows nothing;
        // then drawn again (a normal case, reusing what came before).
        let shown = set(&[placing(&[(0, 60, 500)]), region(0, 4, 2, None, &[(0, 0, 0)]), red(), object(0, &LINE)]);
        let cleared = set(&[page(0, &[(0, 60, 500)]), region(0, 4, 2, Some(0), &[(0, 0, 0)])]);
        let again = set(&[page(0, &[(0, 60, 500)]), object(0, &LINE)]);
        let pictures = pictures(&[(1_000, &shown), (3_000, &cleared), (4_000, &again)]);
        assert_eq!(times(pictures.cues()), [(1_000, 3_000), (4_000, 9_000)]);
        assert_eq!(pictures.picture(3_000), None);
        assert_eq!(pictures.picture(4_000).unwrap().rgba[..4], pictures.picture(1_000).unwrap().rgba[..4]);
        // Its page's time-out ends the last: it is whole.
        assert_eq!(times(pictures.cues_so_far()), [(1_000, 3_000), (4_000, 9_000)]);
        // A page that doesn't time out, last: what ends it may be copied later.
        let endless = set(&[
            seg(PAGE, &[0, 2 << 2, 0, 0, 0, 60, 1, 244]),
            region(0, 4, 2, None, &[(0, 0, 0)]),
            red(),
            object(0, &LINE),
        ]);
        let pictures = self::pictures(&[(1_000, &shown), (3_000, &endless)]);
        assert_eq!(times(pictures.cues()), [(1_000, 3_000), (3_000, 13_000)]);
        assert_eq!(times(pictures.cues_so_far()), [(1_000, 3_000)]);
        // A block's own duration comes first.
        let file = [mks_of(b"S_DVBSUB", None, &[], false), lasting(1_000, 1_500, &shown)].concat();
        let Some(PictureTrack::Dvb(pictures)) = PictureTrack::read(&file) else { panic!("not a DVB track") };
        assert_eq!(times(pictures.cues()), [(1_000, 2_500)]);
    }

    #[test]
    fn regions_show_together_cut_to_an_hd_plane() {
        // A display definition of 1920×1080; two regions, one past the
        // plane's corner.
        let display = seg(DISPLAY, &[0, 0x07, 0x7F, 0x04, 0x37]);
        let data =
            set(&[display, placing(&[(0, 1880, 1060), (1, 1910, 1075)]), filled(0, 4, 2), filled(1, 40, 20), red()]);
        let pictures = pictures(&[(0, &data)]);
        assert_eq!((pictures.width, pictures.height), (1920, 1080));
        let cue = pictures.cues()[0];
        assert_eq!((cue.x, cue.y, cue.width, cue.height), (1880, 1060, 40, 20));
        let picture = pictures.picture(0).unwrap();
        let pixel = |x: usize, y: usize| &picture.rgba[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
        // HD colours: BT.709.
        let [r, g, b] = ycbcr_to_rgb(81, 90, 240, true);
        assert_eq!((pixel(0, 0), pixel(39, 19)), (&[r, g, b, 255][..], &[r, g, b, 255][..]));
        assert_eq!(pixel(10, 5), [0, 0, 0, 0], "between them, nothing");
    }

    #[test]
    fn each_picture_is_placed_by_its_own_display_definition() {
        // An HD programme (1920×1080), its page again in a normal case, then
        // an SD one with no display definition (720×576).
        let display = seg(DISPLAY, &[0, 0x07, 0x7F, 0x04, 0x37]);
        let hd = set(&[display, placing(&[(0, 960, 540)]), filled(0, 8, 4), red()]);
        let again = set(&[page(0, &[(0, 960, 540)])]);
        let sd = set(&[placing(&[(0, 360, 288)]), filled(0, 4, 2), red()]);
        let pictures = pictures(&[(0, &hd), (1_000, &again), (2_000, &sd)]);
        assert_eq!((pictures.width, pictures.height), (1920, 1080));
        let places: Vec<_> = pictures.cues().iter().map(|c| (c.x, c.y, c.width, c.height)).collect();
        // The SD picture at the same fractions of the track's plane.
        assert_eq!(places, [(960, 540, 8, 4), (960, 540, 8, 4), (960, 540, 11, 4)]);
        // Each picture its own size, in its own plane's colours.
        let [r, g, b] = ycbcr_to_rgb(81, 90, 240, true);
        assert_eq!(pictures.picture(1_000).unwrap().rgba, [r, g, b, 255].repeat(32));
        let picture = pictures.picture(2_000).unwrap();
        let [r, g, b] = ycbcr_to_rgb(81, 90, 240, false);
        assert_eq!((picture.width, picture.height, picture.rgba), (4, 2, [r, g, b, 255].repeat(8)));
    }

    #[test]
    fn a_box_goes_onto_another_plane_at_the_same_fractions() {
        assert_eq!(onto((10, 20, 30, 40), (720, 576), (720, 576)), (10, 20, 30, 40));
        assert_eq!(onto((360, 288, 4, 2), (720, 576), (1920, 1080)), (960, 540, 11, 4));
        // A pixel at least, on the plane.
        assert_eq!(onto((1919, 1079, 1, 1), (1920, 1080), (720, 576)), (719, 575, 1, 1));
        // A display definition past 4096 pixels is left out.
        assert_eq!(display_plane(&[0, 0x07, 0x7F, 0x04, 0x37]), Some((1920, 1080)));
        assert_eq!(display_plane(&[0, 0x10, 0x00, 0x04, 0x37]), None);
        assert_eq!(display_plane(&[0, 0x07, 0x7F, 0x04]), None);
    }

    #[test]
    fn a_frames_worth_of_pixels_is_decoded_at_most() {
        // One 3000×2000 region placed twice: twice would pass a 4K screen's.
        let twice = [placing(&[(0, 0, 0), (0, 10, 10)]), filled(0, 3000, 2000), red()];
        assert_eq!(sizes(&twice), Some(vec![(3000, 2000)]));
        // 32 places at most.
        let places: Vec<_> = (0..1000).map(|i| (0, i % 700, 0)).collect();
        assert_eq!(sizes(&[placing(&places), filled(0, 1, 1), red()]).unwrap().len(), 32);
    }

    #[test]
    fn a_display_set_is_read_256_segments_at_most() {
        let with = |empty: usize| {
            [vec![placing(&[(0, 0, 0)])], vec![seg(0x16, &[]); empty], vec![filled(0, 4, 2), red()]].concat()
        };
        assert_eq!(segments(&set(&with(300))).count(), 256);
        assert_eq!(sizes(&with(300)), Some(vec![]));
        assert_eq!(sizes(&with(200)), Some(vec![(4, 2)]));
    }

    #[test]
    fn a_picture_is_decoded_from_4096_display_sets_back_at_most() {
        let start = set(&[placing(&[(0, 0, 0)]), filled(0, 4, 2), red()]);
        // Normal case: the page again.
        let again = set(&[page(0, &[(0, 0, 0)])]);
        let sets: Vec<(i64, &[u8])> =
            std::iter::once(&start).chain([&again; 4097]).enumerate().map(|(i, s)| (i as i64 * 10, &s[..])).collect();
        let pictures = pictures(&sets);
        assert!(pictures.picture(40_960).is_some());
        assert_eq!(pictures.picture(40_970), None);
    }

    #[test]
    fn an_object_is_drawn_in_the_first_64_places_a_region_lists() {
        // A 100×2 region listing object 1 at (0, 0) 64 times, then at (50, 0);
        // the object one pixel of entry 1.
        let places = [vec![(1, 0, 0); 64], vec![(1, 50, 0)]].concat();
        let pixel = object(1, &[0x11, 0x10, 0x00, 0xF0]);
        let data = set(&[placing(&[(0, 0, 0)]), region(0, 100, 2, None, &places), red(), pixel]);
        let picture = pictures(&[(0, &data)]).picture(0).unwrap();
        assert_eq!((picture.rgba[3], picture.rgba[50 * 4 + 3]), (255, 0));
    }

    #[test]
    fn a_picture_that_takes_more_than_eight_4k_screens_work_is_not_decoded() {
        // Region 0, 3840×2000, made and filled, then filled again and again;
        // then region 1, the one placed. Seven fills and the region make 8 ×
        // 7,680,000 pixels; eight, 9 ×, past 8 × 3840 × 2160.
        let fills = |n: usize| {
            [vec![placing(&[(1, 0, 0)])], vec![filled(0, 3840, 2000); n], vec![filled(1, 4, 2), red()]].concat()
        };
        assert_eq!(sizes(&fills(7)), Some(vec![(4, 2)]));
        assert_eq!(sizes(&fills(8)), None);
        // Drawn into places again and again: each draw counts, an empty one
        // too (4096 each: 100 segments of 128 come to 52 million, 150 to 79).
        let places = vec![(0, 0, 0); 64];
        let draws = |n: usize| {
            let regions = vec![placing(&[(1, 0, 0)]), region(0, 1, 1, None, &places), region(2, 1, 1, None, &places)];
            [regions, vec![red()], vec![object(0, &[]); n], vec![filled(1, 4, 2)]].concat()
        };
        assert_eq!(sizes(&draws(100)), Some(vec![(4, 2)]));
        assert_eq!(sizes(&draws(150)), None);
    }

    #[test]
    fn regions_are_kept_a_frames_worth_at_most_of_4096_pixels_a_side() {
        // The second 3000×2000 region would pass a 4K screen's pixels with the
        // first: it isn't kept.
        let two = [placing(&[(1, 0, 0)]), filled(0, 3000, 2000), filled(1, 3000, 2000), red()];
        assert_eq!(sizes(&two), Some(vec![]));
        // A region made again counts once, at its new size.
        let remade = [placing(&[(0, 0, 0)]), filled(0, 3000, 2000), filled(0, 3840, 2160), red()];
        assert_eq!(sizes(&remade), Some(vec![(3840, 2160)]));
        // A mode change lets go of the regions held.
        let changed =
            [placing(&[(0, 0, 0)]), filled(0, 3840, 2160), placing(&[(1, 0, 0)]), filled(1, 3000, 2000), red()];
        assert_eq!(sizes(&changed), Some(vec![(3000, 2000)]));
        assert_eq!(sizes(&[placing(&[(0, 0, 0)]), filled(0, 4096, 1), red()]), Some(vec![(4096, 1)]));
        assert_eq!(sizes(&[placing(&[(0, 0, 0)]), filled(0, 4097, 1), red()]), Some(vec![]));
    }

    #[test]
    fn pixel_strings_decode_at_each_depth() {
        let runs = |data: &[u8], depth: u8| {
            let mut runs = Vec::new();
            pixel_string(&mut Bits { data, at: 0, bit: 0 }, depth, |count, pixel| runs.push((count, pixel)));
            runs
        };
        // 2-bit: a pixel of 3; 3 + 5 of 1; 12 + 2 of 2; the end.
        // 11 | 00 1 101 01 | 00 0 0 10 0010 10 | 00 0 0 00
        assert_eq!(runs(&[0b1100_1101, 0b0100_0010, 0b0010_1000, 0b0000_0000], 2), [(1, 3), (8, 1), (14, 2)]);
        // 4-bit: a pixel of 9; 2 + 3 of 0; 4 + 1 of 7; 25 + 1 of 5; the end.
        // 1001 | 0000 0 011 | 0000 1 0 01 0111 | 0000 1 1 11 00000001 0101 | 0000 0 000
        let four = [0b1001_0000, 0b0011_0000, 0b1001_0111, 0b0000_1111, 0b0000_0001, 0b0101_0000, 0b0000_0000];
        assert_eq!(runs(&four, 4), [(1, 9), (5, 0), (5, 7), (26, 5)]);
        // 8-bit: a pixel of 200; 3 of 0; 70 of 9; the end.
        let eight = [200, 0, 3, 0, 0x80 | 70, 9, 0, 0];
        assert_eq!(runs(&eight, 8), [(1, 200), (3, 0), (70, 9)]);
    }
}
