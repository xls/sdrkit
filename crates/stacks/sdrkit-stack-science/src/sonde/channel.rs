//! The channel bank: one receiving channel for each carrier the search finds. Each channel mixes its carrier to 0 Hz, decimates
//! through a wide and (at 400 MHz) a narrow channel filter, demodulates the FSK and feeds one slicer for each symbol rate and the
//! frame decoders.

use super::frame::{FrameDecoder, SondeFrame, SondeType};
use super::search::{Carrier, CarrierList};
use super::slicer::Slicer;
use sdrkit_algo_modem::fsk::TonePair;
use super::{
    dfm::DfmDecoder, lms6::Lms6Decoder, m10::MeteomodemDecoder, rs41::Rs41Decoder,
    rs92::Rs92Decoder,
};
use crate::family::lowpass;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom,
    RunContext, TagKind, Timed,
};
use sdrkit_util_geo::time::LeapSeconds;

/// The demodulation profile of a channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// First decimation.
    pub r1: usize,
    /// Second decimation.
    pub r2: usize,
    /// Rate after both.
    pub fs2: f64,
    /// Stage-1 cut-off, cycles per input sample.
    pub cut1: f64,
    /// Stage-2 cut-off, Hz.
    pub cut2_hz: f64,
    /// Stage-2 taps.
    pub taps2: usize,
    /// Noise bandwidth for the presence gate, Hz.
    pub noise_bw: f64,
    /// Stage-2 cut-off of the narrow path, Hz; `None` when the profile has no narrow path (L band).
    pub narrow_cut_hz: Option<f64>,
    /// Noise bandwidth of the narrow path for its presence gate, Hz.
    pub narrow_noise_bw: f64,
    /// L band (LMS6) profile.
    pub lband: bool,
}

impl Profile {
    /// The profile for an input rate, 400 MHz sondes or L band. At 400 MHz a narrow path of 4.5 kHz cut-off (9 kHz noise
    /// bandwidth) with the wide path's taps count and decimation runs beside the 10 kHz one.
    pub fn new(rate: f64, lband: bool) -> Profile {
        let (s1, s2, cut2, nbw, tap_ref): (f64, f64, f64, f64, f64) = if lband {
            (400e3, 192e3, 95e3, 190e3, 40e3)
        } else {
            (200e3, 96e3, 10e3, 20e3, 12e3)
        };
        let r1 = ((rate / s1).floor() as usize).max(1);
        let fs1 = rate / r1 as f64;
        let r2 = ((fs1 / s2).floor() as usize).max(1);
        let mut taps2 = ((3.2 * fs1 / tap_ref).round() as usize).max(15);
        if taps2.is_multiple_of(2) {
            taps2 += 1;
        }
        Profile {
            r1,
            r2,
            fs2: fs1 / r2 as f64,
            cut1: if lband { 0.45 } else { 0.5 } / r1 as f64,
            cut2_hz: cut2.min(0.45 * fs1),
            taps2,
            noise_bw: nbw.min(fs1),
            narrow_cut_hz: (!lband).then(|| 4500.0f64.min(0.45 * fs1)),
            narrow_noise_bw: 9000.0f64.min(fs1),
            lband,
        }
    }
}

/// What a channel is doing, for the carriers table and the table of sondes.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct ChannelState {
    /// Channel number.
    pub id: usize,
    /// Offset now, Hz (mixer and residual).
    pub offset_hz: f64,
    /// Carrier level, dB.
    pub level_db: f64,
    /// It has decoded a good frame.
    pub decoded: bool,
}

/// The carriers of the last search with their assignments.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct CarrierReport {
    /// The carriers, strongest first, each with the channel that follows it.
    pub carriers: Vec<(Carrier, Option<usize>)>,
    /// The channels in use.
    pub channels: Vec<ChannelState>,
    /// Channels allowed.
    pub allowed: usize,
    /// The input rate.
    pub rate: f64,
    /// The centre frequency, Hz (0 when unknown).
    pub centre_hz: f64,
    /// Searches so far.
    pub searches: u64,
    /// Log lines from the allocator.
    pub log: Vec<String>,
}

impl Item for CarrierReport {
    const KIND: &'static str = "sonde.report";
}

