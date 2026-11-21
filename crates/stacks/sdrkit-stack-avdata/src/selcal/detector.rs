//! The SELCAL detector: the two-tone call decoder of sdrkit-proto-tones on the channel audio, with its sideband search on while a
//! sideband demodulator is in use, and the measures the display shows (audio level, audio spectrum, strongest pure tone).
//! Standard: ARINC 714.

use super::{Measures, AUDIO_RATE};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use sdrkit_proto_tones::selcal::{SelcalCall, SelcalConfig, SelcalDecoder};

/// 1.0 of audio is this many Hz of deviation.
const HZ: f32 = 5000.0;
/// The spectrum's length.
const NFFT: usize = 512;
/// The bins shown (0 to 3.5 kHz).
const BINS: usize = 112;

/// The detector.
pub struct Detector {
    dec: Option<SelcalDecoder>,
    ssb: bool,
    hist: Vec<f32>,
    n: u64,
    level_acc: f64,
    level_n: u64,
    calls: Vec<Timed<SelcalCall>>,
    fft: Fft,
    spec: Vec<f32>,
    spec_buf: Vec<Complex32>,
    spec_win: Vec<f32>,
    pw: Vec<f64>,
    tone: Option<(f64, f32)>,
    last_level_db: f32,
    last_info: Measures,
}

impl Default for Detector {
    fn default() -> Self {
        Detector {
            dec: None,
            ssb: false,
            hist: Vec::with_capacity(4096),
            n: 0,
            level_acc: 0.0,
            level_n: 0,
            calls: Vec::new(),
            fft: Fft::forward(NFFT),
            spec: vec![-100.0; BINS],
            spec_buf: vec![Complex32::default(); NFFT],
            spec_win: Window::Hann.coefficients(NFFT),
            pw: vec![0.0; NFFT / 2],
            tone: None,
            last_level_db: -100.0,
            last_info: Measures::default(),
        }
    }
}

impl Detector {
    /// The decoder for the current demodulator: the sideband search only for a sideband receiver (the search costs eleven
    /// times the work).
    fn build(&mut self) {
        let cfg = SelcalConfig {
            ssb_search: self.ssb,
            ..SelcalConfig::default()
        };
        self.dec = SelcalDecoder::new(AUDIO_RATE, cfg).ok();
    }

    fn spectrum(&mut self) {
        let n = self.hist.len();
        if n < NFFT {
            return;
        }
        for (i, v) in self.spec_buf.iter_mut().enumerate() {
            *v = Complex32::new(self.hist[n - NFFT + i] * self.spec_win[i], 0.0);
        }
        self.fft.process(&mut self.spec_buf);
        for (i, p) in self.pw.iter_mut().enumerate() {
            *p = (self.spec_buf[i].norm() / 128.0) as f64;
        }
        for k in 0..BINS {
            let a = self.pw[2 * k].max(self.pw[2 * k + 1]);
            let db = (20.0 * a.max(1e-10).log10()).max(-100.0) as f32;
            self.spec[k] = 0.5 * self.spec[k] + 0.5 * db;
        }
        // The strongest pure tone above 250 Hz.
        let lo = (250.0 / (AUDIO_RATE / NFFT as f64)) as usize;
        let p2 = |i: usize| self.pw[i] * self.pw[i];
        let total: f64 = (lo..224).map(p2).sum();
        let k = (lo..224).fold(lo, |b, i| if p2(i) > p2(b) { i } else { b });
        let around = p2(k - 1) + p2(k) + p2(k + 1);
        let rms = (self.hist[n - NFFT..]
            .iter()
            .map(|v| ((v * HZ) as f64).powi(2))
            .sum::<f64>()
            / NFFT as f64)
            .sqrt();
        self.tone = (total > 0.0 && around >= 0.5 * total && rms >= 20.0).then(|| {
            let d = log_parabolic(p2(k - 1), p2(k), p2(k + 1)).offset;
            let f = (k as f64 + d) * AUDIO_RATE / NFFT as f64;
            (f, (10.0 * around.max(1e-20).log10()) as f32)
        });
    }
}

impl Component for Detector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.selcal_detector",
            vec![Port::real("audio"), Port::typed::<Measures>("info")],
            vec![
                Port::typed::<SelcalCall>("calls").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Measures>("measures").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = Detector::default();
        self.build();
        // The calls keep the tuned frequency of the audio, so they leave the graph as events on it.
        let mut calls = inputs[0].with_kind(Kind::typed::<SelcalCall>());
        calls.rate = 0.0;
        Ok(vec![calls, Format::new(Kind::typed::<Measures>(), 0.0)])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        for m in io.items::<Measures>(1) {
            if m.value.ssb != self.ssb {
                self.ssb = m.value.ssb;
                self.build();
            }
            self.last_info = m.value.clone();
        }
        if let Some(d) = self.dec.as_mut() {
            let calls = &mut self.calls;
            d.process(x, |c| {
                calls.push(Timed {
                    time: c.end as f64 / AUDIO_RATE,
                    value: c,
                })
            });
        }
        for &v in x {
            self.hist.push(v);
            self.n += 1;
            let hz = (v * HZ) as f64;
            self.level_acc += hz * hz;
            self.level_n += 1;
            if self.n.is_multiple_of(256) {
                self.spectrum();
            }
            if self.hist.len() > 2048 {
                self.hist.drain(..1024);
            }
        }
        if !self.calls.is_empty() {
            io.items_out::<SelcalCall>(0).append(&mut self.calls);
        }
        if io.tick() {
            if self.level_n > 0 {
                let ms = self.level_acc / self.level_n as f64;
                self.last_level_db =
                    (10.0 * (ms / (HZ as f64 * HZ as f64 / 2.0)).max(1e-12).log10()) as f32;
            }
            self.level_acc = 0.0;
            self.level_n = 0;
            let mut m = self.last_info.clone();
            m.level_db = self.last_level_db;
            m.spectrum.clone_from(&self.spec);
            m.tone = self.tone;
            m.time_s = self.n as f64 / AUDIO_RATE;
            io.items_out::<Measures>(1).push(Timed {
                time: m.time_s,
                value: m,
            });
        }
    }
}
