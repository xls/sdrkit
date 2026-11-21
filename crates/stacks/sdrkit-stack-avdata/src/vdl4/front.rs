//! The VDL4 front end: per channel the shift, the shared resampler to 4 samples per bit, the FM discriminator, the shared soft
//! correlator on the training sequence with a noise floor, the slicer, NRZI decoding and HDLC.
//! Standard: ETSI EN 302 842-2 clause 6 (GFSK at 19 200 bit/s, training sequence, NRZI, HDLC framing).

use super::{Burst, ChannelState, FrontState, BAUD, CHANNELS, MIN_RATE_HZ};
use crate::filters::blackman_lowpass;
use crate::report::freq_list;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::interp::{Linear, Reader};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::correlate::{SoftCorrelator, SoftPeak};
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_link::hdlc::{Line, Receiver};
use sdrkit_util_bits::hdlc::HdlcDeframer;
use sdrkit_util_bits::line::Nrzi;

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The channel list.
        Channels = CHANNELS,
    }
}

/// Reads the `channels` setting: frequencies in MHz in the VHF band 118 to 137 MHz.
///
/// # Errors
///
/// The byte position of the first word that is not such a frequency.
pub fn channel_list(t: &str) -> Result<Vec<f64>, usize> {
    freq_list(t, 1e6, (117.975e6, 137e6))
}

/// Ramp ones before the training sequence.
pub const RAMP_BITS: usize = 16;
/// Training bits (0101 ... 01).
pub const TRAIN_BITS: usize = 24;
/// Line bits collected after the training.
pub const COLLECT_BITS: usize = 480;

const SPB: usize = 4;
/// The channel rate: four samples per bit.
const CHANNEL_RATE: f64 = SPB as f64 * BAUD;
/// The edge of the channel filter, Hz: the deviation and half the bit rate.
const CHANNEL_HZ: f64 = 12e3;
/// The training search's least Pearson correlation.
const THRESHOLD: f32 = 0.75;

/// The line levels of the ramp's last 5 bits and the training (NRZI: a 0 turns the level over), oldest first.
fn pattern() -> Vec<f32> {
    let mut level = 1.0f32;
    let mut v = vec![level];
    for k in 0..4 + TRAIN_BITS {
        let bit = k < 4 || (k - 4) % 2 == 1;
        if !bit {
            level = -level;
        }
        v.push(level);
    }
    v
}

/// A training sequence found: where its data begins and the result of the fit.
#[derive(Clone, Copy)]
struct Lock {
    /// Channel sample of the first data bit (fractional).
    data_start: f64,
    /// The slicing level (the carrier offset after the discriminator).
    centre: f32,
    offset: f32,
    level: f32,
    snr: f32,
}

struct Chan {
    freq: f64,
    nco: Nco,
    down: Resampler<Complex32>,
    channel: Fir<Complex32>,
    disc_fm: Discriminator,
    search: SoftCorrelator,
    disc: Vec<f32>,
    pow: Vec<f32>,
    base: u64,
    n: u64,
    floor: f64,
    locked: std::collections::VecDeque<Lock>,
    busy_until: f64,
    used_bits: usize,
    st: ChannelState,
    s0: Vec<Complex32>,
    s1: Vec<Complex32>,
    s2: Vec<f32>,
    peaks: Vec<SoftPeak>,
    hdlc: Receiver,
}

/// The VDL4 front end.
pub struct Front {
    channels: String,
    rate: f64,
    centre: f64,
    /// The frequency the session listens to: the channel when the list is empty.
    tuned: f64,
    chans: Vec<Chan>,
    out_of_band: Vec<f64>,
    bursts: Vec<Burst>,
    idx: u64,
    refused: String,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            channels: String::new(),
            rate: 0.0,
            centre: 0.0,
            tuned: 0.0,
            chans: Vec::new(),
            out_of_band: Vec::new(),
            bursts: Vec::new(),
            idx: 0,
            refused: String::new(),
        }
    }
}