/// One stage-2 path of a channel: the decimating channel filter, its discriminator (Hz), its power and presence gate, and the
/// running mean of its discriminator (the slicer threshold and the centre of the tone decision).
struct Path {
    dec: FirDecimator<Complex32>,
    out: Vec<Complex32>,
    disc: Discriminator,
    noise_bw: f64,
    pwr: f32,
    noise: f32,
    present: bool,
    mean: f64,
    mean_n: u64,
}

impl Path {
    fn new(taps: usize, cut_hz: f64, fs1: f64, profile: &Profile, noise_bw: f64) -> Path {
        Path {
            dec: FirDecimator::new(&lowpass(taps, cut_hz / fs1), profile.r2),
            out: Vec::new(),
            disc: Discriminator::new(profile.fs2, 1.0).expect("a positive channel rate"),
            noise_bw,
            pwr: 0.0,
            noise: 0.0,
            present: false,
            mean: 0.0,
            mean_n: 0,
        }
    }

    /// Sets the noise in the path from the variance per input sample at `rate`.
    fn set_noise(&mut self, noise_var: f64, rate: f64) {
        self.noise = (noise_var * self.noise_bw / rate) as f32;
    }

    /// Takes the power of one sample: present at 2.6 x the noise, absent under 1.8 x.
    fn gate(&mut self, p: f32, alpha: f32) {
        self.pwr += alpha * (p - self.pwr);
        if self.present {
            self.present = self.pwr >= 1.8 * self.noise;
        } else {
            self.present = self.pwr > 2.6 * self.noise;
        }
    }

    /// Takes one discriminator value: a plain mean over the first `init` values, then the exponential mean of factor `alpha`.
    fn track(&mut self, f: f32, init: u64, alpha: f64) {
        if self.mean_n < init {
            self.mean_n += 1;
            self.mean += (f as f64 - self.mean) / self.mean_n as f64;
        } else {
            self.mean += alpha * (f as f64 - self.mean);
        }
    }
}

/// The slicer of one symbol rate with its decoders. Types of a narrow-path bank read the narrow path when the channel has one;
/// with a tone pair the symbol is the tone-energy decision over one symbol at the clock's decision instant, centred on the
/// path's running mean, otherwise the sign of the slicer's boxcar.
struct SlicerBank {
    slicer: Slicer,
    narrow: bool,
    tones: Option<TonePair>,
    window: usize,
    decoders: Vec<Box<dyn FrameDecoder>>,
}

/// One receiving channel.
pub struct SondeChannel {
    id: usize,
    profile: Profile,
    rate_in: f64,
    osc: Nco,
    mixed: Vec<Complex32>,
    stage1: Vec<Complex32>,
    d1: FirDecimator<Complex32>,
    wide: Path,
    narrow: Option<Path>,
    alpha_p: f32,
    present: bool,
    absent_since: f64,
    dc_init: u64,
    alpha_dc: f64,
    afc_sum: f64,
    afc_n: u64,
    afc_len: u64,
    banks: Vec<SlicerBank>,
    locked: Option<(SondeType, f64)>,
    /// Offset where the channel started, Hz.
    pub start_offset: f64,
    /// Offset now (mixer), Hz.
    pub offset_hz: f64,
    /// Signal time it was made.
    pub created: f64,
    /// Last time its carrier was seen by a search.
    pub last_seen: f64,
    /// Last good frame.
    pub last_good: Option<f64>,
    /// Carrier level, dB.
    pub level_db: f64,
    /// Sample counter at the channel rate.
    n2: u64,
    /// The serial this channel decoded last.
    pub serial: Option<String>,
    /// Marked for release (it duplicates another channel).
    pub duplicate: bool,
}

