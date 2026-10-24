//! Finds the audio two fingerprints share (an intro or credits sequence used
//! in both episodes) and combines the matches of several episode pairs.
//!
//! Fingerprints are raw Chromaprint values, one per ~0.124 s. Alignments
//! where several values of the two sequences are identical are scanned (as
//! Jellyfin's Intro Skipper does: shared audio always has some); values
//! within a few bit errors count as the same audio, and runs of such hits
//! (allowing short gaps) are candidate shared segments. Runs at the same
//! alignment separated by less than half their length are one segment (a
//! title sequence with a gag that changes every episode). The run with the
//! most hits wins.

use std::collections::HashMap;
use std::ops::Range;

/// Values differing in at most this many of their 32 bits are the same audio.
/// Aligned copies of the same (re-encoded) audio average under 2 differing
/// bits; unrelated audio averages ~15 and is within 6 bits well under 1 % of
/// the time.
pub const MAX_BIT_ERRORS: u32 = 6;

/// Matching values an alignment needs before it is scanned.
const MIN_VOTES: usize = 3;
/// Alignments scanned per pair, the ones with most matching values first.
const MAX_SHIFTS: usize = 48;

#[derive(Debug, Clone, Copy)]
pub struct MatchParams {
    /// Longest stretch without a hit that still continues a run.
    pub max_gap: usize,
    /// Shortest run worth reporting.
    pub min_len: usize,
    /// Fraction of positions in a run that must be hits.
    pub min_density: f32,
    /// Runs at least this long are joined across a gap of up to half the
    /// shorter one.
    pub bridge_min: usize,
}

impl MatchParams {
    /// Parameters for fingerprints with values every `item_seconds`.
    pub fn new(item_seconds: f64, min_seconds: f64) -> Self {
        Self {
            max_gap: (3.5 / item_seconds).round() as usize,
            min_len: (min_seconds / item_seconds).round() as usize,
            min_density: 0.5,
            bridge_min: (8.0 / item_seconds).round() as usize,
        }
    }
}

/// A segment present in both fingerprints: item ranges in `a` and in `b`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedRun {
    pub a: Range<usize>,
    pub b: Range<usize>,
    pub hits: usize,
}

/// The best segment shared by `a` and `b`, if any is long and dense enough.
pub fn shared_run(a: &[u32], b: &[u32], p: &MatchParams) -> Option<SharedRun> {
    if a.len() < p.min_len || b.len() < p.min_len {
        return None;
    }
    let mut best: Option<SharedRun> = None;
    for shift in candidate_shifts(a, b) {
        for (start, last, hits) in runs_at(a, b, shift, p) {
            let len = last - start + 1;
            if len >= p.min_len && best.as_ref().is_none_or(|b| hits > b.hits) {
                let b_start = (start as isize + shift) as usize;
                best = Some(SharedRun { a: start..last + 1, b: b_start..b_start + len, hits });
            }
        }
    }
    best
}

