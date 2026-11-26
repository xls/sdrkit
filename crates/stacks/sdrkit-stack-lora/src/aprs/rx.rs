//! The signal side of the APRS receiver: the FM front end (channel, discriminator, click limit, carrier, SNR and offset), the
//! emphasis filters and the two packet modems of `sdrkit_proto_link::modem`. Frames that two modems both find are reported once.

use crate::settings::Speeds;
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{Deemphasis, PreEmphasis};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_proto_link::modem::{Afsk1200, G3ruh9600};
use sdrkit_util_objects::{fingerprint, Dedup};
use serde::{Deserialize, Serialize};

/// The rate of the discriminator audio, Hz.
pub const AUDIO_RATE: f64 = 48_000.0;

/// The time constant of the emphasis filters (a corner at 300 Hz), seconds.
pub const EMPHASIS_TAU_S: f64 = 530.5e-6;

/// A received AX.25 frame (before parsing).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RawFrame {
    /// The frame bytes without the FCS.
    pub bytes: Vec<u8>,
    /// 1200 or 9600.
    pub speed: u32,
    /// Signal time of the frame's end, s.
    pub time: f64,
}

/// Settings of the signal side.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RxConfig {
    /// 12.5 kHz channel filter.
    pub narrow: bool,
    /// Emphasis: 0 none, 1 de-emphasis, 2 pre-emphasis.
    pub emphasis: u8,
    /// The input is FM-demodulated audio.
    pub audio_input: bool,
    /// The channel's offset in the input, Hz.
    pub offset_hz: f64,
}

/// The emphasis filter in front of the AFSK modem.
enum Emphasis {
    None,
    De(Deemphasis),
    Pre(PreEmphasis),
}

impl Emphasis {
    fn run(&mut self, x: &mut [f32]) {
        match self {
            Emphasis::None => {}
            Emphasis::De(f) => f.process(x),
            Emphasis::Pre(f) => f.process(x),
        }
    }
}

/// The carrier detector over 20 ms pieces of the channel: on when the power is 6 times the floor or more.
struct Carrier {
    pow: f64,
    dc: f64,
    n: usize,
    piece: usize,
    floor: Option<f64>,
}

/// The FM front end at 48 kHz.
pub struct FrontEnd {
    nco: Nco,
    res: Resampler<Complex32>,
    wide: Fir<Complex32>,
    narrow1200: Option<Fir<Complex32>>,
    disc_wide: Discriminator,
    disc_1200: Discriminator,
    mixed: Vec<Complex32>,
    ch: Vec<Complex32>,
    ch2: Vec<Complex32>,
    /// Wide discriminator, Hz.
    pub disc: Vec<f32>,
    /// Discriminator of the narrower 1200 bit/s channel, Hz.
    pub disc1200: Vec<f32>,
    carrier_det: Carrier,
    /// A carrier is present now.
    pub carrier: bool,
    /// Signal time the carrier came up.
    pub carrier_since: f64,
    /// Last time a carrier was present.
    pub carrier_last: f64,
    /// Smoothed SNR, dB. `None` until a carrier was heard.
    pub snr_db: Option<f64>,
    /// Smoothed carrier offset, Hz.
    pub offset_hz: f64,
    /// Mean power of the whole input over the last report, dBFS. `None` until a report period had samples.
    pub level_db: Option<f64>,
    lev_acc: f64,
    lev_n: u64,
    audio_res: Option<Resampler<f32>>,
    reals: Vec<f32>,
    clip: f32,
    t: f64,
}

/// The lowest power that the carrier detector takes as its noise floor.
const FLOOR_MIN: f64 = 1e-20;

