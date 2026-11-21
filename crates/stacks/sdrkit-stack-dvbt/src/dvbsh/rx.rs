//! The components of the SH-A receiver that DVB-T does not have: `dvbsh.input` (DC removal, the resampler to the OFDM rate of
//! the width, the clock steering of the timing loop, the automatic width search and the choice between the OFDM and the
//! TDM form), `dvbsh.data` (the DVB-SH fields of
//! TPS, the place of each symbol in the SH frame, the demapping of the chosen stream and the time de-interleaver) and
//! `dvbsh.decode` (the decoding of whole code words of either form into transport packets, and the stack's results). Between
//! the input and the data run the DVB-T finder, synchroniser, FFT, TPS decoder and equaliser in their DVB-SH reading. The
//! components of the TDM form are in [`super::tdmrx`].

use super::coding::{pattern_name, IU_BITS, TS};
use super::ofdm::{
    sh_frame_symbols, Demapper, FieldCollector, ShFields, Width, FRAME_IU, SLICE_UNIT,
};
use super::results as r;
use super::settings::{
    DataSetting, DecodeSetting, InputSetting, WaveformChoice, WidthChoice, ITERATIONS,
    LOW_PRIORITY, MAX_INTERLEAVER_MB, WAVEFORM, WIDTH,
};
use super::tdm::SLOT;
use super::tdmrx::TdmState;
use super::shframe::{FrameRx, FrameShape, WordDecoder};
use crate::items::{DataCells, Link};
use crate::params::{Constellation, Hierarchy, Mode};
use crate::typed::DvbshFields;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Complex32, ConfigError, LockState, Results, SettingError, SettingId, SettingSet,
    Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    SlicePool, Timed,
};
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

/// Signal seconds an automatic width is tried without an OFDM symbol found.
pub const WIDTH_TRY_S: f64 = 0.3;
/// Signal seconds an automatic width is kept with symbols found but no TPS lock.
pub const WIDTH_TPS_S: f64 = 3.0;
/// Signal seconds per usable width that the automatic waveform gives the TDM form without slots found.
pub const TDM_TRY_S: f64 = 1.3;

/// What the components of the stack share beside the DVB-T link: the width in use and the state of the TDM form.
#[derive(Debug, Default)]
pub struct ShLink {
    /// The index of the width in use in [`Width::SEARCH`].
    pub width: AtomicU8,
    /// The input feeds the TDM form.
    pub tdm_on: AtomicBool,
    /// Counts the times the input began to feed the TDM form.
    pub tdm_epoch: AtomicU32,
    /// The TDM demodulator: 0 searching, 1 slots found, 2 an SH frame with its signalling field.
    pub tdm_stage: AtomicU8,
    /// A carrier offset in Hz (the bits of an `f64`) that the TDM demodulator asks the TDM front end to take off, and
    /// the count of such requests.
    pub tdm_shift: AtomicU64,
    /// See `tdm_shift`.
    pub tdm_shifts: AtomicU32,
    /// The input samples taken since the stack was configured.
    pub samples: AtomicU64,
    /// The TDM demodulator erases slots for want of pilots: the TDM front end holds its symbol clock.
    pub tdm_blocked: AtomicBool,
}

/// `dvbsh.input`.
pub struct ShInput {
    link: Arc<Link>,
    sh: Arc<ShLink>,
    choice: WidthChoice,
    waveform: WaveformChoice,
    /// The automatic waveform feeds the TDM form now.
    tdm: bool,
    /// Signal seconds of the TDM turn without slots found.
    tdm_since: f64,
    /// Input samples since the stack was configured.
    total: u64,
    width: Width,
    in_rate: f64,
    correction: f64,
    resampler: Option<FractionalResampler<Complex32>>,
    dc: Complex32,
    clean: Vec<Complex32>,
    since: f64,
    reformatting: bool,
    power: f64,
    power_n: u64,
}

impl ShInput {
    /// An input stage on `link` and `sh`.
    pub fn new(link: Arc<Link>, sh: Arc<ShLink>) -> Self {
        ShInput {
            link,
            sh,
            choice: WidthChoice::Auto,
            waveform: WaveformChoice::Auto,
            tdm: false,
            tdm_since: 0.0,
            total: 0,
            width: Width::W5,
            in_rate: 0.0,
            correction: 1.0,
            resampler: None,
            dc: Complex32::default(),
            clean: Vec::new(),
            since: 0.0,
            reformatting: false,
            power: 0.0,
            power_n: 0,
        }
    }