/// Alignments (index in `b` − index in `a`) where several matching values
/// line up, most first, each with the alignments next to it (the shared
/// audio may sit between two). Values within a few bit errors nearly always
/// have their upper or their lower 16 bits in common, so those halves find
/// them without comparing every pair.
fn candidate_shifts(a: &[u32], b: &[u32]) -> Vec<isize> {
    // Values repeating the one before (silence, static sound) prove nothing.
    let moving = |v: &[u32], i: usize| i == 0 || v[i] != v[i - 1];
    let mut halves: [HashMap<u16, Vec<usize>>; 2] = Default::default();
    for j in (0..b.len()).filter(|&j| moving(b, j)) {
        halves[0].entry((b[j] >> 16) as u16).or_default().push(j);
        halves[1].entry(b[j] as u16).or_default().push(j);
    }
    let mut counts: HashMap<isize, usize> = HashMap::new();
    for i in (0..a.len()).filter(|&i| moving(a, i)) {
        let x = a[i];
        let high = halves[0].get(&((x >> 16) as u16)).map_or(&[][..], Vec::as_slice);
        let low = halves[1].get(&(x as u16)).map_or(&[][..], Vec::as_slice);
        // A value all over the other fingerprint doesn't say where.
        if high.len() + low.len() > 32 {
            continue;
        }
        for &j in high.iter().chain(low.iter().filter(|j| !high.contains(j))) {
            if (x ^ b[j]).count_ones() <= MAX_BIT_ERRORS {
                *counts.entry(j as isize - i as isize).or_default() += 1;
            }
        }
    }
    let mut shifts: Vec<(isize, usize)> = counts.into_iter().filter(|&(_, n)| n >= MIN_VOTES).collect();
    shifts.sort_unstable_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let mut out = Vec::new();
    for (shift, _) in shifts.into_iter().take(MAX_SHIFTS) {
        for s in [shift, shift - 1, shift + 1] {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// The runs of hits at one alignment as `(start, last hit, hits)` in `a`:
/// dense ones only, joined across short interruptions.
fn runs_at(a: &[u32], b: &[u32], shift: isize, p: &MatchParams) -> Vec<(usize, usize, usize)> {
    let i0 = (-shift).max(0) as usize;
    let i1 = a.len().min((b.len() as isize - shift).max(0) as usize);
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    let mut run: Option<(usize, usize, usize)> = None;
    for i in i0..i1 {
        let j = (i as isize + shift) as usize;
        let (x, y) = (a[i], b[j]);
        // Digital silence and other static audio repeat the same value;
        // two static stretches matching each other prove nothing.
        let is_static = i > 0 && j > 0 && x == a[i - 1] && y == b[j - 1];
        if is_static || (x ^ y).count_ones() > MAX_BIT_ERRORS {
            continue;
        }
        run = match run {
            Some((start, last, hits)) if i - last <= p.max_gap => Some((start, i, hits + 1)),
            Some(done) => {
                runs.push(done);
                Some((i, i, 1))
            }
            None => Some((i, i, 1)),
        };
    }
    runs.extend(run);
    let dense = |&(start, last, hits): &(usize, usize, usize)| hits as f32 >= p.min_density * (last - start + 1) as f32;
    let mut joined: Vec<(usize, usize, usize)> = Vec::new();
    for r in runs.into_iter().filter(dense) {
        if let Some(prev) = joined.last_mut() {
            let (prev_len, len) = (prev.1 - prev.0 + 1, r.1 - r.0 + 1);
            if prev_len >= p.bridge_min && len >= p.bridge_min && r.0 - prev.1 <= prev_len.min(len) / 2 {
                *prev = (prev.0, r.1, prev.2 + r.2);
                continue;
            }
        }
        joined.push(r);
    }
    joined
}

/// Combine the segments found for one episode against several others: keep
/// the group that most comparisons agree on and take the median boundaries.
/// Ranges are in ms within the episode.
pub fn consensus(candidates: &[(i64, i64)]) -> Option<(i64, i64)> {
    if candidates.is_empty() {
        return None;
    }
    let overlaps = |x: &(i64, i64), y: &(i64, i64)| {
        let shared = x.1.min(y.1) - x.0.max(y.0);
        let shorter = (x.1 - x.0).min(y.1 - y.0).max(1);
        shared * 2 >= shorter
    };
    let (best, _) = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, candidates.iter().filter(|o| overlaps(c, o)).count()))
        .max_by_key(|&(i, support)| (support, candidates[i].1 - candidates[i].0))?;
    let group: Vec<&(i64, i64)> = candidates.iter().filter(|o| overlaps(&candidates[best], o)).collect();
    let median = |mut v: Vec<i64>| {
        v.sort_unstable();
        v[v.len() / 2]
    };
    Some((median(group.iter().map(|c| c.0).collect()), median(group.iter().map(|c| c.1).collect())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random values (xorshift).
    fn noise(seed: u32, n: usize) -> Vec<u32> {
        let mut x = seed.max(1);
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x
            })
            .collect()
    }

    /// Flip `bits` low-ish bits of every `every`-th value (re-encoding noise).
    fn perturb(v: &[u32], seed: u32, bits: u32) -> Vec<u32> {
        let r = noise(seed, v.len());
        v.iter().zip(r).map(|(x, r)| x ^ (r & ((1u32 << bits) - 1)).rotate_left(r % 32)).collect()
    }

    fn params() -> MatchParams {
        MatchParams { max_gap: 28, min_len: 120, min_density: 0.5, bridge_min: 65 }
    }

    /// `prefix` values of noise, then `shared`, then `suffix` of noise.
    fn episode(seed: u32, prefix: usize, shared: &[u32], suffix: usize) -> Vec<u32> {
        let mut v = noise(seed, prefix);
        v.extend_from_slice(shared);
        v.extend(noise(seed.wrapping_mul(31).wrapping_add(7), suffix));
        v
    }

    #[test]
    fn finds_shifted_shared_segment() {
        let intro = noise(42, 240); // ~30 s
        let a = episode(1, 100, &intro, 600);
        let b = episode(2, 450, &perturb(&intro, 9, 2), 300);
        let run = shared_run(&a, &b, &params()).expect("match");
        assert!(run.a.start.abs_diff(100) <= 1 && run.a.end.abs_diff(340) <= 1, "{run:?}");
        assert!(run.b.start.abs_diff(450) <= 1 && run.b.end.abs_diff(690) <= 1, "{run:?}");
    }

    #[test]
    fn tolerates_dropouts_inside_the_segment() {
        let intro = noise(7, 300);
        let mut damaged = intro.clone();
        // Dialogue over the music for ~2 s, twice.
        for i in (60..75).chain(200..216) {
            damaged[i] = !damaged[i];
        }
        let a = episode(3, 0, &intro, 500);
        let b = episode(4, 222, &damaged, 400);
        let run = shared_run(&a, &b, &params()).expect("match");
        assert!(run.a.start <= 1 && run.a.end >= 298, "{run:?}");
    }

    #[test]
    fn a_title_sequence_around_a_changing_gag_is_one_segment() {
        // 12 s of theme, a 5.6 s gag that differs in every episode, 12 s of theme.
        let (theme, rest) = (noise(31, 100), noise(32, 100));
        let with_gag = |seed| [theme.clone(), noise(seed, 45), rest.clone()].concat();
        let a = episode(33, 200, &with_gag(34), 400);
        let b = episode(35, 60, &perturb(&with_gag(36), 9, 2), 500);
        let run = shared_run(&a, &b, &MatchParams { min_len: 200, ..params() }).expect("one segment");
        assert!(run.a.start.abs_diff(200) <= 1 && run.a.end.abs_diff(445) <= 1, "{run:?}");
        // A longer gap (a different scene) splits them; each half is too short alone.
        let with_scene = |seed| [theme.clone(), noise(seed, 120), rest.clone()].concat();
        let a = episode(37, 200, &with_scene(38), 400);
        let b = episode(39, 60, &with_scene(40), 500);
        assert_eq!(shared_run(&a, &b, &MatchParams { min_len: 200, ..params() }), None);
    }

    #[test]
    fn unrelated_audio_does_not_match() {
        let a = noise(11, 2_000);
        let b = noise(12, 2_000);
        assert_eq!(shared_run(&a, &b, &params()), None);
    }

    #[test]
    fn short_or_static_segments_do_not_count() {
        // 5 s of shared audio is not an intro.
        let jingle = noise(5, 40);
        let a = episode(21, 300, &jingle, 300);
        let b = episode(22, 10, &jingle, 600);
        assert_eq!(shared_run(&a, &b, &params()), None);
        // Long digital silence in both.
        let silence = vec![0x1234_5678u32; 400];
        let a = episode(23, 50, &silence, 50);
        let b = episode(24, 90, &silence, 50);
        assert_eq!(shared_run(&a, &b, &params()), None);
    }

    #[test]
    fn consensus_ignores_outliers() {
        let found = [(15_000, 45_100), (14_800, 44_900), (120_000, 160_000), (15_200, 45_000)];
        assert_eq!(consensus(&found), Some((15_000, 45_000)));
        assert_eq!(consensus(&[(1_000, 31_000)]), Some((1_000, 31_000)));
        assert_eq!(consensus(&[]), None);
    }
}
