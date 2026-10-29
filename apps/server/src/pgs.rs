//! PGS subtitles as a `.sup` file (the Blu-ray's own format, which Kodi
//! reads next to a stream), made of the server's Matroska file of the track
//! (`/api/files/{id}/subtitles/{index}.mks`).
//!
//! A Matroska block of a PGS track is a display set's segments one after
//! the other (type, 16-bit size, data); a `.sup` file has the same segments,
//! each after a header of its own: `PG`, its presentation and decoding
//! times (90 kHz).

/// The `.sup` file of the PGS track in the Matroska file `mks` (one written
/// as the transcoder writes them: one track, blocks not laced, a file being
/// written read to its last whole element), with an empty display set at 0
/// first: a reader that takes a file's first time for its start (FFmpeg
/// does) starts it where the video does. `None` for anything else.
pub fn from_matroska(mks: &[u8]) -> Option<Vec<u8>> {
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
    let mut pgs = false;
    let mut blocks: Vec<(i64, &[u8])> = Vec::new();
    for (id, data) in children(mks, start, end) {
        match id {
            INFO => {
                if let Some((_, scale)) = elements(data).find(|(id, _)| *id == TIMESTAMP_SCALE) {
                    scale_ns = uint(scale).filter(|s| *s > 0)?;
                }
            }
            TRACKS => {
                let entry = elements(data).find(|(id, _)| *id == TRACK_ENTRY)?.1;
                pgs = elements(entry).any(|(id, codec)| id == CODEC_ID && codec.starts_with(b"S_HDMV/PGS"));
                if elements(entry).any(|(id, _)| id == CONTENT_ENCODINGS) {
                    return None;
                }
            }
            CLUSTER => {
                let mut cluster_time = 0i64;
                for (id, data) in elements(data) {
                    match id {
                        CLUSTER_TIMESTAMP => cluster_time = i64::try_from(uint(data)?).ok()?,
                        SIMPLE_BLOCK => blocks.extend(block(data, cluster_time)),
                        BLOCK_GROUP => {
                            if let Some((_, b)) = elements(data).find(|(id, _)| *id == BLOCK) {
                                blocks.extend(block(b, cluster_time));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if !pgs {
        return None;
    }
    let mut out = Vec::with_capacity(blocks.iter().map(|(_, d)| d.len() + 64).sum());
    let mut first = true;
    for (ticks, data) in blocks {
        let pts = (i128::from(ticks) * i128::from(scale_ns) * 90 / 1_000_000_000).clamp(0, 0xFFFF_FFFF) as u32;
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

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const CODEC_ID: u32 = 0x86;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
/// Segment types: presentation composition, end of display set.
const PCS: u8 = 0x16;
const END: u8 = 0x80;

fn push(out: &mut Vec<u8>, pts: u32, segment: &[u8]) {
    out.extend_from_slice(b"PG");
    out.extend_from_slice(&pts.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(segment);
}

/// A block's absolute time (ticks) and its frame; `None` for a laced one.
fn block(data: &[u8], cluster_time: i64) -> Option<(i64, &[u8])> {
    let (_, track_len) = vint(data, 0)?;
    let rest = data.get(track_len..)?;
    if rest.len() < 3 || rest[2] & 0x06 != 0 {
        return None;
    }
    let relative = i16::from_be_bytes([rest[0], rest[1]]);
    Some((cluster_time + i64::from(relative), &rest[3..]))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn el(id: &[u8], data: &[u8]) -> Vec<u8> {
        assert!(data.len() < 0x3FFF);
        [id, &[0x40 | (data.len() >> 8) as u8, data.len() as u8], data].concat()
    }

    /// A display set: a PCS of 1920×1080 with one object, an END.
    fn display_set(composition: u8) -> Vec<u8> {
        let pcs = [0x16, 0, 19, 0x07, 0x80, 0x04, 0x38, 0x10, 0, composition, 0x80, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        [&pcs[..], &[0x80, 0, 0]].concat()
    }

    /// A cluster: its time, and its blocks (time in it, data).
    type Cluster<'a> = (u64, &'a [(i16, Vec<u8>)]);

    fn mks(clusters: &[Cluster], unknown_cluster: bool) -> Vec<u8> {
        let tracks = el(&[0x16, 0x54, 0xAE, 0x6B], &el(&[0xAE], &el(&[0x86], b"S_HDMV/PGS")));
        let info = el(&[0x15, 0x49, 0xA9, 0x66], &el(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]));
        let mut segment = [info, tracks].concat();
        for (time, blocks) in clusters {
            let mut body = el(&[0xE7], &time.to_be_bytes()[6..]);
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
        assert_eq!(&sup[13..13 + 3 + 11], &[0x16, 0, 11, 0x07, 0x80, 0x04, 0x38, 0x10, 0, 0, 0x80, 0, 0, 0]);
        // A cluster still being written (its size unknown) counts too.
        let file = mks(&[(95_000, &[(0, display_set(1))]), (99_000, &[(0, display_set(3))])], true);
        assert_eq!(segments(&from_matroska(&file).unwrap()).len(), 6);
    }

    #[test]
    fn only_pgs_matroska_files_are_made_sup() {
        assert_eq!(from_matroska(b"not matroska"), None);
        let vobsub = mks(&[(0, &[(0, vec![1, 2, 3])])], false);
        let vobsub = String::from_utf8_lossy(&vobsub).replace("S_HDMV/PGS", "S_VOBSUB__").into_bytes();
        assert_eq!(from_matroska(&vobsub), None);
        // No block yet: an empty file.
        assert_eq!(from_matroska(&mks(&[], false)), Some(Vec::new()));
    }
}
