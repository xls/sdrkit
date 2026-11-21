//! Attached sync markers and the telecommand delimiters.
//!
//! A [`Marker`] is a fixed bit pattern sent before every frame. The markers here are those the CCSDS standards print, each with
//! its clause. [`Marker::sync_word`] and [`Marker::soft_sync`] give the `sdrkit-util-bits` matchers set up for it (hard bits with
//! allowed errors in both polarities, or LLR correlation). [`crate::framesync::FrameSync`] uses them to find and follow frames.
//!
//! ```
//! use sdrkit_proto_ccsds::marker;
//! assert_eq!(marker::TM.value(), 0x1ACF_FC1D);
//! assert_eq!(marker::TURBO_1_4.len_bits(), 128);
//! // The rate 1/4 turbo marker is the rate 1/2 one followed by its complement (CCSDS 131.0-B-5 clause 9.3.4).
//! assert_eq!(&marker::TURBO_1_4.bytes()[..8], marker::TURBO_1_2.bytes());
//! ```

use crate::{Error, Param, Result};
use sdrkit_util_bits::sync::{SoftSync, SyncWord};

/// A sync marker: its bytes, sent most significant bit first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    name: &'static str,
    bytes: &'static [u8],
}

impl Marker {
    /// A marker from its bytes (1 to 32 bytes).
    pub const fn new(name: &'static str, bytes: &'static [u8]) -> Self {
        Marker { name, bytes }
    }

    /// The marker's name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The marker's bytes in sending order.
    pub const fn bytes(&self) -> &'static [u8] {
        self.bytes
    }

    /// The marker's length in bits.
    pub const fn len_bits(&self) -> u32 {
        self.bytes.len() as u32 * 8
    }

    /// The last (at most) 128 bits of the marker as a number, first bit most significant. A matcher compares these. For the
    /// 192-bit rate 1/6 turbo marker the first 64 bits are not compared, which still leaves 128 bits for detection.
    pub const fn value(&self) -> u128 {
        let n = self.bytes.len();
        let start = n.saturating_sub(16);
        let mut v = 0u128;
        let mut i = start;
        while i < n {
            v = (v << 8) | self.bytes[i] as u128;
            i += 1;
        }
        v
    }

    /// The bits [`Marker::value`] holds (at most 128).
    pub const fn matched_bits(&self) -> u32 {
        let b = self.len_bits();
        if b > 128 {
            128
        } else {
            b
        }
    }

    /// A hard-bit matcher for the marker allowing `max_errors` differing bits (less than half the matched bits), reporting the
    /// inverted marker too when `inverted` (a 180 degree phase ambiguity).
    pub fn sync_word(&self, max_errors: u32, inverted: bool) -> Result<SyncWord> {
        SyncWord::new(self.value(), self.matched_bits(), max_errors)
            .map(|s| s.with_inverted(inverted))
            .map_err(|_| Error::BadParameter(Param::MarkerErrors))
    }

    /// A soft correlator for the marker (LLRs, positive means 0) reporting normalised scores of at least `threshold` (0 to 1), and
    /// the inverted marker as a negative score when `inverted`. Allocates its window here.
    pub fn soft_sync(&self, threshold: f32, inverted: bool) -> Result<SoftSync> {
        SoftSync::new(self.value(), self.matched_bits(), threshold)
            .map(|s| s.with_inverted(inverted))
            .map_err(|_| Error::BadParameter(Param::SoftMarkerThreshold))
    }

    /// Writes the marker's bits (0 or 1, first sent first) into `out`, inverted when `inverted`.
    pub fn write_bits(&self, inverted: bool, out: &mut [u8]) -> Result<()> {
        let need = self.bytes.len() * 8;
        if out.len() < need {
            return Err(Error::OutputTooShort { need });
        }
        for (i, o) in out[..need].iter_mut().enumerate() {
            *o = ((self.bytes[i / 8] >> (7 - i % 8)) & 1) ^ inverted as u8;
        }
        Ok(())
    }
}

/// The TM attached sync marker 1ACFFC1D (CCSDS 131.0-B-5 clause 9.3.3): uncoded, convolutional, Reed-Solomon, concatenated and
/// LDPC-coded TM frames, and AOS frames on the same links.
pub const TM: Marker = Marker::new("ccsds tm", &[0x1A, 0xCF, 0xFC, 0x1D]);

/// The marker of rate 1/2 turbo-coded frames, 034776C7272895B0 (CCSDS 131.0-B-5 clause 9.3.4).
pub const TURBO_1_2: Marker = Marker::new(
    "ccsds turbo 1/2",
    &[0x03, 0x47, 0x76, 0xC7, 0x27, 0x28, 0x95, 0xB0],
);

/// The marker of rate 1/3 turbo-coded frames, 25D5C0CE8990F6C9461BF79C (CCSDS 131.0-B-5 clause 9.3.4).
pub const TURBO_1_3: Marker = Marker::new(
    "ccsds turbo 1/3",
    &[
        0x25, 0xD5, 0xC0, 0xCE, 0x89, 0x90, 0xF6, 0xC9, 0x46, 0x1B, 0xF7, 0x9C,
    ],
);

/// The marker of rate 1/4 turbo-coded frames, 034776C7272895B0 FCB88938D8D76A4F (CCSDS 131.0-B-5 clause 9.3.4): the rate 1/2
/// marker followed by its complement.
pub const TURBO_1_4: Marker = Marker::new(
    "ccsds turbo 1/4",
    &[
        0x03, 0x47, 0x76, 0xC7, 0x27, 0x28, 0x95, 0xB0, 0xFC, 0xB8, 0x89, 0x38, 0xD8, 0xD7, 0x6A,
        0x4F,
    ],
);

/// The marker of rate 1/6 turbo-coded frames (CCSDS 131.0-B-5 clause 9.3.4): the rate 1/3 marker followed by its complement,
/// 192 bits.
pub const TURBO_1_6: Marker = Marker::new(
    "ccsds turbo 1/6",
    &[
        0x25, 0xD5, 0xC0, 0xCE, 0x89, 0x90, 0xF6, 0xC9, 0x46, 0x1B, 0xF7, 0x9C, 0xDA, 0x2A, 0x3F,
        0x31, 0x76, 0x6F, 0x09, 0x36, 0xB9, 0xE4, 0x08, 0x63,
    ],
);

/// The Proximity-1 attached sync marker FAF320, 24 bits (CCSDS 211.2-B clause 3.2).
pub const PROX1: Marker = Marker::new("ccsds proximity-1", &[0xFA, 0xF3, 0x20]);

/// The telecommand CLTU start sequence EB90 (CCSDS 231.0-B-4 clause 5.2.2).
pub const TC_START: Marker = Marker::new("ccsds cltu start", &[0xEB, 0x90]);

/// The telecommand CLTU tail sequence C5C5C5C5C5C5C579 (CCSDS 231.0-B-4 clause 5.2.4): a BCH code word that cannot be decoded.
pub const TC_TAIL: Marker = Marker::new(
    "ccsds cltu tail",
    &[0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0x79],
);

/// Every named marker, for lookup by name.
pub const ALL: &[Marker] = &[
    TM, TURBO_1_2, TURBO_1_3, TURBO_1_4, TURBO_1_6, PROX1, TC_START, TC_TAIL,
];

/// The marker with this name (as [`Marker::name`] returns it).
pub fn by_name(name: &str) -> Option<Marker> {
    ALL.iter().copied().find(|m| m.name == name)
}
