//! The ISDB-T test signal: the transmit chain of every layer, layer combining, frequency interleaving, pilots, TMCC and AC
//! carriers, and the inverse FFT with its guard at the IFFT rate of the width, resampled to the output rate. It makes the clean
//! signal at unit mean power, and the framework adds the impairments. A band of 1 or 3 segments makes the ISDB-TSB signal.
//!
//! Standard: ARIB STD-B31 chapter 3.

use super::chain::{packets_per_frame, BitDelay, LayerTx};
use super::layout::{Band, Layout, PILOT};
use super::tmcc::{Config, LayerParams, Tmcc};
use super::{Width, WIDTH};
use crate::band::resampler;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::source::{
    null_packet, Data, Fill, Numbered, PacketSource, TestMux, TestService,
};

/// What the layers carry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Payload {
    /// Numbered packets per layer ([`layer_pattern`], checkable exactly).
    #[default]
    Numbered,
    /// One programme (the shared test multiplex with numbered data packets, 90 % of the layer's rate, null packets for the rest)
    /// on layer `programme_layer`, null packets on the others.
    Programme,
}

/// Options.
#[derive(Clone, Debug, PartialEq)]
pub struct IsdbtGenParams {
    /// Output rate.
    pub rate_hz: Hz,
    /// Mode 1..3.
    pub mode: u8,
    /// Guard fraction.
    pub guard: f64,
    /// Channel width, MHz.
    pub width_mhz: f64,
    /// The configuration (layers A, B, C and the partial-reception flag).
    pub config: Config,
    /// A reconfiguration: at this frame the configuration becomes the given one (announced 15 frames ahead by the countdown).
    pub switch: Option<(usize, Config)>,
    /// The bit-delay model.
    pub bit_delay: BitDelay,
    /// Payload.
    pub payload: Payload,
    /// The layer carrying the programme (0 A, 1 B, 2 C).
    pub programme_layer: usize,
    /// Emergency-alarm flag in the TMCC.
    pub emergency: bool,
    /// Seed of the AC bits.
    pub seed: u64,
    /// The band: 13 segments (ISDB-T, TMCC system 00) or 1 or 3 (ISDB-TSB, system 01).
    pub band: Band,
    /// The TMCC's next configuration is all ones (no next information) instead of the current one.
    pub next_unset: bool,
}

/// A layer of `segments` segments.
pub fn layer(modulation: u8, code_rate: u8, interleave: u8, segments: u8) -> LayerParams {
    LayerParams {
        modulation,
        code_rate,
        interleave,
        segments,
    }
}

/// A configuration from up to three layers (unused ones given 0 segments).
pub fn config(partial: bool, layers: [LayerParams; 3]) -> Config {
    let mut c = Config {
        partial,
        layers: [LayerParams::UNUSED; 3],
    };
    for (i, l) in layers.iter().enumerate() {
        if l.segments > 0 {
            c.layers[i] = *l;
        }
    }
    c
}

/// The demo configuration: partial reception, A 1 segment QPSK 2/3 I code 2, B 12 segments 64QAM 3/4 I code 1.
pub fn demo_config() -> Config {
    config(
        true,
        [layer(1, 1, 2, 1), layer(3, 2, 1, 12), LayerParams::UNUSED],
    )
}

impl Default for IsdbtGenParams {
    fn default() -> Self {
        IsdbtGenParams {
            rate_hz: 8e6,
            mode: 3,
            guard: 0.125,
            width_mhz: 6.0,
            config: demo_config(),
            switch: None,
            bit_delay: BitDelay::Frame,
            payload: Payload::Numbered,
            programme_layer: 1,
            emergency: false,
            seed: 1,
            band: Band::FULL,
            next_unset: false,
        }
    }
}

/// The numbered packets of layer `li` (0 A, 1 B, 2 C): PID 0x100 + layer.
pub fn layer_pattern(li: usize) -> Numbered {
    Numbered::new(0x100 + li as u16, Fill::N7I13)
}

/// The PID of the numbered data of the test programme (the shared test multiplex puts its PMT on 0x0100).
pub const PROGRAMME_DATA_PID: u16 = 0x0120;

/// The test programme: one TV service with numbered data packets on [`PROGRAMME_DATA_PID`].
fn programme() -> TestMux {
    TestMux::builder(1)
        .service(TestService::tv(1, "sdrkit ISDB-T", 1))
        .data(PROGRAMME_DATA_PID, Data::Numbered(Fill::N7I13))
        .build()
        .unwrap_or_else(|e| unreachable!("the test programme is a constant: {e}"))
}