impl SondeChannel {
    /// A channel at `offset_hz` for an input at `rate`. `leap` turns the GPS time of the frames into UTC.
    pub fn new(
        id: usize,
        rate: f64,
        offset_hz: f64,
        profile: Profile,
        noise_var: f64,
        t: f64,
        leap: &LeapSeconds,
    ) -> SondeChannel {
        let t1 = 5 * profile.r1 + 1;
        let fs1 = rate / profile.r1 as f64;
        let fs2 = profile.fs2;
        let mut banks = Vec::new();
        // (symbol rate, narrow path, tone deviation in Hz, decoders).
        let mut mk = |sr: f64, narrow: bool, dev: Option<f64>, d: Vec<Box<dyn FrameDecoder>>| {
            let window = ((fs2 / sr).round() as usize).max(1);
            banks.push(SlicerBank {
                slicer: Slicer::new(fs2, sr),
                narrow,
                tones: dev.map(|dev| {
                    TonePair::new(fs2, 0.0, dev, window + 2).expect("a positive rate and deviation")
                }),
                window,
                decoders: d,
            })
        };
        if profile.lband {
            mk(9616.0, false, None, vec![Box::new(Lms6Decoder::default())]);
        } else {
            mk(
                4800.0,
                true,
                Some(2400.0),
                vec![
                    Box::new(Rs41Decoder::default()),
                    Box::new(Rs92Decoder::default()),
                ],
            );
            mk(2500.0, true, Some(2400.0), vec![Box::new(DfmDecoder::default())]);
            mk(9615.0, false, Some(4800.0), vec![Box::new(MeteomodemDecoder::m10())]);
            mk(9600.0, false, Some(4800.0), vec![Box::new(MeteomodemDecoder::m20())]);
        }
        for d in banks.iter_mut().flat_map(|b| b.decoders.iter_mut()) {
            d.set_leap_seconds(leap);
        }
        let mut wide = Path::new(profile.taps2, profile.cut2_hz, fs1, &profile, profile.noise_bw);
        wide.set_noise(noise_var, rate);
        let narrow = profile.narrow_cut_hz.map(|cut| {
            let mut p = Path::new(profile.taps2, cut, fs1, &profile, profile.narrow_noise_bw);
            p.set_noise(noise_var, rate);
            p
        });
        SondeChannel {
            id,
            profile,
            rate_in: rate,
            osc: Nco::new(rate, -offset_hz),
            mixed: Vec::new(),
            stage1: Vec::new(),
            d1: FirDecimator::new(&lowpass(t1, profile.cut1), profile.r1),
            wide,
            narrow,
            alpha_p: (1.0 - (-1.0 / (0.001 * fs2)).exp()) as f32,
            present: false,
            absent_since: t,
            dc_init: (0.08 * fs2) as u64,
            alpha_dc: 1.0 - (-1.0 / (0.08 * fs2)).exp(),
            afc_sum: 0.0,
            afc_n: 0,
            afc_len: (0.1 * fs2) as u64,
            banks,
            locked: None,
            start_offset: offset_hz,
            offset_hz,
            created: t,
            last_seen: t,
            last_good: None,
            level_db: 0.0,
            n2: 0,
            serial: None,
            duplicate: false,
        }
    }

    /// The channel number.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Updates the noise estimate (variance per input sample).
    pub fn set_noise(&mut self, noise_var: f64) {
        self.wide.set_noise(noise_var, self.rate_in);
        if let Some(n) = &mut self.narrow {
            n.set_noise(noise_var, self.rate_in);
        }
    }

    /// Moves the channel to a new offset (phase continuous).
    pub fn retune(&mut self, offset_hz: f64) {
        self.offset_hz = offset_hz;
        self.osc.set_freq(-offset_hz);
    }

    /// The best estimate of the carrier's offset: the mixer plus the residual DC of the wide path's discriminator.
    pub fn measured_offset(&self) -> f64 {
        self.offset_hz + if self.wide.mean_n > 0 { self.wide.mean } else { 0.0 }
    }

    /// Moves the running means of both paths by `-step_hz` (the mixer moved by `step_hz`).
    fn shift_means(&mut self, step_hz: f64) {
        self.wide.mean -= step_hz;
        if let Some(n) = &mut self.narrow {
            n.mean -= step_hz;
        }
    }