    fn widths(&self) -> Vec<Width> {
        match self.choice.width() {
            Some(w) => vec![w],
            None => Width::SEARCH
                .into_iter()
                .filter(|w| w.usable(self.in_rate))
                .collect(),
        }
    }

    /// Feeds the TDM form (`on`) or the OFDM form from the next sample.
    fn set_tdm(&mut self, on: bool) {
        self.tdm = on;
        self.tdm_since = 0.0;
        self.since = 0.0;
        self.sh.tdm_on.store(on, Ordering::Relaxed);
        if on {
            self.sh.tdm_epoch.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn build_resampler(&mut self) -> Result<(), ConfigError> {
        let out = self.width.ofdm_rate();
        let low = self.in_rate.min(out);
        let pass = (0.42 * out).min(0.47 * low);
        let stop = (low - pass).max(pass * 1.05);
        let mut r = FractionalResampler::new(self.in_rate, out, pass, stop, 48.0)
            .map_err(|_| ConfigError::NoRate)?;
        r.set_correction(self.correction);
        self.resampler = Some(r);
        Ok(())
    }
}

impl Component for ShInput {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.input",
            vec![Port::iq("in")],
            vec![Port::iq("out"), Port::iq("tdm")],
        )
        .with_settings(InputSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.in_rate = f.rate;
        self.link.sh.store(true, Ordering::Relaxed);
        let widths = self.widths();
        if std::mem::take(&mut self.reformatting) {
            self.link.request_restart();
        } else {
            self.link.reset();
            self.width = widths.first().copied().unwrap_or(Width::W1_7);
            self.set_tdm(self.waveform == WaveformChoice::Tdm);
            self.total = 0;
        }
        if !widths.contains(&self.width) {
            let need = self.width.ofdm_rate() / 1.02;
            return Err(ConfigError::RateTooLow { need, got: f.rate });
        }
        self.sh
            .width
            .store(self.width.index() as u8, Ordering::Relaxed);
        self.correction = 1.0;
        self.link.set_clock_ppm(0.0);
        self.dc = Complex32::default();
        self.since = 0.0;
        self.build_resampler()?;
        Ok(vec![
            Format {
                rate: self.width.ofdm_rate(),
                bandwidth_hz: self.width.mhz() * 1e6,
                ..f.clone()
            },
            f.clone(),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let n = x.len();
        self.total += n as u64;
        self.sh.samples.store(self.total, Ordering::Relaxed);
        let auto_form = self.waveform == WaveformChoice::Auto;
        if self.tdm {
            // The TDM turn of the automatic waveform ends when no slots are found in time: the OFDM form is tried again
            // from its first width.
            if auto_form && self.sh.tdm_stage.load(Ordering::Relaxed) == 0 {
                self.tdm_since += n as f64 / self.in_rate.max(1.0);
                let widths = self.widths();
                if self.tdm_since > TDM_TRY_S * widths.len().max(1) as f64 {
                    self.set_tdm(false);
                    let first = widths.first().copied().unwrap_or(self.width);
                    if first != self.width {
                        self.width = first;
                        self.reformatting = true;
                        io.request_reformat();
                    } else {
                        self.link.request_restart();
                    }
                }
            } else {
                self.tdm_since = 0.0;
            }
        } else if (self.choice == WidthChoice::Auto || auto_form) && !self.link.locked() {
            // The automatic width: the next usable one when nothing is found in time. After the last width the
            // automatic waveform gives the TDM form its turn.
            self.since += n as f64 / self.in_rate.max(1.0);
            let found = self.link.candidate.load(Ordering::Relaxed);
            let limit = if found { WIDTH_TPS_S } else { WIDTH_TRY_S };
            let widths = self.widths();
            let at = widths.iter().position(|&w| w == self.width).unwrap_or(0);
            let last = at + 1 >= widths.len() || self.choice != WidthChoice::Auto;
            if self.since > limit && auto_form && last {
                self.link.candidate.store(false, Ordering::Relaxed);
                self.set_tdm(true);
            } else if self.since > limit && self.choice == WidthChoice::Auto && widths.len() > 1 {
                self.width = widths[(at + 1) % widths.len()];
                self.reformatting = true;
                self.link.candidate.store(false, Ordering::Relaxed);
                io.request_reformat();
            }
        } else {
            self.since = 0.0;
        }
        let d = self.link.take_clock();
        if d != 0.0 {
            self.correction *= 1.0 + d * 1e-6;
            self.link.set_clock_ppm((self.correction - 1.0) * 1e6);
            if let Some(r) = self.resampler.as_mut() {
                r.set_correction(self.correction);
            }
            // The timing loop measures the clock again on the samples from here.
            self.link.set_clock_from(io.output(0).first());
        }
        self.clean.clear();
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            self.dc += (s - self.dc) * 1e-5;
            let y = s - self.dc;
            self.power += y.norm_sqr() as f64;
            self.power_n += 1;
            self.clean.push(y);
        }
        if let Ok(mut t) = self.link.telemetry.lock() {
            if self.power_n > 0 {
                t.level_dbfs =
                    Some((10.0 * (self.power / self.power_n as f64).max(1e-20).log10()) as f32);
            }
        }
        self.power = 0.0;
        self.power_n = 0;
        if self.tdm {
            io.iq_out(1).extend_from_slice(&self.clean);
            return;
        }
        let out = io.iq_out(0);
        if let Some(r) = self.resampler.as_mut() {
            r.process(&self.clean, out);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match InputSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InputSetting::Width => self.choice = WIDTH.read(id, v)?,
            InputSetting::Waveform => self.waveform = WAVEFORM.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (InputSetting::Width, Value::Choice(self.choice.index())),
            (InputSetting::Waveform, Value::Choice(self.waveform.index())),
        ])
    }
}

/// The data path that runs for one set of parameters.
struct Active {
    key: Key,
    shape: FrameShape,
    demap: Demapper,
    /// The symbol's place in the SH frame, symbols.
    at: usize,
    symbols: usize,
    /// The next symbol expected: (index in the frame, frame number).
    next: (usize, u8),
    /// The signal time of the last symbol fed.
    last_time: f64,
    /// The duration of a symbol, s.
    symbol_s: f64,
}

impl Active {
    /// The symbols missing before `c`: the gap of whole symbols, within 2 of what the time since the last symbol says, that
    /// brings the expected place to `c`'s. `None` when no such gap exists or the time went back.
    fn gap(&self, c: &DataCells) -> Option<usize> {
        let dt = c.time - self.last_time;
        if dt <= 0.0 || dt > MAX_GAP_S {
            return None;
        }
        let est = (dt / self.symbol_s).round() as i64 - 1;
        let next = self.next.1 as i64 * 68 + self.next.0 as i64;
        let want = c.frame as i64 * 68 + c.l as i64;
        (est - 2..=est + 2)
            .filter(|&g| g >= 0)
            .find(|&g| (next + g).rem_euclid(272) == want)
            .map(|g| g as usize)
    }
}

/// The longest gap that erased symbols fill, s; after a longer one the data path starts again.
pub const MAX_GAP_S: f64 = 2.0;

/// What the data path depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    mode: Mode,
    c: Constellation,
    h: Hierarchy,
    id: u8,
    ti: ShTimeInterleaver,
    lp: bool,
}

