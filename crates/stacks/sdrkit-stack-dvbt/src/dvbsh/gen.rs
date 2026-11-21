//! The test signal of the OFDM form: numbered transport packets on the high-priority stream (and on the low-priority stream of
//! hierarchical 16-QAM), coded and interleaved as clauses 5.1 to 5.4, mapped on the carriers of clause 5.7 with the DVB-SH TPS,
//! at the OFDM rate of the width or resampled to another rate.

use super::coding::{IU_BITS, TS};
use super::ofdm::{
    sh_frame_symbols, stream_bits, tps_bits, Mapper, OfdmParams, Width, FRAME_IU, SLICE_UNIT,
};
use super::shframe::{FrameShape, FrameTx};
use crate::generator::ofdm_symbol;
use crate::tables::Layout;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_fec::error::FecError;
use sdrkit_core::{Complex32, Generator};
use sdrkit_proto_mpegts::source::{null_packet, Fill, Numbered, NumberedSource, PacketSource};

/// The numbered packets of the high-priority stream.
pub const PATTERN_HP: Numbered = Numbered::new(0x0100, Fill::I31N7);
/// The numbered packets of the low-priority stream.
pub const PATTERN_LP: Numbered = Numbered::new(0x0200, Fill::I31N7);

/// Numbered packets with a null packet in place of every 20th: the payload at 0.95 of the net rate.
pub struct TestStream {
    numbered: NumberedSource,
    count: u32,
}

impl TestStream {
    /// Numbered packets of `pattern` from number 0.
    pub fn new(pattern: Numbered) -> Self {
        TestStream {
            numbered: NumberedSource::new(pattern),
            count: 0,
        }
    }
}

impl PacketSource for TestStream {
    fn next_packet(&mut self, out: &mut [u8; TS]) {
        self.count = self.count.wrapping_add(1);
        if self.count.is_multiple_of(20) {
            *out = null_packet();
        } else {
            self.numbered.next_packet(out);
        }
    }
}

/// The OFDM test signal.
pub struct OfdmGenerator {
    p: OfdmParams,
    width: Width,
    layout: Layout,
    mapper: Mapper,
    hp: FrameTx,
    lp: Option<FrameTx>,
    hp_bits: Vec<u8>,
    lp_bits: Vec<u8>,
    cells: Vec<Complex32>,
    ifft: Fft,
    bins: Vec<Complex32>,
    sym: Vec<Complex32>,
    pos: usize,
    l: usize,
    frame: u8,
    superframe: u8,
    superframes: u8,
    tps: [u8; 68],
    tps_prev: f32,
    scale: f32,
    out_rate: f64,
    resampler: Option<Resampler<Complex32>>,
    pending: Vec<Complex32>,
    pending_at: usize,
}

fn stream(p: &OfdmParams, id: u8, source: Box<dyn PacketSource>) -> Result<FrameTx, FecError> {
    let n_bil = super::coding::ShCodec::new(id)?.n_bil();
    FrameTx::new(
        FrameShape::new(FRAME_IU, n_bil, p.ti, SLICE_UNIT),
        id,
        source,
    )
}

impl OfdmGenerator {
    /// A generator of `p` in a channel of `width`, at `rate_hz` (the OFDM rate of the width when `None`).
    ///
    /// # Errors
    ///
    /// [`FecError`] for a reserved pattern or an invalid interleaver profile, or a rate that is not a positive number.
    pub fn new(p: OfdmParams, width: Width, rate_hz: Option<f64>) -> Result<Self, FecError> {
        let mut p = p;
        if width == Width::W1_7 {
            p.mode = crate::params::Mode::K1;
        }
        let hier = p.hierarchy.hierarchical();
        let hp = stream(&p, p.hp, Box::new(TestStream::new(PATTERN_HP)))?;
        let lp = if hier {
            Some(stream(&p, p.lp, Box::new(TestStream::new(PATTERN_LP)))?)
        } else {
            None
        };
        let n = p.mode.fft();
        let k = p.mode.carriers();
        let frames = sh_frame_symbols(p.mode, p.constellation, p.hierarchy) / 68;
        let natural = width.ofdm_rate();
        let out_rate = rate_hz.unwrap_or(natural);
        if !(out_rate > 0.0 && out_rate.is_finite()) {
            return Err(FecError::NotFinite { at: 0 });
        }
        let resampler = if (out_rate - natural).abs() > 1e-9 * natural {
            let edge = k as f64 / n as f64 / 2.0 * natural;
            let pass = (1.01 * edge / natural.min(out_rate)).clamp(0.42, 0.48);
            Some(
                Resampler::with_quality(natural, out_rate, pass, 70.0)
                    .map_err(|_| FecError::NotFinite { at: 0 })?,
            )
        } else {
            None
        };
        Ok(OfdmGenerator {
            width,
            layout: Layout::new(p.mode),
            mapper: Mapper::new(p.mode, p.constellation, p.hierarchy),
            hp,
            lp,
            hp_bits: Vec::with_capacity(stream_bits(p.mode, p.constellation, p.hierarchy)),
            lp_bits: Vec::new(),
            cells: Vec::with_capacity(p.mode.data_carriers()),
            ifft: Fft::inverse(n),
            bins: vec![Complex32::default(); n],
            sym: Vec::new(),
            pos: 0,
            l: 0,
            frame: 0,
            superframe: 0,
            superframes: (frames / 4).max(1) as u8,
            tps: tps_bits(&p, 0, 0),
            tps_prev: 1.0,
            scale: 1.0 / (1.04 * k as f32).sqrt(),
            out_rate,
            resampler,
            pending: Vec::new(),
            pending_at: 0,
            p,
        })
    }