    /// Runs input samples `x` (taken from absolute index `first` at the input rate). The mixer and both decimators run on the
    /// whole piece first, so a frequency correction takes effect from the next piece.
    pub fn run(&mut self, x: &[Complex32], first: u64, out: &mut Vec<SondeFrame>) {
        let fs2 = self.profile.fs2;
        let _ = first;
        self.mixed.resize(x.len(), Complex32::new(0.0, 0.0));
        self.osc.mix(x, &mut self.mixed);
        self.stage1.clear();
        self.d1.process(&self.mixed, &mut self.stage1);
        self.wide.out.clear();
        self.wide.dec.process(&self.stage1, &mut self.wide.out);
        let ys = std::mem::take(&mut self.wide.out);
        let mut yn = Vec::new();
        if let Some(n) = &mut self.narrow {
            n.out.clear();
            n.dec.process(&self.stage1, &mut n.out);
            yn = std::mem::take(&mut n.out);
        }
        for (i, &y) in ys.iter().enumerate() {
            self.n2 += 1;
            let t = self.n2 as f64 / fs2 + (self.created * self.rate_in).round() / self.rate_in;
            let alpha_p = self.alpha_p;
            self.wide.gate(y.norm_sqr(), alpha_p);
            let f = self.wide.disc.process_sample(y);
            // The narrow sample and its discriminator value; the wide ones where the channel has no narrow path.
            let (zn, f_n) = match (&mut self.narrow, yn.get(i)) {
                (Some(n), Some(&z)) => {
                    n.gate(z.norm_sqr(), alpha_p);
                    (z, n.disc.process_sample(z))
                }
                _ => (y, f),
            };
            let was = self.present;
            self.present = self.wide.present || self.narrow.as_ref().is_some_and(|n| n.present);
            if !was && self.present {
                let gap = t - self.absent_since;
                for b in &mut self.banks {
                    b.slicer.reset();
                    if gap > 0.15 {
                        for d in &mut b.decoders {
                            d.reset(false);
                        }
                    }
                }
            }
            if was && !self.present {
                self.absent_since = t;
                for b in &mut self.banks {
                    b.slicer.reset();
                }
            }
            if !self.present {
                continue;
            }
            // Slicer thresholds.
            let (init, alpha) = (self.dc_init, self.alpha_dc);
            self.wide.track(f, init, alpha);
            if let Some(n) = &mut self.narrow {
                n.track(f_n, init, alpha);
            }
            // AFC on the wide path.
            self.afc_sum += f as f64;
            self.afc_n += 1;
            if self.afc_n >= self.afc_len {
                let mean = self.afc_sum / self.afc_n as f64;
                if mean.abs() > 40.0 {
                    let step = (0.7 * mean).clamp(-2000.0, 2000.0);
                    self.retune(self.offset_hz + step);
                    self.shift_means(step);
                }
                self.afc_sum = 0.0;
                self.afc_n = 0;
            }
            let wide_mean = self.wide.mean;
            let narrow_mean = self.narrow.as_ref().map_or(wide_mean, |n| n.mean);
            let now = t;
            let locked = self.locked.map(|l| l.0);
            for b in &mut self.banks {
                if let Some(k) = locked {
                    if !b.decoders.iter().any(|d| d.kind() == k) {
                        continue;
                    }
                }
                let (z, v, mean) = if b.narrow {
                    (zn, f_n, narrow_mean)
                } else {
                    (y, f, wide_mean)
                };
                if let Some(tp) = &mut b.tones {
                    tp.push(z);
                }
                let Some(d) = b.slicer.push(v - mean as f32) else {
                    continue;
                };
                let sym = match &mut b.tones {
                    Some(tp) => {
                        let up = tp.upper(b.window, d.back);
                        tp.set_centre(mean);
                        up.unwrap_or(d.soft > 0.0)
                    }
                    None => d.soft > 0.0,
                };
                for dec in &mut b.decoders {
                    if locked.is_some_and(|k| k != dec.kind()) {
                        continue;
                    }
                    let before = out.len();
                    dec.push(sym, now, out);
                    for fr in &mut out[before..] {
                        fr.channel = self.id;
                        fr.offset_hz = self.offset_hz + wide_mean;
                        fr.snr_db = self.level_db;
                        if fr.ok {
                            if fr.serial.is_some() {
                                self.serial.clone_from(&fr.serial);
                            }
                            self.last_good = Some(now);
                            if let Some(k) = fr.kind {
                                self.locked = Some((k, now));
                            }
                        }
                    }
                }
            }
            if let Some((_, since)) = self.locked {
                let last = self.last_good.unwrap_or(since);
                if now - last > 20.0 {
                    self.locked = None;
                }
            }
        }
        self.wide.out = ys;
        if let Some(n) = &mut self.narrow {
            n.out = yn;
        }
    }
}

