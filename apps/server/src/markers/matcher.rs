//! Finds the audio two fingerprints share (an intro or credits sequence used
//! in both episodes) and combines the matches of several episode pairs.
//!
//! Fingerprints are raw Chromaprint values, one per ~0.124 s. Every possible
//! alignment of the two sequences is scanned; values within a few bit errors
//! count as the same audio, and runs of such hits (allowing short gaps) are
//! candidate shared segments. The run with the most hits wins.

use std::ops::Range;

/// Values differing in at most this many of their 32 bits are the same audio.
/// Aligned copies of the same (re-encoded) audio average under 2 differing
/// bits; unrelated audio averages ~15 and is within 6 bits well under 1 % of
/// the time.
pub const MAX_BIT_ERRORS: u32 = 6;

#[derive(Debug, Clone, Copy)]
pub struct MatchParams {
    /// Longest stretch without a hit that still continues a run.
    pub max_gap: usize,
    /// Shortest run worth reporting.
    pub min_len: usize,
    /// Fraction of positions in a run that must be hits.
    pub min_density: f32,
}

impl MatchParams {
    /// Parameters for fingerprints with values every `item_seconds`.
    pub fn new(item_seconds: f64, min_seconds: f64) -> Self {
        Self {
            max_gap: (3.5 / item_seconds).round() as usize,
            min_len: (min_seconds / item_seconds).round() as usize,
            min_density: 0.5,
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
    let mut consider = |start: usize, last: usize, hits: usize, shift: isize| {
        let len = last - start + 1;
        if len < p.min_len || (hits as f32) < p.min_density * len as f32 {
            return;
        }
        if best.as_ref().is_none_or(|b| hits > b.hits) {
            let b_start = (start as isize + shift) as usize;
            best = Some(SharedRun { a: start..last + 1, b: b_start..b_start + len, hits });
        }
    };
    // shift = index in b - index in a
    for shift in -(a.len() as isize - 1)..b.len() as isize {
        let i0 = (-shift).max(0) as usize;
        let i1 = a.len().min((b.len() as isize - shift) as usize);
        if i1 <= i0 || i1 - i0 < p.min_len {
            continue;
        }
        let mut run: Option<(usize, usize, usize)> = None; // (start, last hit, hits)
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
                Some((start, last, hits)) => {
                    consider(start, last, hits, shift);
                    Some((i, i, 1))
                }
                None => Some((i, i, 1)),
            };
        }
        if let Some((start, last, hits)) = run {
            consider(start, last, hits, shift);
        }
    }
    best
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
        MatchParams { max_gap: 28, min_len: 120, min_density: 0.5 }
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
