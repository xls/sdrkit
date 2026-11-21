//! The beacon detection chain: the stage-1 filter bank, the stage-2 channel meter, the run detector and
//! the keying judges.

use super::list::{self, Beacon, Pattern};
use crate::util::Origin;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::exp_order_mean;
use sdrkit_algo_dsp::window::{hann_bin_power, Window};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, HostRequest, Record, Setting, SettingError, SettingId, SettingSet,
    Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port,
    RunContext, Timed,
};
use sdrkit_util_text::morse::{fit_dot, Decoder, DecoderConfig, Event as MorseEvent};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// One watched channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// The beacon's frequency corrected by the ppm setting, Hz.
    pub freq_hz: f64,
    /// Index among the watched stage-1 bins.
    pub slot: usize,
    /// Stage-2 bin of the beacon (-16 .. 15).
    pub j0: i64,
    pub pattern: Pattern,
    pub letter: Option<char>,
    pub search_hz: f64,
    pub wide: bool,
    /// The beacons sharing it (indices into the list).
    pub beacons: Vec<usize>,
    /// 1.5 / the Hann pass-band power shape at the tone's place in its stage-1 bin.
    pub g: f64,
    pub dash_index: u8,
}

/// The channel plan of a capture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub rate: f64,
    pub centre_hz: f64,
    pub n: usize,
    pub r1: f64,
    /// Stage-1 bin numbers watched (negative below the centre).
    pub bins: Vec<i64>,
    pub channels: Vec<Channel>,
    pub list: Vec<Beacon>,
    /// Which beacons are in the capture.
    pub in_capture: Vec<bool>,
    pub ppm: f64,
    /// The kinds watched.
    pub watch: Watch,
}

/// The stage-1 size for a rate: the largest power of two at or below rate / 1500, 8 to 16 384.
pub fn stage1_size(rate: f64) -> usize {
    let mut n = 8usize;
    while (n * 2) as f64 <= rate / 1500.0 && n < 16_384 {
        n *= 2;
    }
    n
}

/// Which kinds are watched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Watch {
    pub ncdxf: bool,
    pub dashes: bool,
    pub letters: bool,
    pub carriers: bool,
}

impl Default for Watch {
    fn default() -> Self {
        Watch {
            ncdxf: true,
            dashes: true,
            letters: true,
            carriers: true,
        }
    }
}

impl Watch {
    /// Whether beacons of keying `p` are watched.
    pub fn takes(&self, p: Pattern) -> bool {
        match p {
            Pattern::Ncdxf => self.ncdxf,
            Pattern::Dashes => self.dashes,
            Pattern::Letter => self.letters,
            Pattern::Carrier => self.carriers,
        }
    }
}

/// Builds the plan for a capture.
pub fn plan(list: &[Beacon], rate: f64, centre_hz: f64, ppm: f64, watch: Watch) -> Plan {
    let n = stage1_size(rate);
    let r1 = rate / n as f64;
    let mut p = Plan {
        rate,
        centre_hz,
        n,
        r1,
        list: list.to_vec(),
        in_capture: vec![false; list.len()],
        ppm,
        watch,
        ..Default::default()
    };
    let reach = 0.4 * rate - 3e3;
    for (i, b) in list.iter().enumerate() {
        if !watch.takes(b.pattern) {
            continue;
        }
        let f = b.khz * 1e3 * (1.0 + ppm * 1e-6);
        let off = f - centre_hz;
        if off.abs() > reach || off.abs() < 3.2 * r1 {
            continue;
        }
        p.in_capture[i] = true;
        if let Some(c) = p.channels.iter_mut().find(|c| {
            (c.freq_hz - f).abs() < 0.5
                && c.search_hz == b.search_hz
                && c.pattern == b.pattern
                && c.letter == b.letter
        }) {
            c.beacons.push(i);
            continue;
        }
        let k = (off / r1).round() as i64;
        let slot = match p.bins.iter().position(|&x| x == k) {
            Some(s) => s,
            None => {
                p.bins.push(k);
                p.bins.len() - 1
            }
        };
        let frac = off / r1 - k as f64;
        let r2 = r1 / 32.0;
        let j0 = ((off - k as f64 * r1) / r2).round() as i64;
        p.channels.push(Channel {
            freq_hz: f,
            slot,
            j0: j0.clamp(-16, 15),
            pattern: b.pattern,
            letter: b.letter,
            search_hz: b.search_hz,
            wide: b.search_hz > 300.0,
            beacons: vec![i],
            g: 1.5 / hann_bin_power(frac).max(1e-3),
            dash_index: b.dash_index,
        });
    }
    p
}

/// The plan of the capture (`hfb.plan`). The stage-1 bank sends it once after each configuration, before its first bins.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem(pub Arc<Plan>);

impl Item for PlanItem {
    const KIND: &'static str = "hfb.plan";
}

