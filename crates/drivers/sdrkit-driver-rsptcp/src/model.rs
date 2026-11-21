//! The SDRplay RSP models, what each has, the LNA state tables and the split of one gain number into the two gain stages.

use crate::proto::caps;
use sdrkit_core::Hz;

/// An RSP model, from the SDRplay API hardware version of the extended greeting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// RSP1 (hardware version 1).
    Rsp1,
    /// RSP1A (255).
    Rsp1a,
    /// RSP1B (6).
    Rsp1b,
    /// RSP2 (2).
    Rsp2,
    /// RSPduo (3).
    RspDuo,
    /// RSPdx (4).
    RspDx,
    /// RSPdx-R2 (7).
    RspDxR2,
    /// Any other number: handled like an RSP1A.
    Unknown(u32),
}

/// One band of an LNA state table: the frequencies below `below` (the last band has infinity), and the reductions of each state.
type Band = (Hz, &'static [u8]);

const INF: Hz = f64::INFINITY;
const M: Hz = 1e6;

const RSP1: &[Band] = &[
    (420.0 * M, &[0, 24, 19, 43]),
    (1000.0 * M, &[0, 7, 19, 26]),
    (INF, &[0, 5, 19, 24]),
];
const RSP1A: &[Band] = &[
    (60.0 * M, &[0, 6, 12, 18, 37, 42, 61]),
    (420.0 * M, &[0, 6, 12, 18, 20, 26, 32, 38, 57, 62]),
    (1000.0 * M, &[0, 7, 13, 19, 20, 27, 33, 39, 45, 64]),
    (INF, &[0, 6, 12, 20, 26, 32, 38, 43, 62]),
];
const RSP1B: &[Band] = &[
    (50.0 * M, &[0, 6, 12, 18, 37, 42, 61]),
    (420.0 * M, &[0, 6, 12, 18, 20, 26, 32, 38, 57, 62]),
    (1000.0 * M, &[0, 7, 13, 19, 20, 27, 33, 39, 45, 64]),
    (INF, &[0, 6, 12, 20, 26, 32, 38, 43, 62]),
];
const RSP2: &[Band] = &[
    (420.0 * M, &[0, 10, 15, 21, 24, 34, 39, 45, 64]),
    (1000.0 * M, &[0, 7, 10, 17, 22, 41]),
    (INF, &[0, 5, 21, 15, 15, 34]),
];
const HIZ: &[Band] = &[(INF, &[0, 6, 12, 18, 37])];
const RSPDX: &[Band] = &[
    (
        12.0 * M,
        &[
            0, 3, 6, 9, 12, 15, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 60,
        ],
    ),
    (
        50.0 * M,
        &[
            0, 3, 6, 9, 12, 15, 18, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 60,
        ],
    ),
    (
        60.0 * M,
        &[
            0, 3, 6, 9, 12, 20, 23, 26, 29, 32, 35, 38, 44, 47, 50, 53, 56, 59, 62, 65, 68, 71, 74,
            77, 80,
        ],
    ),
    (
        250.0 * M,
        &[
            0, 3, 6, 9, 12, 15, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 60, 63, 66, 69, 72,
            75, 78, 81, 84,
        ],
    ),
    (
        420.0 * M,
        &[
            0, 3, 6, 9, 12, 15, 18, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54, 57, 60, 63, 66, 69,
            72, 75, 78, 81, 84,
        ],
    ),
    (
        1000.0 * M,
        &[
            0, 7, 10, 13, 16, 19, 22, 25, 31, 34, 37, 40, 43, 46, 49, 52, 55, 58, 61, 64, 67,
        ],
    ),
    (
        INF,
        &[
            0, 5, 8, 11, 14, 17, 20, 32, 35, 38, 41, 44, 47, 50, 53, 56, 59, 62, 65,
        ],
    ),
];

sdrkit_core::choice_enum! {
    /// The antenna input ([`crate::ANTENNA`]). A model has some of them. Its first input is the default.
    pub enum Antenna {
        /// The first input of the model.
        First = ("first", "First input"),
        /// Antenna A.
        A = ("a", "Antenna A"),
        /// Antenna B.
        B = ("b", "Antenna B"),
        /// Antenna C.
        C = ("c", "Antenna C"),
        /// The Hi-Z input of the RSP2.
        HiZ = ("hiz", "Hi-Z"),
        /// Tuner 1 of the RSPduo, 50 ohm.
        T1 = ("t1", "Tuner 1 (50 ohm)"),
        /// Tuner 2 of the RSPduo, 50 ohm.
        T2 = ("t2", "Tuner 2 (50 ohm)"),
        /// Tuner 1 of the RSPduo, Hi-Z.
        T1HiZ = ("t1hiz", "Tuner 1 Hi-Z"),
    }
}

/// The inputs of the RSP2, in the order of the values of command 0x1F.
pub const INPUTS_RSP2: &[Antenna] = &[Antenna::A, Antenna::B, Antenna::HiZ];
/// The inputs of the RSPduo.
pub const INPUTS_DUO: &[Antenna] = &[Antenna::T1, Antenna::T2, Antenna::T1HiZ];
/// The inputs of the RSPdx and the RSPdx-R2.
pub const INPUTS_DX: &[Antenna] = &[Antenna::A, Antenna::B, Antenna::C];

/// The tallest LNA state any table has, minus one: the top of the `lnastate` setting.
pub const MAX_LNA_STATE: u32 = 27;

/// The lowest and highest IF gain reduction.
pub const IFGR_MIN: u32 = 20;
/// The highest IF gain reduction.
pub const IFGR_MAX: u32 = 59;

impl Model {
    /// From the SDRplay API hardware version.
    pub fn from_hw(hw: u32) -> Model {
        match hw {
            1 => Model::Rsp1,
            2 => Model::Rsp2,
            3 => Model::RspDuo,
            4 => Model::RspDx,
            6 => Model::Rsp1b,
            7 => Model::RspDxR2,
            255 => Model::Rsp1a,
            n => Model::Unknown(n),
        }
    }

    /// The model's name ("RSP1A", "RSP (model 99)").
    pub fn name(&self) -> String {
        match self {
            Model::Rsp1 => "RSP1".into(),
            Model::Rsp1a => "RSP1A".into(),
            Model::Rsp1b => "RSP1B".into(),
            Model::Rsp2 => "RSP2".into(),
            Model::RspDuo => "RSPduo".into(),
            Model::RspDx => "RSPdx".into(),
            Model::RspDxR2 => "RSPdx-R2".into(),
            Model::Unknown(n) => format!("RSP (model {n})"),
        }
    }

    /// The antenna inputs in the order of the values of command 0x1F, when the model has more than one. Empty for one input.
    pub fn inputs(&self) -> &'static [Antenna] {
        match self {
            Model::Rsp2 => INPUTS_RSP2,
            Model::RspDuo => INPUTS_DUO,
            Model::RspDx | Model::RspDxR2 => INPUTS_DX,
            _ => &[],
        }
    }

    /// The 0x1F value of the Hi-Z input, on a model that has one.
    pub fn hiz_input(&self) -> Option<u32> {
        matches!(self, Model::Rsp2 | Model::RspDuo).then_some(2)
    }

    /// The model has a switchable bias-T.
    pub fn has_bias(&self) -> bool {
        !matches!(self, Model::Rsp1)
    }
    /// The model has the FM/MW broadcast notch.
    pub fn has_fm_notch(&self) -> bool {
        !matches!(self, Model::Rsp1)
    }
    /// The model has the DAB notch.
    pub fn has_dab_notch(&self) -> bool {
        !matches!(self, Model::Rsp1 | Model::Rsp2)
    }
    /// The model has the AM notch of the Hi-Z input.
    pub fn has_am_notch(&self) -> bool {
        matches!(self, Model::RspDuo)
    }
    /// The model has a clock output.
    pub fn has_refout(&self) -> bool {
        matches!(self, Model::Rsp2 | Model::RspDuo)
    }
    /// The model has any notch filter.
    pub fn has_notch(&self) -> bool {
        self.has_fm_notch() || self.has_dab_notch() || self.has_am_notch()
    }

    /// The frequency range it is specified for.
    pub fn freq_range(&self) -> (Hz, Hz) {
        match self {
            Model::Rsp1 => (10e3, 2e9),
            _ => (1e3, 2e9),
        }
    }

    /// The LNA state table, for the model and the input (Hi-Z or not).
    pub fn table(&self, hiz: bool) -> &'static [(Hz, &'static [u8])] {
        if hiz && self.hiz_input().is_some() {
            return HIZ;
        }
        match self {
            Model::Rsp1 => RSP1,
            Model::Rsp1b => RSP1B,
            Model::Rsp2 => RSP2,
            Model::RspDx | Model::RspDxR2 => RSPDX,
            Model::Rsp1a | Model::RspDuo | Model::Unknown(_) => RSP1A,
        }
    }

    /// The top of the gain range: the largest reduction in any band, plus 39.
    pub fn gmax(&self, hiz: bool) -> f64 {
        let top = self
            .table(hiz)
            .iter()
            .flat_map(|(_, r)| r.iter())
            .copied()
            .max()
            .unwrap_or(0);
        top as f64 + 39.0
    }

    /// The reductions of the band `f` is in: a frequency on a band edge belongs to the band above.
    pub fn band(&self, hiz: bool, f: Hz) -> &'static [u8] {
        let t = self.table(hiz);
        t.iter()
            .find(|(below, _)| f < *below)
            .unwrap_or(&t[t.len() - 1])
            .1
    }

    /// The number of LNA states in the band `f` is in.
    pub fn states(&self, hiz: bool, f: Hz) -> usize {
        self.band(hiz, f).len()
    }
}

