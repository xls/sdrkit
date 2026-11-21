//! The audio codec plug-in contract: the decoders of compressed speech and audio, registered by four-character code.
//!
//! A graph decodes a stream through the [`CodecSet`] of its run context, behind the guard in `sdrkit-graph`
//! (`sdrkit_graph::codec::GuardedCodec`), which runs each decoder on its own thread with a time limit per call.

use crate::media::{ChannelLayout, Codec, MediaFormat};
use crate::FourCc;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Decoded audio: interleaved 16-bit samples with their rate and channel count. A decoder appends to `samples`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pcm {
    /// Interleaved samples.
    pub samples: Vec<i16>,
    /// Samples per second per channel.
    pub rate_hz: u32,
    /// Channels (1 for speech).
    pub channels: u8,
    /// The order of the channels; [`ChannelLayout::Unknown`] when the decoder does not say (then 1 channel is mono and 2
    /// are stereo).
    pub layout: ChannelLayout,
}

impl Pcm {
    /// Empties it, keeping the allocation.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// The layout of the samples: the one the decoder set, or the one of the channel count.
    pub fn layout(&self) -> ChannelLayout {
        match self.layout {
            ChannelLayout::Unknown => ChannelLayout::of_count(u16::from(self.channels)),
            l => l,
        }
    }

    /// Mixes the 5.1 frames from sample `at` on down to stereo, and sets the channels and the layout to stereo:
    /// `Lo = L + centre·C + surround·Ls` and `Ro = R + centre·C + surround·Rs`, without the low-frequency channel, scaled by
    /// `1 / (1 + centre + surround)` so that no sum overflows (ATSC A/52 7.8.2). The gains are those the stream states, or
    /// the standard's default. Frames of another layout are left as they are.
    pub fn downmix_51(&mut self, at: usize, centre: f32, surround: f32) {
        if self.layout() != ChannelLayout::Surround51 {
            return;
        }
        let g = 1.0 / (1.0 + centre + surround);
        let frames = (self.samples.len() - at) / 6;
        for k in 0..frames {
            let f = &self.samples[at + 6 * k..at + 6 * k + 6];
            let [l, c, r, ls, rs] = [f[0], f[1], f[2], f[3], f[4]].map(f32::from);
            let mix = |a: f32, b: f32| ((a + centre * c + surround * b) * g).round() as i16;
            let (lo, ro) = (mix(l, ls), mix(r, rs));
            self.samples[at + 2 * k] = lo;
            self.samples[at + 2 * k + 1] = ro;
        }
        self.samples.truncate(at + 2 * frames);
        self.channels = 2;
        self.layout = ChannelLayout::Stereo;
    }
}

/// Why a frame could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    /// The frame is not one this codec decodes (wrong length, bad header).
    #[error("the frame is not one this codec decodes")]
    Frame,
    /// The plug-in does not decode the stream's format (codec, packaging or configuration).
    #[error("the codec plug-in does not decode this stream")]
    Unsupported,
    /// The plug-in ran longer than the guard allows; it is turned off.
    #[error("the codec plug-in did not answer in time")]
    Timeout,
    /// The plug-in panicked or stopped; it is turned off.
    #[error("the codec plug-in stopped")]
    Crashed,
}

/// One decoder state for one stream. A plug-in is untrusted: it runs on its own thread behind the guard.
pub trait AudioCodec: Send {
    /// Decodes one frame (or conceals a lost one: `erasure`, `frame` may be empty) and appends its samples to `pcm`, setting
    /// `pcm.rate_hz` and `pcm.channels`.
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError>;
}

/// Makes a decoder state for a stream of the given format. Returns `None` if the plug-in does not decode that format.
///
/// ```
/// use sdrkit_core::codec::{AudioCodec, CodecError, CodecFactory, CodecSet, Pcm};
/// use sdrkit_core::media::{Codec, MediaFormat, Packaging};
/// use std::sync::Arc;
///
/// /// Decodes a frame of one byte `b` into 160 samples of `b * 100`.
/// struct Level;
/// impl AudioCodec for Level {
///     fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
///         pcm.rate_hz = 8000;
///         pcm.channels = 1;
///         let v = if erasure { 0 } else { *frame.first().ok_or(CodecError::Frame)? as i16 * 100 };
///         pcm.samples.extend(std::iter::repeat_n(v, 160));
///         Ok(())
///     }
/// }
///
/// let factory: CodecFactory = Arc::new(|f: &MediaFormat| {
///     (f.packaging == Packaging::Raw).then(|| Box::new(Level) as Box<dyn AudioCodec>)
/// });
/// let mut set = CodecSet::default();
/// set.register(Codec::Ambe2.fourcc(), None, factory);
/// let entry = set.find(Codec::Ambe2.fourcc(), "dmr").unwrap();
/// let mut codec = (entry.factory)(&MediaFormat::new(Codec::Ambe2, Packaging::Raw)).unwrap();
/// let mut pcm = Pcm::default();
/// codec.decode(&[3], false, &mut pcm).unwrap();
/// assert_eq!((pcm.samples.len(), pcm.samples[0]), (160, 300));
/// ```
pub type CodecFactory = Arc<dyn Fn(&MediaFormat) -> Option<Box<dyn AudioCodec>> + Send + Sync>;

