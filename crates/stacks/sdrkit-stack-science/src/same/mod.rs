//! NOAA Weather Radio SAME (`same`): the weather channel cut out by the graph's channel component, its FM audio (for
//! listening), a 4.5 kHz channel centred on the carrier and its FM audio (for the attention tone), the AFSK burst reader on
//! that narrow channel, the 1050 Hz attention tone detector, and the alerts that group the three repeats, vote and parse them.
//! Standard: 47 CFR 11.31

pub mod parse;

use crate::family::{lock_state_of, lowpass, made, status, wall, REPORT_S};
use parse::{Counties, Country, EventKind, Message};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::goertzel::Goertzel;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_modem::follow::{CentredFir, SpectralConfig, SpectralFollow};
use sdrkit_algo_modem::fsk::ToneCorrelator;
use sdrkit_algo_sim::modulate::{Afsk, Fm};
use sdrkit_components::dsp::Channel;
use sdrkit_core::publish::keys::{Family, Urgency};
use crate::typed::{
    SameAlertEvent, SameAlertLocationRow, SameAlertRow, SameEndEvent, SameFields, SameToneRow,
    SAME_ALERT, SAME_ALERTS, SAME_ALERT_LOCATIONS, SAME_END, SAME_TONES,
};
use sdrkit_core::interface::{Fields, Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, Command, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    GenConfig, Generator, Hz, Receiver, Record, Results, ResultsDecl, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status,
    Value, Values,
};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, RateRule, RunContext, SpectrumSpec, StackGraph, TagKind, Timed,
};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use num_complex::Complex64;
use std::f64::consts::TAU;
use std::fmt::Write as _;

/// The bit rate (520 + 5/6 baud).
pub const BAUD: f64 = 520.0 + 5.0 / 6.0;
/// Mark (1).
pub const MARK_HZ: f64 = 2083.0 + 1.0 / 3.0;
/// Space (0).
pub const SPACE_HZ: f64 = 1562.5;
/// The audio rate of the decoder.
pub const AUDIO_RATE: f64 = 48_000.0;
/// The width of a weather channel.
pub const CHANNEL_WIDTH: f64 = 25e3;

sdrkit_core::choice_enum! {
    /// A weather radio channel.
    pub enum WeatherChannel {
        /// 162.400 MHz.
        Wx1 = ("wx1", "WX1"),
        /// 162.425 MHz.
        Wx2 = ("wx2", "WX2"),
        /// 162.450 MHz.
        Wx3 = ("wx3", "WX3"),
        /// 162.475 MHz.
        Wx4 = ("wx4", "WX4"),
        /// 162.500 MHz.
        Wx5 = ("wx5", "WX5"),
        /// 162.525 MHz.
        Wx6 = ("wx6", "WX6"),
        /// 162.550 MHz.
        Wx7 = ("wx7", "WX7"),
    }
}

impl WeatherChannel {
    /// The frequency of the channel.
    pub fn freq_hz(self) -> f64 {
        match self {
            WeatherChannel::Wx1 => 162.400e6,
            WeatherChannel::Wx2 => 162.425e6,
            WeatherChannel::Wx3 => 162.450e6,
            WeatherChannel::Wx4 => 162.475e6,
            WeatherChannel::Wx5 => 162.500e6,
            WeatherChannel::Wx6 => 162.525e6,
            WeatherChannel::Wx7 => 162.550e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// What a row of the alerts table is.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum MessageKind {
        /// A header that parses.
        Alert = ("alert", "Alert"),
        /// The end of a message ("NNNN").
        End = ("end", "End of message"),
        /// A header that does not parse.
        Damaged = ("damaged", "Damaged message"),
    }
}

sdrkit_core::choice_enum! {
    /// The test signal.
    pub enum Scene {
        /// A tornado warning and a weekly test.
        Two = ("two", "Two alerts"),
        /// A tornado warning.
        Tornado = ("tornado", "Tornado warning"),
        /// No message.
        Noise = ("noise", "No message"),
    }
}

/// One SAME data burst. A burst with no bytes marks a header just heard: the burst is on the air and follows when it ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// Signal time of the first byte, s.
    pub start: f64,
    /// Signal time of the end, s.
    pub end: f64,
    /// The bytes after the preamble.
    pub bytes: Vec<u8>,
}

impl Item for Burst {
    const KIND: &'static str = "same.burst";
}

/// One attention tone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tone {
    /// Signal time of the start, s.
    pub start: f64,
    /// Duration, s.
    pub duration: f64,
    /// Level, dB of the audio full scale.
    pub level_db: f64,
}

impl Item for Tone {
    const KIND: &'static str = "same.tone";
}

/// Taps of the audio low-pass of [`Demod`].
const AUDIO_TAPS: usize = 63;

/// The FM demodulator of a weather channel at [`AUDIO_RATE`]: the discriminator, the slow mean of its values (the carrier
/// error) and the audio in units of the deviation (2.4 kHz peak is 0.5), low-passed at 3.3 kHz.
struct Demod {
    disc: Discriminator,
    lp: Fir<f32>,
    dc: f64,
}

impl Demod {
    fn new() -> Self {
        Demod {
            disc: Discriminator::new(AUDIO_RATE, 1.0).expect("a positive rate"),
            lp: Fir::new(&lowpass(AUDIO_TAPS, 3_300.0 / AUDIO_RATE)),
            dc: 0.0,
        }
    }

    /// The audio sample of one channel sample. The audio is [`Demod::DELAY`] samples behind the channel.
    fn sample(&mut self, y: Complex32) -> f32 {
        let f = self.disc.process_sample(y) as f64;
        self.dc += 0.0002 * (f - self.dc);
        self.lp.process_sample(((f - self.dc) / 4800.0) as f32)
    }

    /// The delay of the audio behind the channel samples: the low-pass and the half sample of the discriminator.
    const DELAY: f64 = (AUDIO_TAPS - 1) as f64 / 2.0 + 0.5;

    /// The carrier error (Hz): the mean of the discriminator over about 0.1 s.
    fn carrier_hz(&self) -> f64 {
        self.dc
    }
}

/// The FM audio (`same.fm`): the weather channel at 48 kHz to real audio ([`Demod`]). The narrow form (`same.fm_narrow`)
/// first passes the channel through [`NarrowChannel`] and also gives the narrow channel itself (`iq`).
pub struct FmAudio {
    demod: Demod,
    narrow: Option<NarrowChannel>,
    buf: Vec<Complex32>,
}

impl Default for FmAudio {
    fn default() -> Self {
        FmAudio {
            demod: Demod::new(),
            narrow: None,
            buf: Vec::new(),
        }
    }
}

impl FmAudio {
    /// The FM audio of the narrow channel, for the tone demodulators.
    pub fn narrow() -> Self {
        FmAudio {
            narrow: Some(NarrowChannel::new()),
            ..Self::default()
        }
    }
}

/// The width of the channel the narrow channel's follow-up searches, wider than a weather channel so that the quietest quarter
/// of its spectrum at [`AUDIO_RATE`] is noise and the follow-up's floor is the noise floor.
pub const SEARCH_WIDTH: f64 = 36e3;
/// Pass band of the narrow channel (Hz each side of the carrier).
pub const NARROW_PASS_HZ: f64 = 4500.0;
/// Stop band edge of the narrow channel (Hz each side of the carrier).
pub const NARROW_STOP_HZ: f64 = 8100.0;
/// Taps each side of the centre of the narrow channel filter: a Blackman window's transition of about 5.5 x 48 kHz / 73 taps
/// spans the pass band edge to the stop band edge.
const NARROW_HALF_TAPS: usize = 36;

/// The narrow channel of the tone demodulators at [`AUDIO_RATE`]: a complex FIR low-pass of +-[`NARROW_PASS_HZ`] pass and
/// +-[`NARROW_STOP_HZ`] stop centred on the carrier that an open-loop spectral follow-up estimates over the weather channel, fed
/// a channel of [`SEARCH_WIDTH`]. Until the follow-up first settles the filter has the weather channel's width at its centre.
pub struct NarrowChannel {
    follow: SpectralFollow,
    fir: CentredFir,
}

