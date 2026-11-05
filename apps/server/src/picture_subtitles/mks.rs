//! The Matroska file the server makes of a picture subtitle track
//! (`/api/files/{id}/subtitles/{index}.mks`), as the transcoder writes them:
//! one track, blocks not laced, a file being written read to its last whole
//! element. Only what the pictures need is read: the track's codec and
//! private data, and every block with its time and duration
//! (packages/picture-subtitles/src/mks.ts reads them for the apps).

/// A track's private data read at most: a VobSub index's header is a KiB
/// or two.
const MAX_PRIVATE_BYTES: usize = 64 << 10;

/// The track of a `.mks` file: its codec, its private data, nanoseconds a
/// tick, and its blocks in the order of the file.
pub struct Track<'a> {
    pub codec_id: &'a [u8],
    /// A VobSub track's `.idx` header; its first [`MAX_PRIVATE_BYTES`].
    pub codec_private: Option<&'a [u8]>,
    pub scale_ns: u64,
    pub blocks: Vec<Block<'a>>,
}

/// A block of the track: its time and duration (ticks; the duration when
/// the block says) and its data.
pub struct Block<'a> {
    pub ticks: i64,
    pub duration: Option<u64>,
    pub data: &'a [u8],
}

/// A block's data at its time, and its own duration when its group says
/// (ms of the track's time).
pub struct Sample<'a> {
    pub time_ms: i64,
    pub duration_ms: Option<i64>,
    pub data: &'a [u8],
}

impl<'a> Track<'a> {
    /// Ticks as milliseconds of the track's time.
    pub fn ms(&self, ticks: i64) -> i64 {
        (i128::from(ticks) * i128::from(self.scale_ns) / 1_000_000).clamp(0, i128::from(i64::MAX)) as i64
    }

    /// The blocks in order of time, in milliseconds.
    pub fn samples(&self) -> Vec<Sample<'a>> {
        let mut samples: Vec<Sample> = self
            .blocks
            .iter()
            .map(|b| Sample {
                time_ms: self.ms(b.ticks),
                duration_ms: b.duration.map(|d| self.ms(i64::try_from(d).unwrap_or(i64::MAX))),
                data: b.data,
            })
            .collect();
        samples.sort_by_key(|s| s.time_ms);
        samples
    }
}