/// Why the data path does not run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    /// A reserved puncturing pattern.
    Pattern(u8),
    /// The de-interleaver needs more memory than the setting allows, MB.
    Memory(f64),
    /// The interleaver profile is not valid.
    Profile,
}

impl Refusal {
    /// The reason in words.
    pub fn text(self) -> String {
        match self {
            Refusal::Pattern(id) => format!("reserved puncturing pattern {id}"),
            Refusal::Memory(mb) => format!("interleaver too long for this stack ({mb:.0} MB)"),
            Refusal::Profile => "invalid interleaver profile".into(),
        }
    }
}

/// A code word of the running data path, de-interleaved: its soft values in the order of the code and the puncturing
/// pattern of its code. The time of the item is that of the symbol that completed it.
#[derive(Clone, Debug)]
pub struct ShWord {
    /// The soft values.
    pub llr: Arc<[i8]>,
    /// The Punct_Pat_ID.
    pub id: u8,
}

impl Item for ShWord {
    const KIND: &'static str = "dvbsh.word";
}

/// The state of `dvbsh.data` that the results show, after a pass with symbols or a clock tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShState {
    fields: Option<ShFields>,
    superframe: Option<u8>,
    /// The parameters and the frame shape of the running data path.
    path: Option<(Key, FrameShape)>,
    refused: Option<Refusal>,
    /// The low-priority stream is chosen.
    lp: bool,
    /// The data paths started so far.
    starts: u32,
}

impl Item for ShState {
    const KIND: &'static str = "dvbsh.state";
}