impl NarrowChannel {
    fn new() -> Self {
        let cfg = SpectralConfig {
            search_hz: 0.45 * CHANNEL_WIDTH,
            ..SpectralConfig::open_loop(AUDIO_RATE)
        };
        let half = 0.5 * (NARROW_PASS_HZ + NARROW_STOP_HZ);
        NarrowChannel {
            follow: SpectralFollow::new(cfg).expect("a valid follow-up at 48 kHz"),
            fir: CentredFir::new(AUDIO_RATE, half, NARROW_HALF_TAPS, 0.5 * CHANNEL_WIDTH)
                .expect("widths below half of 48 kHz"),
        }
    }

    /// The centre of the narrow filter in use (Hz), `None` until the follow-up first settles.
    pub fn centre_hz(&self) -> Option<f64> {
        self.fir.centre_hz()
    }

    /// Filters `buf` in place, after the follow-up has looked at it.
    fn process(&mut self, buf: &mut [Complex32]) {
        self.follow.process(buf);
        self.fir.set_estimate(self.follow.correction_hz(), self.follow.settled());
        self.fir.process(buf);
    }
}

impl Component for FmAudio {
    fn info(&self) -> ComponentInfo {
        let mut outputs = vec![Port::real("audio").same_format_as(0)];
        let name = if self.narrow.is_some() {
            outputs.push(Port::iq("iq").same_format_as(0));
            "same.fm_narrow"
        } else {
            "same.fm"
        };
        ComponentInfo::processor(
            name,
            vec![Port::iq("in").rate(RateRule::Exact(AUDIO_RATE))],
            outputs,
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.demod = Demod::new();
        let mut out = vec![Format {
            bandwidth_hz: 3300.0,
            ..inputs[0].with_kind(Kind::Real)
        }];
        if self.narrow.is_some() {
            self.narrow = Some(NarrowChannel::new());
            out.push(Format {
                bandwidth_hz: NARROW_PASS_HZ + NARROW_STOP_HZ,
                ..inputs[0].clone()
            });
        }
        Ok(out)
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.buf.clear();
        self.buf.extend_from_slice(x);
        if let Some(n) = &mut self.narrow {
            n.process(&mut self.buf);
            io.iq_out(1).extend_from_slice(&self.buf);
        }
        let out = io.real_out(0);
        for &y in &self.buf {
            out.push(self.demod.sample(y));
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// The header of an alert.
const HEADER_ALERT: &[u8; 4] = b"ZCZC";
/// The header of an end of message.
const HEADER_END: &[u8; 4] = b"NNNN";
/// Bits of the pattern the reader searches: the preamble of 16 bytes 0xAB and the header.
const KNOWN_BITS: usize = 160;
/// The pattern of a header, one value a bit in the order sent (least significant bit of each byte first): 1 mark, -1 space.
fn known_pattern(header: &[u8; 4]) -> [f32; KNOWN_BITS] {
    let mut p = [0.0; KNOWN_BITS];
    for (k, v) in p.iter_mut().enumerate() {
        let byte = if k < 128 { 0xAB } else { header[(k - 128) / 8] };
        *v = if (byte >> (k % 8)) & 1 == 1 { 1.0 } else { -1.0 };
    }
    p
}

/// The least score of the pattern for a header: `sum(pattern x soft) / sum(|soft|)` over the 160 bits. Noise alone and the
/// rest of a transmission stay under 0.55; a header at -2 dB carrier to noise in 25 kHz reaches 0.77.
const HEADER_SCORE: f32 = 0.68;
/// The least mean soft magnitude of a header: tones with a deviation of about 300 Hz over the whole pattern. It keeps out
/// silence, and a pattern that lies on the first bits of a preamble only.
const HEADER_LEVEL: f32 = 3.0;
/// Bits a candidate header waits for one with a larger sum. The pattern a whole number of bytes early or late also scores
/// high (the preamble repeats every byte); the header in place has the largest sum.
const HEADER_HOLD_BITS: f64 = 40.0;
/// Samples the reader keeps of the channel, of the soft values and of the tone sums: the pattern, the hold and a bit.
const RING: usize = 1 << 15;
/// A burst ends when the mean of the audio soft magnitudes over about 16 bits falls under this share of the header's.
const END_LEVEL: f32 = 0.4;
/// The most bytes of a burst (the header and the longest message with room to spare).
const MAX_BURST_BYTES: usize = 300;

/// One tone of a burst as the header shows it: the audio is `dev sin(2 pi f t + phase)` Hz with `t` from the start of a bit.
#[derive(Clone, Copy, Debug, Default)]
struct ToneFit {
    f: f64,
    dev: f64,
    phase: f64,
}

/// The mean of values over their deviation.
#[derive(Clone, Copy, Debug, Default)]
struct Margin {
    n: f64,
    sum: f64,
    squares: f64,
}

impl Margin {
    fn add(&mut self, v: f32) {
        self.n += 1.0;
        self.sum += f64::from(v);
        self.squares += f64::from(v) * f64::from(v);
    }

    fn value(&self) -> f64 {
        let mean = self.sum / self.n.max(1.0);
        mean / (self.squares / self.n.max(1.0) - mean * mean).max(1e-30).sqrt()
    }
}

/// A candidate header: the sum of the soft values with the signs of the pattern, the sample its last bit ends at, which
/// header, and the mean of the soft magnitudes.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    sum: f32,
    at: u64,
    header: usize,
    level: f32,
}

/// A burst that is read.
#[derive(Clone, Debug)]
struct Reading {
    /// The sample at which the last header bit ends.
    at: u64,
    /// The next bit after the header.
    k: u32,
    tones: [ToneFit; 2],
    carrier: f64,
    /// Whether the bits are decided on the channel samples (else on the audio).
    matched: bool,
    level: f32,
    mean: f32,
    byte: u8,
    bits: u8,
    bad_run: u8,
    bytes: Vec<u8>,
}

/// The AFSK demodulator and burst reader (`same.afsk`: the narrow channel to `same.burst`).
///
/// The audio of the channel ([`Demod`]) gives for every sample a soft value, the mark amplitude minus the space amplitude over
/// the last bit. A header is found where the soft values of 160 bits match the preamble and "ZCZC" or "NNNN" best
/// ([`HEADER_SCORE`], [`HEADER_HOLD_BITS`]); this also gives the bit clock, which is then held (no tracking: the clocks may
/// differ by about 200 ppm over the longest message). The known bits give the deviation and the phase of each tone and the
/// carrier error. A bit of the message is then decided on the channel samples: the larger of their correlations with the two
/// frequency-modulated waveforms of a bit, which holds far below the threshold of the discriminator. The waveform assumes whole
/// tone cycles in a bit (4 of mark, 3 of space) with a phase that stays from bit to bit; when the audio soft values read the
/// known bits with a larger margin than the waveforms do, the audio decides the bits of that burst.
///
/// A burst ends when the tones fade ([`END_LEVEL`]), after two characters in a row that are not printable, or at
/// [`MAX_BURST_BYTES`]. The seven low bits of each byte are kept (the eighth is a null bit, either value).
pub struct AfskReader {
    n: usize,
    sps: f64,
    demod: Demod,
    tones: Option<ToneCorrelator>,
    patterns: [[f32; KNOWN_BITS]; 2],
    /// The samples from the end of the pattern back to the end of each bit of it.
    lags: Vec<usize>,
    iq: Vec<Complex32>,
    soft: Vec<f32>,
    sums: Vec<[Complex32; 2]>,
    count: u64,
    candidate: Option<Candidate>,
    quiet_until: u64,
    reading: Option<Reading>,
}

impl Default for AfskReader {
    fn default() -> Self {
        AfskReader {
            n: 92,
            sps: AUDIO_RATE / BAUD,
            demod: Demod::new(),
            tones: None,
            patterns: [known_pattern(HEADER_ALERT), known_pattern(HEADER_END)],
            lags: Vec::new(),
            iq: vec![Complex32::default(); RING],
            soft: vec![0.0; RING],
            sums: vec![[Complex32::default(); 2]; RING],
            count: 0,
            candidate: None,
            quiet_until: 0,
            reading: None,
        }
    }
}

impl AfskReader {
    /// The signal time of sample `at`, s.
    fn time(at: f64) -> f64 {
        at / AUDIO_RATE
    }

    /// The best header whose pattern ends at sample `t`, when it reaches [`HEADER_SCORE`].
    fn score(&self, t: u64) -> Option<Candidate> {
        let (mut alert, mut end, mut level) = (0f32, 0f32, 0f32);
        for (k, &lag) in self.lags.iter().enumerate() {
            let v = self.soft[(t as usize).wrapping_sub(lag) % RING];
            alert += self.patterns[0][k] * v;
            end += self.patterns[1][k] * v;
            level += v.abs();
        }
        let (sum, header) = if alert >= end { (alert, 0) } else { (end, 1) };
        (sum >= HEADER_SCORE * level && level >= HEADER_LEVEL * KNOWN_BITS as f32).then_some(Candidate {
            sum,
            at: t,
            header,
            level: level / KNOWN_BITS as f32,
        })
    }

    /// The soft value of a bit whose audio window ends at sample `end`, on the channel samples: the amplitude of their
    /// correlation with the mark waveform minus that with the space waveform.
    fn matched_soft(&self, r: &Reading, end: f64) -> f32 {
        let start = end - (self.n - 1) as f64 - Demod::DELAY;
        let first = start.round();
        let mut sums = [Complex64::default(); 2];
        for m in 0..self.n {
            let v = self.iq[(first as usize + m) % RING];
            let v = Complex64::new(f64::from(v.re), f64::from(v.im));
            let t = (first + m as f64 - start) / AUDIO_RATE;
            for (sum, tone) in sums.iter_mut().zip(&r.tones) {
                let turn = TAU * tone.f * t + tone.phase;
                let phase = TAU * r.carrier * t - tone.dev / tone.f * (turn.cos() - tone.phase.cos());
                *sum += v * Complex64::from_polar(1.0, -phase);
            }
        }
        (sums[0].norm() - sums[1].norm()) as f32
    }

    /// Starts to read the burst of the header `c`: fits the tones on the known bits and chooses what decides the bits.
    fn start(&mut self, c: Candidate, out: &mut Vec<Timed<Burst>>) {
        self.end_burst(c.at as f64, out);
        let header = if c.header == 0 { HEADER_ALERT } else { HEADER_END };
        let pattern = &self.patterns[c.header];
        let mut fit = [Complex64::default(); 2];
        let mut bits = [0usize; 2];
        for (k, &lag) in self.lags.iter().enumerate() {
            let tone = usize::from(pattern[k] < 0.0);
            let f = [MARK_HZ, SPACE_HZ][tone];
            // The exact start of the bit, against which the phase of the tone is counted.
            let start = c.at as f64 - (KNOWN_BITS - 1 - k) as f64 * self.sps - (self.n - 1) as f64;
            let s = self.sums[(c.at as usize).wrapping_sub(lag) % RING][tone];
            fit[tone] += Complex64::new(f64::from(s.re), f64::from(s.im))
                * Complex64::from_polar(1.0, TAU * f * start / AUDIO_RATE);
            bits[tone] += 1;
        }
        let tones = [0, 1].map(|i| {
            // A sum over a bit of `a sin(x + p)` against `e^(-jx)` is `-j a e^(jp) n / 2`.
            let p = fit[i] * Complex64::new(0.0, 2.0 / (self.n * bits[i].max(1)) as f64);
            ToneFit {
                f: [MARK_HZ, SPACE_HZ][i],
                dev: p.norm() * 4800.0,
                phase: p.arg(),
            }
        });
        let mut r = Reading {
            at: c.at,
            k: 0,
            tones,
            carrier: self.demod.carrier_hz(),
            matched: true,
            level: c.level,
            mean: c.level,
            byte: 0,
            bits: 0,
            bad_run: 0,
            bytes: header.to_vec(),
        };
        // What reads the known bits with the larger margin (the mean over the deviation of the soft values, each with the
        // sign of its bit) decides the bits of the burst.
        let mut audio = Margin::default();
        let mut matched = Margin::default();
        for (k, (p, &lag)) in pattern.iter().zip(&self.lags).enumerate() {
            audio.add(p * self.soft[(c.at as usize).wrapping_sub(lag) % RING]);
            matched.add(p * self.matched_soft(&r, c.at as f64 - (KNOWN_BITS - 1 - k) as f64 * self.sps));
        }
        r.matched = matched.value() >= audio.value();
        let t = Self::time(c.at as f64);
        out.push(Timed {
            time: t,
            value: Burst {
                start: t,
                end: t,
                bytes: Vec::new(),
            },
        });
        self.quiet_until = c.at + (KNOWN_BITS as f64 * self.sps) as u64;
        self.reading = Some(r);
    }

    /// Ends the burst that is read, at sample `at`.
    fn end_burst(&mut self, at: f64, out: &mut Vec<Timed<Burst>>) {
        if let Some(r) = self.reading.take() {
            let start = Self::time(r.at as f64);
            out.push(Timed {
                time: start,
                value: Burst {
                    start,
                    end: Self::time(at),
                    bytes: r.bytes,
                },
            });
        }
    }

    /// Reads the bits of the burst whose windows have ended by sample `t`.
    fn read(&mut self, t: u64, out: &mut Vec<Timed<Burst>>) {
        while let Some(mut r) = self.reading.take() {
            let end = r.at as f64 + f64::from(r.k + 1) * self.sps;
            if end.round() > t as f64 {
                self.reading = Some(r);
                return;
            }
            let audio = self.soft[end.round() as usize % RING];
            let soft = if r.matched { self.matched_soft(&r, end) } else { audio };
            r.k += 1;
            r.mean += (audio.abs() - r.mean) / 16.0;
            r.byte = (r.byte >> 1) | (u8::from(soft > 0.0) << 7);
            r.bits += 1;
            let mut done = r.mean < END_LEVEL * r.level;
            if r.bits == 8 {
                r.bits = 0;
                let c = r.byte & 0x7F;
                r.bad_run = if parse::valid_char(c) { 0 } else { r.bad_run + 1 };
                done |= r.bad_run >= 2;
                if !done {
                    r.bytes.push(c);
                }
                done |= r.bytes.len() >= MAX_BURST_BYTES;
            }
            self.reading = Some(r);
            if done {
                self.end_burst(end, out);
            }
        }
    }
}

impl Component for AfskReader {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "same.afsk",
            vec![Port::iq("in").rate(RateRule::Exact(AUDIO_RATE))],
            vec![Port::typed::<Burst>("bursts").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = AfskReader::default();
        self.n = self.sps.round() as usize;
        self.tones = Some(ToneCorrelator::new(
            &[MARK_HZ / AUDIO_RATE, SPACE_HZ / AUDIO_RATE],
            self.n,
        )?);
        self.lags = (0..KNOWN_BITS)
            .map(|k| ((KNOWN_BITS - 1 - k) as f64 * self.sps).round() as usize)
            .collect();
        Ok(vec![Format::new(Kind::typed::<Burst>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(mut tones) = self.tones.take() else {
            return;
        };
        let x = io.iq(0);
        let out = io.items_out::<Burst>(0);
        let hold = (HEADER_HOLD_BITS * self.sps) as u64;
        for &y in x {
            let t = self.count;
            let audio = self.demod.sample(y);
            tones.push(Complex32::new(audio, 0.0));
            let s = tones.sums();
            let at = t as usize % RING;
            self.iq[at] = y;
            self.sums[at] = [s[0], s[1]];
            self.soft[at] = s[0].norm() - s[1].norm();
            if t >= self.quiet_until && t as usize >= self.lags[0] + self.n {
                if let Some(c) = self.score(t) {
                    if self.candidate.is_none_or(|b| c.sum > b.sum) {
                        self.candidate = Some(c);
                    }
                }
            }
            if let Some(c) = self.candidate.filter(|c| t - c.at >= hold) {
                self.candidate = None;
                self.start(c, out);
            }
            self.read(t, out);
            self.count += 1;
        }
        self.tones = Some(tones);
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let t = self.count as f64;
        self.end_burst(t, io.items_out::<Burst>(0));
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// The attention-tone detector (`same.tone`: real audio to `same.tone`): 1050 Hz Goertzel over 50 ms blocks. A tone is a run of
/// blocks where 1050 Hz holds most of the audio power.
pub struct ToneDetector {
    g: Option<Goertzel>,
    block: usize,
    k: usize,
    e: f64,
    run_start: Option<f64>,
    run_level: f64,
    blocks: u32,
    fs: f64,
    /// A tone is on now.
    pub on: bool,
}

impl Default for ToneDetector {
    fn default() -> Self {
        ToneDetector {
            g: None,
            block: 2400,
            k: 0,
            e: 0.0,
            run_start: None,
            run_level: 0.0,
            blocks: 0,
            fs: AUDIO_RATE,
            on: false,
        }
    }
}

impl Component for ToneDetector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "same.tone",
            vec![Port::real("audio")],
            vec![Port::typed::<Tone>("tones").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.block = (0.05 * self.fs) as usize;
        self.g = Some(Goertzel::with_rate(self.fs, 1050.0, self.block)?);
        self.k = 0;
        self.e = 0.0;
        self.run_start = None;
        Ok(vec![Format::new(Kind::typed::<Tone>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let Some(g) = self.g.as_mut() else { return };
        let x = io.real(0);
        let out = io.items_out::<Tone>(0);
        for (i, &v) in x.iter().enumerate() {
            self.e += (v * v) as f64;
            self.k += 1;
            if let Some(a) = g.push(v) {
                // The mean square of a tone is half its squared amplitude.
                let p = a.norm_sqr() as f64 / 2.0;
                let total = self.e / self.k as f64;
                self.e = 0.0;
                self.k = 0;
                let t = (first + i as u64 + 1) as f64 / self.fs;
                let is_tone = total > 1e-4 && p > 0.6 * total;
                if is_tone {
                    if self.run_start.is_none() {
                        self.run_start = Some(t - 0.05);
                        self.run_level = 0.0;
                        self.blocks = 0;
                    }
                    self.run_level = self.run_level.max(p);
                    self.blocks += 1;
                    self.on = self.blocks >= 4;
                } else if let Some(s) = self.run_start.take() {
                    self.on = false;
                    let d = t - 0.05 - s;
                    if d >= 0.5 {
                        out.push(Timed {
                            time: s,
                            value: Tone {
                                start: s,
                                duration: d,
                                level_db: power_to_db(self.run_level),
                            },
                        });
                    }
                }
            }
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// One row of the alerts table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    /// The row key of the alert in the alerts table, from the count of rows ever added.
    #[serde(default)]
    pub seq: u64,
    /// The host time when decoded, when the samples are live.
    pub host_time: Option<f64>,
    /// Signal time of the first burst.
    pub signal_time: f64,
    /// What the row is.
    pub kind: MessageKind,
    /// The parsed message.
    pub message: Message,
    /// Repeats heard.
    pub bursts: usize,
    /// Repeats identical to the result.
    pub agreeing: usize,
    /// Bits the vote changed.
    pub bits_changed: usize,
    /// SNR, dB.
    pub snr_db: f64,
    /// Carrier error, Hz.
    pub carrier_hz: f64,
    /// The attention tone that followed, s.
    pub tone_s: Option<f64>,
    /// The raw text as sent.
    pub raw: String,
    /// The year of the issue time.
    #[serde(default)]
    pub year: i64,
}

impl Shift for Alert {
    fn shift_times(&mut self, by_s: f64) {
        self.signal_time += by_s;
    }
}

impl Shift for Tone {
    fn shift_times(&mut self, by_s: f64) {
        self.start += by_s;
    }
}

/// Alerts kept.
pub const MAX_ALERTS: usize = 100;
/// Tones kept.
pub const MAX_TONES: usize = 40;
/// The most locations of one message (47 CFR 11.31 (c)).
pub const MAX_LOCATIONS: usize = 31;

/// What `same` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("same").results_decl()
}

/// The urgency of an alert: emergency for a warning or an emergency, urgent for a watch, a notice for a statement or a message,
/// none for a test.
pub fn urgency(kind: Option<EventKind>) -> Urgency {
    match kind {
        Some(EventKind::Warning | EventKind::Emergency) => Urgency::Emergency,
        Some(EventKind::Watch) => Urgency::Urgent,
        Some(EventKind::Statement | EventKind::Message) => Urgency::Notice,
        Some(EventKind::Test) | None => Urgency::None,
    }
}

impl Shaped for Alert {
    type Row = SameAlertRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = &self.message;
        fn text(s: &str) -> Option<&str> {
            (!s.is_empty()).then_some(s)
        }
        out.put(&SameAlertRow {
            signal_time_s: self.signal_time,
            host_time: self.host_time.map(Time),
            kind: self.kind,
            event: text(&m.event),
            event_name: m.event_name.as_deref(),
            event_kind: m.event_kind,
            originator: text(&m.originator),
            originator_name: m.originator_name.as_deref(),
            locations: m.locations.len() as i64,
            valid_for: (m.purge_min > 0).then_some(Seconds(f64::from(m.purge_min) * 60.0)),
            issued: (m.valid && !m.eom)
                .then(|| parse::issue_epoch(self.year, m.issue_day, m.issue_hour, m.issue_min))
                .flatten()
                .map(|u| Time(u as f64)),
            callsign: text(&m.callsign),
            repeats: self.bursts as i64,
            agreeing: self.agreeing as i64,
            bits_changed: self.bits_changed as i64,
            snr_db: self.snr_db,
            carrier: Hertz(self.carrier_hz),
            tone: self.tone_s.map(Seconds),
            raw: &self.raw,
            error: m.error,
        });
    }
}

impl Shaped for Tone {
    type Row = SameToneRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SameToneRow {
            signal_time_s: self.start,
            duration: Seconds(self.duration),
            level_db: self.level_db,
        });
    }
}

/// What the alerts component keeps: the messages and the tones.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The messages, oldest first.
    pub alerts: LogRing<Alert>,
    /// The tones, oldest first.
    pub tones: LogRing<Tone>,
    /// The messages ever added. It names the next message's row, as the alerts log counts its entries.
    #[serde(default)]
    added: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            alerts: LogRing::new(MAX_ALERTS),
            tones: LogRing::new(MAX_TONES),
            added: 0,
        }
    }
}

impl Store {
    fn push(&mut self, mut a: Alert) {
        a.seq = self.added;
        self.added += 1;
        self.alerts.push(a);
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.alerts.shift_times(by_s);
        self.tones.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("science.same_alerts", 2);
    fn clear(&mut self) {
        self.alerts.clear();
        self.tones.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.alerts.adopt(saved.alerts);
        self.tones.adopt(saved.tones);
        self.added = saved.added;
    }
}

/// The weather channel.
pub const CHANNEL_SETTING: Setting<field::Choice<WeatherChannel>> =
    Setting::choice("channel", "Weather channel", WeatherChannel::Wx7);
/// The table the state and province names come from.
pub const COUNTRY_SETTING: Setting<field::Choice<Country>> =
    Setting::choice("country", "Location names", Country::Us);
/// The text of a county table.
pub const COUNTY_TEXT: Setting<field::Text> = Setting::new("county_text", "County table", "", "")
    .help("The text of a county table: lines SSCCC,Name or SS,CCC,Name or the Census form. The host reads the file.");
/// The year of the issue times, 0 for the year of the host clock.
pub const YEAR: Setting<field::I64> =
    Setting::new("year", "Year for issue times", "", 0).range(0.0, 3000.0, 1.0);
/// Play the channel audio.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false);
/// The test signal.
pub const GEN_SCENE: Setting<field::Choice<Scene>> =
    Setting::choice("gen_scene", "Test signal: scene", Scene::Two)
        .section("Test signal")
        .advanced();
/// Play the test signal again and again.
pub const GEN_LOOP: Setting<field::Bool> = Setting::new("gen_loop", "Test signal: loop", "", true)
    .section("Test signal")
    .advanced();

/// Forgets the loaded county table.
pub const CLEAR_COUNTIES: Command = Command::new("clear_counties", "Clear the county table")
    .help("Forgets the loaded county table.");

sdrkit_core::settings! {
    /// The settings of the SAME stack.
    pub enum SameSetting {
        /// The weather channel.
        Channel = CHANNEL_SETTING,
        /// The table the state and province names come from.
        Country = COUNTRY_SETTING,
        /// The text of a county table.
        CountyText = COUNTY_TEXT,
        /// The year of the issue times.
        Year = YEAR,
        /// Play the channel audio.
        Listen = LISTEN,
        /// The test signal.
        GenScene = GEN_SCENE,
        /// Play the test signal again and again.
        GenLoop = GEN_LOOP,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Alerts`].
    pub enum AlertsSetting {
        /// The weather channel.
        Channel = CHANNEL_SETTING,
        /// The table the state and province names come from.
        Country = COUNTRY_SETTING,
        /// The text of a county table.
        CountyText = COUNTY_TEXT,
        /// The year of the issue times.
        Year = YEAR,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Alerts`].
    pub enum AlertsCommand {
        /// Forgets every message and tone.
        Clear = objcmd::CLEAR,
        /// Forgets the loaded county table.
        ClearCounties = CLEAR_COUNTIES,
    }
}

/// The spectrum of the radio input that the alerts measure the channel in: 1024 bins, no window, so the sum of the bins is
/// the mean power.
pub const MEASURE: SpectrumSpec = SpectrumSpec {
    size: sdrkit_graph::SizeRule::Bins(SpectrumSize::S1024),
    window: SpectrumWindow::Rectangular,
    average: 1,
    frames: sdrkit_graph::FrameRule::PerSecond(25.0),
    scale: sdrkit_graph::components::SpectrumScale::Db,
};

/// The alerts component (`same.alerts`): groups repeats, votes, parses, keeps the tables and measures the channel in the
/// spectrum of the radio input.
pub struct Alerts {
    group: Vec<Burst>,
    texts: Vec<String>,
    last_end: f64,
    longest: f64,
    on_air: bool,
    t: f64,
    store: Store,
    good: u64,
    bad: u64,
    country: Country,
    counties: Counties,
    county_text: String,
    last_activity: f64,
    last_message: f64,
    tone_on: bool,
    published: Published,
    status: Status,
    places_dirty: bool,
    events: Vec<sdrkit_core::Event>,
    channel: WeatherChannel,
    tuned_hz: f64,
    signal_offset_hz: f64,
    span_hz: f64,
    year: i64,
    fs: f64,
    snr: f64,
    carrier: f64,
    level: f64,
    live: bool,
    clock: Clock,
    tone_check: Option<Goertzel>,
    power: Vec<f32>,
    scratch: Vec<f32>,
    key: String,
    alert_key: String,
}

impl Default for Alerts {
    fn default() -> Self {
        Alerts {
            group: Vec::new(),
            texts: Vec::new(),
            last_end: f64::NEG_INFINITY,
            longest: 0.0,
            on_air: false,
            t: 0.0,
            store: Store::default(),
            good: 0,
            bad: 0,
            country: COUNTRY_SETTING.default,
            counties: Counties::default(),
            county_text: String::new(),
            last_activity: f64::NEG_INFINITY,
            last_message: f64::NEG_INFINITY,
            tone_on: false,
            published: Published::new(SameFields::SHAPE.fields).with_tables(&[
                SAME_ALERTS.def(),
                SAME_ALERT_LOCATIONS.def(),
                SAME_TONES.def(),
            ]),
            status: Status::default(),
            places_dirty: true,
            events: Vec::new(),
            channel: CHANNEL_SETTING.default,
            tuned_hz: 0.0,
            signal_offset_hz: 0.0,
            span_hz: 0.0,
            year: YEAR.default,
            fs: AUDIO_RATE,
            snr: 0.0,
            carrier: 0.0,
            level: -120.0,
            live: false,
            clock: Clock::fixed(0.0),
            tone_check: None,
            power: Vec::new(),
            scratch: Vec::new(),
            key: String::new(),
            alert_key: String::new(),
        }
    }
}

impl Alerts {
    /// What the component keeps.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The frequency the receiver is tuned to, else the frequency of the channel setting.
    fn freq_hz(&self) -> f64 {
        if self.tuned_hz > 0.0 {
            self.tuned_hz
        } else {
            self.channel.freq_hz()
        }
    }

    /// The year of the issue times: the setting, else the year of the host clock.
    fn year(&self) -> i64 {
        if self.year > 0 {
            self.year
        } else {
            crate::calendar::civil(self.clock.now() as i64).year
        }
    }

    /// The channel SNR and carrier error from one spectrum frame (dB, lowest frequency first): the channel bins over the
    /// median floor, and their power-weighted centroid (the mean instantaneous frequency of the FM signal).
    fn measure(&mut self, frame: &[f32], span_hz: f64) {
        let n = frame.len();
        if n == 0 || span_hz <= 0.0 {
            return;
        }
        self.power.clear();
        self.power
            .extend(frame.iter().map(|&db| 10f32.powf(db / 10.0)));
        let df = span_hz / n as f64;
        let centre = self.signal_offset_hz;
        let off = (centre / df).round() as i64;
        let half = ((CHANNEL_WIDTH / 2.0 / df) as i64).max(1);
        let tot: f64 = self.power.iter().map(|&v| v as f64).sum::<f64>();
        let floor = percentile(&self.power, 0.5, &mut self.scratch) as f64;
        let (mut chan, mut nb) = (0.0f64, 0.0f64);
        let mut cen = (0.0f64, 0.0f64);
        for k in -half..=half {
            let i = (n as i64 / 2 + off + k).rem_euclid(n as i64) as usize;
            let p = self.power[i] as f64;
            chan += p;
            nb += 1.0;
            let w = (p - floor).max(0.0);
            cen.0 += w * k as f64 * df;
            cen.1 += w;
        }
        self.level = power_to_db(tot);
        let snr = power_to_db(((chan - floor * nb) / (floor * nb)).max(1e-6));
        self.snr += 0.25 * (snr - self.snr);
        if cen.1 > 0.0 && snr > 6.0 {
            let c = cen.0 / cen.1 + (centre - off as f64 * df);
            self.carrier += 0.15 * (c - self.carrier);
        }
    }

    /// The longest silence within a group of repeats: the 1 s pause after a repeat, or, when a repeat was not heard, two pauses
    /// and a repeat as long as the longest of the group, each pause with 0.1 s to spare.
    fn group_gap(&self) -> f64 {
        2.2 + self.longest
    }

    fn finish_group(&mut self) {
        if self.group.is_empty() {
            return;
        }
        self.longest = 0.0;
        let year = self.year();
        self.texts.clear();
        self.texts
            .extend(self.group.iter().map(|b| parse::trim(&b.bytes)));
        let texts = &self.texts;
        let mut chosen = vote(texts).map(|v| {
            let m = parse::parse(&v.text, year, self.country, &self.counties);
            (v, m)
        });
        if chosen.as_ref().is_none_or(|c| !c.1.valid) {
            // A single repeat that is whole by itself.
            for t in texts {
                let m = parse::parse(t, year, self.country, &self.counties);
                if m.valid {
                    let agreeing = texts.iter().filter(|x| *x == t).count();
                    chosen = Some((
                        Vote {
                            text: t.clone(),
                            heard: texts.len(),
                            agreeing,
                            bits_changed: 0,
                        },
                        m,
                    ));
                    break;
                }
            }
        }
        let (v, mut m) = match chosen {
            Some(c) => c,
            None => {
                let t = texts
                    .iter()
                    .find(|t| !t.is_empty())
                    .cloned()
                    .unwrap_or_else(|| String::from_utf8_lossy(&self.group[0].bytes).into_owned());
                let m = parse::parse(&t, year, self.country, &self.counties);
                (
                    Vote {
                        text: t,
                        heard: texts.len(),
                        agreeing: 0,
                        bits_changed: 0,
                    },
                    m,
                )
            }
        };
        // A header that parses but that the repeats do not confirm at every place may hold a wrong character. An end of message
        // is its header alone, which the reader matched as a whole.
        m.valid &= m.eom || confirmed(&v.text, texts);
        let kind = if !m.valid {
            MessageKind::Damaged
        } else if m.eom {
            MessageKind::End
        } else {
            MessageKind::Alert
        };
        if m.valid {
            self.good += 1;
        } else {
            self.bad += 1;
        }
        self.last_message = self.t;
        let a = Alert {
            seq: 0,
            host_time: wall(self.live, &self.clock),
            signal_time: self.group[0].start,
            kind,
            raw: m.raw.clone(),
            message: m,
            bursts: v.heard,
            agreeing: v.agreeing,
            bits_changed: v.bits_changed,
            snr_db: self.snr,
            carrier_hz: self.carrier,
            tone_s: None,
            year,
        };
        self.group.clear();
        if a.message.valid {
            let e = self.event_of(&a);
            self.events.push(e);
        }
        if a.kind == MessageKind::Alert {
            let m = &a.message;
            let name = m.event_name.as_deref().unwrap_or(&m.event);
            self.published.results_mut().log(format!(
                "SAME: {name} ({}), {} locations",
                m.event,
                m.locations.len()
            ));
        }
        self.store.push(a);
        self.places_dirty = true;
    }

    /// The event of a message that parses.
    fn event_of(&self, a: &Alert) -> sdrkit_core::Event {
        let m = &a.message;
        if m.eom {
            return SAME_END.event(
                a.signal_time,
                self.freq_hz(),
                &SameEndEvent {
                    text: "end of message",
                    family: Family::Alert,
                    raw: &a.raw,
                },
            );
        }
        let mut text = String::from(m.event_name.as_deref().unwrap_or(&m.event));
        for (i, l) in m.locations.iter().enumerate() {
            text.push_str(if i == 0 { ": " } else { "; " });
            text.push_str(&l.text);
        }
        let first_place = m.locations.iter().find_map(|l| l.marker);
        SAME_ALERT.event(
            a.signal_time,
            self.freq_hz(),
            &SameAlertEvent {
                source: &m.callsign,
                text: &text,
                family: Family::Alert,
                urgency: urgency(m.event_kind),
                position: first_place,
                event: &m.event,
                event_name: m.event_name.as_deref(),
                event_kind: m.event_kind,
                originator: &m.originator,
                valid_for: Seconds(f64::from(m.purge_min) * 60.0),
                callsign: &m.callsign,
                locations: m.locations.len() as i64,
                raw: &a.raw,
            },
        )
    }

    fn tone(&mut self, t: &Tone) {
        // A tone that follows a header within about 20 s belongs to the newest alert.
        if let Some(a) = self
            .store
            .alerts
            .find_recent_mut(MAX_ALERTS, |a| a.kind == MessageKind::Alert)
            .filter(|a| t.start - a.signal_time < 25.0 && t.start > a.signal_time)
        {
            if a.tone_s.is_none() {
                a.tone_s = Some(t.duration);
            }
        }
        self.store.tones.push(t.clone());
    }

    fn state(&self) -> u8 {
        if self.t - self.last_message < 300.0 && self.good > 0 {
            2
        } else if self.t - self.last_activity < 30.0 {
            1
        } else {
            0
        }
    }

    /// Writes the locations table and the map points again. It runs only after the messages changed.
    fn publish_places(&mut self) {
        let p = &mut self.published;
        p.clear_rows(&SAME_ALERT_LOCATIONS);
        p.results_mut().clear_points();
        for a in self
            .store
            .alerts
            .iter()
            .filter(|a| a.kind == MessageKind::Alert)
        {
            for (n, l) in a.message.locations.iter().enumerate() {
                self.alert_key.clear();
                let _ = write!(self.alert_key, "{:06}", a.seq);
                self.key.clear();
                let _ = write!(self.key, "{}/{n}", self.alert_key);
                SAME_ALERT_LOCATIONS.put(
                    p,
                    &self.key,
                    &SameAlertLocationRow {
                        alert: &self.alert_key,
                        code: &l.code,
                        place: &l.text,
                        position: l.marker,
                    },
                );
                if let Some((lat, lon)) = l.marker {
                    let label = format!("{}: {}", a.message.event, l.text);
                    p.results_mut()
                        .update_point(&self.key, lat, lon, &label, None);
                }
            }
        }
        self.places_dirty = false;
    }

    fn publish(&mut self) {
        if self.places_dirty {
            self.publish_places();
        }
        let state = self.state();
        let (good, tone_on) = (self.good, self.tone_on);
        let newest = self
            .store
            .alerts
            .newest_first()
            .find(|a| a.kind == MessageKind::Alert)
            .filter(|_| state == 2);
        status(
            &mut self.status,
            lock_state_of(state),
            Some(self.snr as f32),
            Some(self.level as f32),
            Some(self.carrier),
            |s| {
                let _ = if tone_on {
                    write!(s, "1050 Hz alert tone")
                } else if let Some(a) = newest {
                    let m = &a.message;
                    write!(
                        s,
                        "{} ({}), {} locations, {good} messages",
                        m.event_name.as_deref().unwrap_or(&m.event),
                        m.event,
                        m.locations.len()
                    )
                } else {
                    write!(s, "listening for SAME alerts, {good} messages")
                };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.status);
        p.put(&SameFields {
            channel: self.channel,
            frequency: Hertz(self.channel.freq_hz()),
            messages_ok: self.good as i64,
            messages_bad: self.bad as i64,
            tone_now: self.tone_on,
            country: self.country,
            counties: self.counties.names.len() as i64,
            snr_db: self.snr,
            level_dbfs: self.level,
            carrier: Hertz(self.carrier),
        });
        log_rows(&self.store.alerts, &SAME_ALERTS, self.t, p);
        log_rows(&self.store.tones, &SAME_TONES, self.t, p);
    }

    /// Names the places of every message again, with the county table and country in use.
    fn reparse(&mut self) {
        let country = self.country;
        for a in self.store.alerts.iter_mut() {
            if a.kind != MessageKind::End && !a.raw.is_empty() {
                a.message = parse::parse(&a.raw, a.year, country, &self.counties);
            }
        }
        self.places_dirty = true;
    }
}

impl Component for Alerts {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "same.alerts",
            vec![
                Port::real("audio"),
                Port::typed::<Burst>("bursts"),
                Port::typed::<Tone>("tones"),
                Port::spectrum("spectrum", MEASURE),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(AlertsSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fs = inputs[0].rate;
        self.tuned_hz = inputs[3].tuned_hz;
        self.signal_offset_hz = inputs[3].signal_offset_hz();
        self.span_hz = inputs[3].bandwidth_hz;
        self.tone_check = Some(Goertzel::with_rate(self.fs, 1050.0, 1200)?);
        self.group.clear();
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let n = io.input_len(0);
        let end_t = (first + n as u64) as f64 / self.fs;
        let bursts = io.items::<Burst>(1);
        let tones = io.items::<Tone>(2);
        let spectrum = io.input(3);
        for t in spectrum.tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                self.tuned_hz = tuned_hz;
                self.signal_offset_hz = tuned_hz - centre_hz;
                self.carrier = 0.0;
            }
        }
        let frames = io.spectrum(3);
        let span = self.span_hz;
        for i in 0..frames.len() {
            self.measure(frames.frame(i), span);
        }
        // Bursts (grouped) and tones, in time order.
        let mut bi = 0;
        let mut ti = 0;
        loop {
            let tb = bursts.get(bi).map(|b| b.time);
            let tt = tones.get(ti).map(|t| t.time);
            match (tb, tt) {
                (Some(a), b) if b.is_none_or(|b| a <= b) => {
                    let b = &bursts[bi].value;
                    bi += 1;
                    self.on_air = b.bytes.is_empty();
                    if self.on_air {
                        continue;
                    }
                    let kind = |x: &[u8]| parse::trim(x).starts_with("NNNN");
                    let other = self
                        .group
                        .first()
                        .is_some_and(|g| kind(&g.bytes) != kind(&b.bytes));
                    if b.start - self.last_end > self.group_gap() || other {
                        self.t = b.start;
                        self.finish_group();
                    }
                    self.last_end = b.end;
                    // The length of a repeat: the preamble and the text, without what the reader took after its end.
                    let text = parse::trim(&b.bytes).len();
                    self.longest = self.longest.max((16 + text) as f64 * 8.0 / BAUD);
                    self.last_activity = b.end;
                    self.group.push(b.clone());
                }
                (_, Some(_)) => {
                    let t = &tones[ti].value;
                    self.last_activity = t.start + t.duration;
                    self.t = t.start;
                    // The attention signal follows the last repeat of a header.
                    self.finish_group();
                    self.tone(t);
                    ti += 1;
                }
                _ => break,
            }
        }
        self.t = end_t;
        // A burst is delivered at its end: the group stays open while one is on the air.
        if !self.group.is_empty() && !self.on_air && end_t - self.last_end > self.group_gap() {
            self.finish_group();
        }
        let x = io.real(0);
        // A tone on now: the 1050 Hz share of the last 25 ms of this pass.
        if let (true, Some(g)) = (x.len() > 1200, self.tone_check.as_mut()) {
            g.reset();
            let mut e = 0.0;
            let mut p = None;
            for &v in &x[x.len() - 1200..] {
                e += (v * v) as f64;
                p = g.push(v).map(|a| a.norm_sqr() as f64 / 2.0).or(p);
            }
            self.tone_on = p.is_some_and(|p| p > 0.6 * e / 1200.0 && e / 1200.0 > 1e-4);
            if self.tone_on {
                self.last_activity = end_t;
            }
        }
        if io.tick() || !self.events.is_empty() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        self.finish_group();
        self.publish();
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AlertsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AlertsSetting::Channel => self.channel = CHANNEL_SETTING.read(id, v)?,
            AlertsSetting::Country => {
                self.country = COUNTRY_SETTING.read(id, v)?;
                self.reparse();
            }
            AlertsSetting::CountyText => {
                let text = COUNTY_TEXT.read(id, v)?;
                if !text.is_empty() {
                    self.counties = Counties::load(text)
                        .map_err(|_| SettingError::Malformed { id, at: None })?;
                    self.reparse();
                }
                text.clone_into(&mut self.county_text);
            }
            AlertsSetting::Year => self.year = YEAR.read(id, v)?,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (AlertsSetting::Channel, Value::Choice(self.channel.index())),
            (AlertsSetting::Country, Value::Choice(self.country.index())),
            (
                AlertsSetting::CountyText,
                Value::Text(self.county_text.clone()),
            ),
            (AlertsSetting::Year, Value::I64(self.year)),
        ])
    }
    fn commands(&self) -> &Commands {
        AlertsCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match AlertsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AlertsCommand::Clear => {
                self.store.clear();
                self.good = 0;
                self.bad = 0;
                self.places_dirty = true;
            }
            AlertsCommand::ClearCounties => {
                self.counties = Counties::default();
                self.county_text.clear();
                self.reparse();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.t).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.t)?;
        self.places_dirty = true;
        self.publish();
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of [`Listen`].
    pub enum ListenSetting {
        /// Play the channel audio.
        Listen = LISTEN,
    }
}

/// "Listen" (`same.listen`: real audio to mono audio at the same rate): the channel audio when on, nothing when off.
pub struct Listen {
    on: bool,
}

impl Default for Listen {
    fn default() -> Self {
        Listen { on: LISTEN.default }
    }
}

impl Component for Listen {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "same.listen",
            vec![Port::real("audio")],
            vec![Port::audio("out").same_format_as(0)],
        )
        .with_settings(ListenSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Audio)])
    }
    fn process(&mut self, io: &mut Io) {
        if !self.on {
            return;
        }
        let x = io.real(0);
        io.audio_out(0).samples.extend_from_slice(x);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ListenSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ListenSetting::Listen => self.on = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(ListenSetting::Listen, Value::Bool(self.on))])
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 64e3;