/// The track of the Matroska file `mks`; `None` for anything else, or a
/// track whose blocks are compressed.
pub fn read(mks: &[u8]) -> Option<Track<'_>> {
    let (id, size, at) = header(mks, 0)?;
    if id != EBML {
        return None;
    }
    let (id, segment_size, start) = header(mks, at.checked_add(usize::try_from(size?).ok()?)?)?;
    if id != SEGMENT {
        return None;
    }
    let end =
        segment_size.and_then(|s| start.checked_add(usize::try_from(s).ok()?)).map_or(mks.len(), |e| e.min(mks.len()));
    let mut scale_ns = 1_000_000u64;
    let mut codec = None;
    let mut blocks = Vec::new();
    for (id, data) in children(mks, start, end) {
        match id {
            INFO => {
                if let Some((_, scale)) = elements(data).find(|(id, _)| *id == TIMESTAMP_SCALE) {
                    scale_ns = uint(scale).filter(|s| *s > 0)?;
                }
            }
            TRACKS => {
                let entry = elements(data).find(|(id, _)| *id == TRACK_ENTRY)?.1;
                if elements(entry).any(|(id, _)| id == CONTENT_ENCODINGS) {
                    return None;
                }
                let codec_id = elements(entry).find(|(id, _)| *id == CODEC_ID)?.1;
                let private = elements(entry).find(|(id, _)| *id == CODEC_PRIVATE).map(|(_, p)| p);
                codec = Some((codec_id, private.map(|p| &p[..p.len().min(MAX_PRIVATE_BYTES)])));
            }
            CLUSTER => {
                let mut cluster_time = 0i64;
                for (id, data) in elements(data) {
                    match id {
                        CLUSTER_TIMESTAMP => cluster_time = i64::try_from(uint(data)?).ok()?,
                        SIMPLE_BLOCK => blocks.extend(block(data, cluster_time, None)),
                        BLOCK_GROUP => {
                            let duration =
                                elements(data).find(|(id, _)| *id == BLOCK_DURATION).and_then(|(_, d)| uint(d));
                            if let Some((_, b)) = elements(data).find(|(id, _)| *id == BLOCK) {
                                blocks.extend(block(b, cluster_time, duration));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let (codec_id, codec_private) = codec?;
    Some(Track { codec_id, codec_private, scale_ns, blocks })
}

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const BLOCK_DURATION: u32 = 0x9B;

/// A block at its absolute time (ticks), with its `duration` when its group
/// says; `None` for a laced one.
fn block(data: &[u8], cluster_time: i64, duration: Option<u64>) -> Option<Block<'_>> {
    let (_, track_len) = vint(data, 0)?;
    let rest = data.get(track_len..)?;
    if rest.len() < 3 || rest[2] & 0x06 != 0 {
        return None;
    }
    let relative = i16::from_be_bytes([rest[0], rest[1]]);
    Some(Block { ticks: cluster_time.checked_add(i64::from(relative))?, duration, data: &rest[3..] })
}

/// The children of the element whose data runs from `start` to `end`, an
/// element of unknown size (a cluster being written) to the next of its
/// level, the last one cut short left out.
fn children(bytes: &[u8], start: usize, end: usize) -> Vec<(u32, &[u8])> {
    let mut found = Vec::new();
    let mut at = start;
    while let Some((id, size, data)) = header(bytes, at) {
        let next = match size {
            Some(size) => match usize::try_from(size).ok().and_then(|size| data.checked_add(size)) {
                Some(next) if next <= end => next,
                _ => break,
            },
            // A cluster of unknown size: up to the next cluster, or the end.
            None => (data..end.saturating_sub(4)).find(|&i| bytes[i..i + 4] == CLUSTER.to_be_bytes()).unwrap_or(end),
        };
        found.push((id, &bytes[data..next]));
        at = next;
        if at >= end {
            break;
        }
    }
    found
}

/// The child elements of an element's data, as far as they are whole.
fn elements(data: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let (id, size, start) = header(data, at)?;
        let end = start.checked_add(usize::try_from(size?).ok()?)?;
        let body = data.get(start..end)?;
        at = end;
        Some((id, body))
    })
}

/// An element's id, size (`None`: unknown) and where its data starts.
fn header(bytes: &[u8], at: usize) -> Option<(u32, Option<u64>, usize)> {
    let first = *bytes.get(at)?;
    let id_len = first.leading_zeros() as usize + 1;
    if id_len > 4 {
        return None;
    }
    let id = bytes.get(at..at + id_len)?.iter().fold(0u32, |v, b| (v << 8) | u32::from(*b));
    let (size, len) = vint(bytes, at + id_len)?;
    let unknown = size == (1u64 << (7 * len)) - 1;
    Some((id, (!unknown).then_some(size), at + id_len + len))
}

/// A variable-length integer (its marker taken off) and its length.
fn vint(bytes: &[u8], at: usize) -> Option<(u64, usize)> {
    let first = *bytes.get(at)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let raw = bytes.get(at..at + len)?;
    let marker = if len == 8 { 0 } else { 0xFFu8 >> len };
    let value = raw[1..].iter().fold(u64::from(raw[0] & marker), |v, b| (v << 8) | u64::from(*b));
    Some((value, len))
}

fn uint(data: &[u8]) -> Option<u64> {
    (data.len() <= 8).then(|| data.iter().fold(0u64, |v, b| (v << 8) | u64::from(*b)))
}

/// Matroska files of one track, as the transcoder writes them, for the
/// decoders' tests.
#[cfg(test)]
pub(super) mod testing {
    pub fn el(id: &[u8], data: &[u8]) -> Vec<u8> {
        assert!(data.len() < 0x3FFF);
        [id, &[0x40 | (data.len() >> 8) as u8, data.len() as u8], data].concat()
    }

    /// A cluster: its time, and its blocks (time in it, data).
    pub type Cluster<'a> = (u64, &'a [(i16, Vec<u8>)]);

    /// A file of a track of `codec` (with its private data), its clusters'
    /// sizes unknown when `unknown_cluster` (FFmpeg writing them).
    pub fn mks_of(codec: &[u8], private: Option<&[u8]>, clusters: &[Cluster], unknown_cluster: bool) -> Vec<u8> {
        let mut entry = el(&[0x86], codec);
        if let Some(private) = private {
            entry.extend(el(&[0x63, 0xA2], private));
        }
        let tracks = el(&[0x16, 0x54, 0xAE, 0x6B], &el(&[0xAE], &entry));
        let info = el(&[0x15, 0x49, 0xA9, 0x66], &el(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]));
        let mut segment = [info, tracks].concat();
        for (time, blocks) in clusters {
            let mut body = el(&[0xE7], &time.to_be_bytes()[5..]);
            for (relative, data) in blocks.iter() {
                let block = [&[0x81][..], &relative.to_be_bytes(), &[0x80], data].concat();
                body.extend(el(&[0xA3], &block));
            }
            if unknown_cluster {
                segment.extend(
                    [&[0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF][..], &body].concat(),
                );
            } else {
                segment.extend(el(&[0x1F, 0x43, 0xB6, 0x75], &body));
            }
        }
        let ebml = el(&[0x1A, 0x45, 0xDF, 0xA3], &el(&[0x42, 0x82], b"matroska"));
        // The segment's size unknown, as FFmpeg writes it until it is done.
        [ebml, vec![0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], segment].concat()
    }

    /// A cluster at `time_ms` holding one block of `data` that lasts
    /// `duration_ms` (a BlockGroup's duration).
    pub fn lasting(time_ms: u16, duration_ms: u16, data: &[u8]) -> Vec<u8> {
        let block = el(&[0xA1], &[&[0x81, 0, 0, 0][..], data].concat());
        let group = el(&[0xA0], &[block, el(&[0x9B], &duration_ms.to_be_bytes())].concat());
        el(&[0x1F, 0x43, 0xB6, 0x75], &[el(&[0xE7], &time_ms.to_be_bytes()), group].concat())
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn a_track_its_codec_private_data_and_blocks_are_read() {
        let file = mks_of(
            b"S_VOBSUB",
            Some(b"size: 720x480\n"),
            &[(1_000, &[(0, vec![1, 2]), (500, vec![3])]), (900, &[(0, vec![4])])],
            false,
        );
        let track = read(&file).unwrap();
        assert_eq!((track.codec_id, track.codec_private), (&b"S_VOBSUB"[..], Some(&b"size: 720x480\n"[..])));
        let samples = track.samples();
        // In order of time, whatever the file's order.
        let times: Vec<_> = samples.iter().map(|s| (s.time_ms, s.data)).collect();
        assert_eq!(times, [(900, &[4u8][..]), (1_000, &[1, 2][..]), (1_500, &[3][..])]);
        assert!(samples.iter().all(|s| s.duration_ms.is_none()));
        // A block group's own duration.
        let file = [mks_of(b"S_DVBSUB", None, &[], false), lasting(2_000, 1_500, &[9])].concat();
        let track = read(&file).unwrap();
        assert_eq!(track.codec_private, None);
        let samples = track.samples();
        assert_eq!((samples[0].time_ms, samples[0].duration_ms), (2_000, Some(1_500)));
    }

    #[test]
    fn only_matroska_with_a_track_is_read() {
        assert!(read(b"not matroska").is_none());
        let ebml = testing::el(&[0x1A, 0x45, 0xDF, 0xA3], &[]);
        let no_tracks = [ebml, vec![0x18, 0x53, 0x80, 0x67, 0x80]].concat();
        assert!(read(&no_tracks).is_none());
    }
}
