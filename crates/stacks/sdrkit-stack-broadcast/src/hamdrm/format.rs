//! The parts of the HamDRM format that no document fixes, loaded at run time from a JSON file: the maps that the document does
//! not draw, the pilot phases, the FAC code, the values of some FAC bits, and the packet layer. The receiver and the generator
//! do nothing without the file.

use super::frame::{
    CellKind, CellMap, FacValues, HamMode, MapFault, FAC_BITS, FAC_CELLS, MSC_CELLS_PRINTED, RATE,
};
use crate::drm::mlc::interleaver_multiplier;
use crate::drm::params::{gain_phase, pilot_value, Qam};
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::{Llr8, Quantizer};
use sdrkit_algo_modem::ofdm::FrameLayout;
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;
use std::collections::BTreeMap;

/// Steps of the FAC code: the 40 data bits and the 6 tail bits of the mother code.
const FAC_STEPS: usize = FAC_BITS + 6;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawValues {
    long_interleave: u8,
    qam64: u8,
    data: u8,
    qam4: u8,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFormat {
    #[serde(default)]
    note: String,
    maps: BTreeMap<String, Vec<String>>,
    time_phases: BTreeMap<String, Vec<(i32, u16)>>,
    frequency_phases: BTreeMap<String, Vec<(i32, u16)>>,
    gain_phases_e: Vec<(usize, i32, u16)>,
    fac_puncture: Vec<u8>,
    fac_values: RawValues,
    packet_length: usize,
    application: String,
}

/// The loaded format.
#[derive(Clone, Debug)]
pub struct HamFormat {
    note: String,
    maps: [[CellMap; 2]; 3],
    time: [BTreeMap<i32, u16>; 3],
    freq: [BTreeMap<i32, u16>; 3],
    gain_e: BTreeMap<(usize, i32), u16>,
    fac_puncture: Vec<u8>,
    values: FacValues,
    packet_length: usize,
}

/// The key of a mode and occupancy in the file: "B/2.5".
pub fn map_key(mode: HamMode, wide: bool) -> String {
    format!("{}/{}", mode.letter(), super::frame::occupancy_text(wide))
}

/// A list of pilot phases in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseList {
    /// `time_phases`.
    Time,
    /// `frequency_phases`.
    Frequency,
}

impl std::fmt::Display for PhaseList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PhaseList::Time => "time_phases",
            PhaseList::Frequency => "frequency_phases",
        })
    }
}

/// A FAC value of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacBit {
    /// `long_interleave`.
    LongInterleave,
    /// `qam64`.
    Qam64,
    /// `data`.
    Data,
    /// `qam4`.
    Qam4,
}

impl std::fmt::Display for FacBit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FacBit::LongInterleave => "long_interleave",
            FacBit::Qam64 => "qam64",
            FacBit::Data => "data",
            FacBit::Qam4 => "qam4",
        })
    }
}