    /// The parameters (the mode is 1K for the 1.7 MHz width).
    pub fn params(&self) -> &OfdmParams {
        &self.p
    }

    /// The OFDM rate of the width.
    pub fn natural_rate(&self) -> f64 {
        self.width.ofdm_rate()
    }

    fn take(tx: &mut FrameTx, n: usize, out: &mut Vec<u8>) {
        out.clear();
        while out.len() < n {
            out.extend_from_slice(tx.next_iu());
        }
    }

    /// Makes the next OFDM symbol (guard and N samples at the OFDM rate) and returns it.
    pub fn next_symbol(&mut self) -> &[Complex32] {
        let p = self.p;
        let n = stream_bits(p.mode, p.constellation, p.hierarchy);
        debug_assert!(n.is_multiple_of(IU_BITS));
        Self::take(&mut self.hp, n, &mut self.hp_bits);
        if let Some(lp) = self.lp.as_mut() {
            Self::take(lp, n, &mut self.lp_bits);
        }
        self.mapper
            .map(self.l, &self.hp_bits, &self.lp_bits, &mut self.cells);
        let tps_sign = if self.l == 0 {
            1.0
        } else if self.tps[self.l] == 1 {
            -self.tps_prev
        } else {
            self.tps_prev
        };
        self.tps_prev = tps_sign;
        ofdm_symbol(
            &self.layout,
            self.l,
            &self.cells,
            tps_sign,
            self.scale,
            p.guard.samples(p.mode.fft()),
            &mut self.ifft,
            &mut self.bins,
            &mut self.sym,
        );
        self.l += 1;
        if self.l == 68 {
            self.l = 0;
            self.frame = (self.frame + 1) & 3;
            if self.frame == 0 {
                self.superframe = (self.superframe + 1) % self.superframes;
            }
            self.tps = tps_bits(&p, self.frame, self.superframe);
        }
        &self.sym
    }
}

impl Generator for OfdmGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.resampler.is_some() {
                if self.pending_at >= self.pending.len() {
                    self.next_symbol();
                    self.pending.clear();
                    self.pending_at = 0;
                    if let Some(r) = self.resampler.as_mut() {
                        r.process(&self.sym, &mut self.pending);
                    }
                    continue;
                }
                let k = (self.pending.len() - self.pending_at).min(out.len() - i);
                out[i..i + k].copy_from_slice(&self.pending[self.pending_at..self.pending_at + k]);
                self.pending_at += k;
                i += k;
            } else {
                if self.pos >= self.sym.len() {
                    self.next_symbol();
                    self.pos = 0;
                }
                let k = (self.sym.len() - self.pos).min(out.len() - i);
                out[i..i + k].copy_from_slice(&self.sym[self.pos..self.pos + k]);
                self.pos += k;
                i += k;
            }
        }
        out.len()
    }

    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        let rate = self.natural_rate();
        let m = self.p.mode;
        let width = m.carriers() as f64 * rate / m.fft() as f64;
        let symbol = (m.fft() + self.p.guard.samples(m.fft())) as f64;
        Some(
            sdrkit_core::SignalInfo::new(width)
                .with_symbol_rate(m.data_carriers() as f64 * rate / symbol)
                .with_power(crate::generator::MEAN_POWER),
        )
    }

    /// The signal is made at its own centre: no tune offset is applied.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
