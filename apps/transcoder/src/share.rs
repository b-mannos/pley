// SPDX-License-Identifier: LicenseRef-Restricted
//! How transcodes that tone map with libplacebo share the GPU.
//!
//! Each process tone maps at the best quality the GPU keeps ahead of real
//! time with the other jobs tone mapping on it (see
//! [`crate::plan::tonemap_quality`]). Where even the fastest falls behind,
//! the GPU's video engine decodes as well as encodes (AMD's and Intel's do
//! both, NVIDIA's are apart) and another job uses it, the video is decoded
//! by the CPU instead, while the CPU has the cores: its frames come to
//! memory for libplacebo either way. On a Ryzen 5 5600G two 4K films went
//! from 1.0× to 1.2× real time each (decoding 4K HEVC and encoding 4K
//! H.264 at once is more than its Radeon's engine can do for two); made
//! 1080p, which its Radeon keeps ahead, they gained nothing, so a GPU that
//! keeps up decodes, and the CPU stays free. A running job moves at once
//! when another starts using the GPU, between two segments, and back once
//! it is left alone and well ahead of its player.

use std::time::Duration;

use pley_transcoder_api::TonemapQuality;

/// Cores the CPU takes to decode 4K HEVC at the speed a 24 fps film is
/// transcoded at with two on the GPU, 1.2× real time: 3.6 to 4.8 measured
/// on a Ryzen 5 5600G, with the rest of FFmpeg's work.
const DECODE_CORES_4K: f64 = 4.0;
/// Cores left to the rest: audio, the server, the system.
const CORES_KEPT: f64 = 2.0;
/// A move that gives the GPU more to do (its job is left alone on it) waits
/// until the process has run this long, so that jobs coming and going don't
/// restart it each time.
pub const MORE_AFTER: Duration = Duration::from_secs(60);
/// ... and its job is this far ahead of the player (seconds of video), so
/// that the restart can't make it wait.
pub const MORE_LEAD_SECS: f64 = 30.0;

/// What one process does on the GPU, besides its plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share {
    pub quality: TonemapQuality,
    /// The video is decoded by the CPU rather than the GPU planned.
    pub cpu_decode: bool,
}

impl Share {
    /// Whether `to` gives the GPU less to do than `self` in any way: a
    /// cheaper quality, or the decoding moved to the CPU.
    pub fn relieved_by(self, to: Share) -> bool {
        to.quality < self.quality || (to.cpu_decode && !self.cpu_decode)
    }
}

/// The cores decoding `pixels` a frame at `fps` frames a second takes.
pub fn decode_cores(pixels: u64, fps: f64) -> f64 {
    DECODE_CORES_4K * (pixels as f64 / (3840.0 * 2160.0)) * (fps / 24.0)
}

/// The GPU a job would decode on: see [`cpu_decodes`].
#[derive(Debug, Clone, Copy)]
pub struct Gpu {
    /// It decodes on the video engine it encodes with.
    pub engine_shared: bool,
    /// Other jobs using that engine.
    pub others: u32,
    /// It falls behind the jobs tone mapping on it, this one included, even
    /// at the fastest quality ([`crate::plan::tonemap_falls_behind`]).
    pub behind: bool,
}

/// What the CPU is doing besides, and what it has: see [`cpu_decodes`].
#[derive(Debug, Clone, Copy)]
pub struct Cpu {
    pub cores: u32,
    /// What the other processes decoding on it take ([`decode_cores`]).
    pub decoding: f64,
    /// Software encodes running: x264 and x265 take what cores there are.
    pub software_encodes: u32,
}

/// Whether a job decodes on the CPU rather than on `gpu`, taking `cores`
/// ([`decode_cores`]).
pub fn cpu_decodes(gpu: Gpu, cpu: Cpu, cores: f64) -> bool {
    gpu.engine_shared
        && gpu.behind
        && gpu.others > 0
        && cpu.software_encodes == 0
        && f64::from(cpu.cores) - CORES_KEPT >= cpu.decoding + cores
}

#[cfg(test)]
mod tests {
    use super::*;

    const UHD: u64 = 3840 * 2160;

    fn cpu(cores: u32, decoding: f64) -> Cpu {
        Cpu { cores, decoding, software_encodes: 0 }
    }

    /// AMD's or Intel's, falling behind, with `others` jobs on its engine.
    fn behind(others: u32) -> Gpu {
        Gpu { engine_shared: true, others, behind: true }
    }

    #[test]
    fn the_cpu_decodes_for_a_gpu_behind_another_job_on_its_video_engine_while_it_has_the_cores() {
        let film = decode_cores(UHD, 23.976);
        // Alone: the GPU decodes.
        assert!(!cpu_decodes(behind(0), cpu(12, 0.0), film));
        // A second film: both on a 6-core, 12-thread Ryzen.
        assert!(cpu_decodes(behind(1), cpu(12, 0.0), film));
        assert!(cpu_decodes(behind(1), cpu(12, film), film));
        // Not a third: the GPU decodes it.
        assert!(!cpu_decodes(behind(2), cpu(12, 2.0 * film), film));
        // A 4-core, 8-thread processor takes one.
        assert!(cpu_decodes(behind(1), cpu(8, 0.0), film));
        assert!(!cpu_decodes(behind(1), cpu(8, film), film));
        // 1080p takes a quarter: many fit.
        let hd = decode_cores(1920 * 1080, 24.0);
        assert!(cpu_decodes(behind(3), cpu(8, 3.0 * hd), hd));
        // A GPU that keeps up decodes.
        assert!(!cpu_decodes(Gpu { behind: false, ..behind(1) }, cpu(12, 0.0), film));
        // NVIDIA decodes apart from its encoder; software encodes take the cores.
        assert!(!cpu_decodes(Gpu { engine_shared: false, ..behind(1) }, cpu(12, 0.0), film));
        assert!(!cpu_decodes(behind(1), Cpu { software_encodes: 1, ..cpu(12, 0.0) }, film));
    }

    #[test]
    fn less_for_the_gpu_is_a_cheaper_quality_or_the_decoding_moved_to_the_cpu() {
        let alone = Share { quality: TonemapQuality::Balanced, cpu_decode: false };
        let shared = Share { quality: TonemapQuality::Fast, cpu_decode: true };
        assert!(alone.relieved_by(shared));
        assert!(alone.relieved_by(Share { quality: TonemapQuality::Fast, ..alone }));
        assert!(alone.relieved_by(Share { cpu_decode: true, ..alone }));
        assert!(!shared.relieved_by(alone));
        assert!(!shared.relieved_by(shared));
    }
}