/// Why a format file is refused. The text starts with the field of the file that is wrong.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// The file cannot be read.
    #[error("format file: cannot be read ({kind})")]
    Read {
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// The text is not JSON of the format.
    #[error("format file: not the JSON of the format at line {line}, column {column}")]
    Json {
        /// The line.
        line: usize,
        /// The column.
        column: usize,
    },
    /// The file supplies a map that the document draws.
    #[error("maps.{}: the document draws this map, the file must not give it", map_key(*mode, *wide))]
    MapDrawn {
        /// The mode.
        mode: HamMode,
        /// The wide occupancy.
        wide: bool,
    },
    /// A map is missing.
    #[error("maps.{}: missing", map_key(*mode, *wide))]
    MapMissing {
        /// The mode.
        mode: HamMode,
        /// The wide occupancy.
        wide: bool,
    },
    /// A map is refused.
    #[error("maps.{}: {fault}", map_key(*mode, *wide))]
    Map {
        /// The mode.
        mode: HamMode,
        /// The wide occupancy.
        wide: bool,
        /// What is wrong.
        fault: MapFault,
    },
    /// A map has a different number of MSC cells than the document specifies.
    #[error("maps.{}: {got} MSC cells, the document gives {expected}", map_key(*mode, *wide))]
    MscCells {
        /// The mode.
        mode: HamMode,
        /// The wide occupancy.
        wide: bool,
        /// The cells of the map.
        got: usize,
        /// The number of cells the document specifies.
        expected: usize,
    },
    /// A key of `maps` is not a mode and occupancy.
    #[error("maps: a key that is not a mode and occupancy")]
    UnknownMap,
    /// The phases of a mode are missing.
    #[error("{list}.{}: missing", mode.letter())]
    PhasesMissing {
        /// The list.
        list: PhaseList,
        /// The mode.
        mode: HamMode,
    },
    /// A phase is 1024 or more.
    #[error("{list}.{}: carrier {carrier} has phase {phase}, not below 1024", mode.letter())]
    PhaseTooLarge {
        /// The list.
        list: PhaseList,
        /// The mode.
        mode: HamMode,
        /// The carrier.
        carrier: i32,
        /// The phase.
        phase: u16,
    },
    /// A carrier is given twice.
    #[error("{list}.{}: carrier {carrier} is given twice", mode.letter())]
    PhaseTwice {
        /// The list.
        list: PhaseList,
        /// The mode.
        mode: HamMode,
        /// The carrier.
        carrier: i32,
    },
    /// A gain phase of mode E is 1024 or more.
    #[error("gain_phases_e: symbol {symbol} carrier {carrier} has phase {phase}, not below 1024")]
    GainPhaseTooLarge {
        /// The symbol.
        symbol: usize,
        /// The carrier.
        carrier: i32,
        /// The phase.
        phase: u16,
    },
    /// A gain phase of mode E is given twice.
    #[error("gain_phases_e: symbol {symbol} carrier {carrier} is given twice")]
    GainPhaseTwice {
        /// The symbol.
        symbol: usize,
        /// The carrier.
        carrier: i32,
    },
    /// The puncturing of the FAC is not a pattern of 0 and 1 over whole steps of 6 bits.
    #[error("fac_puncture: a pattern of 0 and 1 over whole steps of 6 mother code bits")]
    FacPattern,
    /// The puncturing of the FAC keeps another number of bits than the FAC cells hold.
    #[error("fac_puncture: keeps {kept} mother code bits, the FAC cells hold 90")]
    FacKept {
        /// The bits it keeps.
        kept: usize,
    },
    /// A FAC value is not 0 or 1.
    #[error("fac_values.{bit}: {value} is not 0 or 1")]
    FacValue {
        /// The value.
        bit: FacBit,
        /// The value in the file.
        value: u8,
    },
    /// The packet length is not 1 to 255 bytes.
    #[error("packet_length: {got} is not 1 to 255 bytes")]
    PacketLength {
        /// The count in the file.
        got: usize,
    },
    /// The application is not MOT.
    #[error("application: not \"mot\"")]
    Application,
    /// A pilot of a map has no value. The text names the list that lacks it.
    #[error("{}: no value for the pilot of map {} at symbol {symbol} carrier {carrier}", pilot_list(*kind, *mode, *wide), map_key(*mode, *wide))]
    NoPilot {
        /// The mode.
        mode: HamMode,
        /// The wide occupancy.
        wide: bool,
        /// The symbol.
        symbol: usize,
        /// The carrier.
        carrier: i32,
        /// The kind of pilot.
        kind: CellKind,
    },
}

/// Returns the name of the file field that holds the value of a pilot of `kind` in the map of `mode` and `wide`.
fn pilot_list(kind: CellKind, mode: HamMode, wide: bool) -> String {
    match (kind, mode) {
        (CellKind::Time, _) => format!("{}.{}", PhaseList::Time, mode.letter()),
        (CellKind::Frequency, _) => format!("{}.{}", PhaseList::Frequency, mode.letter()),
        (_, HamMode::E) => "gain_phases_e".into(),
        _ => format!("maps.{}", map_key(mode, wide)),
    }
}

fn bit(bit: FacBit, value: u8) -> Result<u8, FormatError> {
    if value > 1 {
        return Err(FormatError::FacValue { bit, value });
    }
    Ok(value)
}

fn phases(
    list: PhaseList,
    mode: HamMode,
    given: &[(i32, u16)],
) -> Result<BTreeMap<i32, u16>, FormatError> {
    let mut m = BTreeMap::new();
    for &(carrier, phase) in given {
        if phase >= 1024 {
            return Err(FormatError::PhaseTooLarge {
                list,
                mode,
                carrier,
                phase,
            });
        }
        if m.insert(carrier, phase).is_some() {
            return Err(FormatError::PhaseTwice {
                list,
                mode,
                carrier,
            });
        }
    }
    Ok(m)
}