/// What the capability word and the model together allow: a control is offered when the model has it and the capability bit
/// is set, or the capability word is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Controls {
    /// The antenna input (a model with several inputs).
    pub antenna: bool,
    /// Bias-T.
    pub bias: bool,
    /// The FM/MW notch.
    pub fm_notch: bool,
    /// The DAB notch.
    pub dab_notch: bool,
    /// The Hi-Z AM notch.
    pub am_notch: bool,
    /// The clock output.
    pub refout: bool,
}

impl Controls {
    /// From the model and the capability word.
    pub fn of(model: Model, cap: u32) -> Controls {
        let allowed = |bit: u32| cap == 0 || cap & bit != 0;
        Controls {
            antenna: model.inputs().len() > 1 && allowed(caps::ANTENNA),
            bias: model.has_bias() && allowed(caps::BIAS),
            fm_notch: model.has_fm_notch() && allowed(caps::FM_NOTCH),
            dab_notch: model.has_dab_notch() && allowed(caps::DAB_NOTCH),
            am_notch: model.has_am_notch() && allowed(caps::AM_NOTCH),
            refout: model.has_refout() && allowed(caps::REFOUT),
        }
    }

    /// Any notch is offered.
    pub fn any_notch(&self) -> bool {
        self.fm_notch || self.dab_notch || self.am_notch
    }

