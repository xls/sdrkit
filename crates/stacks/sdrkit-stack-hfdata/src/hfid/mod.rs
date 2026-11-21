//! The HF waveform identifier, identification only. It measures the physical layer of an HF signal in
//! the upper sideband and ranks candidate waveforms by how well the measurements fit their published parameters. Nothing is decoded.
//!
//! ```text
//! iq -> [front: dial to 0 Hz, integrate and dump to ~48 kHz, 63-tap low pass to ~12 kHz, 2 x real] --+--> [analyser, every 2 s] -> hfid.result --+
//!                                                                                                    +-------------------------------------------+--> [view] -> results
//! ```

pub mod features;
pub mod synth;

use crate::typed::{HfidCandidateRow, HfidFields, HFID_CANDIDATES};
use features::{Candidate, Features, SignalClass};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    Results, ResultsDecl, SettingSet, StackConfig, StackDescriptor, Status, Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io,
    Item, Kind, Port, StackGraph, TagKind, Timed,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The decimations: D1 = round(input / 48 kHz) above 48 kHz (else 1), D2 = round((input / D1) / 12 kHz).
pub fn decimations(rate: f64) -> (usize, usize) {
    let d1 = if rate > 48_000.0 {
        (rate / 48_000.0).round().max(1.0) as usize
    } else {
        1
    };
    let d2 = ((rate / d1 as f64) / 12_000.0).round().max(1.0) as usize;
    (d1, d2)
}

/// `hfdata.hfid_front`, the audio front end: `Iq` in, `Real` at fsA out. It mixes the dial to 0 Hz, integrates and dumps by D1, low-passes and decimates
/// by D2, and returns twice the real part.
pub struct HfidFront {
    nco: Nco,
    box_dec: FirDecimator<f32>,
    lp_dec: FirDecimator<f32>,
    mixed: Vec<Complex32>,
    re: Vec<f32>,
    mid: Vec<f32>,
    out: Vec<f32>,
}

impl Default for HfidFront {
    fn default() -> Self {
        HfidFront {
            nco: Nco::new(1.0, 0.0),
            box_dec: FirDecimator::new(&[1.0], 1),
            lp_dec: FirDecimator::new(&[1.0], 1),
            mixed: Vec::new(),
            re: Vec::new(),
            mid: Vec::new(),
            out: Vec::new(),
        }
    }
}

impl HfidFront {
    /// Adds the audio of `x` to `self.out`.
    fn run(&mut self, x: &[Complex32]) {
        self.mixed.clear();
        self.mixed.resize(x.len(), Complex32::new(0.0, 0.0));
        self.nco.mix(x, &mut self.mixed);
        // The filters have real taps, so the real part can be taken first.
        self.re.clear();
        self.re.extend(self.mixed.iter().map(|v| v.re));
        self.mid.clear();
        self.box_dec.process(&self.re, &mut self.mid);
        let k = self.out.len();
        self.lp_dec.process(&self.mid, &mut self.out);
        self.out[k..].iter_mut().for_each(|v| *v *= 2.0);
    }
}

impl Component for HfidFront {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfid_front",
            vec![Port::iq("in")],
            vec![Port::real("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let (d1, d2) = decimations(f.rate);
        let fsa = f.rate / (d1 * d2) as f64;
        let mid = f.rate / d1 as f64;
        let fc = (5200.0f64).min(0.43 * fsa) / mid;
        let taps = design_by_taps(63, fc, Window::Hamming)?;
        *self = HfidFront {
            nco: Nco::new(f.rate, -f.signal_offset_hz()),
            box_dec: FirDecimator::new(&vec![1.0 / d1 as f32; d1], d1),
            lp_dec: FirDecimator::new(&taps, d2),
            ..Default::default()
        };
        let mut o = Format::new(Kind::Real, fsa);
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let mut start = 0;
        self.out.clear();
        for t in io.input(0).tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                let at = (t.at.saturating_sub(first) as usize).min(x.len());
                self.run(&x[start..at]);
                start = at;
                self.nco.set_freq(centre_hz - tuned_hz);
            }
        }
        self.run(&x[start..]);
        io.real_out(0).extend_from_slice(&self.out);
    }
}

/// One analysis (`hfid.result`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HfidResult {
    pub features: Features,
    pub candidates: Vec<Candidate>,
    pub lines: Vec<String>,
    pub time: f64,
}

impl Item for HfidResult {
    const KIND: &'static str = "hfid.result";
}

/// Analyses audio: the features and the candidates.
pub fn identify(x: &[f32], fs: f64) -> Option<HfidResult> {
    if !(8_000.0..=24_500.0).contains(&fs) || x.len() < 8192 {
        return None;
    }
    let n = 1usize << (usize::BITS - 1 - x.len().min(65_536).leading_zeros());
    let seg = &x[x.len() - n..];
    let f = features::measure(seg, fs);
    Some(HfidResult {
        candidates: features::candidates(&f),
        lines: features::describe(&f),
        features: f,
        time: 0.0,
    })
}