impl HamFormat {
    /// Reads the format from its JSON text.
    ///
    /// # Errors
    ///
    /// [`FormatError`]: its text starts with the field that is wrong.
    pub fn from_json(text: &str) -> Result<HamFormat, FormatError> {
        let raw: RawFormat = serde_json::from_str(text).map_err(|e| FormatError::Json {
            line: e.line(),
            column: e.column(),
        })?;
        // The maps: the drawn ones from the document, the others from the file.
        let mut maps: Vec<[CellMap; 2]> = Vec::with_capacity(3);
        for m in HamMode::ALL {
            let mut pair = Vec::with_capacity(2);
            for wide in [false, true] {
                let key = map_key(m, wide);
                if wide == m.drawn_wide() {
                    if raw.maps.contains_key(&key) {
                        return Err(FormatError::MapDrawn { mode: m, wide });
                    }
                    pair.push(CellMap::drawn(m));
                    continue;
                }
                let rows = raw
                    .maps
                    .get(&key)
                    .ok_or(FormatError::MapMissing { mode: m, wide })?;
                let map = CellMap::parse(m, wide, rows).map_err(|fault| FormatError::Map {
                    mode: m,
                    wide,
                    fault,
                })?;
                let expected = MSC_CELLS_PRINTED[m.index()][wide as usize];
                let got = map.count(CellKind::Msc);
                if got != expected {
                    return Err(FormatError::MscCells {
                        mode: m,
                        wide,
                        got,
                        expected,
                    });
                }
                pair.push(map);
            }
            let mut pair = pair.into_iter();
            if let (Some(a), Some(b)) = (pair.next(), pair.next()) {
                maps.push([a, b]);
            }
        }
        if raw.maps.keys().any(|k| {
            !HamMode::ALL
                .iter()
                .any(|&m| [false, true].iter().any(|&w| map_key(m, w) == **k))
        }) {
            return Err(FormatError::UnknownMap);
        }
        let mut maps = maps.into_iter();
        let (Some(ma), Some(mb), Some(me)) = (maps.next(), maps.next(), maps.next()) else {
            unreachable!("three modes, two maps each")
        };
        let maps = [ma, mb, me];
        let mut time: [BTreeMap<i32, u16>; 3] = Default::default();
        let mut freq: [BTreeMap<i32, u16>; 3] = Default::default();
        for m in HamMode::ALL {
            let l = m.letter();
            let t = raw.time_phases.get(l).ok_or(FormatError::PhasesMissing {
                list: PhaseList::Time,
                mode: m,
            })?;
            time[m.index()] = phases(PhaseList::Time, m, t)?;
            let f = raw
                .frequency_phases
                .get(l)
                .ok_or(FormatError::PhasesMissing {
                    list: PhaseList::Frequency,
                    mode: m,
                })?;
            freq[m.index()] = phases(PhaseList::Frequency, m, f)?;
        }
        let mut gain_e = BTreeMap::new();
        for &(symbol, carrier, phase) in &raw.gain_phases_e {
            if phase >= 1024 {
                return Err(FormatError::GainPhaseTooLarge {
                    symbol,
                    carrier,
                    phase,
                });
            }
            if gain_e.insert((symbol, carrier), phase).is_some() {
                return Err(FormatError::GainPhaseTwice { symbol, carrier });
            }
        }
        if raw.fac_puncture.is_empty()
            || !raw.fac_puncture.len().is_multiple_of(6)
            || raw.fac_puncture.iter().any(|&x| x > 1)
        {
            return Err(FormatError::FacPattern);
        }
        let kept = Puncture::new(&raw.fac_puncture).punctured_len(0, 6 * FAC_STEPS);
        if kept != 2 * FAC_CELLS {
            return Err(FormatError::FacKept { kept });
        }
        let v = &raw.fac_values;
        let values = FacValues {
            long_interleave: bit(FacBit::LongInterleave, v.long_interleave)?,
            qam64: bit(FacBit::Qam64, v.qam64)?,
            data: bit(FacBit::Data, v.data)?,
            qam4: bit(FacBit::Qam4, v.qam4)?,
        };
        if !(1..=255).contains(&raw.packet_length) {
            return Err(FormatError::PacketLength {
                got: raw.packet_length,
            });
        }
        if raw.application != "mot" {
            return Err(FormatError::Application);
        }
        let f = HamFormat {
            note: raw.note,
            maps,
            time,
            freq,
            gain_e,
            fac_puncture: raw.fac_puncture,
            values,
            packet_length: raw.packet_length,
        };
        // Every pilot of every map has a value.
        for m in HamMode::ALL {
            for wide in [false, true] {
                let map = f.map(m, wide);
                for s in 0..map.nsym() {
                    for c in 0..map.carriers() {
                        if map.kind(s, c).is_pilot() && f.pilot(map, s, c).is_none() {
                            return Err(FormatError::NoPilot {
                                mode: m,
                                wide,
                                symbol: s,
                                carrier: map.kmin() + c as i32,
                                kind: map.kind(s, c),
                            });
                        }
                    }
                }
            }
        }
        Ok(f)
    }

