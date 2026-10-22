// SPDX-License-Identifier: LicenseRef-Restricted
//! Instant keyframe index for Matroska/WebM files, read from the Cues element
//! (written by mkvmerge and FFmpeg for video keyframes). Only the segment's
//! metadata elements are read, not the clusters, so this takes milliseconds
//! where a packet scan reads the whole file.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const EBML: u32 = 0x1A45_DFA3;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CLUSTER: u32 = 0x1F43_B675;

const UNKNOWN_SIZE: u64 = u64::MAX;
const MAX_ELEMENT_READ: u64 = 64 * 1024 * 1024;
const MAX_TOP_LEVEL_ELEMENTS: usize = 50_000;

/// Keyframe times (ms) of the first video track, or `None` when the file is not
/// Matroska or has no usable Cues.
pub fn cue_keyframes(path: &Path) -> Option<Vec<i64>> {
    let mut file = BufReader::with_capacity(64 * 1024, File::open(path).ok()?);
    let (id, size) = element_header(&mut file)?;
    if id != EBML || size == UNKNOWN_SIZE {
        return None;
    }
    file.seek_relative(size as i64).ok()?;
    let (id, segment_size) = element_header(&mut file)?;
    if id != SEGMENT {
        return None;
    }
    let segment_start = file.stream_position().ok()?;
    let segment_end = if segment_size == UNKNOWN_SIZE { u64::MAX } else { segment_start + segment_size };

    let mut info = None;
    let mut tracks = None;
    let mut cues = None;
    let mut pos = segment_start;
    for _ in 0..MAX_TOP_LEVEL_ELEMENTS {
        if pos >= segment_end || (info.is_some() && tracks.is_some() && cues.is_some()) {
            break;
        }
        file.seek(SeekFrom::Start(pos)).ok()?;
        let Some((id, size)) = element_header(&mut file) else { break };
        let data = file.stream_position().ok()?;
        match id {
            SEEK_HEAD => {
                for (target, rel) in parse_seek_head(&read_payload(&mut file, size)?) {
                    let abs = segment_start + rel;
                    match target {
                        INFO => info = info.or(Some(abs)),
                        TRACKS => tracks = tracks.or(Some(abs)),
                        CUES => cues = cues.or(Some(abs)),
                        _ => {}
                    }
                }
            }
            INFO => info = Some(pos),
            TRACKS => tracks = Some(pos),
            CUES => cues = Some(pos),
            CLUSTER if cues.is_some() && tracks.is_some() => break,
            _ => {}
        }
        if size == UNKNOWN_SIZE {
            break;
        }
        pos = data + size;
    }

    let scale = info
        .and_then(|p| child_payload(&mut file, p, INFO))
        .and_then(|d| find_uint(&d, TIMESTAMP_SCALE))
        .unwrap_or(1_000_000);
    let video_track = tracks.and_then(|p| child_payload(&mut file, p, TRACKS)).and_then(|d| first_video_track(&d))?;
    let cues = child_payload(&mut file, cues?, CUES)?;
    let mut times: Vec<i64> =
        cue_times(&cues, video_track).into_iter().map(|t| (t as u128 * scale as u128 / 1_000_000) as i64).collect();
    times.sort_unstable();
    times.dedup();
    (times.len() >= 2).then_some(times)
}

/// Variable-length integer: (value, length). Element ids keep their marker bit.
fn vint(r: &mut impl Read, keep_marker: bool) -> Option<(u64, usize)> {
    let mut first = [0u8; 1];
    r.read_exact(&mut first).ok()?;
    let b = first[0];
    if b == 0 {
        return None;
    }
    let len = b.leading_zeros() as usize + 1;
    // For 8-byte vints the first byte holds only the marker (mask = 0).
    let mut value = if keep_marker { b as u64 } else { b as u64 & (0xFF_u64 >> len) };
    let mut rest = [0u8; 7];
    r.read_exact(&mut rest[..len - 1]).ok()?;
    for byte in &rest[..len - 1] {
        value = (value << 8) | *byte as u64;
    }
    Some((value, len))
}

fn element_header(r: &mut impl Read) -> Option<(u32, u64)> {
    let (id, id_len) = vint(r, true)?;
    if id_len > 4 {
        return None;
    }
    let (size, size_len) = vint(r, false)?;
    let unknown = size == (1u64 << (7 * size_len)) - 1;
    Some((id as u32, if unknown { UNKNOWN_SIZE } else { size }))
}

fn read_payload(r: &mut impl Read, size: u64) -> Option<Vec<u8>> {
    if size == UNKNOWN_SIZE || size > MAX_ELEMENT_READ {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    r.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// Payload of the element at `pos`, checking it has the expected id.
fn child_payload<R: Read + Seek>(r: &mut R, pos: u64, expected: u32) -> Option<Vec<u8>> {
    r.seek(SeekFrom::Start(pos)).ok()?;
    let (id, size) = element_header(r)?;
    if id != expected {
        return None;
    }
    read_payload(r, size)
}

/// Iterate `(id, payload)` over the direct children in `data`.
fn children(data: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    let mut cursor = std::io::Cursor::new(data);
    std::iter::from_fn(move || {
        let (id, size) = element_header(&mut cursor)?;
        let start = cursor.position() as usize;
        let end = start.checked_add(usize::try_from(size).ok()?)?;
        if end > data.len() {
            return None;
        }
        cursor.set_position(end as u64);
        Some((id, &data[start..end]))
    })
}

fn uint(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }
    Some(bytes.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64))
}