impl Front {
    fn build(&mut self) {
        self.chans.clear();
        self.out_of_band.clear();
        self.refused.clear();
        let r = self.rate;
        if !(r.is_finite() && r >= MIN_RATE_HZ) {
            let k = if r.is_finite() && r > 0.0 {
                r / 1e3
            } else {
                0.0
            };
            self.refused = format!(
                "sample rate {k:.0} kHz is too low, use at least {:.0} kHz",
                MIN_RATE_HZ / 1e3
            );
            return;
        }
        let mut list: Vec<f64> = channel_list(&self.channels).unwrap_or_default();
        if list.is_empty() {
            list.push(self.tuned);
        }
        list.sort_by(|a, b| a.total_cmp(b));
        list.dedup();
        for f in list {
            if (f - self.centre).abs() >= 0.45 * r {
                self.out_of_band.push(f);
                continue;
            }
            let made = Resampler::with_quality(r, CHANNEL_RATE, 0.3, 60.0)
                .and_then(|down| Ok((down, Discriminator::try_new(CHANNEL_RATE, 4800.0)?)));
            let search = SoftCorrelator::new(&pattern(), SPB as f64, THRESHOLD)
                .map(|c| c.with_inverted(true));
            let ((down, disc_fm), search) = match (made, search) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) => {
                    self.refused = e.to_string();
                    self.chans.clear();
                    return;
                }
                (_, Err(e)) => {
                    self.refused = e.to_string();
                    self.chans.clear();
                    return;
                }
            };
            self.chans.push(Chan {
                freq: f,
                nco: Nco::new(r, -(f - self.centre)),
                down,
                channel: Fir::new(&blackman_lowpass(CHANNEL_RATE, CHANNEL_HZ, 61)),
                disc_fm,
                search,
                disc: Vec::new(),
                pow: Vec::new(),
                base: 0,
                n: 0,
                floor: 0.0,
                locked: Default::default(),
                busy_until: 0.0,
                used_bits: 0,
                st: ChannelState {
                    freq_hz: f,
                    in_band: true,
                    ..Default::default()
                },
                s0: Vec::new(),
                s1: Vec::new(),
                s2: Vec::new(),
                peaks: Vec::new(),
                // A burst holds the header (6 octets) and at least a little information (EN 302 842-2 clause 6.4).
                hdlc: Receiver::with_deframer(Line::Nrz, HdlcDeframer::x25(6, 62)),
            });
        }
    }
}

impl Chan {
    /// The discriminator value at channel sample `t` (linear between samples), if it is still held.
    fn at(&self, t: f64) -> Option<f32> {
        let i = t - self.base as f64;
        (i >= 0.0 && i + 1.0 < self.disc.len() as f64)
            .then(|| Reader::new(Linear).at(&self.disc, i))
    }

    /// Takes the channel's input samples (already shifted to 0 Hz).
    fn take(&mut self, x: &[Complex32], out: &mut Vec<Burst>) {
        self.s1.clear();
        self.down.process(x, &mut self.s1);
        for v in self.s1.iter_mut() {
            *v = self.channel.process_sample(*v);
        }
        self.s2.clear();
        self.disc_fm.process(&self.s1, &mut self.s2);
        for (&z, &d) in self.s1.iter().zip(&self.s2) {
            let p = z.norm_sqr();
            self.disc.push(d);
            self.pow.push(p);
            self.n += 1;
            // Noise floor: fast down, slow up.
            let pf = p as f64;
            if self.floor == 0.0 {
                self.floor = pf;
            } else if pf < self.floor {
                self.floor += 0.01 * (pf - self.floor);
            } else {
                self.floor += 0.00005 * (pf - self.floor);
            }
        }
        self.peaks.clear();
        self.search.process(&self.s2, &mut self.peaks);
        for k in 0..self.peaks.len() {
            let pk = self.peaks[k];
            let len = pattern().len();
            let end = pk.index + (len * SPB) as f64;
            // The power over the training must stand above the floor.
            let from = (pk.index.max(self.base as f64) as u64 - self.base) as usize;
            let to = ((end as u64).saturating_sub(self.base) as usize).min(self.pow.len());
            if to <= from + SPB {
                continue;
            }
            let pw = self.pow[from..to].iter().map(|&v| v as f64).sum::<f64>() / (to - from) as f64;
            if pw <= 2.5 * self.floor.max(1e-20) {
                continue;
            }
            self.st.found += 1;
            self.locked.push_back(Lock {
                data_start: end,
                centre: pk.offset,
                offset: pk.offset * 4800.0,
                level: (10.0 * pw.max(1e-20).log10()) as f32,
                snr: (10.0 * (pw / self.floor.max(1e-20)).log10()) as f32,
            });
        }
        // Decode a training sequence once its bits are all in.
        while self
            .locked
            .front()
            .is_some_and(|c| self.n as f64 >= c.data_start + (COLLECT_BITS * SPB) as f64 + 2.0)
        {
            let Some(c) = self.locked.pop_front() else {
                break;
            };
            if c.data_start < self.busy_until {
                continue;
            }
            let before = out.len();
            self.decode(&c, out);
            if out.len() > before {
                // A good burst: nothing inside it is another training sequence.
                self.busy_until = c.data_start + (self.used_bits * SPB) as f64;
            }
        }
        // Keep the history bounded.
        if self.disc.len() > 16_384 {
            let drop = self.disc.len() - 4096;
            let keep_from = self
                .locked
                .front()
                .map_or(u64::MAX, |c| (c.data_start as u64).saturating_sub(400));
            let drop = drop.min((keep_from.saturating_sub(self.base)) as usize);
            self.disc.drain(..drop);
            self.pow.drain(..drop);
            self.base += drop as u64;
        }
    }