/// The generator.
pub struct IsdbtGenerator {
    p: IsdbtGenParams,
    cfg: Config,
    layout: Layout,
    tx: Vec<Option<LayerTx>>,
    ifft: Fft,
    frame: usize,
    sym: usize,
    counters: [u32; 3],
    mux: TestMux,
    prog_n: u64,
    tmcc_bits: Vec<[bool; 204]>,
    dstate: Vec<Complex32>,
    rng_ac: Rng,
    comb: Vec<Complex32>,
    fid: Vec<Complex32>,
    lcell: Vec<Complex32>,
    band: Vec<Complex32>,
    f: Vec<Complex32>,
    t: Vec<Complex32>,
    rs: FractionalResampler<Complex32>,
    pending: Vec<Complex32>,
    at: usize,
    scale: f32,
}

impl IsdbtGenerator {
    /// With these options.
    ///
    /// # Errors
    ///
    /// [`DspError`] if no resampler serves the output rate.
    pub fn new(p: IsdbtGenParams) -> Result<IsdbtGenerator, DspError> {
        let band = p.band;
        let fs = band.ifft_rate(p.width_mhz);
        let n = band.fft_size(p.mode);
        let nc = band.carriers(p.mode);
        let layout = Layout::new(p.mode, &p.config, band);
        let d = layout.d;
        let nd = band.ns * d;
        let pc = (nd as f64 + (nc - nd) as f64 * (PILOT * PILOT) as f64) / (n as f64 * n as f64);
        // A narrow band at an output rate below its width keeps the centre flat and filters the rest.
        let pass = if band.ns == 13 {
            0.35 * fs
        } else {
            (0.515 * band.occupied_hz(p.width_mhz)).min(0.4 * p.rate_hz)
        };
        // The inverse FFT is not normalised: its samples have power N² pc.
        let scale = (1.0 / pc).sqrt() as f32 / n as f32;
        let mut g = IsdbtGenerator {
            cfg: p.config,
            tx: Vec::new(),
            ifft: Fft::inverse(n),
            frame: 0,
            sym: 0,
            counters: [0; 3],
            mux: programme(),
            prog_n: 0,
            tmcc_bits: Vec::new(),
            dstate: vec![Complex32::new(0.0, 0.0); nc],
            rng_ac: Rng::new(p.seed ^ 0x5EED_AC00),
            comb: vec![Complex32::new(0.0, 0.0); nd],
            fid: vec![Complex32::new(0.0, 0.0); nd],
            lcell: vec![Complex32::new(0.0, 0.0); nd],
            band: vec![Complex32::new(0.0, 0.0); nc],
            f: vec![Complex32::new(0.0, 0.0); n],
            t: Vec::with_capacity(2 * n),
            rs: resampler(fs, p.rate_hz, pass, 0.5 * fs.min(p.rate_hz))?,
            pending: Vec::new(),
            at: 0,
            scale,
            layout,
            p,
        };
        g.build_layers();
        Ok(g)
    }
    fn build_layers(&mut self) {
        self.layout = Layout::new(self.p.mode, &self.cfg, self.p.band);
        self.tx = self
            .cfg
            .layers
            .iter()
            .map(|l| {
                l.used()
                    .then(|| LayerTx::new(self.p.mode, *l, self.p.bit_delay))
            })
            .collect();
    }
    /// From the engine's configuration: the demo signal (mode 3, guard 1/8, partial reception, A 1 segment QPSK 2/3, B 12
    /// segments 64QAM 3/4 carrying the test programme), width from the `width` setting (6 when automatic).
    ///
    /// # Errors
    ///
    /// As [`IsdbtGenerator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<IsdbtGenerator, DspError> {
        let w = match cfg.settings.get(&WIDTH) {
            Width::W7 => 7.0,
            Width::W8 => 8.0,
            Width::W6 | Width::Auto => 6.0,
        };
        IsdbtGenerator::new(IsdbtGenParams {
            rate_hz: cfg.rate_hz,
            width_mhz: w,
            seed: cfg.seed,
            payload: Payload::Programme,
            ..Default::default()
        })
    }
    /// The configuration in use.
    pub fn config(&self) -> Config {
        self.cfg
    }
    fn layer_packets(&mut self, li: usize, n: usize) -> Vec<[u8; TS]> {
        let mut v = Vec::with_capacity(n);
        let programme = self.p.payload == Payload::Programme;
        for _ in 0..n {
            if !programme {
                let c = self.counters[li];
                self.counters[li] += 1;
                v.push(layer_pattern(li).packet(c));
                continue;
            }
            if li != self.p.programme_layer {
                v.push(null_packet());
                continue;
            }
            let k = self.prog_n;
            self.prog_n += 1;
            if k % 10 == 9 {
                v.push(null_packet());
            } else {
                let mut pk = [0u8; TS];
                self.mux.next_packet(&mut pk);
                v.push(pk);
            }
        }
        v
    }
    fn start_frame(&mut self) {
        if let Some((at, new)) = self.p.switch {
            if self.frame == at {
                self.cfg = new;
                self.build_layers();
            }
        }
        let (countdown, next) = match self.p.switch {
            Some((at, new)) if self.frame < at && self.frame + 15 >= at => {
                ((at - self.frame - 1) as u8, new)
            }
            _ if self.p.next_unset => (
                15,
                Config {
                    partial: true,
                    layers: [LayerParams::UNUSED; 3],
                },
            ),
            _ => (15, self.cfg),
        };
        let tmcc = Tmcc {
            system: u8::from(self.p.band.ns != 13),
            countdown,
            emergency: self.p.emergency,
            current: self.cfg,
            next,
        };
        let odd = self.frame % 2 == 1;
        self.tmcc_bits = vec![tmcc.frame_bits(odd, false), tmcc.frame_bits(odd, true)];
        for li in 0..3 {
            if self.tx[li].is_none() {
                continue;
            }
            let n = packets_per_frame(self.p.mode, &self.cfg.layers[li]);
            let pk = self.layer_packets(li, n);
            if let Some(tx) = &mut self.tx[li] {
                tx.encode_frame(&pk);
            }
        }
    }
    fn symbol(&mut self) {
        if self.sym == 0 {
            self.start_frame();
        }
        let t = self.sym;
        let lay = &self.layout;
        let d = lay.d;
        // Layer cells in segment-number order, then frequency interleaving.
        let mut at = 0usize;
        for li in 0..3 {
            if let Some(tx) = &mut self.tx[li] {
                let l = tx.cells_per_symbol();
                tx.symbol(t, &mut self.lcell[..l]);
                self.comb[at..at + l].copy_from_slice(&self.lcell[..l]);
                at += l;
            }
        }
        debug_assert_eq!(at, lay.ns() * d);
        lay.interleave(&self.comb, &mut self.fid);
        self.band
            .iter_mut()
            .for_each(|v| *v = Complex32::new(0.0, 0.0));
        lay.place(&self.fid, t, &mut self.band);
        for s in 0..lay.ns() {
            let diff = lay.diff[s];
            let tab = &lay.tables[s];
            if diff {
                let c = lay.band(s, 0);
                self.band[c] = Complex32::new(lay.pilot(c), 0.0);
            } else {
                for k in (3 * (t % 4)..lay.ls).step_by(12) {
                    let c = lay.band(s, k);
                    self.band[c] = Complex32::new(lay.pilot(c), 0.0);
                }
            }
            let bits = &self.tmcc_bits[diff as usize];
            for &k in &tab.tmcc {
                let c = lay.band(s, k as usize);
                let v = if t == 0 {
                    Complex32::new(lay.pilot(c), 0.0)
                } else if bits[t] {
                    -self.dstate[c]
                } else {
                    self.dstate[c]
                };
                self.dstate[c] = v;
                self.band[c] = v;
            }
            for &k in tab.ac1.iter().chain(&tab.ac2) {
                let c = lay.band(s, k as usize);
                let v = if t == 0 {
                    Complex32::new(lay.pilot(c), 0.0)
                } else {
                    if self.rng_ac.bit() == 1 {
                        -self.dstate[c]
                    } else {
                        self.dstate[c]
                    }
                };
                self.dstate[c] = v;
                self.band[c] = v;
            }
        }
        let nc = lay.nc;
        self.band[nc - 1] = Complex32::new(lay.top_pilot(), 0.0);
        let n = lay.n_fft;
        self.f
            .iter_mut()
            .for_each(|v| *v = Complex32::new(0.0, 0.0));
        for (c, &v) in self.band.iter().enumerate() {
            self.f[(c + n - (nc - 1) / 2) % n] = v;
        }
        self.ifft.process(&mut self.f);
        let gl = (n as f64 * self.p.guard) as usize;
        self.t.clear();
        self.t.extend_from_slice(&self.f[n - gl..]);
        self.t.extend_from_slice(&self.f);
        let sc = self.scale;
        self.t.iter_mut().for_each(|v| *v *= sc);
        self.sym += 1;
        if self.sym == 204 {
            self.sym = 0;
            self.frame += 1;
        }
        self.pending.clear();
        self.rs.process(&self.t, &mut self.pending);
        self.at = 0;
    }
}

impl Generator for IsdbtGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The segments and the top pilot: carriers × spacing.
        let fs = self.p.band.ifft_rate(self.p.width_mhz);
        let n = self.layout.n_fft as f64;
        let bw = self.layout.nc as f64 * fs / n;
        let net: f64 = self
            .cfg
            .layers
            .iter()
            .filter(|l| l.used())
            .map(|l| super::layer_rate(self.p.mode, self.p.guard, self.p.width_mhz, l))
            .sum();
        Some(
            SignalInfo::new(bw)
                .with_symbol_rate(
                    fs * (self.layout.ns() * self.layout.d) as f64 / (n * (1.0 + self.p.guard)),
                )
                .with_bit_rate(net),
        )
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                self.symbol();
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