/// `dvbsh.data`.
pub struct ShData {
    link: Arc<Link>,
    lp: bool,
    max_mb: f64,
    collector: FieldCollector,
    fields: Option<ShFields>,
    /// The super-frame number of the current super-frame in its SH frame, once known.
    superframe: Option<u8>,
    active: Option<Active>,
    refused: Option<Refusal>,
    /// The time de-interleaver of the running data path.
    rx: Option<FrameRx>,
    /// The soft units of the symbol in work.
    units: Vec<i8>,
    pool: SlicePool<i8>,
    words: Vec<Timed<ShWord>>,
    starts: u32,
    now: f64,
}

impl ShData {
    /// A data stage on `link`.
    pub fn new(link: Arc<Link>) -> Self {
        ShData {
            link,
            lp: false,
            max_mb: 768.0,
            collector: FieldCollector::default(),
            fields: None,
            superframe: None,
            active: None,
            refused: None,
            rx: None,
            units: Vec::new(),
            pool: SlicePool::default(),
            words: Vec::new(),
            starts: 0,
            now: 0.0,
        }
    }

    fn reset(&mut self) {
        self.collector = FieldCollector::default();
        self.fields = None;
        self.superframe = None;
        self.stop();
    }

    fn stop(&mut self) {
        if self.active.take().is_some() {
            self.rx = None;
        }
    }

    /// The key of the parameters in force, when TPS has given them all.
    fn key(&self, c: &DataCells) -> Option<Key> {
        let f = self.fields?;
        let p = c.params;
        let lp = self.lp && p.hierarchy.hierarchical();
        Some(Key {
            mode: p.mode,
            c: p.constellation,
            h: p.hierarchy,
            id: if lp { f.lp } else { f.hp },
            ti: ShTimeInterleaver::from_bits(f.ti_bits),
            lp,
        })
    }

    /// Whether symbol `c` starts an SH frame.
    fn frame_start(&self, c: &DataCells) -> bool {
        if c.l != 0 {
            return false;
        }
        let frames =
            sh_frame_symbols(c.params.mode, c.params.constellation, c.params.hierarchy) / 68;
        if frames <= 4 {
            return (c.frame as usize).is_multiple_of(frames);
        }
        c.frame == 0 && self.superframe == Some(0)
    }

    fn start(&mut self, key: Key, c: &DataCells) {
        let n_bil = match super::coding::ShCodec::new(key.id) {
            Ok(c) => c.n_bil(),
            Err(_) => {
                self.refused = Some(Refusal::Pattern(key.id));
                return;
            }
        };
        let shape = FrameShape::new(FRAME_IU, n_bil, key.ti, SLICE_UNIT);
        match FrameRx::memory(&shape).map(|b| b as f64 / 1e6) {
            Ok(mb) if mb <= self.max_mb => {}
            Ok(mb) => {
                self.refused = Some(Refusal::Memory(mb));
                return;
            }
            Err(_) => {
                self.refused = Some(Refusal::Profile);
                return;
            }
        }
        self.rx = FrameRx::new(shape).ok();
        self.refused = None;
        self.starts = self.starts.wrapping_add(1);
        self.active = Some(Active {
            key,
            shape,
            demap: Demapper::new(key.mode, key.c, key.h, key.lp),
            at: 0,
            symbols: sh_frame_symbols(key.mode, key.c, key.h),
            next: (c.l, c.frame),
            last_time: c.time,
            symbol_s: c.params.symbol_samples() as f64 / c.rate.max(1.0),
        });
    }

    fn symbol(&mut self, c: &DataCells) {
        if c.fresh {
            // A new acquisition: the TPS words start again; the data path goes on if the symbols fit its place (below).
            self.collector = FieldCollector::default();
            self.superframe = None;
        }
        let frames =
            sh_frame_symbols(c.params.mode, c.params.constellation, c.params.hierarchy) / 68;
        let s = (frames / 4).max(1) as u8;
        if let Some(w) = c.tps_word {
            if let Some(f) = self.collector.push(w) {
                self.fields = Some(f);
                // The word of frame 4 arrives with the first symbol of the next super-frame.
                self.superframe = Some((f.superframe + 1) % s);
            }
        } else if c.l == 0 && c.frame == 0 {
            self.superframe = self.superframe.map(|n| (n + 1) % s);
        }
        let key = self.key(c);
        // New parameters, or a symbol out of its place that a gap of whole symbols does not explain: the data path starts
        // again at the next SH frame.
        let verdict = self.active.as_ref().map(|a| {
            if Some(a.key) != key {
                Err(())
            } else if a.next == (c.l, c.frame) {
                Ok(None)
            } else {
                match a.gap(c) {
                    Some(g) => Ok(Some((g, a.next, a.last_time, a.symbol_s))),
                    None if c.time <= a.last_time => Ok(Some((usize::MAX, a.next, 0.0, 0.0))),
                    None => Err(()),
                }
            }
        });
        match verdict {
            Some(Err(())) => self.stop(),
            // A symbol again, or one that went back in time: dropped.
            Some(Ok(Some((usize::MAX, _, _, _)))) => return,
            Some(Ok(Some((g, next, last, symbol_s)))) => {
                for i in 0..g {
                    let at = (next.1 as usize * 68 + next.0 + i) % 272;
                    let t = last + (i + 1) as f64 * symbol_s;
                    self.push(at % 68, (at / 68) as u8, &[], &[], true, t);
                }
            }
            _ => {}
        }
        if self.active.is_none() {
            match key {
                Some(k) if self.frame_start(c) => self.start(k, c),
                _ => return,
            }
        }
        self.push(c.l, c.frame, &c.cells, &c.noise, c.erased, c.time);
    }