/// The receiver: the graph's channel component and the FM audio of the weather channel for the listen path; a wider channel, cut
/// down to the narrow channel, for the AFSK reader, and its FM audio for the tone detector and the alerts.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let chan = made("chan.normal", Channel::new(CHANNEL_WIDTH, AUDIO_RATE))?;
    let chan_search = made("chan.normal", Channel::new(SEARCH_WIDTH, AUDIO_RATE))?;
    let graph = GraphBuilder::new()
        .add("chan", chan)
        .add("chan_search", chan_search)
        .add("fm", FmAudio::default())
        .add("fm_narrow", FmAudio::narrow())
        .add("afsk", AfskReader::default())
        .add("tone", ToneDetector::default())
        .add("alerts", Alerts::default())
        .add("listen", Listen::default())
        .input("chan")
        .input("chan_search")
        .input("alerts.spectrum")
        .connect("chan", "fm")
        .connect("chan_search", "fm_narrow")
        .connect("fm_narrow.iq", "afsk")
        .connect("fm_narrow.audio", "tone")
        .connect("fm_narrow.audio", "alerts.audio")
        .connect("afsk", "alerts.bursts")
        .connect("tone", "alerts.tones")
        .connect("fm.audio", "listen")
        .output("alerts.events")
        .output("listen")
        .clock(REPORT_S)
        .status_from("alerts")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("same"), graph)
        .min_rate(MIN_RATE, "SAME needs a sample rate of at least 64 ksps")
        .route(LISTEN.key(), &["listen.listen"])?
        .route(CHANNEL_SETTING.key(), &["alerts.channel"])?
        .route(COUNTRY_SETTING.key(), &["alerts.country"])?
        .route(COUNTY_TEXT.key(), &["alerts.county_text"])?
        .route(YEAR.key(), &["alerts.year"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("alerts", "clear"),
        )
        .command(
            CLEAR_COUNTIES.key,
            sdrkit_graph::CommandTarget::of("alerts", "clear_counties"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The result of voting a group of repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct Vote {
    /// The text chosen.
    pub text: String,
    /// Bursts in the group.
    pub heard: usize,
    /// Bursts identical to the result.
    pub agreeing: usize,
    /// Bits the vote changed against some burst.
    pub bits_changed: usize,
}

/// Votes the trimmed texts of a group bit by bit, position by position, up to the second-longest length (every position is
/// heard twice or more): each position among the texts that reach it, so a repeat cut short still votes on its start; where
/// exactly two reach it, a differing bit takes the value that makes a character the format allows. `None` with fewer than two
/// texts, an empty result, or two texts of unequal length.
pub fn vote(texts: &[String]) -> Option<Vote> {
    let mut lens: Vec<usize> = texts.iter().map(String::len).collect();
    lens.sort_unstable_by(|a, b| b.cmp(a));
    let len = *lens.get(1)?;
    if len == 0 || (texts.len() == 2 && lens[0] != lens[1]) {
        return None;
    }
    let all: Vec<&[u8]> = texts.iter().map(|t| t.as_bytes()).collect();
    let mut out = vec![0u8; len];
    let mut changed = 0;
    let mut group: Vec<u8> = Vec::with_capacity(texts.len());
    for (i, o) in out.iter_mut().enumerate() {
        group.clear();
        group.extend(all.iter().filter_map(|t| t.get(i)));
        let mut byte = 0u8;
        for bit in 0..8 {
            let ones = group.iter().filter(|&&g| (g >> bit) & 1 == 1).count();
            let one = if group.len() == 2 && ones == 1 {
                // Take the bit that makes an allowed character.
                let alt = if format_char(group[0]) { group[0] } else { group[1] };
                (alt >> bit) & 1 == 1
            } else {
                2 * ones > group.len()
            };
            if one {
                byte |= 1 << bit;
            }
            changed += group
                .iter()
                .filter(|&&g| ((g >> bit) & 1 == 1) != one)
                .count()
                .min(1);
        }
        *o = byte;
    }
    let text = String::from_utf8_lossy(&out).to_string();
    let agreeing = all.iter().filter(|g| **g == out.as_slice()).count();
    Some(Vote {
        text,
        heard: texts.len(),
        agreeing,
        bits_changed: changed,
    })
}

/// Whether the repeats confirm every character of `text`: two or more of `texts` hold it at its place.
pub fn confirmed(text: &str, texts: &[String]) -> bool {
    text.bytes()
        .enumerate()
        .all(|(i, c)| texts.iter().filter(|t| t.as_bytes().get(i) == Some(&c)).count() >= 2)
}

fn format_char(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'+' || c == b'-' || c == b'/' || c == b' '
}

/// One message to send.
#[derive(Clone, Debug)]
pub struct GenMessage {
    /// The text.
    pub text: String,
    /// Repeats (default 3).
    pub repeats: usize,
    /// Attention tone after the header, s (0 none).
    pub tone_s: f64,
    /// Send "NNNN" three times after.
    pub eom: bool,
    /// Flip this many bits in the text of the repeat with this index.
    pub flips: Option<(usize, usize)>,
    /// Leave out the repeat with this index.
    pub drop: Option<usize>,
}

impl GenMessage {
    /// A message with three repeats, no tone, end of message on.
    pub fn new(text: &str) -> GenMessage {
        GenMessage {
            text: text.into(),
            repeats: 3,
            tone_s: 0.0,
            eom: true,
            flips: None,
            drop: None,
        }
    }
}

/// The tornado warning of the demo scene.
pub const TORNADO: &str = "ZCZC-WXR-TOR-048201-348113-029047+0030-1301530-KHGX/NWS-";
/// The weekly test of the demo scene.
pub const WEEKLY: &str = "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/NWS-";

/// The SAME generator: one weather channel at 0 Hz with amplitude 1. The audio (the AFSK bursts and the attention tone) is made
/// once at 48 kHz, held at the generator's rate and frequency-modulated with a peak deviation of 2.4 kHz (3.5 kHz for the tone).
pub struct SameGenerator {
    rate: f64,
    audio: Vec<f32>,
    pos: u64,
    fm: Fm,
    held: Vec<f32>,
    looped: bool,
}

impl SameGenerator {
    /// The messages after a lead of 0.6 s, with a second of silence after each repeat. `seed` decides which bits a message's
    /// `flips` change.
    pub fn new(rate: f64, msgs: &[GenMessage], seed: u64, looped: bool) -> SameGenerator {
        let fa = AUDIO_RATE;
        let mut a: Vec<f32> = Vec::new();
        let silence =
            |a: &mut Vec<f32>, s: f64| a.extend(std::iter::repeat_n(0.0, (s * fa) as usize));
        let mut afsk = Afsk::new(fa, BAUD, MARK_HZ, SPACE_HZ).expect("the SAME tones fit 48 kHz");
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed ^ 0x5A5E);
        let mut bits = Vec::new();
        let mut burst = |a: &mut Vec<f32>, bytes: &[u8], bits: &mut Vec<u8>| {
            bits.clear();
            for &b in bytes {
                // Each byte is sent least significant bit first (47 CFR 11.31 (a)(1)).
                bits.extend((0..8).map(|k| (b >> k) & 1));
            }
            afsk.push(&bits[..], a);
            afsk.flush(a);
        };
        silence(&mut a, 0.6);
        for m in msgs {
            for r in 0..m.repeats {
                if m.drop == Some(r) {
                    silence(
                        &mut a,
                        16.0 * 8.0 / BAUD + m.text.len() as f64 * 8.0 / BAUD + 1.0,
                    );
                    continue;
                }
                let mut bytes = vec![0xABu8; 16];
                let mut text = m.text.clone().into_bytes();
                if let Some((which, n)) = m.flips {
                    if which == r {
                        for _ in 0..n {
                            let i = rng.below(text.len() as u64) as usize;
                            let bit = rng.below(7) as u8;
                            text[i] ^= 1 << bit;
                        }
                    }
                }
                bytes.extend(text);
                burst(&mut a, &bytes, &mut bits);
                silence(&mut a, 1.0);
            }
            if m.tone_s > 0.0 {
                let n = (m.tone_s * fa) as usize;
                let k = 3.5 / 2.4;
                a.extend((0..n).map(|i| (k * (TAU * 1050.0 * (i + 1) as f64 / fa).sin()) as f32));
                silence(&mut a, 1.0);
            }
            if m.eom {
                for _ in 0..3 {
                    let mut bytes = vec![0xABu8; 16];
                    bytes.extend_from_slice(b"NNNN");
                    burst(&mut a, &bytes, &mut bits);
                    silence(&mut a, 1.0);
                }
            }
        }
        silence(&mut a, 1.0);
        SameGenerator {
            rate,
            audio: a,
            pos: 0,
            fm: Fm::new(rate, 2400.0).expect("a positive rate"),
            held: Vec::new(),
            looped,
        }
    }

    /// From the stack's settings: the demo scenes.
    pub fn from_config(cfg: &GenConfig) -> SameGenerator {
        let s = &cfg.settings;
        let msgs = match s.get(&GEN_SCENE) {
            Scene::Noise => vec![],
            Scene::Tornado => vec![GenMessage {
                tone_s: 1.5,
                ..GenMessage::new(TORNADO)
            }],
            Scene::Two => vec![
                GenMessage {
                    tone_s: 1.5,
                    ..GenMessage::new(TORNADO)
                },
                GenMessage::new(WEEKLY),
            ],
        };
        SameGenerator::new(cfg.rate_hz, &msgs, cfg.seed, s.get(&GEN_LOOP))
    }

    /// The length of one pass, s.
    pub fn duration(&self) -> f64 {
        self.audio.len() as f64 / AUDIO_RATE
    }
}

impl Generator for SameGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = AUDIO_RATE / self.rate;
        self.held.clear();
        for _ in 0..out.len() {
            let mut i = (self.pos as f64 * step) as usize;
            if i >= self.audio.len() {
                if !self.looped || self.audio.is_empty() {
                    if self.audio.is_empty() {
                        self.held.push(0.0);
                        self.pos += 1;
                        continue;
                    }
                    break;
                }
                self.pos = 0;
                i = 0;
            }
            self.held.push(self.audio[i]);
            self.pos += 1;
        }
        let n = self.held.len();
        // `out[..n]` holds one sample for each held audio sample, so the lengths agree.
        let _ = self.fm.process(&self.held, &mut out[..n]);
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of a weather channel is its carrier to noise in 25 kHz.
        Some(SignalInfo {
            symbol_rate_hz: Some(BAUD),
            bit_rate_hz: Some(BAUD),
            ..SignalInfo::new(25e3)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator. The SAME, time-signal and e-Czas generators must never be routed to a transmitting radio.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(SameGenerator::from_config(cfg)))
}

/// The verify case: the two alerts of the demo scene.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two alerts", o, 0.0);
    c.seconds = o.seconds.max(30.0);
    c.expect_audio = None;
    c.check = Some(|r| {
        let n = crate::family::field(r, SameFields::MESSAGES_OK.key);
        vec![CheckLine::new(
            SameFields::MESSAGES_OK.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn votes() {
        let good = TORNADO.to_string();
        let mut a = good.clone().into_bytes();
        a[10] ^= 1;
        let mut b = good.clone().into_bytes();
        b[20] ^= 2;
        let mut c = good.clone().into_bytes();
        c[30] ^= 4;
        let s = |v: Vec<u8>| String::from_utf8_lossy(&v).to_string();
        let v = vote(&[s(a.clone()), s(b.clone()), s(c)]).unwrap();
        assert_eq!(v.text, good);
        assert_eq!(v.bits_changed, 3);
        assert_eq!(v.agreeing, 0);
        let v = vote(&[good.clone(), s(a.clone()), good.clone()]).unwrap();
        assert_eq!((v.text.as_str(), v.agreeing), (good.as_str(), 2));
        let v = vote(&[good.clone(), "ZCZC-SHORT".into(), good.clone()]).unwrap();
        assert_eq!(v.agreeing, 2);
        assert!(vote(&[good.clone(), "ZCZC-SHORT".into()]).is_none());
        let v = vote(&[good.clone(), s(a)]).unwrap();
        assert_eq!(v.text, good);
        assert!(vote(&[good]).is_none());
    }
}