/// `hfdata.hfid_analyser`: audio in; every 2 s, once 16 384 samples exist, the newest 5.5 s analysed.
pub struct HfidAnalyser {
    fs: f64,
    ring: VecDeque<f32>,
    n: u64,
    next: u64,
    buf: Vec<f32>,
    out: Vec<HfidResult>,
}

impl Default for HfidAnalyser {
    fn default() -> Self {
        HfidAnalyser {
            fs: 12_000.0,
            ring: VecDeque::new(),
            n: 0,
            next: 0,
            buf: Vec::new(),
            out: Vec::new(),
        }
    }
}

impl HfidAnalyser {
    fn run(&mut self) {
        let want = ((5.5 * self.fs) as usize).min(65_536).min(self.ring.len());
        self.buf.clear();
        self.buf
            .extend(self.ring.iter().skip(self.ring.len() - want));
        if let Some(mut r) = identify(&self.buf, self.fs) {
            r.time = self.n as f64 / self.fs;
            self.out.push(r);
        }
    }
}

impl Component for HfidAnalyser {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfid_analyser",
            vec![Port::real("in")],
            vec![Port::typed::<HfidResult>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.ring.clear();
        self.next = (2.0 * self.fs) as u64;
        Ok(vec![Format::new(Kind::typed::<HfidResult>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let cap = 131_072;
        for &v in x {
            if self.ring.len() >= cap {
                self.ring.pop_front();
            }
            self.ring.push_back(v);
            self.n += 1;
            if self.n >= self.next {
                self.next = self.n + (2.0 * self.fs) as u64;
                if self.ring.len() >= 16_384 {
                    self.run();
                }
            }
        }
        let o = io.items_out::<HfidResult>(0);
        for r in self.out.drain(..) {
            o.push(Timed {
                time: r.time,
                value: r,
            });
        }
    }
}

/// What the identifier publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(HfidFields::SHAPE.fields)
        .with_table(&HFID_CANDIDATES)
}

/// `hfdata.hfid_view`: audio and analyses in, the results out.
pub struct HfidView {
    p: Published,
    shown: usize,
    fs: f64,
    analyses: u64,
    result: Option<HfidResult>,
    level: VecDeque<f32>,
    n: u64,
}

impl Default for HfidView {
    fn default() -> Self {
        HfidView {
            p: Published::new(HfidFields::SHAPE.fields).with_tables(&[&HFID_CANDIDATES]),
            shown: 0,
            fs: 12_000.0,
            analyses: 0,
            result: None,
            level: VecDeque::new(),
            n: 0,
        }
    }
}

impl HfidView {
    fn publish(&mut self) {
        let lvl =
            self.level.iter().map(|&v| v as f64).sum::<f64>() / self.level.len().max(1) as f64;
        let level_dbfs = 10.0 * lvl.max(1e-15).log10();
        let r = self.result.as_ref();
        let f = r.map(|r| &r.features);
        let signal = f.is_some_and(|f| f.signal);
        let best = r.and_then(|r| r.candidates.first());
        let summary = if self.n == 0 {
            "HF ID: waiting for samples".to_string()
        } else if self.analyses == 0 {
            "HF ID: collecting audio".into()
        } else if !signal {
            format!("HF ID: no signal ({level_dbfs:.0} dBFS)")
        } else if let Some(c) = best.filter(|c| c.waveform != SignalClass::Unclassified) {
            format!(
                "HF ID (estimate): {}, {:.0} %",
                c.waveform.name(),
                c.confidence * 100.0
            )
        } else {
            "HF ID: signal, no estimate".into()
        };
        let p = &mut self.p;
        let measured = r.map(|r| r.lines.join("\n")).unwrap_or_default();
        p.put(&HfidFields {
            signal,
            best: best.map(|c| c.waveform),
            best_confidence_pct: best.map(|c| c.confidence * 100.0),
            analyses: self.analyses as i64,
            band_low: f.and_then(|f| f.band).map(|b| Hertz(b.0)),
            band_high: f.and_then(|f| f.band).map(|b| Hertz(b.1)),
            snr_db: f.filter(|f| f.signal).map(|f| f.band_snr_db),
            noise_dbfs: f.map(|f| f.floor_dbfs),
            carrier_line: f.and_then(|f| f.carrier_line).map(|l| Hertz(l.0)),
            carrier_line_db: f.and_then(|f| f.carrier_line).map(|l| l.1),
            tone_605_db: f.and_then(|f| f.tone_605),
            tone_2915: f.is_some_and(|f| f.tone_2915),
            comb_spacing: f.and_then(|f| f.comb).map(|c| Hertz(c.0)),
            comb_regularity_pct: f.and_then(|f| f.comb).map(|c| c.1 * 100.0),
            comb_tones: f.and_then(|f| f.comb).map(|c| c.2 as i64),
            shift: f.and_then(|f| f.shift_hz).map(Hertz),
            symbol_rate_hz: f.and_then(|f| f.clock).map(|c| Hertz(c.0)),
            clock_line_db: f.and_then(|f| f.clock).map(|c| c.1),
            envelope_modulation: f.and_then(|f| f.env_mod).map(|e| Hertz(e.0)),
            envelope_modulation_db: f.and_then(|f| f.env_mod).map(|e| e.1),
            bursts: f.map(|f| f.bursts as i64),
            duty_pct: f.map(|f| f.duty * 100.0),
            order: f.and_then(|f| f.order).map(|o| o as i64),
            repeat_symbols: f.and_then(|f| f.repeat).map(|r| r.0 as i64),
            repeat_period: f.and_then(|f| f.repeat).map(|r| Seconds(r.1 / 1000.0)),
            repeat_correlation_pct: f.and_then(|f| f.repeat).map(|r| r.2 * 100.0),
            measured: &measured,
            level_dbfs,
            audio_rate: Hertz(self.fs),
        });
        let candidates = r.map_or(&[][..], |r| &r.candidates[..]);
        for (i, c) in candidates.iter().enumerate() {
            HFID_CANDIDATES.put(
                p,
                &i.to_string(),
                &HfidCandidateRow {
                    waveform: c.waveform,
                    confidence_pct: c.confidence * 100.0,
                    basis: &c.basis,
                    note: c.waveform.note(),
                    link: c.waveform.link(),
                },
            );
        }
        for i in candidates.len()..self.shown {
            p.remove_row(&HFID_CANDIDATES, &i.to_string());
        }
        self.shown = candidates.len();
        // Locked only while the latest analysis found a signal; an analysis of noise leaves the stack searching.
        let lock_state = if self.n == 0 {
            LockState::Off
        } else if signal {
            LockState::Locked
        } else {
            LockState::Searching
        };
        p.results_mut().set_status(&Status {
            lock_state,
            snr_db: f.filter(|f| f.signal).map(|f| f.band_snr_db as f32),
            level_dbfs: Some(level_dbfs as f32),
            offset_hz: None,
            summary,
        });
    }
}

impl Component for HfidView {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfid_view",
            vec![Port::real("audio"), Port::typed::<HfidResult>("results")],
            vec![Port::new("events", Kind::Messages)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.result = None;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for &v in io.real(0) {
            self.n += 1;
            if self.level.len() >= self.fs as usize {
                self.level.pop_front();
            }
            self.level.push_back(v * v);
        }
        let mut changed = false;
        for r in io.items::<HfidResult>(1) {
            self.analyses += 1;
            if let Some(c) = r.value.candidates.first() {
                if r.value.features.signal && c.waveform != SignalClass::Unclassified {
                    self.p.results_mut().log(format!(
                        "HF ID: best estimate {} ({:.0} %), identification only",
                        c.waveform.name(),
                        c.confidence * 100.0
                    ));
                }
            }
            self.result = Some(r.value.clone());
            changed = true;
        }
        if changed || io.tick() {
            self.publish();
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

sdrkit_core::settings! {
    /// The settings of the identifier: those of its test signal, the 2G ALE generator.
    pub enum HfidSetting {
        /// Calls of the test signal.
        GenCalls = crate::ale::GEN_CALLS,
        /// Channel bit errors of the test signal.
        GenBer = crate::ale::GEN_BER,
    }
}

/// The identifier's graph.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("front", HfidFront::default())
        .add("analyser", HfidAnalyser::default())
        .add("view", HfidView::default())
        .input("front")
        .connect("front", "analyser")
        .connect("front", "view.audio")
        .connect("analyser", "view.results")
        .output("view")
        .clock(0.25)
        .status_from("view")
}

/// The identifier's receiver.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("hfid"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(24e3, "HF ID: the sample rate is too low (24 kHz at least)")
    .min_rate_live(
        250e3,
        "HF ID: the sample rate of a live radio is too low (250 kHz at least)",
    )
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    // The test signal is that of 2G ALE.
    Ok(Box::new(crate::ale::gen::AleGenerator::from_config(cfg)))
}

/// The HF waveform identifier.
pub static HFID: StackDescriptor = StackDescriptor {
    id: "hfid",
    name: "HF waveform ID",
    group: "HF utility",
    blurb: "Identification only: measures an HF data signal's physical layer and ranks the waveforms it could be (estimates).",
    tuning: Tuning {
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(4.625e6, 2e6, 250e3, 6e3)
    },
    submodes: &[],
    settings: HfidSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HFID),
    benchmarks: None,
};
