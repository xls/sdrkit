//! The inner code of DVB-S and ISDB-S: the code rates of the punctured K = 7 code, taken from `sdrkit-algo-fec`.
//!
//! Standard: ETSI EN 300 421 clause 4.4.3, table 2.

use sdrkit_algo_fec::conv::{codes, ConvCode, Encoder, Puncture, Viterbi};

/// The mother code: K = 7, X = 171 and Y = 133 octal, X sent first.
pub const INNER: ConvCode = codes::DVB_K7;

/// Trace-back depth of the streaming Viterbi decoder, in steps.
pub const TRACEBACK: usize = 96;

/// Bits released at a time by the streaming Viterbi decoder.
pub const CHUNK: usize = 256;

sdrkit_core::choice_enum! {
    /// A code rate of EN 300 421 table 2.
    pub enum PunctureRate {
        /// 1/2.
        R12 = ("1/2", "1/2"),
        /// 2/3.
        R23 = ("2/3", "2/3"),
        /// 3/4.
        R34 = ("3/4", "3/4"),
        /// 5/6.
        R56 = ("5/6", "5/6"),
        /// 7/8.
        R78 = ("7/8", "7/8"),
    }
}

impl PunctureRate {
    /// Every rate, lowest first.
    pub const ALL: [PunctureRate; 5] = [Self::R12, Self::R23, Self::R34, Self::R56, Self::R78];

    /// The puncturing pattern over the serial stream X1 Y1 X2 Y2 ... of the mother code.
    pub fn puncture(self) -> Puncture<'static> {
        match self {
            Self::R12 => codes::NONE_1_2,
            Self::R23 => codes::DVB_2_3,
            Self::R34 => codes::DVB_3_4,
            Self::R56 => codes::DVB_5_6,
            Self::R78 => codes::DVB_7_8,
        }
    }

    /// Input bits per puncturing period.
    pub fn period(self) -> usize {
        self.puncture().period() / 2
    }

    /// Sent bits per puncturing period.
    pub fn coded(self) -> usize {
        self.puncture().kept()
    }

    /// The code rate k / n.
    pub fn value(self) -> f64 {
        self.period() as f64 / self.coded() as f64
    }

    /// The rate as text, for example "3/4".
    pub fn name(self) -> &'static str {
        match self {
            Self::R12 => "1/2",
            Self::R23 => "2/3",
            Self::R34 => "3/4",
            Self::R56 => "5/6",
            Self::R78 => "7/8",
        }
    }

    /// The place in the mother-code pattern of sent bit `phase` of a period. At rate 3/4 the sent bits are X1 Y1 Y2 X3, so sent
    /// bit 2 is place 3.
    pub fn pattern_index(self, phase: usize) -> usize {
        let p = self.puncture();
        p.keep()
            .iter()
            .enumerate()
            .filter(|(_, &k)| k == 1)
            .nth(phase % p.kept())
            .map_or(0, |(i, _)| i)
    }

    /// Whether sent bit `phase` of a period is a Y of the mother code. A stream that starts with such a bit needs one erased
    /// value before it, so that the decoder takes its pairs as X then Y.
    pub fn starts_with_y(self, phase: usize) -> bool {
        self.pattern_index(phase) % 2 == 1
    }
}

/// A streaming Viterbi decoder of the mother code with the trace-back of this crate.
pub fn inner_decoder() -> Viterbi {
    // The code and the window are constants of this crate, so both calls always succeed.
    let mut v = Viterbi::new(INNER, CHUNK).unwrap_or_else(|_| unreachable!("K = 7 is valid"));
    v.set_traceback(TRACEBACK, CHUNK)
        .unwrap_or_else(|_| unreachable!("the window is valid"));
    v
}

/// The encoder of the mother code, from state 0.
pub fn encoder() -> Encoder {
    Encoder::new(INNER)
}