    /// Feeds symbol `l` of frame `frame` to the running data path. A code word that completes goes out at `time`.
    fn push(
        &mut self,
        l: usize,
        frame: u8,
        cells: &[Complex32],
        noise: &[f32],
        erased: bool,
        time: f64,
    ) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        self.units.clear();
        a.demap.push(l, cells, noise, erased, &mut self.units);
        if let Some(r) = self.rx.as_mut() {
            let (pool, words, id) = (&mut self.pool, &mut self.words, a.key.id);
            for u in self.units.as_chunks::<IU_BITS>().0 {
                r.push(u, |cw| {
                    let llr = pool.exact(cw.len(), |v| v.copy_from_slice(cw));
                    words.push(Timed {
                        time,
                        value: ShWord { llr, id },
                    });
                });
            }
        }
        a.at = (a.at + 1) % a.symbols;
        a.last_time = time;
        a.next = if l == 67 {
            (0, (frame + 1) & 3)
        } else {
            (l + 1, frame)
        };
    }

    fn state(&self) -> ShState {
        ShState {
            fields: self.fields,
            superframe: self.superframe,
            path: self.active.as_ref().map(|a| (a.key, a.shape)),
            refused: self.refused,
            lp: self.lp,
            starts: self.starts,
        }
    }
}

/// "cm / lt / ns / sd / nli" of a profile.
pub fn ti_text(t: &ShTimeInterleaver) -> String {
    format!(
        "{} / {} / {} / {} / {}",
        t.common_multiplier, t.late_taps, t.slices, t.slice_distance, t.non_late_increment
    )
}