impl FrontEnd {
    /// A front end for input at `rate`.
    ///
    /// # Errors
    ///
    /// The error of a filter or a resampler that the rate does not suit.
    pub fn new(rate: f64, cfg: &RxConfig) -> Result<Self, ConfigError> {
        let (pass, stop) = if cfg.narrow {
            (6.5e3, 10.5e3)
        } else {
            (9e3, 17e3)
        };
        let clip = if cfg.narrow { 4e3 } else { 12e3 } + 8e3;
        let res = Resampler::new(rate, AUDIO_RATE)?;
        let audio_res = if cfg.audio_input {
            Some(Resampler::new(rate, AUDIO_RATE)?)
        } else {
            None
        };
        // A discriminator that outputs Hz: unit output at a deviation of 1 Hz.
        let disc = Discriminator::new(AUDIO_RATE, 1.0)?;
        let narrow1200 = if cfg.narrow {
            None
        } else {
            let taps = lowpass(AUDIO_RATE, 4.5e3, 8.1e3, 50.0)?;
            Some(Fir::new(&taps))
        };
        Ok(FrontEnd {
            nco: Nco::new(rate, -cfg.offset_hz),
            res,
            wide: Fir::new(&lowpass(AUDIO_RATE, pass, stop, 50.0)?),
            narrow1200,
            disc_wide: disc.clone(),
            disc_1200: disc,
            mixed: Vec::new(),
            ch: Vec::new(),
            ch2: Vec::new(),
            disc: Vec::new(),
            disc1200: Vec::new(),
            carrier_det: Carrier {
                pow: 0.0,
                dc: 0.0,
                n: 0,
                piece: (0.02 * AUDIO_RATE) as usize,
                floor: None,
            },
            carrier: false,
            carrier_since: 0.0,
            carrier_last: -1e9,
            snr_db: None,
            offset_hz: 0.0,
            level_db: None,
            lev_acc: 0.0,
            lev_n: 0,
            audio_res,
            reals: Vec::new(),
            clip,
            t: 0.0,
        })
    }

    /// Closes a report period and returns the level of the input, dBFS; `None` before any samples, and for a period
    /// of digital silence, whose level has no value in dB.
    pub fn take_level(&mut self) -> Option<f64> {
        if self.lev_n > 0 {
            self.level_db = (self.lev_acc > 0.0).then(|| 10.0 * (self.lev_acc / self.lev_n as f64).log10());
        }
        self.lev_acc = 0.0;
        self.lev_n = 0;
        self.level_db
    }

    fn carrier_piece(&mut self, p: f64, dc: f64) {
        // A floor of zero (digital silence) would make every later piece a carrier of infinite SNR.
        let floor = self.carrier_det.floor.get_or_insert(p).max(FLOOR_MIN);
        let on = p > 6.0 * floor;
        if on {
            if !self.carrier {
                self.carrier_since = self.t;
            }
            self.carrier_last = self.t;
            let snr = 10.0 * (p / floor - 1.0).max(1e-3).log10();
            self.snr_db = Some(self.snr_db.map_or(snr, |s| s + 0.3 * (snr - s)));
            self.offset_hz += 0.3 * (dc - self.offset_hz);
            // The floor rises slowly even under a carrier.
            self.carrier_det.floor = Some(floor * 1.0005);
        } else {
            self.carrier_det.floor = Some(if p < floor {
                floor + 0.3 * (p - floor)
            } else {
                floor * 1.02
            });
        }
        self.carrier = on;
    }

    /// Runs input samples. The discriminator outputs are in `disc` and `disc1200` (cleared first).
    pub fn process(&mut self, iq: &[Complex32]) {
        self.disc.clear();
        self.disc1200.clear();
        for s in iq {
            self.lev_acc += s.norm_sqr() as f64;
        }
        self.lev_n += iq.len() as u64;
        if let Some(r) = &mut self.audio_res {
            self.reals.clear();
            self.reals.extend(iq.iter().map(|s| s.re * 4000.0));
            r.process(&self.reals, &mut self.disc);
            self.disc1200.extend_from_slice(&self.disc);
            self.t += self.disc.len() as f64 / AUDIO_RATE;
            self.carrier = true;
            self.carrier_last = self.t;
            return;
        }
        self.mixed.resize(iq.len(), Complex32::default());
        self.nco.mix(iq, &mut self.mixed);
        self.ch.clear();
        self.res.process(&self.mixed, &mut self.ch);
        self.ch2.clear();
        if self.narrow1200.is_some() {
            self.ch2.extend_from_slice(&self.ch);
        }
        self.wide.process_in_place(&mut self.ch);
        self.disc_wide.process(&self.ch, &mut self.disc);
        let ch = std::mem::take(&mut self.ch);
        for (i, x) in ch.iter().enumerate() {
            let d = self.disc[i].clamp(-self.clip, self.clip);
            self.disc[i] = d;
            let c = &mut self.carrier_det;
            c.pow += x.norm_sqr() as f64;
            c.dc += d as f64;
            c.n += 1;
            self.t += 1.0 / AUDIO_RATE;
            if c.n >= c.piece {
                let (p, dc) = (c.pow / c.n as f64, c.dc / c.n as f64);
                c.pow = 0.0;
                c.dc = 0.0;
                c.n = 0;
                self.carrier_piece(p, dc);
            }
        }
        self.ch = ch;
        if let Some(f) = &mut self.narrow1200 {
            f.process_in_place(&mut self.ch2);
            self.disc_1200.process(&self.ch2, &mut self.disc1200);
            for d in &mut self.disc1200 {
                *d = d.clamp(-self.clip, self.clip);
            }
        } else {
            self.disc1200.extend_from_slice(&self.disc);
        }
    }

