//! Picture subtitles (PGS, VobSub, DVB) made of the server's Matroska file
//! of the track (`/api/files/{id}/subtitles/{index}.mks`, [`mks`]), for
//! players that can't read that file: the pictures themselves, placed and
//! timed ([`PictureTrack`], for a TV's own player that can't decode them:
//! Roku's), or a PGS track as a `.sup` file ([`pgs::from_matroska`], which
//! Kodi reads next to a stream).
//!
//! The apps that draw picture subtitles decode the same files themselves
//! (packages/picture-subtitles); the pictures here are read as theirs are:
//! the same times, places and colours.

mod dvb;
mod mks;
pub mod pgs;
mod vobsub;

use dvb::DvbPictures;
use pgs::PgsPictures;
use vobsub::VobSubPictures;

/// The widest and tallest a plane or picture is read (a Blu-ray's plane is
/// 1920×1080, a DVD's 720×576).
const MAX_SIDE: u32 = 4096;
/// The most pixels a picture is decoded of: a 4K screen's worth. A file can
/// say a picture is 4096² or place one many times over.
const MAX_PIXELS: u64 = 3840 * 2160;
/// How long a picture shows that nothing follows (ms): the last of a track.
const LAST_PICTURE_MS: i64 = 10_000;

/// A picture showing from `start_ms` until `end_ms` (the track's time),
/// `width`×`height` at `x`, `y` of the plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureCue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A picture's pixels: `width`×`height`, RGBA, not premultiplied.
#[derive(Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A picture subtitle track's pictures, for a player that shows them
/// without decoding the track: when each shows and where
/// ([`PictureTrack::cues`]), and each picture ([`PictureTrack::picture`]),
/// on a plane of [`PictureTrack::plane`].
pub enum PictureTrack<'a> {
    Pgs(PgsPictures<'a>),
    VobSub(VobSubPictures<'a>),
    Dvb(DvbPictures<'a>),
}

impl<'a> PictureTrack<'a> {
    /// The pictures of the PGS, VobSub or DVB track in the Matroska file
    /// `mks`; `None` for anything else.
    pub fn read(mks: &'a [u8]) -> Option<Self> {
        let track = mks::read(mks)?;
        Some(match track.codec_id {
            id if id.starts_with(b"S_HDMV/PGS") => Self::Pgs(PgsPictures::of(&track)),
            b"S_VOBSUB" => Self::VobSub(VobSubPictures::of(&track)),
            b"S_DVBSUB" => Self::Dvb(DvbPictures::of(&track)),
            _ => return None,
        })
    }

    /// The plane the pictures are placed on: width and height.
    pub fn plane(&self) -> (u32, u32) {
        match self {
            Self::Pgs(p) => (p.width, p.height),
            Self::VobSub(p) => (p.width, p.height),
            Self::Dvb(p) => (p.width, p.height),
        }
    }

    /// When each picture shows and where, in order of time.
    pub fn cues(&self) -> Vec<PictureCue> {
        match self {
            Self::Pgs(p) => p.cues(),
            Self::VobSub(p) => p.cues(),
            Self::Dvb(p) => p.cues(),
        }
    }

    /// [`PictureTrack::cues`] of a track copied only so far: a last picture
    /// that nothing after it ends is left out, as what ends it may not be
    /// copied yet. A player asks again, and has it with its end.
    pub fn cues_so_far(&self) -> Vec<PictureCue> {
        match self {
            Self::Pgs(p) => p.cues_so_far(),
            Self::VobSub(p) => p.cues_so_far(),
            Self::Dvb(p) => p.cues_so_far(),
        }
    }

    /// The picture that shows from `start_ms` (a cue's start), or `None`.
    pub fn picture(&self, start_ms: i64) -> Option<Picture> {
        match self {
            Self::Pgs(p) => p.picture(start_ms),
            Self::VobSub(p) => p.picture(start_ms),
            Self::Dvb(p) => p.picture(start_ms),
        }
    }
}

/// When a picture showing from `start` stops showing: when the next one
/// starts (`next`; [`LAST_PICTURE_MS`] on for the last), or after its own
/// duration (`own`, when it says) when that is sooner, as players show them.
fn end_ms(start: i64, next: Option<i64>, own: Option<i64>) -> i64 {
    let next = next.unwrap_or(start.saturating_add(LAST_PICTURE_MS));
    match own {
        Some(own) if own > 0 => next.min(start.saturating_add(own)),
        _ => next,
    }
}

/// `cues` of a track copied so far, its last sample starting at `last`
/// (when it has one) and ending by itself or not: a cue of that sample is
/// left out when nothing ends it.
fn so_far(mut cues: Vec<PictureCue>, last: Option<(i64, bool)>) -> Vec<PictureCue> {
    if let Some((start, false)) = last
        && cues.last().is_some_and(|cue| cue.start_ms == start)
    {
        cues.pop();
    }
    cues
}

/// The part of the box `(x, y, width, height)` on a plane of `plane`, or
/// `None` when nothing of it is.
fn on_plane((x, y, width, height): (u32, u32, u32, u32), plane: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let width = width.min(plane.0.saturating_sub(x));
    let height = height.min(plane.1.saturating_sub(y));
    (width > 0 && height > 0).then_some((x, y, width, height))
}

/// The smallest box holding every one of `boxes` (x, y, width, height).
fn union(boxes: impl Iterator<Item = (u32, u32, u32, u32)>) -> Option<(u32, u32, u32, u32)> {
    boxes.fold(None, |all, (x, y, w, h)| {
        let (x0, y0, x1, y1) = match all {
            None => (x, y, x + w, y + h),
            Some((ax, ay, aw, ah)) => (ax.min(x), ay.min(y), (ax + aw).max(x + w), (ay + ah).max(y + h)),
        };
        Some((x0, y0, x1 - x0, y1 - y0))
    })
}

/// Limited-range YCbCr to RGB, as FFmpeg's decoders take subtitles' colours:
/// BT.709 for HD planes, BT.601 for SD ones.
fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8, hd: bool) -> [u8; 3] {
    let yy = 1.164_383 * (f32::from(y) - 16.0);
    let (u, v) = (f32::from(cb) - 128.0, f32::from(cr) - 128.0);
    let (r, g, b) = if hd {
        (yy + 1.792_741 * v, yy - 0.213_249 * u - 0.532_909 * v, yy + 2.112_402 * u)
    } else {
        (yy + 1.596_027 * v, yy - 0.391_762 * u - 0.812_968 * v, yy + 2.017_232 * u)
    };
    [r, g, b].map(|c| c.round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_ends_at_the_next_or_after_its_own_duration() {
        assert_eq!(end_ms(1_000, Some(3_000), None), 3_000);
        assert_eq!(end_ms(1_000, Some(3_000), Some(500)), 1_500);
        assert_eq!(end_ms(1_000, Some(3_000), Some(5_000)), 3_000);
        // The last: 10 s, or its own.
        assert_eq!(end_ms(1_000, None, None), 11_000);
        assert_eq!(end_ms(1_000, None, Some(0)), 11_000);
        assert_eq!(end_ms(1_000, None, Some(2_000)), 3_000);
    }

    #[test]
    fn colors_are_bt709_for_hd_and_bt601_for_sd() {
        assert_eq!(ycbcr_to_rgb(235, 128, 128, true), [255, 255, 255]);
        assert_eq!(ycbcr_to_rgb(16, 128, 128, false), [0, 0, 0]);
        assert_ne!(ycbcr_to_rgb(81, 90, 240, true), ycbcr_to_rgb(81, 90, 240, false));
    }

    #[test]
    fn only_picture_subtitle_tracks_are_read() {
        use mks::testing::mks_of;
        for (codec, plane) in [(&b"S_HDMV/PGS"[..], (1920, 1080)), (b"S_VOBSUB", (720, 576)), (b"S_DVBSUB", (720, 576))]
        {
            let file = mks_of(codec, None, &[], false);
            let track = PictureTrack::read(&file).unwrap();
            assert_eq!((track.plane(), track.cues()), (plane, Vec::new()));
        }
        assert!(PictureTrack::read(&mks_of(b"S_TEXT/UTF8", None, &[], false)).is_none());
        assert!(PictureTrack::read(b"not matroska").is_none());
    }
}