    /// Reads the format from the file at `path`.
    ///
    /// # Errors
    ///
    /// [`FormatError::Read`] if the file cannot be read, else as [`HamFormat::from_json`].
    pub fn load(path: &str) -> Result<HamFormat, FormatError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| FormatError::Read { kind: e.kind() })?;
        Self::from_json(&text)
    }

    /// The note of the file (where the format comes from).
    pub fn note(&self) -> &str {
        &self.note
    }
    /// The map of a mode and occupancy.
    pub fn map(&self, mode: HamMode, wide: bool) -> &CellMap {
        &self.maps[mode.index()][wide as usize]
    }
    /// The bit values of the FAC options.
    pub fn values(&self) -> &FacValues {
        &self.values
    }
    /// The bytes of a packet: header, data field of `packet_length` bytes, and CRC (ES 201 980 clause 6.6.4).
    pub fn packet_bytes(&self) -> usize {
        self.packet_length + 3
    }

    /// The value of the pilot of `map` at symbol `s` and column `c`, `None` when the cell is not a pilot or has no value. Time,
    /// frequency and scattered pilots have amplitude √2, boosted scattered pilots 2 (ES 201 980 clause 8.4). The scattered
    /// pilots of modes A and B have the phases of clause 8.4.4.3, the others the phases of the file.
    pub fn pilot(&self, map: &CellMap, s: usize, c: usize) -> Option<Complex32> {
        let m = map.mode();
        let k = map.kmin() + c as i32;
        let sqrt2 = std::f32::consts::SQRT_2;
        let (a, theta) = match map.kind(s, c) {
            CellKind::Msc | CellKind::Fac => return None,
            CellKind::Time => (sqrt2, *self.time[m.index()].get(&k)?),
            CellKind::Frequency => (sqrt2, *self.freq[m.index()].get(&k)?),
            kind => {
                let a = if kind == CellKind::Boosted {
                    2.0
                } else {
                    sqrt2
                };
                let t = match m.drm() {
                    Some(r) => gain_phase(r, s, k)?,
                    None => *self.gain_e.get(&(s, k))?,
                };
                (a, t)
            }
        };
        Some(pilot_value(a, theta))
    }

    /// The OFDM layout of a mode and occupancy, its known cells the pilots of the map.
    pub fn layout(&self, mode: HamMode, wide: bool) -> FrameLayout {
        let map = self.map(mode, wide);
        layout_with(map, |s, c| self.pilot(map, s, c))
    }

    /// The FAC code of the file.
    pub fn fac_code(&self) -> FacCode {
        FacCode::new(&self.fac_puncture)
    }
}

/// The layout of `map` with the known cells `known(s, c)`.
pub(crate) fn layout_with(
    map: &CellMap,
    known: impl Fn(usize, usize) -> Option<Complex32>,
) -> FrameLayout {
    let m = map.mode();
    let mut rows = vec![Vec::new(); map.nsym()];
    for (s, row) in rows.iter_mut().enumerate() {
        for c in 0..map.carriers() {
            if let Some(v) = known(s, c) {
                row.push((c, v));
            }
        }
    }
    FrameLayout {
        rate: RATE,
        nfft: m.nfft(),
        cp: m.cp(),
        cp0: m.cp(),
        nsym: m.nsym(),
        kmin: map.kmin(),
        kmax: map.kmax(),
        known: rows,
        null: vec![],
        // One carrier: the scattered pilot patterns repeat at shifts of whole carriers and symbols, and a wider search
        // locks on those aliases.
        max_shift: 1,
        adv: m.cp() / 2,
        sync_threshold: 0.5,
        guard_threshold: 0.3,
        search_taps: None,
        pair_band: None,
    }
}

