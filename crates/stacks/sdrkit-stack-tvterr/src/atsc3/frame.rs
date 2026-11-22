//! The symbols of an ATSC 3.0 frame as the bootstrap and the L1 signalling describe them: the Preamble symbols and the symbols
//! of the first subframe, with their carriers, guards and the data cells that take part in cell multiplexing.
//!
//! Standard: ATSC A/322:2023-03 clauses 7.2.2 to 7.2.6 and 8.5.

use super::l1::{DetailSubframe, FrameLength, L1Basic, SubframeParams};
use super::layout::{
    boundary_active_cells, common_pilots, guard_samples, Carriers, FftSize, Pattern,
    PreambleStructure, SymbolKind,
};

sdrkit_core::choice_enum! {
    /// Why the stack does not decode a frame.
    pub enum NotDecoded {
        /// The bootstrap signals a preamble structure of the reserved range.
        Structure = ("structure", "reserved preamble structure"),
        /// The frame runs at a sample rate other than 6.144 Msps.
        SampleRate = ("sample_rate", "sample rate other than 6.144 Msps"),
        /// L1-Basic did not decode or its CRC is wrong.
        L1Basic = ("l1_basic", "L1-Basic not read"),
        /// L1-Detail did not decode or its CRC is wrong.
        L1Detail = ("l1_detail", "L1-Detail not read"),
        /// L1-Detail is sent in several coded blocks with additional parity.
        AdditionalParity = ("additional_parity", "L1-Detail blocks with additional parity"),
        /// The L1 signalling holds a reserved or impossible value.
        Signalling = ("signalling", "reserved value in the L1 signalling"),
        /// The subframe uses MIMO.
        Mimo = ("mimo", "MIMO"),
        /// The subframe uses MISO.
        Miso = ("miso", "MISO"),
        /// The frame reserves carriers for PAPR reduction.
        ToneReservation = ("tone_reservation", "tone reservation"),
        /// No PLP of the first subframe is one that the stack decodes.
        Plp = ("plp", "no PLP that the stack decodes"),
        /// The samples of the frame did not all arrive.
        Samples = ("samples", "samples missing"),
        /// The PLP is in a subframe with an enhanced layer, in a layout that the stack does not decode.
        Ldm = ("ldm", "layered division multiplexing"),
        /// The first subframe ends more than 1 s after the bootstrap.
        Length = ("length", "subframe longer than 1 s"),
        /// One or more FEC blocks of the PLP did not decode.
        Fec = ("fec", "FEC blocks not decoded"),
    }
}

/// One OFDM symbol of a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    /// Its carriers.
    pub carriers: Carriers,
    /// Its guard interval, samples.
    pub guard: usize,
    /// The spacing of its carriers that bear Preamble, scattered or boundary pilots.
    pub grid: usize,
    /// Its number for the frequency interleaver (the Preamble symbols count in the first subframe).
    pub l: usize,
    /// Its data cells are frequency interleaved.
    pub interleaved: bool,
    /// The null cells before its first active data cell (a subframe boundary symbol).
    pub null_low: usize,
    /// Its active data cells: the data cells without the null cells.
    pub active: usize,
}

impl Symbol {
    /// The samples of the symbol with its guard.
    pub fn samples(&self) -> usize {
        self.carriers.fft.size() + self.guard
    }
}

/// The Preamble symbols of a frame: `count` symbols of `structure`, the first with the fewest carriers, the others at carrier
/// reduction `cred` (clause 7.2.5.1).
pub fn preamble_symbols(structure: &PreambleStructure, count: u8, cred: u8) -> Vec<Symbol> {
    (0..usize::from(count))
        .map(|l| {
            let carriers = Carriers::new(
                structure.fft,
                if l == 0 { 4 } else { cred.min(4) },
                SymbolKind::Preamble {
                    dx: structure.dx,
                    amplitude: structure.pilot_amplitude,
                },
            );
            let active = carriers.data_cells();
            Symbol {
                carriers,
                guard: structure.guard,
                grid: structure.dx,
                l,
                interleaved: true,
                null_low: 0,
                active,
            }
        })
        .collect()
}

