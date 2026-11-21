//! The voice modes of the transmitter: the settings of the stack as the shared speech processor and modulators take them
//! (`sdrkit_components::voice`).

use super::settings::{Channel, SsbBand, TxSettings};
use crate::types::Demod;
use sdrkit_components::voice::{self as shared, VoiceMode, SSB_BAND_HZ};

/// The voice mode of `ch.demod` (NFM, AM, USB or LSB) with the settings `s`. `None` for WFM and CW.
pub fn mode(ch: &Channel, s: &TxSettings) -> Option<VoiceMode> {
    Some(match ch.demod {
        Demod::Nfm => VoiceMode::Fm {
            deviation_hz: s.deviation,
            preemphasis_s: s.preemphasis.micros() * 1e-6,
            ctcss_hz: s.ctcss,
        },
        Demod::Am => VoiceMode::Am {
            depth: s.am_depth / 100.0,
            audio_hz: (ch.width / 2.0).clamp(1000.0, 5000.0),
        },
        Demod::Usb | Demod::Lsb => VoiceMode::Ssb {
            upper: ch.demod == Demod::Usb,
            method: s.ssb_method,
            band_hz: match s.ssb_band {
                SsbBand::Fixed => SSB_BAND_HZ,
                SsbBand::Filter => (SSB_BAND_HZ.0, SSB_BAND_HZ.0 + ch.width),
            },
            compression_db: s.compression,
        },
        Demod::Wfm | Demod::Cw => return None,
    })
}

/// The speech processor and the modulator of one voice mode of the stack.
pub struct VoiceChain(shared::VoiceChain);

impl VoiceChain {
    /// The chain of the voice mode `ch.demod` (NFM, AM, USB or LSB) with the settings `s`. `None` for WFM and CW.
    pub fn new(ch: &Channel, s: &TxSettings) -> Option<Self> {
        shared::VoiceChain::new(&mode(ch, s)?).map(VoiceChain)
    }

    /// The shared chain.
    pub fn into_inner(self) -> shared::VoiceChain {
        self.0
    }
}

impl std::ops::Deref for VoiceChain {
    type Target = shared::VoiceChain;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for VoiceChain {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
