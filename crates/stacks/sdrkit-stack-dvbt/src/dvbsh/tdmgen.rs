//! The test signal of the TDM form: numbered transport packets coded and interleaved as clauses 5.1 to 5.4, in SH frames
//! of the width of table 5.12 with the SOF and the signalling field, mapped on physical layer slots with their pilot
//! fields, scrambled and shaped by a root raised cosine at the symbol rate of table 5.13.

use super::coding::{dispersal, ShCodec, IU_BITS, SIGNALLING_BITS};
use super::gen::{TestStream, PATTERN_HP};
use super::ofdm::Width;
use super::shframe::FrameTx;
use super::tdm::{
    data_place, frame_width_cu, pilot_fields, quarter, scrambling, sof_bits, symbol_rate,
    Modulation, RollOff, Signalling, TdmFrame, HEADER_BITS, SECOND_FIELD, SIGNALLING_CODED, SLOT,
    SLOT_DATA, SOF_BITS,
};
use crate::params::Guard;
use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_algo_fec::error::FecError;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_core::{Complex32, Generator};

/// The symbols that the pulse of the test signal spans.
const SPAN: f64 = 32.0;

/// The parameters of a TDM test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TdmParams {
    /// The modulation.
    pub modulation: Modulation,
    /// The roll-off.
    pub rolloff: RollOff,
    /// The channel width, which with the guard and the roll-off gives the symbol rate.
    pub width: Width,
    /// The guard interval of the OFDM form that the frame is aligned to.
    pub guard: Guard,
    /// The OFDM form that the frame is aligned to is 16-QAM: the frame is half as long.
    pub ofdm_qam16: bool,
    /// Punct_Pat_ID.
    pub id: u8,
    /// The time interleaver profile.
    pub ti: ShTimeInterleaver,
    /// The code number of the slot scrambling.
    pub gold: u32,
}

impl Default for TdmParams {
    /// QPSK, roll-off 0.25, 5 MHz, guard 1/8, pattern 4 (1/3), the interleaver 4 / 48 / 1 / 0 / 0, code 0.
    fn default() -> Self {
        TdmParams {
            modulation: Modulation::Qpsk,
            rolloff: RollOff::R25,
            width: Width::W5,
            guard: Guard::G8,
            ofdm_qam16: false,
            id: 4,
            ti: ShTimeInterleaver {
                common_multiplier: 4,
                late_taps: 48,
                slices: 1,
                slice_distance: 0,
                non_late_increment: 0,
            },
            gold: 0,
        }
    }
}

impl TdmParams {
    /// The symbol rate, symbols per second.
    pub fn symbol_rate(&self) -> f64 {
        symbol_rate(self.width, self.guard, self.rolloff)
    }

    /// The frame width in capacity units.
    pub fn width_cu(&self) -> usize {
        frame_width_cu(self.guard, self.rolloff, self.ofdm_qam16, self.modulation)
    }
}

/// The TDM test signal.
pub struct TdmGenerator {
    p: TdmParams,
    frame: TdmFrame,
    tx: FrameTx,
    /// The header of every frame: the SOF and the coded signalling field.
    header: Vec<u8>,
    padding: Vec<u8>,
    /// The bits of the frame in work.
    bits: Vec<u8>,
    /// The slot of the frame that comes next.
    slot: usize,
    points: Vec<Complex32>,
    scr: Vec<u8>,
    pilots: [[Complex32; super::tdm::PILOTS]; 2],
    symbols: Vec<Complex32>,
    shaper: Linear,
    out_rate: f64,
    pending: Vec<Complex32>,
    pending_at: usize,
}

impl TdmGenerator {
    /// A generator of `p` at `rate_hz` (2 samples per symbol when `None`).
    ///
    /// # Errors
    ///
    /// [`FecError`] for a reserved pattern, an invalid interleaver profile, or a rate below the width of the signal.
    pub fn new(p: TdmParams, rate_hz: Option<f64>) -> Result<Self, FecError> {
        let bad = FecError::NotFinite { at: 0 };
        let rs = p.symbol_rate();
        let out_rate = rate_hz.unwrap_or(2.0 * rs);
        if !(out_rate.is_finite() && out_rate >= rs * (1.0 + p.rolloff.value())) {
            return Err(bad);
        }
        let frame = TdmFrame::new(p.width_cu(), p.modulation, p.id, p.ti).ok_or(bad)?;
        let tx = FrameTx::new(frame.shape, p.id, Box::new(TestStream::new(PATTERN_HP)))?;
        let mut field = vec![0u8; SIGNALLING_BITS];
        Signalling {
            width_cu: frame.width_cu as u16,
            id: p.id,
            ti: p.ti,
            low_latency: false,
        }
        .write(&mut field);
        let mut header = vec![0u8; HEADER_BITS];
        header[..SOF_BITS].copy_from_slice(&sof_bits());
        ShCodec::signalling()?.encode(&field, &mut header[SOF_BITS..])?;
        debug_assert_eq!(header.len() - SOF_BITS, SIGNALLING_CODED);
        let scr = scrambling(p.gold);
        let shaper =
            Linear::rrc(p.rolloff.value(), SPAN, out_rate / rs).map_err(|_| bad)?;
        Ok(TdmGenerator {
            p,
            frame,
            tx,
            header,
            padding: dispersal(frame.padding_bits()),
            bits: Vec::new(),
            slot: frame.slots,
            points: p.modulation.point_set().points().to_vec(),
            pilots: pilot_fields(&scr),
            scr,
            symbols: Vec::with_capacity(SLOT),
            shaper,
            out_rate,
            pending: Vec::new(),
            pending_at: 0,
        })
    }