/// The plan items of input `i`, newest last.
fn take_plan(io: &Io, i: usize) -> Option<Arc<Plan>> {
    io.items::<PlanItem>(i)
        .last()
        .map(|p| Arc::clone(&p.value.0))
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Stage 1

/// `hfdata.hfb_stage1`, the stage-1 filter bank: `Iq` in. The watched bins as interleaved `Iq` at r1 x bins out, and the plan. It
/// also requests the host to step through the bands (`hop`) when the capture of a live radio is narrower than the watched list.
pub struct BeaconBank {
    list: Vec<Beacon>,
    list_text: String,
    plan_sent: bool,
    ppm: f64,
    watch: Watch,
    plan: Arc<Plan>,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    work: Vec<Complex32>,
    fill: usize,
    hop: bool,
    dwell_s: f64,
    live: bool,
    tuned_hz: f64,
    hopping: bool,
    asked: bool,
    requests: Vec<HostRequest>,
}

impl Default for BeaconBank {
    fn default() -> Self {
        BeaconBank {
            list: list::builtin(),
            list_text: String::new(),
            plan_sent: false,
            ppm: 0.0,
            watch: Watch::default(),
            plan: Arc::default(),
            fft: None,
            win: Vec::new(),
            buf: Vec::new(),
            work: Vec::new(),
            fill: 0,
            hop: true,
            dwell_s: 60.0,
            live: false,
            tuned_hz: 0.0,
            hopping: false,
            asked: false,
            requests: Vec::new(),
        }
    }
}

/// The frequency error of the radio.
pub const PPM: Setting<field::F64> = Setting::new("ppm", "Frequency error", "ppm", 0.0)
    .range(-100.0, 100.0, 0.1)
    .help("The frequency error of the radio's oscillator: 1 ppm is 14 Hz at 14 MHz. Signals are looked for at f x (1 + ppm 10^-6).");
/// Step through the bands.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Step through the bands", "", true)
    .help("Step through the bands when the capture is narrower than the list.");
/// The time on each stop.
pub const DWELL: Setting<field::Duration> = Setting::new("dwell_s", "Dwell", "s", 60.0)
    .range(10.0, 600.0, 1.0)
    .help("Time per stop. 180 s per stop hears all 18 NCDXF beacons on every band in the capture. The Long Dashes cycle is 27 s. Letters repeat within seconds.");
/// Watch the NCDXF/IARU beacons.
pub const WATCH_NCDXF: Setting<field::Bool> = Setting::new("watch_ncdxf", "NCDXF/IARU", "", true);
/// Watch the Long Dashes.
pub const WATCH_DASHES: Setting<field::Bool> =
    Setting::new("watch_dashes", "Long Dashes", "", true);
/// Watch the single letters.
pub const WATCH_LETTERS: Setting<field::Bool> =
    Setting::new("watch_letters", "Single letters", "", true);
/// Watch the carriers of the list.
pub const WATCH_CARRIERS: Setting<field::Bool> =
    Setting::new("watch_carriers", "Your carriers", "", true);
/// The beacon list in the format of the list file. Empty: the built-in list.
pub const LIST: Setting<field::Text> = Setting::new("list_json", "Beacon list", "", "")
    .help("A beacon list in the JSON format of the list file. Empty: the built-in list.");

sdrkit_core::settings! {
    /// The settings of [`BeaconBank`]: the beacon list, the watch and the stepping.
    pub enum BankSetting {
        /// Frequency error.
        Ppm = PPM,
        /// Step through the bands.
        Hop = HOP,
        /// Dwell.
        Dwell = DWELL,
        /// NCDXF/IARU.
        WatchNcdxf = WATCH_NCDXF,
        /// Long Dashes.
        WatchDashes = WATCH_DASHES,
        /// Single letters.
        WatchLetters = WATCH_LETTERS,
        /// Your carriers.
        WatchCarriers = WATCH_CARRIERS,
        /// Beacon list.
        List = LIST,
    }
}

/// Goes back to the built-in beacon list.
pub const RESTORE: Command = Command::new("list.restore", "Restore the built-in list")
    .help("Goes back to the built-in beacon list.");

sdrkit_core::commands! {
    /// The commands of [`BeaconBank`].
    pub enum BankCommand {
        /// Restore the built-in list.
        Restore = RESTORE,
    }
}

impl BeaconBank {
    /// The radio centres that cover the watched beacons with this capture.
    pub fn stops(&self) -> Vec<f64> {
        let khz: Vec<f64> = self
            .list
            .iter()
            .filter(|b| self.watch.takes(b.pattern))
            .map(|b| b.khz * (1.0 + self.ppm * 1e-6))
            .collect();
        list::band_plan(&khz, self.plan.rate)
    }

    /// The hop request the settings ask for, once samples from a live radio come in.
    fn hop_request(&mut self) {
        if !(self.asked && self.live) || self.plan.rate <= 0.0 {
            return;
        }
        let stops = self.stops();
        if self.hop && stops.len() > 1 {
            self.requests.push(HostRequest::Hop {
                channels: stops,
                dwell_s: self.dwell_s,
                stop_on_lock: false,
            });
            self.hopping = true;
        } else if self.hopping {
            // Stay where the stepping is now.
            self.requests.push(HostRequest::retune(
                self.tuned_hz,
                "HF beacons: stepping off",
            ));
            self.hopping = false;
        }
    }
}

impl Component for BeaconBank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_stage1",
            vec![Port::iq("in")],
            vec![
                Port::iq("bins"),
                Port::typed::<HostRequest>("requests"),
                Port::typed::<PlanItem>("plan"),
            ],
        )
        .with_settings(BankSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let p = plan(&self.list, f.rate, f.centre_hz, self.ppm, self.watch);
        let n = p.n;
        self.tuned_hz = f.tuned_hz;
        self.fft = Some(Fft::forward(n));
        self.win = (0..n)
            .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
            .collect();
        self.buf = vec![Complex32::new(0.0, 0.0); n];
        self.work = vec![Complex32::new(0.0, 0.0); n];
        self.fill = 0;
        let m = p.bins.len().max(1);
        let o = Format::iq(p.r1 * m as f64, f.centre_hz, f.tuned_hz);
        self.plan = Arc::new(p);
        self.plan_sent = false;
        Ok(vec![
            o,
            Format::new(Kind::typed::<HostRequest>(), 0.0),
            Format::new(Kind::typed::<PlanItem>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        if !self.asked {
            // The first request waits for the first samples: the run context (live or not) is known by then.
            self.asked = true;
            self.hop_request();
        }
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
        if !std::mem::replace(&mut self.plan_sent, true) {
            io.items_out::<PlanItem>(2).push(Timed {
                time: 0.0,
                value: PlanItem(Arc::clone(&self.plan)),
            });
        }
        let x = io.iq(0);
        let n = self.plan.n;
        if n == 0 {
            return;
        }
        let out = io.iq_out(0);
        for &v in x {
            self.buf[self.fill] = v;
            self.fill += 1;
            if self.fill == n {
                self.fill = 0;
                if self.plan.bins.is_empty() {
                    continue;
                }
                for i in 0..n {
                    self.work[i] = self.buf[i] * self.win[i];
                }
                if let Some(f) = self.fft.as_mut() {
                    f.process(&mut self.work);
                }
                for &k in &self.plan.bins {
                    out.push(self.work[k.rem_euclid(n as i64) as usize]);
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::Ppm => self.ppm = PPM.read(id, v)?,
            BankSetting::List => {
                let t = LIST.read(id, v)?;
                self.list = if t.trim().is_empty() {
                    list::builtin()
                } else {
                    list::parse(t).map_err(|e| SettingError::Malformed { id, at: e.at() })?
                };
                self.list_text = t.to_owned();
            }
            BankSetting::WatchNcdxf => self.watch.ncdxf = WATCH_NCDXF.read(id, v)?,
            BankSetting::WatchDashes => self.watch.dashes = WATCH_DASHES.read(id, v)?,
            BankSetting::WatchLetters => self.watch.letters = WATCH_LETTERS.read(id, v)?,
            BankSetting::WatchCarriers => self.watch.carriers = WATCH_CARRIERS.read(id, v)?,
            BankSetting::Hop => {
                self.hop = HOP.read(id, v)?;
                self.hop_request();
                return Ok(Applied::Live);
            }
            BankSetting::Dwell => {
                self.dwell_s = DWELL.read(id, v)?;
                self.hop_request();
                return Ok(Applied::Live);
            }
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([
            (BankSetting::Ppm, Value::F64(self.ppm)),
            (BankSetting::Hop, Value::Bool(self.hop)),
            (BankSetting::Dwell, Value::Duration(self.dwell_s)),
            (BankSetting::WatchNcdxf, Value::Bool(self.watch.ncdxf)),
            (BankSetting::WatchDashes, Value::Bool(self.watch.dashes)),
            (BankSetting::WatchLetters, Value::Bool(self.watch.letters)),
            (BankSetting::WatchCarriers, Value::Bool(self.watch.carriers)),
            (BankSetting::List, Value::Text(self.list_text.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        BankCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match BankCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BankCommand::Restore => {
                self.list = list::builtin();
                self.list_text.clear();
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Stage 2 and the channel meter

/// The excess of every channel at one hop (`hfb.excess`).
#[derive(Clone, Debug, PartialEq)]
pub struct ExcessFrame {
    pub hop: u64,
    /// Signal time of the hop, seconds.
    pub time: f64,
    /// Per channel: excess x, and the SNR in 100 Hz it stands for.
    pub x: Vec<f32>,
    pub snr: Vec<f32>,
    /// The noise trained (the first 20 hops only train it).
    pub trained: bool,
}

impl Item for ExcessFrame {
    const KIND: &'static str = "hfb.excess";
}

/// `hfdata.hfb_stage2`, stage 2 and the channel meter: the watched bins and the plan in, `hfb.excess` out every hop.
#[derive(Default)]
pub struct BeaconMeter {
    plan: Arc<Plan>,
    m: usize,
    rings: Vec<Vec<Complex32>>,
    k: u64,
    fft: Option<Fft>,
    win: Vec<f32>,
    work: Vec<Complex32>,
    pow: Vec<Vec<f64>>,
    noise: Vec<f64>,
    run_bin: Vec<Option<i64>>,
    hops: u64,
    scratch: Vec<f64>,
    out: Vec<ExcessFrame>,
    /// Set by the run detector's state: whether a channel's run is on (kept here from the excess, so the meter needs no feedback).
    on: Vec<bool>,
    xs: Vec<VecDeque<f32>>,
}

impl BeaconMeter {
    fn hop(&mut self) {
        let n2 = 32usize;
        for s in 0..self.m {
            for i in 0..n2 {
                let idx = (self.k as usize + i) % n2;
                self.work[i] = self.rings[s][idx] * self.win[i];
            }
            if let Some(f) = self.fft.as_mut() {
                f.process(&mut self.work);
            }
            for j in 0..n2 {
                self.pow[s][j] = self.work[j].norm_sqr() as f64;
            }
        }
        self.hops += 1;
        let dt = 16.0 / self.plan.r1;
        let alpha = (dt / 3.0).max(1.0 / self.hops as f64);
        let nchan = self.plan.channels.len();
        let mut fx = Vec::with_capacity(nchan);
        let mut fs = Vec::with_capacity(nchan);
        for (c, ch) in self.plan.channels.iter().enumerate() {
            let p = &self.pow[ch.slot];
            let at = |j: i64| p[j.rem_euclid(32) as usize];
            let s3 = |j: i64| at(j - 1) + at(j) + at(j + 1);
            let r2 = self.plan.r1 / 32.0;
            let (j, s) = if ch.wide {
                if let (true, Some(b)) = (self.on[c], self.run_bin[c]) {
                    // While a run is on, keep the bin that started it. The best of +-1 bins lifts the noise between two dashes by
                    // the maximum of three, and that holds a run open across the 0.1 s gap in place 0 of the Long Dashes.
                    (b, s3(b))
                } else {
                    let w = ((ch.search_hz / r2).round() as i64).min(13);
                    let mut best = (ch.j0, s3(ch.j0));
                    for jj in ch.j0 - w..=ch.j0 + w {
                        if s3(jj) > best.1 {
                            best = (jj, s3(jj));
                        }
                    }
                    best
                }
            } else {
                (ch.j0, s3(ch.j0))
            };
            if ch.wide && !self.on[c] {
                self.run_bin[c] = Some(j);
            }
            // The noise: the 25 % point of the other bins, scaled to a mean.
            self.scratch.clear();
            for jj in 0..32i64 {
                let d = (jj - j.rem_euclid(32)).rem_euclid(32);
                if d > 2 && d < 30 {
                    self.scratch.push(p[jj as usize]);
                }
            }
            let nn = self.scratch.len();
            let r = (nn / 4).max(1);
            self.scratch
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let q = self.scratch[r - 1];
            let e = exp_order_mean(nn, r).unwrap_or(1.0);
            let nb_now = q / e.max(1e-9);
            let nb = &mut self.noise[c];
            if *nb <= 0.0 {
                *nb = nb_now;
            } else {
                *nb += alpha * (nb_now - *nb);
            }
            let nb = nb.max(1e-30);
            let x = (s - 3.0 * nb) / nb;
            let snr = 10.0 * (x.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
            fx.push(x as f32);
            fs.push(snr as f32);
            // Whether a run is on, from the mean of the last 3 x (as the run detector sees it).
            let xs = &mut self.xs[c];
            if xs.len() >= 3 {
                xs.pop_front();
            }
            xs.push_back(x as f32);
            let m3: f32 = xs.iter().sum::<f32>() / xs.len() as f32;
            let thr = if ch.wide { 14.0 } else { 9.0 };
            if m3 > thr {
                self.on[c] = true;
            } else if m3 < 0.4 * thr {
                self.on[c] = false;
            }
        }
        let time = self.k as f64 / self.plan.r1;
        self.out.push(ExcessFrame {
            hop: self.hops,
            time,
            x: fx,
            snr: fs,
            trained: self.hops > 20,
        });
    }
}

impl Component for BeaconMeter {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_stage2",
            vec![Port::iq("bins"), Port::typed::<PlanItem>("plan")],
            vec![Port::typed::<ExcessFrame>("excess")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.set_plan(Arc::default());
        let mut o = Format::new(Kind::typed::<ExcessFrame>(), 0.0);
        o.centre_hz = inputs[0].centre_hz;
        o.tuned_hz = inputs[0].tuned_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = take_plan(io, 1) {
            self.set_plan(p);
        }
        self.run(io);
    }
}

impl BeaconMeter {
    fn set_plan(&mut self, p: Arc<Plan>) {
        self.m = p.bins.len();
        self.rings = vec![vec![Complex32::new(0.0, 0.0); 32]; self.m];
        self.pow = vec![vec![0.0; 32]; self.m];
        self.k = 0;
        self.fft = Some(Fft::forward(32));
        self.win = Window::Hann.coefficients(32);
        self.work = vec![Complex32::new(0.0, 0.0); 32];
        self.noise = vec![0.0; p.channels.len()];
        self.run_bin = vec![None; p.channels.len()];
        self.on = vec![false; p.channels.len()];
        self.xs = vec![VecDeque::new(); p.channels.len()];
        self.hops = 0;
        self.plan = p;
    }
    fn run(&mut self, io: &mut Io) {
        if self.m == 0 {
            return;
        }
        let x = io.iq(0);
        for frame in x.chunks_exact(self.m) {
            let i = (self.k % 32) as usize;
            for (s, &v) in frame.iter().enumerate() {
                self.rings[s][i] = v;
            }
            self.k += 1;
            if self.k >= 32 && self.k.is_multiple_of(16) {
                self.hop();
            }
        }
        let o = io.items_out::<ExcessFrame>(0);
        for f in self.out.drain(..) {
            o.push(Timed {
                time: f.time,
                value: f,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Runs

/// An on or off run of a channel (`hfb.run`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub ch: usize,
    /// Signal time of the start, seconds.
    pub start: f64,
    pub duration: f64,
    pub on: bool,
    /// Mean excess over its middle half.
    pub level: f64,
}

impl Item for Run {
    const KIND: &'static str = "hfb.run";
}

#[derive(Clone, Debug, Default)]
struct RunState {
    xs: VecDeque<f32>,
    on: bool,
    above: u32,
    below: u32,
    start_hop: u64,
    start_t: f64,
    hist: VecDeque<f32>,
    off_start: Option<f64>,
}

/// `hfdata.hfb_runs`, the run detector: `hfb.excess` and the plan in, `hfb.run` out.
#[derive(Default)]
pub struct RunDetector {
    plan: Arc<Plan>,
    st: Vec<RunState>,
    pub started: u64,
    out: Vec<Run>,
}

impl RunDetector {
    fn take(&mut self, f: &ExcessFrame) {
        let dt = 16.0 / self.plan.r1;
        for (c, ch) in self.plan.channels.iter().enumerate() {
            let s = &mut self.st[c];
            let x = f.x[c];
            if s.hist.len() >= 4096 {
                s.hist.pop_front();
            }
            s.hist.push_back(x);
            if s.xs.len() >= 3 {
                s.xs.pop_front();
            }
            s.xs.push_back(x);
            if !f.trained {
                continue;
            }
            let m3: f32 = s.xs.iter().sum::<f32>() / s.xs.len() as f32;
            let thr = if ch.wide { 14.0 } else { 9.0 };
            if !s.on {
                if m3 > thr {
                    s.above += 1;
                    if s.above >= 2 {
                        s.on = true;
                        s.below = 0;
                        s.start_hop = f.hop - 1;
                        s.start_t = f.time - dt;
                        self.started += 1;
                        if let Some(o) = s.off_start.take() {
                            self.out.push(Run {
                                ch: c,
                                start: o,
                                duration: s.start_t - o,
                                on: false,
                                level: 0.0,
                            });
                        }
                    }
                } else {
                    s.above = 0;
                }
            } else if m3 < 0.4 * thr {
                s.below += 1;
                if s.below >= 3 {
                    s.on = false;
                    s.above = 0;
                    let end_t = f.time - 2.0 * dt;
                    // The run's hops as x saw them (the mean of 3 lags x by one hop): start_hop - 1 ..= f.hop - 4.
                    let h = s.hist.len() as u64;
                    let j0 = s.start_hop.saturating_sub(1);
                    let j1 = f.hop.saturating_sub(4).max(j0);
                    let level = if f.hop - j0 < h {
                        let a = (h - 1 - (f.hop - j0)) as usize;
                        let b = (h - 1 - (f.hop - j1)) as usize;
                        let seg: Vec<f32> =
                            s.hist.iter().skip(a).take(b - a + 1).copied().collect();
                        let q = seg.len() / 4;
                        let e = (seg.len() - seg.len() / 4).max(q + 1).min(seg.len());
                        seg[q..e].iter().map(|&v| v as f64).sum::<f64>() / (e - q) as f64
                    } else {
                        x as f64
                    };
                    self.out.push(Run {
                        ch: c,
                        start: s.start_t,
                        duration: end_t - s.start_t,
                        on: true,
                        level,
                    });
                    s.off_start = Some(end_t);
                }
            } else {
                s.below = 0;
            }
        }
    }
}

impl Component for RunDetector {
    fn values(&self) -> Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_runs",
            vec![
                Port::typed::<ExcessFrame>("excess"),
                Port::typed::<PlanItem>("plan"),
            ],
            vec![Port::typed::<Run>("runs")],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.plan = Arc::default();
        self.st.clear();
        Ok(vec![Format::new(Kind::typed::<Run>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = take_plan(io, 1) {
            self.st = vec![RunState::default(); p.channels.len()];
            self.plan = p;
        }
        for f in io.items::<ExcessFrame>(0) {
            self.take(&f.value);
        }
        let o = io.items_out::<Run>(0);
        for r in self.out.drain(..) {
            o.push(Timed {
                time: r.start,
                value: r,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Keying judges

/// A judgment about one beacon (`hfb.detection`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    /// Index into the list.
    pub beacon: usize,
    /// Signal time, seconds.
    pub time: f64,
    pub snr_db: f64,
    /// 0 nothing, 1 energy (keying not confirmed), 2 identity confirmed.
    pub confidence: u8,
    pub steps: u8,
    pub step_snrs: Vec<f64>,
    pub note: String,
    /// A judged NCDXF slot (a history point, heard or not).
    pub slot: bool,
    pub heard: bool,
}

impl Item for Detection {
    const KIND: &'static str = "hfb.detection";
}

/// What the judges send: detections, and a periodic tick with the signal time.
#[derive(Clone, Debug, PartialEq)]
pub enum Judged {
    Detection(Detection),
    /// Once a second: the time, the frames processed, the runs started, whether the clock is valid.
    Tick {
        time: f64,
        hops: u64,
        runs: u64,
        clock: bool,
    },
}

impl Item for Judged {
    const KIND: &'static str = "hfb.detection";
}

#[derive(Clone, Debug, Default)]
struct ChanJudge {
    runs: VecDeque<Run>,
    dashes: VecDeque<(f64, bool)>,
    unmatched_until: f64,
    xs: VecDeque<(f64, f32)>,
}

/// How far the computer clock runs ahead of true UTC.
pub const CLOCK_OFFSET: Setting<field::F64> = Setting::new("clock_offset_s", "Clock offset", "s", 0.0)
    .range(-30.0, 30.0, 0.1)
    .help("How far the computer clock runs ahead of true UTC. The NCDXF slots need it right to about a second.");
/// The UTC of the first sample of a recording, seconds since 1970. 0 is unknown.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc_s",
    "Recording start (UTC seconds; 0 unknown)",
    "s",
    0.0,
)
.range(0.0, 1e11, 0.0);

sdrkit_core::settings! {
    /// The settings of [`KeyingJudges`].
    pub enum JudgeSetting {
        /// Clock offset.
        ClockOffset = CLOCK_OFFSET,
        /// Recording start.
        StartUtc = START_UTC,
    }
}

/// `hfdata.hfb_judges`, the keying judges: `hfb.excess`, `hfb.run` and the plan in, `hfb.detection` out.
pub struct KeyingJudges {
    plan: Arc<Plan>,
    ch: Vec<ChanJudge>,
    origin: Origin,
    live: bool,
    clock_offset: f64,
    start_utc: Option<f64>,
    next_slot_judge: f64,
    next_tick: f64,
    hops: u64,
    runs: u64,
    out: Vec<Judged>,
    now: f64,
}

impl Default for KeyingJudges {
    fn default() -> Self {
        KeyingJudges {
            plan: Arc::default(),
            ch: Vec::new(),
            origin: Origin::default(),
            live: false,
            clock_offset: 0.0,
            start_utc: None,
            next_slot_judge: 0.0,
            next_tick: 1.0,
            hops: 0,
            runs: 0,
            out: Vec::new(),
            now: 0.0,
        }
    }
}

impl KeyingJudges {
    fn clock(&self) -> Option<f64> {
        if let Some(s) = self.start_utc {
            return Some(s);
        }
        self.origin.valid.then_some(self.origin.unix_s)
    }

    /// True UTC of signal time t.
    fn utc(&self, t: f64) -> Option<f64> {
        self.clock().map(|c| c + t - self.clock_offset)
    }

    fn emit(&mut self, ch: usize, d: Detection) {
        for &b in &self.plan.channels[ch].beacons.clone() {
            let mut d = d.clone();
            d.beacon = b;
            self.out.push(Judged::Detection(d));
        }
    }

    fn letter(&mut self, c: usize, now: f64) {
        let Some(letter) = self.plan.channels[c].letter else {
            return;
        };
        let runs: Vec<Run> = self.ch[c].runs.iter().copied().collect();
        if runs.iter().filter(|r| !r.on).count() < 3 {
            return;
        }
        let keys: Vec<(bool, f64)> = runs.iter().map(|r| (r.on, r.duration)).collect();
        let unit = fit_dot(&keys, 0.1).unwrap_or(0.0);
        let mut matched = 0;
        let mut mismatched = 0;
        if (0.03..=0.7).contains(&unit) {
            // Characters end at a long gap: a character still being keyed is not read.
            let mut chars: Vec<Option<&str>> = Vec::new();
            if let Ok(mut d) = Decoder::new(DecoderConfig::fixed((1.2 / unit).clamp(1.0, 200.0))) {
                for &(on, s) in &keys {
                    d.push(on, s, &mut |e| {
                        if let MorseEvent::Char(c) = e {
                            chars.push(c.text);
                        }
                    });
                }
            }
            let mut b = [0u8; 4];
            let want: &str = letter.encode_utf8(&mut b);
            for g in chars.iter().rev().take(8) {
                if g.is_some_and(|g| g.eq_ignore_ascii_case(want)) {
                    matched += 1;
                } else {
                    mismatched += 1;
                }
            }
        }
        if matched >= 2 && mismatched * 3 <= matched {
            let ons: Vec<f64> = runs
                .iter()
                .filter(|r| r.on)
                .rev()
                .take(12)
                .map(|r| r.level)
                .collect();
            let lvl = ons.iter().sum::<f64>() / ons.len().max(1) as f64;
            let ch = &self.plan.channels[c];
            let snr = 10.0 * (lvl.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
            self.emit(
                c,
                Detection {
                    beacon: 0,
                    time: now,
                    snr_db: snr,
                    confidence: 2,
                    steps: 0,
                    step_snrs: vec![],
                    note: format!("'{letter}' keyed, unit {:.0} ms", unit * 1000.0),
                    slot: false,
                    heard: true,
                },
            );
        } else {
            self.ch[c].unmatched_until = now + 20.0;
        }
    }

    fn dash(&mut self, c: usize, r: &Run) {
        if !(2.2..=3.6).contains(&r.duration) {
            return;
        }
        let place = self.plan.channels[c].dash_index;
        let t = r.start;
        let mut confirmed = false;
        let mut note = "dash seen, waiting for the next cycle";
        for &(pt, _) in &self.ch[c].dashes {
            let d = t - pt;
            if [27.0, 54.0, 81.0, 108.0]
                .iter()
                .any(|k| (d - k).abs() <= 0.9)
                || (place == 0 && ((d - 3.0).abs() <= 0.5 || (d - 24.0).abs() <= 0.9))
            {
                confirmed = true;
                note = "dash repeats on the 27 s cycle";
            }
        }
        // Another Long Dashes frequency in step with the 3 s ladder of the same cycle.
        let phase = |t: f64, p: u8| (t - 3.0 * p as f64).rem_euclid(27.0);
        let me = phase(t, place);
        let mut partners = Vec::new();
        for (o, och) in self.plan.channels.iter().enumerate() {
            if o == c
                || och.pattern != Pattern::Dashes
                || (och.freq_hz - self.plan.channels[c].freq_hz).abs() < 1.0
            {
                continue;
            }
            for &(pt, _) in &self.ch[o].dashes {
                if t - pt > 100.0 {
                    continue;
                }
                let mut d = (phase(pt, och.dash_index) - me).abs();
                if place == 0 {
                    d = d.min((phase(pt, och.dash_index) - phase(t, 1)).abs());
                }
                let d = d.min(27.0 - d);
                if d <= 0.6 {
                    partners.push(o);
                    if !confirmed {
                        confirmed = true;
                        note = "dash in step with another frequency";
                    }
                }
            }
        }
        self.ch[c].dashes.push_back((t, confirmed));
        while self.ch[c].dashes.front().is_some_and(|d| t - d.0 > 130.0) {
            self.ch[c].dashes.pop_front();
        }
        let ch = &self.plan.channels[c];
        let snr = 10.0 * (r.level.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
        self.emit(
            c,
            Detection {
                beacon: 0,
                time: t + r.duration,
                snr_db: snr,
                confidence: if confirmed { 2 } else { 1 },
                steps: 0,
                step_snrs: vec![],
                note: note.into(),
                slot: false,
                heard: true,
            },
        );
        partners.sort();
        partners.dedup();
        for o in partners {
            let last = self.ch[o].dashes.back().map_or(t, |d| d.0);
            self.emit(
                o,
                Detection {
                    beacon: 0,
                    time: last + 2.9,
                    snr_db: f64::NAN,
                    confidence: 2,
                    steps: 0,
                    step_snrs: vec![],
                    note: "dash in step with another frequency".into(),
                    slot: false,
                    heard: true,
                },
            );
        }
    }

    fn mean_x(&self, c: usize, a: f64, b: f64) -> f64 {
        let v: Vec<f64> = self.ch[c]
            .xs
            .iter()
            .filter(|x| x.0 >= a && x.0 < b)
            .map(|x| x.1 as f64)
            .collect();
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    }

    /// Judges the NCDXF slot that ended before `t_utc` for every NCDXF channel.
    fn ncdxf(&mut self, slot_start_sig: f64, slot_start_utc: f64) {
        let chans: Vec<usize> = (0..self.plan.channels.len())
            .filter(|&c| self.plan.channels[c].pattern == Pattern::Ncdxf)
            .collect();
        for c in chans {
            let band = self.plan.list[self.plan.channels[c].beacons[0]]
                .ncdxf_band
                .unwrap_or(0);
            let k = list::ncdxf_on(band, slot_start_utc + 5.0);
            let Some(&bi) = self.plan.channels[c]
                .beacons
                .iter()
                .find(|&&b| self.plan.list[b].ncdxf_index == Some(k))
            else {
                continue;
            };
            let s0 = slot_start_sig;
            let runs: Vec<Run> = self.ch[c]
                .runs
                .iter()
                .copied()
                .filter(|r| r.on && r.start >= s0 - 0.5 && r.start < s0 + 10.0)
                .collect();
            // The 100 W dash: the first on-run starting 2.8 to 7 s into the slot lasting 0.75 to 4.6 s.
            let run = runs.iter().find(|r| {
                (2.8..=7.0).contains(&(r.start - s0)) && (0.75..=4.6).contains(&r.duration)
            });
            let dash_start = match run {
                Some(r) => {
                    // Its edge refined to the first hop reaching half the dash's own level.
                    let half = r.level / 2.0;
                    self.ch[c]
                        .xs
                        .iter()
                        .find(|x| x.0 >= r.start - 0.3 && x.1 as f64 >= half)
                        .map_or(r.start, |x| x.0)
                }
                None => {
                    // The 1 s stretch of highest level from 2.8 to 6 s.
                    let mut best = (s0 + 2.8, f64::MIN);
                    let mut t = s0 + 2.8;
                    let mut levels = Vec::new();
                    while t <= s0 + 6.0 {
                        let m = self.mean_x(c, t, t + 1.0);
                        levels.push((t, m));
                        if m > best.1 {
                            best = (t, m);
                        }
                        t += 0.05;
                    }
                    let plateau: Vec<f64> = levels
                        .iter()
                        .filter(|l| l.1 >= 0.9 * best.1)
                        .map(|l| l.0)
                        .collect();
                    if plateau.is_empty() {
                        best.0
                    } else {
                        (plateau[0] + plateau[plateau.len() - 1]) / 2.0
                    }
                }
            };
            // The gap between the dashes (0 when they are back to back), from the 100 W dash's falling edge and the 10 W dash's
            // rising edge when both stand out.
            let l0 = self.mean_x(c, dash_start + 0.15, dash_start + 0.85);
            let mut gap = 0.0;
            if l0 >= 40.0 {
                let xs: Vec<(f64, f32)> = self.ch[c]
                    .xs
                    .iter()
                    .copied()
                    .filter(|x| x.0 >= dash_start + 0.5 && x.0 <= dash_start + 1.6)
                    .collect();
                if let Some(e) = xs.iter().find(|x| (x.1 as f64) < l0 / 2.0) {
                    if let Some(r) = xs
                        .iter()
                        .find(|x| x.0 > e.0 + 0.02 && (x.1 as f64) > l0 / 20.0)
                    {
                        let g = r.0 - e.0;
                        if (0.05..=0.4).contains(&g) {
                            gap = g;
                        }
                    }
                }
            }
            let lv: Vec<f64> = (0..4)
                .map(|i| {
                    let a = dash_start + i as f64 * (1.0 + gap);
                    self.mean_x(c, a + 0.15, a + 0.85)
                })
                .collect();
            let mut steps = 0u8;
            for &l in &lv {
                if l >= 1.0 {
                    steps += 1;
                } else {
                    break;
                }
            }
            let keyed = runs
                .iter()
                .filter(|r| r.start - s0 >= 0.05 && r.start - s0 <= 3.6)
                .count()
                >= 4;
            let call = self.mean_x(c, s0 + 0.1, dash_start - 0.1);
            let after = self.mean_x(c, dash_start + 4.0, dash_start + 5.4);
            let weak = lv[0] >= 1.4 && call >= 0.3 * lv[0] && after <= 0.4 * lv[0];
            let heard = steps >= 1 && (run.is_some() || keyed || weak);
            let ch = &self.plan.channels[c];
            let snr_of = |x: f64| 10.0 * (x.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
            let names = ["100 W only", "to 10 W", "to 1 W", "100 W to 0.1 W"];
            let note = if heard {
                format!(
                    "{steps} of 4 power steps ({}){}",
                    names[(steps.max(1) - 1) as usize],
                    if keyed { ", callsign keyed" } else { "" }
                )
            } else {
                "slot listened, nothing heard".into()
            };
            let d = Detection {
                beacon: bi,
                time: s0 + 5.0,
                snr_db: if heard { snr_of(lv[0]) } else { f64::NAN },
                confidence: if !heard {
                    0
                } else if run.is_some() && keyed {
                    2
                } else {
                    1
                },
                steps: if heard { steps } else { 0 },
                step_snrs: if heard {
                    lv[..steps as usize].iter().map(|&x| snr_of(x)).collect()
                } else {
                    vec![]
                },
                note,
                slot: true,
                heard,
            };
            self.out.push(Judged::Detection(d));
        }
    }

    fn take_frame(&mut self, f: &ExcessFrame) {
        self.hops += 1;
        self.now = f.time;
        for (c, &x) in f.x.iter().enumerate() {
            let q = &mut self.ch[c].xs;
            if q.len() >= 4096 {
                q.pop_front();
            }
            q.push_back((f.time, x));
        }
        // NCDXF: 0.5 s after each slot ends.
        if let Some(u) = self.utc(f.time) {
            let slot_end = (u / 10.0).floor() * 10.0;
            if u - slot_end >= 0.5 && slot_end > self.next_slot_judge - 1e-6 && f.trained {
                let slot_start_utc = slot_end - 10.0;
                let slot_start_sig = f.time - (u - slot_start_utc);
                self.next_slot_judge = slot_end + 10.0;
                if slot_start_sig >= 0.0 {
                    self.ncdxf(slot_start_sig, slot_start_utc);
                }
            }
        }
        if f.time >= self.next_tick {
            self.next_tick = f.time.floor() + 1.0;
            // Carriers: a mean x of at least 1.5 over the last 3 s.
            for c in 0..self.plan.channels.len() {
                let ch = &self.plan.channels[c];
                let m = self.mean_x(c, f.time - 3.0, f.time + 1.0);
                match ch.pattern {
                    Pattern::Carrier => {
                        if m >= 1.5 {
                            let snr =
                                10.0 * (m.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
                            self.emit(
                                c,
                                Detection {
                                    beacon: 0,
                                    time: f.time,
                                    snr_db: snr,
                                    confidence: 2,
                                    steps: 0,
                                    step_snrs: vec![],
                                    note: "carrier".into(),
                                    slot: false,
                                    heard: true,
                                },
                            );
                        }
                    }
                    Pattern::Letter | Pattern::Dashes
                        if self.ch[c].unmatched_until > f.time && m >= 1.0 =>
                    {
                        let snr = 10.0 * (m.max(1e-3) * ch.g / 32.0 * self.plan.r1 / 100.0).log10();
                        self.emit(
                            c,
                            Detection {
                                beacon: 0,
                                time: f.time,
                                snr_db: snr,
                                confidence: 1,
                                steps: 0,
                                step_snrs: vec![],
                                note: "energy, keying not matched".into(),
                                slot: false,
                                heard: false,
                            },
                        );
                    }
                    _ => {}
                }
            }
            let clock = self.clock().is_some();
            self.out.push(Judged::Tick {
                time: f.time,
                hops: self.hops,
                runs: self.runs,
                clock,
            });
        }
    }

    fn take_run(&mut self, r: &Run) {
        let c = r.ch;
        if r.on {
            self.runs += 1;
        }
        let q = &mut self.ch[c].runs;
        if q.len() >= 90 {
            q.pop_front();
        }
        q.push_back(*r);
        match self.plan.channels[c].pattern {
            Pattern::Letter => self.letter(c, r.start + r.duration),
            Pattern::Dashes if r.on => self.dash(c, r),
            Pattern::Dashes => {}
            Pattern::Carrier | Pattern::Ncdxf => {}
        }
        if r.on && self.plan.channels[c].pattern == Pattern::Letter {
            // A long steady run is energy without keying.
            if r.duration > 2.0 {
                self.ch[c].unmatched_until = r.start + r.duration + 20.0;
            }
        }
    }
}

impl Component for KeyingJudges {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_judges",
            vec![
                Port::typed::<ExcessFrame>("excess"),
                Port::typed::<Run>("runs"),
                Port::typed::<PlanItem>("plan"),
            ],
            vec![Port::typed::<Judged>("detections")],
        )
        .with_settings(JudgeSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.plan = Arc::default();
        self.ch.clear();
        self.origin = Origin::new(self.live);
        self.next_slot_judge = 0.0;
        self.next_tick = 1.0;
        let mut o = Format::new(Kind::typed::<Judged>(), 0.0);
        o.centre_hz = inputs[0].centre_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(p) = take_plan(io, 2) {
            self.ch = vec![ChanJudge::default(); p.channels.len()];
            self.plan = p;
        }
        // Frames and runs in time order (a run is complete before the frame that ended it is judged).
        let frames = io.items::<ExcessFrame>(0);
        let runs = io.items::<Run>(1);
        let mut ri = 0;
        for f in frames {
            while ri < runs.len() && runs[ri].value.start + runs[ri].value.duration <= f.value.time
            {
                let r = runs[ri].value;
                self.take_run(&r);
                ri += 1;
            }
            self.take_frame(&f.value);
        }
        while ri < runs.len() {
            let r = runs[ri].value;
            self.take_run(&r);
            ri += 1;
        }
        let o = io.items_out::<Judged>(0);
        for d in self.out.drain(..) {
            o.push(Timed {
                time: 0.0,
                value: d,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match JudgeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            JudgeSetting::ClockOffset => self.clock_offset = CLOCK_OFFSET.read(id, v)?,
            JudgeSetting::StartUtc => {
                let s = START_UTC.read(id, v)?;
                self.start_utc = (s > 0.0).then_some(s);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (JudgeSetting::ClockOffset, Value::F64(self.clock_offset)),
            (
                JudgeSetting::StartUtc,
                Value::F64(self.start_utc.unwrap_or(0.0)),
            ),
        ])
    }
}