impl Component for ShData {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.data",
            vec![Port::typed::<DataCells>("in")],
            vec![
                Port::typed::<ShWord>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<ShState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DataSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![
            Format::new(Kind::typed::<ShWord>(), 0.0),
            Format::new(Kind::typed::<ShState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        self.words = std::mem::take(io.items_out::<ShWord>(0));
        let mut n = 0;
        for item in io.items::<DataCells>(0) {
            self.now = item.value.time;
            self.symbol(&item.value);
            n += 1;
        }
        if let Ok(mut t) = self.link.telemetry.lock() {
            t.now = self.now;
        }
        *io.items_out::<ShWord>(0) = std::mem::take(&mut self.words);
        if io.tick() || n > 0 {
            let value = self.state();
            io.items_out::<ShState>(1).push(Timed {
                time: self.now,
                value,
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DataSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DataSetting::LowPriority => self.lp = LOW_PRIORITY.read(id, v)?,
            DataSetting::MaxInterleaverMb => self.max_mb = MAX_INTERLEAVER_MB.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (DataSetting::LowPriority, Value::Bool(self.lp)),
            (DataSetting::MaxInterleaverMb, Value::F64(self.max_mb)),
        ])
    }
}

/// `dvbsh.decode`: the turbo decoding of whole code words into transport packets, and the stack's results from the state
/// of `dvbsh.data` and the telemetry of the link.
pub struct ShDecode {
    link: Arc<Link>,
    sh: Arc<ShLink>,
    iters: u32,
    dec: WordDecoder,
    ts: TsOut,
    packets: Vec<Timed<TsPacket>>,
    first_packet: Option<f64>,
    state: ShState,
    tdm: TdmState,
    p: Published,
    summary: String,
}

impl ShDecode {
    /// A decoder on `link` and `sh`.
    pub fn new(link: Arc<Link>, sh: Arc<ShLink>) -> Self {
        ShDecode {
            link,
            sh,
            iters: 8,
            dec: WordDecoder::default(),
            ts: TsOut::default(),
            packets: Vec::new(),
            first_packet: None,
            state: ShState::default(),
            tdm: TdmState::default(),
            p: Published::new(DvbshFields::SHAPE.fields),
            summary: String::new(),
        }
    }

    fn word(&mut self, w: &ShWord, time: f64) {
        let (ts, packets, first) = (&mut self.ts, &mut self.packets, &mut self.first_packet);
        self.dec.decode(w.id, &w.llr, self.iters, |p: &[u8; TS], ok| {
            if ok && first.is_none() {
                *first = Some(time);
            }
            ts.push(packets, p, time, false);
        });
    }

    /// Takes the state of the data stage, and logs a data path that started.
    fn set_state(&mut self, s: ShState) {
        if s.starts != self.state.starts {
            if let Some((key, _)) = s.path {
                self.p.results_mut().log(format!(
                    "data path: {} {} rate {}, interleaver {}",
                    key.mode.text(),
                    key.c.text(),
                    pattern_name(key.id),
                    ti_text(&key.ti)
                ));
            }
        }
        self.state = s;
    }

    /// Takes the state of the TDM demodulator, and logs a data path that started. Without an SH frame no packet counts
    /// as the first.
    fn set_tdm(&mut self, s: TdmState) {
        if s.starts != self.tdm.starts {
            if let (Some(m), Some(sig)) = (s.modulation, s.signalling) {
                self.p.results_mut().log(format!(
                    "data path: TDM {} rate {}, frame {} CU, interleaver {}",
                    m.label(),
                    pattern_name(sig.id),
                    sig.width_cu,
                    ti_text(&sig.ti)
                ));
            }
        }
        if s.stage < 2 {
            self.first_packet = None;
        }
        self.tdm = s;
    }

    /// The results of the TDM form.
    fn publish_tdm(&mut self) {
        let s = self.tdm;
        let t = self.link.telemetry.lock().map(|t| t.clone()).unwrap_or_default();
        let stats = self.dec.stats;
        let found = s.stage >= 1;
        let lock = match s.stage {
            2 if self.first_packet.is_some() => 4,
            2 => 3,
            1 if s.modulation.is_some() => 2,
            n => i64::from(n),
        };
        let c = s.candidate;
        let rate = s.signalling.map(|g| pattern_name(g.id));
        let interleaver = s.signalling.map(|g| ti_text(&g.ti));
        let lmax = s.frame.and_then(|f| f.shape.ti.max_delay(f.shape.slice_unit).ok());
        let interleaver_s = s.frame.zip(lmax).zip(c).map(|((f, lmax), c)| {
            // 48 L_max units at the units of the DATA part per frame.
            let frame_s = (f.slots * SLOT) as f64 / c.rate;
            Seconds(48.0 * lmax as f64 / f.shape.data_iu() as f64 * frame_s)
        });
        let refused = s.refused.map(Refusal::text);
        let mer = s.mer_db;
        self.p.put(&DvbshFields {
            waveform: r::Waveform::Tdm,
            width_mhz: c.map_or(0.0, |c| c.width.mhz()),
            lock,
            cfo_hz: found.then_some(Hertz(s.cfo_hz)),
            fft_size: None,
            guard_samples: None,
            mode: None,
            guard: None,
            constellation: None,
            hierarchy: None,
            rate_hp: rate.as_deref(),
            rate_lp: None,
            stream: "HP",
            interleaver: interleaver.as_deref(),
            interleaver_s,
            interleaver_mb: s
                .frame
                .map(|f| FrameRx::memory(&f.shape).unwrap_or(0) as f64 / 1e6),
            refused: refused.as_deref(),
            cell_id: None,
            superframe: None,
            codewords_ok: stats.codewords_ok as i64,
            codewords_bad: stats.codewords_bad as i64,
            packets_ok: stats.packets_ok as i64,
            packets_bad: stats.packets_bad as i64,
            turbo_iterations: (stats.codewords_ok > 0)
                .then(|| stats.iterations as f64 / stats.codewords_ok as f64),
            first_packet_s: self.first_packet.map(Seconds),
            note: super::NOTE,
            clock_ppm: if found { f64::from(s.clock_ppm) } else { 0.0 },
            level_dbfs: t.level_dbfs.map(f64::from),
            tdm_modulation: s.modulation.map(|m| m.label()),
            tdm_symbol_rate: c.map(|c| c.rate),
            tdm_rolloff: c.filter(|_| found).map(|c| c.rolloff.value()),
            tdm_frame_width_cu: s.signalling.map(|g| i64::from(g.width_cu)),
            tdm_mer_db: mer.map(f64::from),
            tdm_slots_erased: s.slots_erased as i64,
            tdm_signalling_failed: s.signalling_failed as i64,
        });
        self.summary.clear();
        let text = &mut self.summary;
        let msym = c.map_or(0.0, |c| c.rate / 1e6);
        // A write to a String cannot fail.
        let _ = match (s.modulation, s.signalling) {
            (Some(m), Some(g)) => write!(
                text,
                "DVB-SH TDM {} {msym:.3} Msym/s, rate {}, codewords {} ok {} bad",
                m.label(),
                pattern_name(g.id),
                stats.codewords_ok,
                stats.codewords_bad
            ),
            (Some(m), None) => write!(text, "DVB-SH TDM {} {msym:.3} Msym/s, SOF found", m.label()),
            _ if found => write!(text, "DVB-SH TDM {msym:.3} Msym/s, slots found"),
            _ if c.is_none() => write!(text, "Searching (DVB-SH TDM, no symbol rate fits the sample rate)"),
            _ => write!(text, "Searching (DVB-SH TDM {msym:.3} Msym/s)"),
        };
        if let Some(x) = mer {
            let _ = write!(text, ", MER {x:.1} dB");
        }
        let lock_state = match lock {
            0 => LockState::Searching,
            1 | 2 => LockState::Tracking,
            _ => LockState::Locked,
        };
        self.status(lock_state, mer);
    }

    /// Writes the status if a part of it changed.
    fn status(&mut self, lock_state: LockState, snr: Option<f32>) {
        let summary = &self.summary;
        self.p.results_mut().update_status(|x| {
            let same = x.lock_state == lock_state && x.snr_db == snr && x.summary == *summary;
            if !same {
                x.lock_state = lock_state;
                x.snr_db = snr;
                x.summary.clear();
                x.summary.push_str(summary);
            }
            !same
        });
    }

    fn publish(&mut self) {
        if self.sh.tdm_on.load(Ordering::Relaxed) {
            return self.publish_tdm();
        }
        let link = &self.link;
        let s = self.state;
        let t = link.telemetry.lock().map(|t| t.clone()).unwrap_or_default();
        let width = Width::SEARCH
            .into_iter()
            .find(|w| w.index() as u8 == self.sh.width.load(Ordering::Relaxed))
            .unwrap_or(Width::W5);
        let stats = self.dec.stats;
        let state = link.state.load(Ordering::Relaxed);
        let tps = link.locked().then_some(t.tps).flatten();
        let streaming = self.first_packet.is_some() && s.path.is_some();
        let lock = if streaming {
            4
        } else if s.path.is_some() || (tps.is_some() && s.fields.is_some()) {
            3
        } else if tps.is_some() {
            2
        } else if state >= 1 || link.candidate.load(Ordering::Relaxed) {
            1
        } else {
            0
        };
        let snr = t.eq.data_valid.then_some(t.eq.snr_db);
        let tp = tps.map(|(tp, _, _)| tp);
        let rate_hp = s.fields.map(|f| pattern_name(f.hp));
        let rate_lp = s
            .fields
            .filter(|_| tp.is_some_and(|tp| tp.hierarchy.hierarchical()))
            .map(|f| pattern_name(f.lp));
        let interleaver = s
            .fields
            .map(|f| ti_text(&ShTimeInterleaver::from_bits(f.ti_bits)));
        let interleaver_s = s.path.map(|(key, shape)| {
            let lmax = shape.ti.max_delay(SLICE_UNIT).unwrap_or(0);
            // 48 L_max units of 126 bits at the stream's bit rate.
            let rate = super::ofdm::stream_bits(key.mode, key.c, key.h) as f64
                / ((key.mode.fft() + t.fft.map_or(0, |f| f.1)) as f64 / width.ofdm_rate());
            Seconds(48.0 * lmax as f64 * IU_BITS as f64 / rate.max(1.0))
        });
        let interleaver_mb = s
            .path
            .map(|(_, shape)| FrameRx::memory(&shape).unwrap_or(0) as f64 / 1e6);
        let refused = s.refused.map(Refusal::text);
        self.p.put(&DvbshFields {
            waveform: r::Waveform::Ofdm,
            width_mhz: width.mhz(),
            lock,
            cfo_hz: t.fft.map(|(n, _)| {
                let spacing = width.ofdm_rate() / n as f64;
                let frac = if t.mirrored { -t.frac } else { t.frac };
                Hertz((frac + f64::from(t.int_offset.unwrap_or(0))) * spacing)
            }),
            fft_size: t.fft.map(|(n, _)| n as i64),
            guard_samples: t.fft.map(|(_, g)| g as i64),
            mode: tp.map(|tp| tp.mode.text()),
            guard: tp.map(|tp| tp.guard.text()),
            constellation: tp.map(|tp| tp.constellation.text()),
            hierarchy: tp.map(|tp| tp.hierarchy.text()),
            rate_hp: rate_hp.as_deref(),
            rate_lp: rate_lp.as_deref(),
            stream: if s.lp { "LP" } else { "HP" },
            interleaver: interleaver.as_deref(),
            interleaver_s,
            interleaver_mb,
            refused: refused.as_deref(),
            cell_id: tp.filter(|_| t.cell_known).map(|tp| i64::from(tp.cell_id)),
            superframe: s.superframe.map(i64::from),
            codewords_ok: stats.codewords_ok as i64,
            codewords_bad: stats.codewords_bad as i64,
            packets_ok: stats.packets_ok as i64,
            packets_bad: stats.packets_bad as i64,
            turbo_iterations: (stats.codewords_ok > 0)
                .then(|| stats.iterations as f64 / stats.codewords_ok as f64),
            first_packet_s: self.first_packet.map(Seconds),
            note: super::NOTE,
            clock_ppm: link.clock_ppm(),
            level_dbfs: t.level_dbfs.map(f64::from),
            tdm_modulation: None,
            tdm_symbol_rate: None,
            tdm_rolloff: None,
            tdm_frame_width_cu: None,
            tdm_mer_db: None,
            tdm_slots_erased: self.tdm.slots_erased as i64,
            tdm_signalling_failed: self.tdm.signalling_failed as i64,
        });
        self.summary.clear();
        let text = &mut self.summary;
        // A write to a String cannot fail.
        let _ = match tps {
            Some((tp, _, _)) => write!(
                text,
                "DVB-SH {} {} GI {}, rate {}, codewords {} ok {} bad",
                tp.mode.text(),
                tp.constellation.text(),
                tp.guard.text(),
                s.fields.map_or("?".into(), |f| pattern_name(if s.lp {
                    f.lp
                } else {
                    f.hp
                })),
                stats.codewords_ok,
                stats.codewords_bad
            ),
            None if lock >= 1 => write!(text, "DVB-SH {} MHz, OFDM symbols found", width.mhz()),
            None => write!(text, "Searching (DVB-SH {} MHz)", width.mhz()),
        };
        if let Some(x) = snr {
            let _ = write!(text, ", SNR {x:.1} dB");
        }
        let lock_state = match lock {
            0 => LockState::Searching,
            1 | 2 => LockState::Tracking,
            _ => LockState::Locked,
        };
        self.status(lock_state, snr);
    }
}

impl Component for ShDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbsh.decode",
            vec![
                Port::typed::<ShWord>("in"),
                Port::typed::<ShState>("state"),
                Port::typed::<ShWord>("tdm"),
                Port::typed::<TdmState>("tdm_state"),
            ],
            vec![stream::port("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DecodeSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.state = ShState {
            starts: self.state.starts,
            ..ShState::default()
        };
        self.tdm = TdmState {
            starts: self.tdm.starts,
            ..TdmState::default()
        };
        self.p
            .results_mut()
            .set_status(&Status::new(LockState::Searching).with_summary("Searching"));
        Ok(vec![Format::new(Kind::typed::<TsPacket>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        self.packets = std::mem::take(io.items_out::<TsPacket>(0));
        let mut n = 0;
        for s in io.items::<ShState>(1) {
            self.set_state(s.value);
            n += 1;
        }
        for s in io.items::<TdmState>(3) {
            self.set_tdm(s.value);
            n += 1;
        }
        for w in io.items::<ShWord>(0).iter().chain(io.items::<ShWord>(2)) {
            self.word(&w.value, w.time);
            n += 1;
        }
        *io.items_out::<TsPacket>(0) = std::mem::take(&mut self.packets);
        if io.tick() || n > 0 {
            self.publish();
        }
    }

    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        self.publish();
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecodeSetting::Iterations => self.iters = ITERATIONS.read(id, v)? as u32,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(DecodeSetting::Iterations, Value::I64(self.iters as i64))])
    }

    /// The fields measure the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.p.results_mut().fields_mut().clear();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
