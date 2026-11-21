//! The test signal generator of the stack: one data PLP of the test multiplex (or numbered packets) through the transmitter, at
//! unit mean power, resampled to the requested rate. It makes the clean signal only. The framework adds the impairments.

use crate::params::{k_bch, Fft, Modulation};
use crate::pilots::FrameShape;
use crate::settings::{gen_settings, GenSettings};
use crate::tx::{PlpKind, PlpSpec, TimeIl, Transmitter, TxConfig};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_mpegts::source::{Fill, Numbered, NumberedSource, PacketSource, TestMux};

/// The numbered packets of the pattern test signal: PID 0x0101, [`Fill::I31N7`].
pub const PATTERN: Numbered = Numbered::new(0x0101, Fill::I31N7);

/// The channel of MISO group 2 relative to group 1 in the test signal (both groups arrive together).
pub const MISO_GROUP2: Complex32 = Complex32::new(0.6, 0.35);

/// The transmitter configuration of a set of generator settings: one type 1 data PLP with one TI-block per frame.
pub fn tx_config(g: &GenSettings, source: Box<dyn PacketSource>) -> TxConfig {
    TxConfig {
        bandwidth: g.bandwidth,
        shape: FrameShape {
            fft: g.fft,
            extended: g.extended,
            pp: g.pp,
            guard: g.guard,
            miso: g.miso,
            tr: false,
            l_data: 0,
        },
        profile: g.profile,
        l1_mod: Modulation::Qpsk,
        num_t2_frames: 2,
        plps: vec![PlpSpec {
            id: 0,
            kind: PlpKind::Type1,
            group: 0,
            coding: g.coding,
            blocks: g.blocks,
            time_il: TimeIl::Blocks(1),
            source,
        }],
        sub_slices: 1,
        cell_id: 1,
        network_id: 0x3001,
        t2_system_id: 0x8001,
        repetition: false,
        post_scrambled: false,
        miso_group2: MISO_GROUP2,
        fef: None,
    }
}

/// The DVB-T2 test signal generator.
pub struct T2Generator {
    tx: Option<Transmitter>,
    out_rate: f64,
    resampler: Option<Resampler<Complex32>>,
    pending: Vec<Complex32>,
    at: usize,
    info: Option<SignalInfo>,
}

impl T2Generator {
    /// A generator from the stack settings at `cfg.rate_hz`. A combination of settings that [`gen_settings`] refuses produces
    /// silence.
    pub fn from_config(cfg: &GenConfig) -> T2Generator {
        let g = gen_settings(&cfg.settings);
        let tx = g.as_ref().ok().and_then(|g| {
            let source: Box<dyn PacketSource> = if g.pattern {
                Box::new(NumberedSource::new(PATTERN))
            } else {
                match TestMux::dvbt(1, "sdrkit DVB-T2") {
                    Ok(m) => Box::new(m),
                    Err(_) => Box::new(NumberedSource::new(PATTERN)),
                }
            };
            Transmitter::new(tx_config(g, source)).ok()
        });
        T2Generator::new(tx, cfg.rate_hz)
    }

    /// A generator of a ready transmitter at `rate_hz` (`None` produces silence).
    pub fn new(tx: Option<Transmitter>, rate_hz: f64) -> T2Generator {
        let natural = tx.as_ref().map(|t| t.rate()).unwrap_or(rate_hz);
        let resampler = ((rate_hz - natural).abs() > 1e-6 * natural)
            .then(|| Resampler::new(natural, rate_hz).ok())
            .flatten();
        let info = tx.as_ref().map(|t| {
            let s = t.shape();
            let n = s.fft.size() as f64;
            let spacing = natural / n;
            let frame_s = t.frame_len() as f64 / natural;
            let bits = t.plp_bits_per_frame() as f64;
            let cells = (s.l_f() * s.k_total()) as f64 / frame_s;
            SignalInfo::new(s.k_total() as f64 * spacing)
                .with_symbol_rate(cells)
                .with_bit_rate(bits / frame_s)
        });
        T2Generator {
            tx,
            out_rate: rate_hz,
            resampler,
            pending: Vec::new(),
            at: 0,
            info,
        }
    }

    fn refill(&mut self) {
        self.pending.clear();
        self.at = 0;
        let frame = match self.tx.as_mut().map(|t| t.frame()) {
            Some(Ok(f)) => f,
            _ => vec![Complex32::default(); 8192],
        };
        match self.resampler.as_mut() {
            Some(r) => r.process(&frame, &mut self.pending),
            None => self.pending.extend_from_slice(&frame),
        }
    }
}

impl Generator for T2Generator {
    fn rate_hz(&self) -> Hz {
        self.out_rate
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        self.info
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                self.refill();
                if self.pending.is_empty() {
                    break;
                }
                continue;
            }
            let n = (out.len() - w).min(self.pending.len() - self.at);
            out[w..w + n].copy_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            w += n;
        }
        w
    }
}

/// The net bits of a PLP per frame: FEC blocks times (K_bch - 80).
pub fn plp_bits(
    frame: crate::params::FecFrame,
    rate: crate::params::CodeRate,
    blocks: usize,
) -> usize {
    blocks * k_bch(frame, rate).unwrap_or(80).saturating_sub(80)
}

/// The fewest data symbols of a T2-frame of an FFT size (clause 8.3.1). A 32K frame also has an even number of symbols.
pub fn min_data_symbols(fft: Fft) -> usize {
    if fft == Fft::K32 {
        3
    } else {
        7
    }
}