    /// The names of the controls offered, for the server info.
    pub fn names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        for (on, name) in [
            (self.antenna, "antenna"),
            (self.bias, "bias-T"),
            (self.fm_notch, "FM/MW notch"),
            (self.dab_notch, "DAB notch"),
            (self.am_notch, "Hi-Z MW notch"),
            (self.refout, "clock output"),
        ] {
            if on {
                v.push(name);
            }
        }
        v
    }
}

sdrkit_core::choice_enum! {
    /// How one gain number is shared between the LNA and the IF stage.
    pub enum GainSplit {
        /// Keep the LNA gain as long as possible (best noise figure).
        Sensitivity = ("sensitivity", "Sensitivity"),
        /// Give up LNA gain first (better with strong neighbours).
        Linearity = ("linearity", "Linearity"),
    }
}

/// The result of the split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stages {
    /// The LNA state sent with 0x20.
    pub lna_state: u32,
    /// Its reduction, dB.
    pub lna_reduction: u32,
    /// The IF gain reduction sent with 0x21, 20 to 59 dB.
    pub ifgr: u32,
}

/// Splits gain `gain` (dB, 0 to Gmax) at frequency `f` into an LNA state and an IF gain reduction. `lna_hand` and `ifgr_hand`
/// replace a stage when 0 or more. -1 follows the gain number. A hand LNA state past the last of the band returns the last.
pub fn split_gain(
    model: Model,
    hiz: bool,
    f: Hz,
    gain: f64,
    split: GainSplit,
    lna_hand: i32,
    ifgr_hand: i32,
) -> Stages {
    let gmax = model.gmax(hiz);
    let g = gain.clamp(0.0, gmax).round();
    let r = (gmax - g + 20.0) as i32;
    let band = model.band(hiz, f);
    let sensitivity = || {
        let need = r - 50;
        let mut best: Option<usize> = None;
        for (i, &red) in band.iter().enumerate() {
            if red as i32 >= need && best.is_none_or(|b| red < band[b]) {
                best = Some(i);
            }
        }
        best.unwrap_or_else(|| largest(band))
    };
    let state = if lna_hand >= 0 {
        (lna_hand as usize).min(band.len() - 1)
    } else {
        match split {
            GainSplit::Sensitivity => sensitivity(),
            GainSplit::Linearity => {
                let most = r - 20;
                let mut best: Option<usize> = None;
                for (i, &red) in band.iter().enumerate() {
                    if red as i32 <= most && best.is_none_or(|b| red > band[b]) {
                        best = Some(i);
                    }
                }
                best.unwrap_or_else(sensitivity)
            }
        }
    };
    let red = band[state] as i32;
    let ifgr = if ifgr_hand >= 0 { ifgr_hand } else { r - red }
        .clamp(IFGR_MIN as i32, IFGR_MAX as i32) as u32;
    Stages {
        lna_state: state as u32,
        lna_reduction: red as u32,
        ifgr,
    }
}

/// The state with the largest reduction (the first, on a tie).
fn largest(band: &[u8]) -> usize {
    let mut best = 0;
    for (i, &r) in band.iter().enumerate() {
        if r > band[best] {
            best = i;
        }
    }
    best
}