/// The channel bank component (`sonde_bank`, iq + `sonde.carriers` → `sonde.frame` + `sonde.report`).
pub struct ChannelBank {
    rate: f64,
    centre: f64,
    allowed: usize,
    channels: Vec<SondeChannel>,
    blocked: Vec<(f64, f64)>, // (offset, until)
    next_id: usize,
    noise_var: f64,
    searches: u64,
    lband: bool,
    frames: Vec<SondeFrame>,
    log: Vec<String>,
    /// Treat the centre as known (absolute frequencies).
    centre_known: bool,
    leap: LeapSeconds,
}

impl Default for ChannelBank {
    fn default() -> Self {
        ChannelBank {
            rate: 0.0,
            centre: 0.0,
            allowed: 8,
            channels: Vec::new(),
            blocked: Vec::new(),
            next_id: 0,
            noise_var: 0.0,
            searches: 0,
            lband: false,
            frames: Vec::new(),
            log: Vec::new(),
            centre_known: false,
            leap: LeapSeconds::BUILTIN,
        }
    }
}

impl ChannelBank {
    fn reset_all(&mut self) {
        self.channels.clear();
        self.blocked.clear();
        self.searches = 0;
        self.next_id = 0;
    }

    fn allocate(&mut self, list: &CarrierList, t: f64) -> CarrierReport {
        self.noise_var = list.noise_var;
        self.searches += 1;
        self.blocked.retain(|b| b.1 > t);
        let profile = Profile::new(self.rate, self.lband);
        let mut assigned: Vec<(Carrier, Option<usize>)> = Vec::new();
        for ch in &mut self.channels {
            ch.set_noise(list.noise_var);
        }
        let near = if self.lband { 75_000.0 } else { 7500.0 };
        for c in &list.carriers {
            if let Some(ch) = self
                .channels
                .iter_mut()
                .find(|ch| (ch.offset_hz - c.offset_hz).abs() < near)
            {
                ch.last_seen = t;
                ch.level_db = c.level_db;
                if ch.last_good.is_none()
                    && (ch.offset_hz - c.offset_hz).abs() > if self.lband { 30e3 } else { 300.0 }
                {
                    ch.retune(c.offset_hz);
                }
                assigned.push((*c, Some(ch.id)));
                continue;
            }
            let blocked = self
                .blocked
                .iter()
                .any(|b| (b.0 - c.offset_hz).abs() < near * 0.8);
            if c.level_db >= 6.0 && !blocked && self.channels.len() < self.allowed {
                let id = self.next_id;
                self.next_id += 1;
                let mut ch = SondeChannel::new(
                    id,
                    self.rate,
                    c.offset_hz,
                    profile,
                    list.noise_var,
                    t,
                    &self.leap,
                );
                ch.level_db = c.level_db;
                self.channels.push(ch);
                self.log.push(format!(
                    "Radiosonde: carrier at {:+.1} kHz, {:.0} dB",
                    c.offset_hz / 1e3,
                    c.level_db
                ));
                assigned.push((*c, Some(id)));
            } else {
                assigned.push((*c, None));
            }
        }
        // Releases.
        let mut keep = Vec::with_capacity(self.channels.len());
        for ch in self.channels.drain(..) {
            let never = ch.last_good.is_none() && t - ch.created > 15.0;
            let silent = ch.last_good.is_some_and(|g| t - g > 30.0);
            let walked =
                (ch.offset_hz - ch.start_offset).abs() > if self.lband { 120e3 } else { 12e3 };
            if never {
                self.blocked.push((ch.start_offset, t + 60.0));
            }
            if !(never || silent || walked) {
                keep.push(ch);
            }
        }
        self.channels = keep;
        CarrierReport {
            carriers: assigned,
            channels: self
                .channels
                .iter()
                .map(|c| ChannelState {
                    id: c.id,
                    offset_hz: c.measured_offset(),
                    level_db: c.level_db,
                    decoded: c.last_good.is_some(),
                })
                .collect(),
            allowed: self.allowed,
            rate: self.rate,
            centre_hz: if self.centre_known { self.centre } else { 0.0 },
            searches: self.searches,
            log: std::mem::take(&mut self.log),
        }
    }
}