    /// The parameters.
    pub fn params(&self) -> &TdmParams {
        &self.p
    }

    /// The layout of the frame.
    pub fn frame(&self) -> &TdmFrame {
        &self.frame
    }

    /// The bits of the next SH frame: the header, the interleaved DATA part and the PADDING part.
    fn make_frame(&mut self) {
        self.bits.clear();
        self.bits.extend_from_slice(&self.header);
        for _ in 0..self.frame.shape.data_iu() {
            let iu = self.tx.next_iu();
            debug_assert_eq!(iu.len(), IU_BITS);
            self.bits.extend_from_slice(iu);
        }
        self.bits.extend_from_slice(&self.padding);
    }

    /// Makes the next slot (2176 scrambled symbols at unit mean power) and returns it.
    pub fn next_slot(&mut self) -> &[Complex32] {
        if self.slot == self.frame.slots {
            self.make_frame();
            self.slot = 0;
        }
        let m = self.p.modulation.bits();
        let bits = &self.bits[self.slot * SLOT_DATA * m..(self.slot + 1) * SLOT_DATA * m];
        self.slot += 1;
        self.symbols.clear();
        self.symbols.resize(SLOT, Complex32::default());
        for (k, field) in self.pilots.iter().enumerate() {
            let at = k * SECOND_FIELD;
            self.symbols[at..at + field.len()].copy_from_slice(field);
        }
        for (d, group) in bits.chunks_exact(m).enumerate() {
            let label = group.iter().fold(0usize, |a, &b| a << 1 | b as usize);
            let at = data_place(d);
            self.symbols[at] = quarter(self.points[label], self.scr[at]);
        }
        &self.symbols
    }
}

impl Generator for TdmGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pending_at >= self.pending.len() {
                self.next_slot();
                self.pending.clear();
                self.pending_at = 0;
                self.shaper.push(&self.symbols, &mut self.pending);
                continue;
            }
            let k = (self.pending.len() - self.pending_at).min(out.len() - i);
            out[i..i + k].copy_from_slice(&self.pending[self.pending_at..self.pending_at + k]);
            self.pending_at += k;
            i += k;
        }
        out.len()
    }

    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        let rs = self.p.symbol_rate();
        Some(
            sdrkit_core::SignalInfo::new(rs * (1.0 + self.p.rolloff.value()))
                .with_symbol_rate(rs)
                .with_power(1.0),
        )
    }

    /// The signal is made at its own centre: no tune offset is applied.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dvbsh::tdm::{PILOT, PILOTS};

    #[test]
    fn a_frame_starts_with_pilots_and_the_sof() {
        for m in Modulation::ALL {
            let p = TdmParams {
                modulation: m,
                ..TdmParams::default()
            };
            let mut g = TdmGenerator::new(p, None).unwrap();
            assert!((g.rate_hz() - 2.0 * 35e6 / 9.0).abs() < 1e-6);
            let scr = scrambling(0);
            let points = m.point_set();
            let slot = g.next_slot().to_vec();
            assert_eq!(slot.len(), SLOT);
            for k in 0..2 {
                for i in 0..PILOTS {
                    let at = k * SECOND_FIELD + i;
                    assert_eq!(slot[at], quarter(PILOT, scr[at]), "{m:?} pilot {at}");
                }
            }
            // The first data symbols carry the SOF, the first bit of a symbol in the most significant place.
            let sof = sof_bits();
            for (d, group) in sof.chunks_exact(m.bits()).enumerate() {
                let at = data_place(d);
                let label = group.iter().fold(0usize, |a, &b| a << 1 | b as usize);
                let want = quarter(points.point(label).unwrap(), scr[at]);
                assert!((slot[at] - want).norm() < 1e-6, "{m:?} SOF symbol {d}");
            }
            let power: f32 = slot.iter().map(|z| z.norm_sqr()).sum::<f32>() / SLOT as f32;
            assert!((power - 1.0).abs() < 0.2, "{m:?} power {power}");
            // The frame has the slots of table 5.12: the next SOF comes after them.
            let slots = g.frame().slots;
            assert_eq!(slots * m.bits(), p.width_cu());
            for _ in 1..slots {
                g.next_slot();
            }
            let next = g.next_slot();
            let at = data_place(0);
            let label = sof[..m.bits()].iter().fold(0usize, |a, &b| a << 1 | b as usize);
            assert!((next[at] - quarter(points.point(label).unwrap(), scr[at])).norm() < 1e-6);
        }
    }
}
