//! The values that the results of the DVB-T receiver show: the state of the receiver, the quality class and the
//! standard that decodes. The shapes of the results are in [`crate::typed`].

use sdrkit_proto_mpegts::layer::TsLayer;

/// The parts of the transport stream layer after the DVB-T decoder: teletext and subtitles.
pub const TS_LAYER: TsLayer = TsLayer {
    teletext: true,
    subtitles: true,
};

sdrkit_core::choice_enum! {
    /// The state of the receiver, shown by the status lock state and line.
    pub enum RxState {
        /// Looking for a DVB-T signal.
        Searching = ("searching", "Searching"),
        /// The FFT mode and the guard interval are known.
        Tracking = ("tracking", "Tracking"),
        /// TPS is locked.
        Locked = ("locked", "Locked"),
        /// Transport packets flow.
        Streaming = ("streaming", "Streaming"),
    }
}

sdrkit_core::choice_enum! {
    /// The quality class of the reception.
    pub enum Quality {
        /// 85 % or more.
        Excellent = ("excellent", "Excellent"),
        /// 65 % or more.
        Good = ("good", "Good"),
        /// 40 % or more.
        Marginal = ("marginal", "Marginal"),
        /// Below 40 %.
        Poor = ("poor", "Poor"),
        /// No lock.
        NoLock = ("no_lock", "No lock"),
    }
}

sdrkit_core::choice_enum! {
    /// The standard that decodes now (DVB-T2 / DVB-T stack).
    pub enum Decoding {
        /// DVB-T2.
        Dvbt2 = ("dvbt2", "DVB-T2"),
        /// DVB-T.
        Dvbt = ("dvbt", "DVB-T"),
    }
}

/// The quality class of a score.
pub fn quality_of(pct: f32, locked: bool) -> Quality {
    if !locked {
        Quality::NoLock
    } else if pct >= 85.0 {
        Quality::Excellent
    } else if pct >= 65.0 {
        Quality::Good
    } else if pct >= 40.0 {
        Quality::Marginal
    } else {
        Quality::Poor
    }
}