/// The sondes followed at once.
pub const CHANNELS: Setting<field::I64> =
    Setting::new("channels", "Sondes followed at once", "", 8)
        .range(1.0, 8.0, 1.0)
        .help("Each channel costs processing time.");

sdrkit_core::settings! {
    /// The settings of [`ChannelBank`].
    pub enum BankSetting {
        /// The sondes followed at once.
        Channels = CHANNELS,
    }
}

impl Component for ChannelBank {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sonde.bank",
            vec![Port::iq("in"), Port::typed::<CarrierList>("carriers")],
            vec![
                Port::typed::<SondeFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<CarrierReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
        .with_settings(BankSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.leap = ctx.station.leap_seconds();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.centre_known = inputs[0].centre_hz != 0.0;
        self.lband = self.centre > 1500e6;
        self.reset_all();
        Ok(vec![
            Format::new(Kind::typed::<SondeFrame>(), 0.0),
            Format::new(Kind::typed::<CarrierReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let lists = io.items::<CarrierList>(1);
        let mut retune_at = None;
        for t in io.input(0).tags() {
            if let TagKind::Retuned { centre_hz, .. } = t.kind {
                retune_at = Some((t.at, centre_hz));
            }
        }
        // Cut the pass at each search result and at a retune, so allocation happens at the same sample whatever the cuts.
        let mut cuts: Vec<(u64, Option<usize>)> = lists
            .iter()
            .enumerate()
            .map(|(k, l)| (((l.time * self.rate).round() as u64).max(first), Some(k)))
            .collect();
        if let Some((at, _)) = retune_at {
            cuts.push((at, None));
        }
        cuts.sort_by_key(|c| c.0);
        let mut pos = 0usize;
        let mut frames = std::mem::take(&mut self.frames);
        for (at, what) in cuts {
            let end = ((at.saturating_sub(first)) as usize).min(x.len());
            if end > pos {
                for ch in &mut self.channels {
                    ch.run(&x[pos..end], first + pos as u64, &mut frames);
                }
                pos = end;
            }
            match what {
                Some(k) => {
                    let t = lists[k].time;
                    let rep = self.allocate(&lists[k].value, t);
                    io.items_out::<CarrierReport>(1).push(Timed {
                        time: t,
                        value: rep,
                    });
                }
                None => {
                    if let Some((_, c)) = retune_at {
                        self.centre = c;
                        self.centre_known = c != 0.0;
                        self.lband = c > 1500e6;
                    }
                    self.reset_all();
                }
            }
        }
        if pos < x.len() {
            for ch in &mut self.channels {
                ch.run(&x[pos..], first + pos as u64, &mut frames);
            }
        }
        // A channel that decodes a sonde another (stronger) channel already follows is given up, and its frames dropped.
        let n = self.channels.len();
        for i in 0..n {
            for j in 0..n {
                if i != j
                    && self.channels[i].serial.is_some()
                    && self.channels[i].serial == self.channels[j].serial
                    && !self.channels[j].duplicate
                    && (self.channels[i].level_db, self.channels[j].id)
                        < (self.channels[j].level_db, self.channels[i].id)
                {
                    self.channels[i].duplicate = true;
                }
            }
        }
        let dups: Vec<usize> = self
            .channels
            .iter()
            .filter(|c| c.duplicate)
            .map(|c| c.id())
            .collect();
        if !dups.is_empty() {
            frames.retain(|f| !dups.contains(&f.channel));
            let t = (first + x.len() as u64) as f64 / self.rate;
            for c in self.channels.iter().filter(|c| c.duplicate) {
                self.blocked.push((c.offset_hz, t + 60.0));
            }
            self.channels.retain(|c| !c.duplicate);
        }
        frames.sort_by(|a, b| a.time.total_cmp(&b.time));
        let out = io.items_out::<SondeFrame>(0);
        for f in frames.drain(..) {
            out.push(Timed {
                time: f.time,
                value: f,
            });
        }
        self.frames = frames;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::Channels => self.allowed = CHANNELS.read(id, v)? as usize,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(BankSetting::Channels, Value::I64(self.allowed as i64))])
    }
}