fn find_uint(data: &[u8], id: u32) -> Option<u64> {
    children(data).find(|(child, _)| *child == id).and_then(|(_, payload)| uint(payload))
}

fn parse_seek_head(data: &[u8]) -> Vec<(u32, u64)> {
    children(data)
        .filter(|(id, _)| *id == SEEK)
        .filter_map(|(_, seek)| {
            let target = children(seek).find(|(id, _)| *id == SEEK_ID).and_then(|(_, p)| uint(p))? as u32;
            let position = find_uint(seek, SEEK_POSITION)?;
            Some((target, position))
        })
        .collect()
}

fn first_video_track(tracks: &[u8]) -> Option<u64> {
    children(tracks)
        .filter(|(id, _)| *id == TRACK_ENTRY)
        .find(|(_, entry)| find_uint(entry, TRACK_TYPE) == Some(1))
        .and_then(|(_, entry)| find_uint(entry, TRACK_NUMBER))
}

fn cue_times(cues: &[u8], track: u64) -> Vec<u64> {
    children(cues)
        .filter(|(id, _)| *id == CUE_POINT)
        .filter_map(|(_, point)| {
            let time = find_uint(point, CUE_TIME)?;
            let for_track = children(point)
                .filter(|(id, _)| *id == CUE_TRACK_POSITIONS)
                .any(|(_, pos)| find_uint(pos, CUE_TRACK) == Some(track));
            for_track.then_some(time)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode an element (id bytes as written, 8-byte size).
    fn el(id: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.push(0x01);
        out.extend_from_slice(&(payload.len() as u64).to_be_bytes()[1..]);
        out.extend_from_slice(payload);
        out
    }

    fn uint_el(id: &[u8], v: u64) -> Vec<u8> {
        el(id, &v.to_be_bytes())
    }

    /// `PLEY_TEST_MKV=<file> cargo test -p pley-transcoder real_file -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_file() {
        let path = std::env::var("PLEY_TEST_MKV").expect("set PLEY_TEST_MKV");
        let mut file = BufReader::new(File::open(&path).unwrap());
        let (id, size) = element_header(&mut file).unwrap();
        println!("first element {id:#x} size {size}");
        file.seek_relative(size as i64).unwrap();
        let (id, size) = element_header(&mut file).unwrap();
        let start = file.stream_position().unwrap();
        println!("segment {id:#x} size {size} data at {start}");
        let mut pos = start;
        for _ in 0..12 {
            file.seek(SeekFrom::Start(pos)).unwrap();
            let Some((id, size)) = element_header(&mut file) else { break };
            let data = file.stream_position().unwrap();
            println!("  top-level {id:#x} at {pos} size {size}");
            if id == SEEK_HEAD {
                println!("    seek head: {:x?}", parse_seek_head(&read_payload(&mut file, size).unwrap()));
            }
            if size == UNKNOWN_SIZE {
                break;
            }
            pos = data + size;
        }
        println!("result: {:?}", cue_keyframes(Path::new(&path)).map(|t| t.into_iter().take(5).collect::<Vec<_>>()));
    }

    #[test]
    fn reads_vints() {
        assert_eq!(vint(&mut &[0x81u8][..], false), Some((1, 1)));
        assert_eq!(vint(&mut &[0x40u8, 0x02][..], false), Some((2, 2)));
        assert_eq!(vint(&mut &[0x1A, 0x45, 0xDF, 0xA3][..], true), Some((0x1A45_DFA3, 4)));
        let unknown = element_header(&mut &[0xA3u8, 0xFF][..]).unwrap();
        assert_eq!(unknown, (0xA3, UNKNOWN_SIZE));
    }

    #[test]
    fn parses_a_synthetic_file() {
        let video_entry = el(&[0xAE], &[uint_el(&[0xD7], 1), uint_el(&[0x83], 1)].concat());
        let audio_entry = el(&[0xAE], &[uint_el(&[0xD7], 2), uint_el(&[0x83], 2)].concat());
        let tracks = el(&[0x16, 0x54, 0xAE, 0x6B], &[audio_entry, video_entry].concat());
        let info = el(&[0x15, 0x49, 0xA9, 0x66], &uint_el(&[0x2A, 0xD7, 0xB1], 1_000_000));
        let cue = |time: u64, track: u64| {
            el(&[0xBB], &[uint_el(&[0xB3], time), el(&[0xB7], &uint_el(&[0xF7], track))].concat())
        };
        let cues = el(&[0x1C, 0x53, 0xBB, 0x6B], &[cue(0, 1), cue(2002, 1), cue(500, 2), cue(4004, 1)].concat());
        let segment = el(&[0x18, 0x53, 0x80, 0x67], &[info, tracks, cues].concat());
        let file = [el(&[0x1A, 0x45, 0xDF, 0xA3], &[0x42, 0x86, 0x81, 0x01]), segment].concat();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mkv");
        std::fs::write(&path, file).unwrap();
        assert_eq!(cue_keyframes(&path), Some(vec![0, 2002, 4004]));

        std::fs::write(&path, b"not a matroska file").unwrap();
        assert_eq!(cue_keyframes(&path), None);
    }
}