    /// Signal time, s.
    pub fn time(&self) -> f64 {
        self.t
    }
}

/// The signal side of the APRS receiver.
pub struct AprsRx {
    /// The front end.
    pub front: FrontEnd,
    afsk: Afsk1200,
    g3ruh: G3ruh9600,
    emphasis: Emphasis,
    em_buf: Vec<f32>,
    recent: Dedup<u64>,
    found: Vec<(Vec<u8>, u64, u32)>,
    /// Good frames per speed.
    pub good: [u64; 2],
    /// Failed transmissions: frames whose FCS was wrong, one per transmission.
    pub bad: u64,
    /// Which speeds run.
    pub speeds: Speeds,
}

impl AprsRx {
    /// A receiver for input at `rate`.
    ///
    /// # Errors
    ///
    /// The error of the front end or a modem that the rate does not suit.
    pub fn new(rate: f64, cfg: RxConfig) -> Result<Self, ConfigError> {
        let emphasis = match cfg.emphasis {
            1 => Emphasis::De(Deemphasis::new(EMPHASIS_TAU_S, AUDIO_RATE)),
            2 => Emphasis::Pre(PreEmphasis::bilinear(EMPHASIS_TAU_S, 8000.0, AUDIO_RATE)?),
            _ => Emphasis::None,
        };
        Ok(AprsRx {
            front: FrontEnd::new(rate, &cfg)?,
            afsk: Afsk1200::new(AUDIO_RATE)?,
            g3ruh: G3ruh9600::new(AUDIO_RATE)?,
            emphasis,
            em_buf: Vec::new(),
            recent: Dedup::new(0.03, 64),
            found: Vec::new(),
            good: [0; 2],
            bad: 0,
            speeds: Speeds::Both,
        })
    }

    /// The discriminator audio of the last call (Hz at 48 kHz), for listening.
    pub fn audio(&self) -> &[f32] {
        &self.front.disc
    }

    /// Feeds input samples. Frames go to `out`.
    pub fn process(&mut self, iq: &[Complex32], out: &mut Vec<RawFrame>) {
        self.front.process(iq);
        let t_end = self.front.time();
        let t0 = t_end - self.front.disc.len() as f64 / AUDIO_RATE;
        let found = &mut self.found;
        found.clear();
        if self.speeds != Speeds::Only9600 {
            self.em_buf.clear();
            self.em_buf.extend_from_slice(&self.front.disc1200);
            self.emphasis.run(&mut self.em_buf);
            let bad = &mut self.bad;
            self.afsk.process_frames(&self.em_buf, |f| {
                if f.fcs_ok {
                    found.push((f.bytes.to_vec(), f.at, 1200));
                } else {
                    *bad += 1;
                }
            });
        }
        if self.speeds != Speeds::Only1200 {
            let bad = &mut self.bad;
            self.g3ruh.process_frames(&self.front.disc, |f| {
                if f.fcs_ok {
                    found.push((f.bytes.to_vec(), f.at, 9600));
                } else {
                    *bad += 1;
                }
            });
        }
        for (bytes, at, speed) in self.found.drain(..) {
            let time = (at as f64 / AUDIO_RATE).clamp(t0.min(t_end), t_end);
            if self.recent.seen(fingerprint(&bytes), time) {
                continue;
            }
            self.good[(speed == 9600) as usize] += 1;
            out.push(RawFrame { bytes, speed, time });
        }
    }
}