/// A registered plug-in.
#[derive(Clone)]
pub struct CodecEntry {
    /// The codec's four-character code (as in [`crate::media::CODECS`]).
    pub fourcc: FourCc,
    /// The frame layout it accepts ("dmr", "ysf144", "codewords"); `None` for any.
    pub layout: Option<String>,
    /// Makes a decoder state.
    pub factory: CodecFactory,
}

impl std::fmt::Debug for CodecEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodecEntry")
            .field("fourcc", &self.fourcc)
            .field("layout", &self.layout)
            .finish()
    }
}

/// The plug-ins a receiver may use, those turned off this session, and notices for the user.
#[derive(Clone, Debug, Default)]
pub struct CodecSet {
    entries: Vec<CodecEntry>,
    disabled: BTreeSet<FourCc>,
    notices: Vec<String>,
}

impl CodecSet {
    /// Registers (or replaces) the plug-in for `fourcc` in `layout` (`None`: any layout), and turns it on again.
    pub fn register(&mut self, fourcc: FourCc, layout: Option<&str>, factory: CodecFactory) {
        self.entries
            .retain(|e| !(e.fourcc == fourcc && e.layout.as_deref() == layout));
        self.entries.push(CodecEntry {
            fourcc,
            layout: layout.map(str::to_string),
            factory,
        });
        self.disabled.remove(&fourcc);
    }
    /// The plug-in for a stream of `fourcc` in `layout`, unless it was turned off. One registered for that exact layout is
    /// preferred over one for any layout.
    pub fn find(&self, fourcc: FourCc, layout: &str) -> Option<&CodecEntry> {
        if self.disabled.contains(&fourcc) {
            return None;
        }
        let mut fits = self.entries.iter().filter(|e| e.fourcc == fourcc);
        let exact = fits.clone().find(|e| e.layout.as_deref() == Some(layout));
        exact.or_else(|| fits.find(|e| e.layout.is_none()))
    }
    /// Whether a plug-in for `fourcc` is registered and on.
    pub fn has(&self, fourcc: FourCc) -> bool {
        !self.disabled.contains(&fourcc) && self.entries.iter().any(|e| e.fourcc == fourcc)
    }
    /// Turns off the plug-in for `fourcc` for this session (it hung or crashed) and adds a notice for the user.
    pub fn disable(&mut self, fourcc: FourCc, why: &CodecError) {
        if self.disabled.insert(fourcc) {
            let name = match Codec::from_fourcc(fourcc) {
                Some(c) => c.name(),
                None => fourcc.as_str(),
            };
            let what = match why {
                CodecError::Timeout => "stopped responding",
                CodecError::Crashed => "stopped",
                CodecError::Frame | CodecError::Unsupported => "failed",
            };
            self.notices
                .push(format!("the {name} plug-in {what} and was turned off"));
        }
    }
    /// Notices for the user since the last call (taken, so each is shown once).
    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Silence;
    impl AudioCodec for Silence {
        fn decode(&mut self, _: &[u8], _: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
            pcm.rate_hz = 8000;
            pcm.channels = 1;
            pcm.samples.extend([0; 160]);
            Ok(())
        }
    }

    #[test]
    fn a_5_1_frame_mixes_down_to_stereo_with_the_gains_of_the_stream() {
        // One frame before the 5.1 sound stays; L C R Ls Rs LFE = 1000 2000 3000 4000 5000 30000.
        let mut pcm = Pcm {
            samples: vec![7, 7, 7, 7, 7, 7, 1000, 2000, 3000, 4000, 5000, 30000],
            rate_hz: 48_000,
            channels: 6,
            layout: ChannelLayout::Surround51,
        };
        pcm.downmix_51(6, 0.5, 0.25);
        // Lo = (1000 + 1000 + 1000) / 1.75, Ro = (3000 + 1000 + 1250) / 1.75; the LFE is left out.
        assert_eq!(pcm.samples, [7, 7, 7, 7, 7, 7, 1714, 3000]);
        assert_eq!((pcm.channels, pcm.layout()), (2, ChannelLayout::Stereo));
    }

    #[test]
    fn an_exact_layout_wins_and_a_disabled_codec_is_not_found() {
        let mut set = CodecSet::default();
        let any: CodecFactory =
            Arc::new(|_: &MediaFormat| Some(Box::new(Silence) as Box<dyn AudioCodec>));
        let imbe = FourCc::new("imbe");
        set.register(imbe, None, any.clone());
        set.register(imbe, Some("p25"), any.clone());
        assert_eq!(
            set.find(imbe, "p25").unwrap().layout.as_deref(),
            Some("p25")
        );
        assert_eq!(set.find(imbe, "dmr").unwrap().layout, None);
        set.disable(imbe, &CodecError::Timeout);
        set.disable(imbe, &CodecError::Timeout);
        assert!(set.find(imbe, "p25").is_none() && !set.has(imbe));
        assert_eq!(
            set.take_notices(),
            ["the IMBE plug-in stopped responding and was turned off"]
        );
        assert!(set.take_notices().is_empty());
        set.register(imbe, None, any);
        assert!(set.has(imbe));
    }
}