/// The FAC code: the 40 bits and the 6 tail bits through the DRM mother code (ES 201 980 clause 7.3.2), the file's puncturing,
/// the 4-QAM bit interleaver (clause 7.3.3) and the 4-QAM mapping (clause 7.4) on 45 cells.
#[derive(Clone, Debug)]
pub struct FacCode {
    mask: Vec<u8>,
    il: Permutation,
    vit: Viterbi,
    quant: Quantizer,
    mother: Vec<u8>,
    sent: Vec<u8>,
    soft: Vec<f32>,
    deint: Vec<f32>,
    msoft: Vec<f32>,
    llr: Vec<Llr8>,
}

impl FacCode {
    /// The code with the puncturing `mask` (a keep pattern over the mother code bits). The caller makes sure that the mask
    /// keeps 90 bits of the 276.
    pub fn new(mask: &[u8]) -> FacCode {
        let n = 2 * FAC_CELLS;
        let t0 = interleaver_multiplier(Qam::Q4, 0).unwrap_or(1);
        FacCode {
            mask: mask.to_vec(),
            il: Permutation::drm(n, t0)
                .map(Permutation::inverse)
                .unwrap_or_else(|_| Permutation::identity(n)),
            vit: Viterbi::new(codes::DRM_K7_R6, FAC_STEPS).expect("a Viterbi decoder of 46 steps"),
            quant: Quantizer::default(),
            mother: Vec::with_capacity(6 * FAC_STEPS),
            sent: Vec::with_capacity(n),
            soft: vec![0.0; n],
            deint: vec![0.0; n],
            msoft: vec![0.0; 6 * FAC_STEPS],
            llr: vec![0; 6 * FAC_STEPS],
        }
    }

    /// Encodes the 40 FAC bits into the 45 FAC cells.
    pub fn encode(&mut self, bits: &[u8], cells: &mut [Complex32]) {
        self.mother.clear();
        let mut enc = Encoder::new(codes::DRM_K7_R6);
        let mut info = [0u8; FAC_BITS];
        for (o, b) in info.iter_mut().zip(bits) {
            *o = b & 1;
        }
        enc.encode(&info, &mut self.mother);
        enc.terminate(&mut self.mother);
        self.sent.clear();
        puncture(&Puncture::new(&self.mask), 0, &self.mother, &mut self.sent);
        let mut x = vec![0u8; self.sent.len()];
        let _ = self.il.apply(&self.sent, &mut x);
        for (c, cell) in cells.iter_mut().enumerate().take(FAC_CELLS) {
            *cell = Qam::Q4.map(x[2 * c] as usize, x[2 * c + 1] as usize);
        }
    }

    /// Decodes 45 equalised FAC cells with their weights into 40 bits.
    pub fn decode(&mut self, z: &[Complex32], w: &[f32], out: &mut Vec<u8>) {
        let soft = |y: f32, wc: f32| {
            let d0 = (y - Qam::Q4.axis(0)).powi(2);
            let d1 = (y - Qam::Q4.axis(1)).powi(2);
            wc * (d1 - d0)
        };
        for c in 0..FAC_CELLS {
            let (zc, wc) = match (z.get(c), w.get(c)) {
                (Some(&zc), Some(&wc)) if wc.is_finite() => (zc, wc),
                _ => (Complex32::default(), 0.0),
            };
            self.soft[2 * c] = soft(zc.re, wc);
            self.soft[2 * c + 1] = soft(zc.im, wc);
        }
        let _ = self.il.invert(&self.soft, &mut self.deint);
        let _ = depuncture_frame(&Puncture::new(&self.mask), 0, &self.deint, &mut self.msoft);
        let _ = self.quant.quantize_auto(&self.msoft, &mut self.llr);
        let mut info = [0u8; FAC_BITS];
        let _ = self.vit.decode(&self.llr, Ends::Zero, &mut info);
        out.clear();
        out.extend_from_slice(&info);
    }
}