    fn decode(&mut self, c: &Lock, out: &mut Vec<Burst>) {
        let thr = c.centre;
        let Some(last) = self.at(c.data_start - SPB as f64) else {
            return;
        };
        self.hdlc.reset();
        let mut nrzi = Nrzi::new((last > thr) as u8);
        for k in 0..COLLECT_BITS {
            let Some(v) = self.at(c.data_start + (k * SPB) as f64) else {
                break;
            };
            let mut bit = [(v > thr) as u8];
            nrzi.decode(&mut bit);
            if self.hdlc.push(bit[0]).is_some() {
                self.used_bits = k + 1;
                let time = (c.data_start - (TRAIN_BITS * SPB) as f64) / CHANNEL_RATE;
                self.st.good += 1;
                self.st.level_db = c.level;
                self.st.snr_db = c.snr;
                self.st.last_heard_s = time;
                self.st.last_offset_hz = c.offset;
                out.push(Burst {
                    time_s: time,
                    freq_hz: self.freq,
                    level_db: c.level,
                    snr_db: c.snr,
                    offset_hz: c.offset,
                    line_bits: COLLECT_BITS,
                    frame: self.hdlc.frame().to_vec(),
                });
                return;
            }
        }
        self.st.bad += 1;
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl4_front",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Burst>("bursts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FrontState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.tuned = inputs[0].tuned_hz;
        self.idx = 0;
        self.build();
        Ok(vec![
            Format::new(Kind::typed::<Burst>(), 0.0),
            Format::new(Kind::typed::<FrontState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        for c in self.chans.iter_mut() {
            c.s0.resize(x.len(), Complex32::default());
            c.nco.mix(x, &mut c.s0);
            let buf = std::mem::take(&mut c.s0);
            c.take(&buf, &mut self.bursts);
            c.s0 = buf;
        }
        self.idx += x.len() as u64;
        if !self.bursts.is_empty() {
            self.bursts.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
            let o = io.items_out::<Burst>(0);
            for b in self.bursts.drain(..) {
                o.push(Timed {
                    time: b.time_s,
                    value: b,
                });
            }
        }
        if io.tick() {
            let mut chs: Vec<ChannelState> = self
                .chans
                .iter()
                .map(|c| {
                    let mut s = c.st.clone();
                    s.floor_db = (10.0 * c.floor.max(1e-20).log10()) as f32;
                    s
                })
                .collect();
            for &f in &self.out_of_band {
                chs.push(ChannelState {
                    freq_hz: f,
                    ..Default::default()
                });
            }
            chs.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
            let t = if self.rate.is_finite() && self.rate > 0.0 {
                self.idx as f64 / self.rate
            } else {
                0.0
            };
            io.items_out::<FrontState>(1).push(Timed {
                time: t,
                value: FrontState {
                    time_s: t,
                    rate_hz: self.rate,
                    channels: chs,
                    refused: self.refused.clone(),
                },
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                channel_list(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.channels);
                if self.rate > 0.0 {
                    self.build();
                }
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(FrontSetting::Channels, Value::Text(self.channels.clone()))])
    }
}