/// What of a subframe's waveform the layout needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waveform {
    /// The FFT size.
    pub fft: FftSize,
    /// The guard interval as signalled, samples.
    pub guard: usize,
    /// The scattered pilot pattern.
    pub pattern: Pattern,
    /// The amplitude of the scattered pilots.
    pub amplitude: f32,
}

impl Waveform {
    /// The waveform that `p` signals.
    ///
    /// # Errors
    ///
    /// The reason when the stack does not decode it.
    pub fn of(p: &SubframeParams) -> Result<Waveform, NotDecoded> {
        if p.mimo {
            return Err(NotDecoded::Mimo);
        }
        if p.miso != 0 {
            return Err(NotDecoded::Miso);
        }
        let pattern = Pattern::from_code(p.pilot_pattern).ok_or(NotDecoded::Signalling)?;
        if p.reduced_carriers > 4 {
            return Err(NotDecoded::Signalling);
        }
        Ok(Waveform {
            fft: FftSize::from_code(p.fft_size).ok_or(NotDecoded::Signalling)?,
            guard: guard_samples(p.guard_interval).ok_or(NotDecoded::Signalling)?,
            pattern,
            amplitude: pattern
                .amplitude(p.pilot_boost)
                .ok_or(NotDecoded::Signalling)?,
        })
    }
}

/// The null cells of each subframe boundary symbol of a subframe with waveform `p` (Annex F).
///
/// # Errors
///
/// As [`Waveform::of`].
pub fn boundary_null_cells(p: &SubframeParams) -> Result<usize, NotDecoded> {
    let w = Waveform::of(p)?;
    let data = Carriers::new(
        w.fft,
        p.reduced_carriers,
        SymbolKind::Data {
            pattern: w.pattern,
            amplitude: w.amplitude,
            l: 0,
        },
    )
    .data_cells();
    let boundary = Carriers::new(
        w.fft,
        p.reduced_carriers,
        SymbolKind::Boundary {
            pattern: w.pattern,
            amplitude: w.amplitude,
        },
    );
    let common = common_pilots(w.fft, p.reduced_carriers).len();
    Ok(boundary.data_cells() - boundary_active_cells(data, &boundary, common, w.amplitude))
}

/// The data and subframe boundary symbols of the first subframe of a frame with L1-Basic `basic`, whose L1-Detail entry is
/// `detail`. The symbol numbers go on from the `preamble` Preamble symbols. In a time-aligned frame the guards hold the
/// excess samples.
///
/// # Errors
///
/// The reason when the stack does not decode the subframe.
pub fn first_subframe(
    basic: &L1Basic,
    detail: &DetailSubframe,
    preamble: usize,
) -> Result<Vec<Symbol>, NotDecoded> {
    subframe_symbols(
        &basic.first,
        basic.papr,
        excess_samples(basic),
        detail,
        preamble,
    )
}

/// The samples that each symbol after the Preamble holds more in its guard in a time-aligned frame, in every subframe
/// (clause 8.5.1).
pub fn excess_samples(basic: &L1Basic) -> usize {
    match basic.frame_length {
        FrameLength::TimeAligned { excess_samples, .. } => usize::from(excess_samples),
        FrameLength::SymbolAligned { .. } => 0,
    }
}

/// The data and subframe boundary symbols of a subframe with the waveform `p` and the L1-Detail entry `detail`, in a
/// frame whose PAPR code is `papr`. Each guard holds `extra` samples more. The first symbol has the number `first_l`: the
/// count of Preamble symbols for the first subframe, 0 for the others.
///
/// # Errors
///
/// The reason when the stack does not decode the subframe.
pub fn subframe_symbols(
    p: &SubframeParams,
    papr: u8,
    extra: usize,
    detail: &DetailSubframe,
    first_l: usize,
) -> Result<Vec<Symbol>, NotDecoded> {
    if papr & 1 == 1 {
        return Err(NotDecoded::ToneReservation);
    }
    let w = Waveform::of(p)?;
    let n = usize::from(p.symbols);
    let nulls = usize::from(detail.sbs_null_cells.unwrap_or(0));
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let l = first_l + i;
        let boundary = (i == 0 && p.sbs_first) || (i + 1 == n && p.sbs_last);
        let kind = if boundary {
            SymbolKind::Boundary {
                pattern: w.pattern,
                amplitude: w.amplitude,
            }
        } else {
            SymbolKind::Data {
                pattern: w.pattern,
                amplitude: w.amplitude,
                l,
            }
        };
        let carriers = Carriers::new(w.fft, p.reduced_carriers, kind);
        let cells = carriers.data_cells();
        let (null_low, active) = if boundary {
            if nulls > cells {
                return Err(NotDecoded::Signalling);
            }
            (nulls / 2, cells - nulls)
        } else {
            (0, cells)
        };
        out.push(Symbol {
            carriers,
            guard: w.guard + extra,
            grid: w.pattern.dx(),
            l,
            interleaved: detail.frequency_interleaver,
            null_low,
            active,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> SubframeParams {
        SubframeParams {
            mimo: false,
            miso: 0,
            fft_size: 0,
            reduced_carriers: 0,
            guard_interval: 1,
            symbols: 8,
            pilot_pattern: 10,
            pilot_boost: 1,
            sbs_first: true,
            sbs_last: true,
        }
    }

    /// Tables 7.2, 7.4, 7.6 and F.1.2 for 8K, 192 samples, SP16_2 at boost 1.
    #[test]
    fn the_symbols_of_a_frame_have_the_cells_of_the_tables() {
        let s = PreambleStructure::of(3).unwrap();
        let pre = preamble_symbols(&s, 2, 1);
        assert_eq!((pre[0].active, pre[1].active), (6075, 6342));
        assert_eq!(pre[0].samples(), 8192 + 192);
        assert!(pre[0].interleaved);
        let p = params();
        assert_eq!(boundary_null_cells(&p), Ok(6432 - 6132));
        let basic = L1Basic {
            version: 0,
            mimo_pilot_encoding: false,
            lls: false,
            time_info: 0,
            return_channel: false,
            papr: 0,
            frame_length: FrameLength::TimeAligned {
                units_5ms: 20,
                excess_samples: 5,
            },
            subframes: 1,
            preamble_symbols: 1,
            preamble_reduced_carriers: 0,
            detail_content_tag: 0,
            detail_bytes: 25,
            detail_mode: 2,
            detail_additional_parity: 0,
            detail_cells: 774,
            first: p,
        };
        let detail = DetailSubframe {
            params: None,
            multiplex: None,
            frequency_interleaver: false,
            sbs_null_cells: Some(300),
            plps: vec![],
        };
        let sub = first_subframe(&basic, &detail, 1).unwrap();
        assert_eq!(sub.len(), 8);
        assert_eq!((sub[0].active, sub[0].null_low), (6132, 150));
        assert_eq!((sub[7].active, sub[7].null_low), (6132, 150));
        assert!(sub[1..7]
            .iter()
            .all(|s| s.active == 6647 && s.null_low == 0));
        assert_eq!(sub[3].l, 4);
        assert_eq!(sub[3].guard, 197);
        assert!(!sub[3].interleaved);
        let refuse = |f: &dyn Fn(&mut L1Basic)| {
            let mut b = basic;
            f(&mut b);
            first_subframe(&b, &detail, 1).unwrap_err()
        };
        assert_eq!(refuse(&|b| b.first.mimo = true), NotDecoded::Mimo);
        assert_eq!(refuse(&|b| b.first.miso = 1), NotDecoded::Miso);
        assert_eq!(refuse(&|b| b.papr = 1), NotDecoded::ToneReservation);
        assert_eq!(refuse(&|b| b.first.fft_size = 3), NotDecoded::Signalling);
        assert_eq!(
            refuse(&|b| b.first.guard_interval = 0),
            NotDecoded::Signalling
        );
        assert_eq!(
            refuse(&|b| b.first.pilot_pattern = 16),
            NotDecoded::Signalling
        );
        assert_eq!(refuse(&|b| b.first.pilot_boost = 5), NotDecoded::Signalling);
        assert_eq!(
            refuse(&|b| b.first.reduced_carriers = 5),
            NotDecoded::Signalling
        );
    }
}
