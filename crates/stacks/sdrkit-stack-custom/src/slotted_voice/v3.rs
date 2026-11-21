//! Frame version 3: one voice stream in 160-symbol frames of 16-QAM at 8000 symbols a second (48 kHz, 6 samples a symbol), with
//! the sweep signature and the beacon slice added; and its receiver. The 16000 symbols a second profile is the same stream read
//! as 96 kHz, so both use this code at a notional 48 kHz.

use super::beacon::{self, Beacon, BeaconReceiver, Heard};
use super::phy::{self, N};
use super::sweep::{self, IdReader, Scanner, PERIOD};
use crate::family::fit::{line_gain, phase_slope};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::interp::{Cubic, Reader};
use sdrkit_algo_fec::ldpc::LdpcDecoder;
use sdrkit_codec_custom::voice3::{self, V3Rate, Variant};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// Samples a symbol.
pub const SPS: usize = 6;
/// Symbols a frame.
pub const FRAME: usize = 160;
/// Samples a frame.
pub const FRAME_SAMPLES: usize = SPS * FRAME;
/// Data symbols a frame.
pub const DATA: usize = 120;
/// Coded bits a frame.
pub const FRAME_BITS: usize = 480;
/// Frames in an interleaver block.
pub const BLOCK: usize = 4;
/// The notional sample rate.
pub const FS: f64 = 48_000.0;
/// Symbols a second at the notional rate.
pub const SYMBOL_RATE: f64 = 8000.0;
/// The output gain.
pub const GAIN: f32 = 0.4;
/// The mean data power a sample, fixed: (32 + 120 + 8 × 1.8) / 160 / 6.
pub const P_DATA: f64 = 166.4 / 160.0 / 6.0;
/// Samples between two header-only beacon frames.
pub const BEACON_HEADER_EVERY: u64 = 96_000;
/// Samples between two full beacon frames.
pub const BEACON_FULL_EVERY: u64 = 480_000;

/// Preamble symbol `i` (0 to 31).
pub fn preamble(i: usize) -> Complex32 {
    let a = if (7 * i + 3) % 5 < 2 { 1.0 } else { -1.0 };
    let b = if !(11 * i + 1).is_multiple_of(3) { 1.0 } else { -1.0 };
    Complex32::new(a, b) * std::f32::consts::FRAC_1_SQRT_2
}

/// The pilot value.
pub fn pilot() -> Complex32 {
    Complex32::new(3.0, 3.0) / 10f32.sqrt()
}

/// True for the frame positions that hold a pilot (47, 63, ..., 159).
pub fn is_pilot(i: usize) -> bool {
    i >= 32 && (i - 32) % 16 == 15
}

/// The 160 symbols of a frame that carries the 480 coded bits `bits`.
pub fn frame(bits: &[u8]) -> Vec<Complex32> {
    let mut data = Vec::with_capacity(DATA);
    phy::map(phy::qam16(), bits, &mut data);
    let mut out = Vec::with_capacity(FRAME);
    out.extend((0..32).map(preamble));
    let mut d = data.into_iter();
    for i in 32..FRAME {
        out.push(if is_pilot(i) { pilot() } else { d.next().unwrap_or_default() });
    }
    out
}

/// The information length of the words of a bit rate.
pub fn word_k(rate: V3Rate) -> usize {
    match rate {
        V3Rate::R12 | V3Rate::R24 => 240,
        V3Rate::R16 | V3Rate::R32 => 320,
    }
}

/// The rate-matched 480-bit words of one codec frame (`frame_bits` bits, MSB first).
pub fn words(frame_bits: &[u8], rate: V3Rate) -> Vec<Vec<u8>> {
    let k = word_k(rate);
    frame_bits
        .chunks(k)
        .map(|info| {
            let mut cw = [0u8; N];
            phy::encode(info, &mut cw);
            let mut w = info.to_vec();
            w.extend_from_slice(&cw[phy::K..phy::K + FRAME_BITS - k]);
            w
        })
        .collect()
}

/// The four frames (640 symbols) of an interleaver block of four words.
pub fn block(words: &[Vec<u8>]) -> Vec<Complex32> {
    let joined: Vec<u8> = words.iter().flatten().copied().collect();
    let p = phy::permutation(BLOCK * FRAME_BITS);
    let inter: Vec<u8> = p.iter().map(|&i| joined[i as usize]).collect();
    inter.chunks(FRAME_BITS).flat_map(frame).collect()
}

/// The sweep a modulator adds.
enum Sweep {
    /// The variants of the identity cycle, one a period.
    Cycle([u8; sweep::CYCLE]),
    /// The continuous up sweep of the early format: the running sum of its frequency, Hz.
    Continuous(f64),
}

/// The pulse-shaping, sweep and beacon stage: symbols in, 48 kHz samples out (times the output gain).
pub struct Modulator {
    h: Vec<f32>,
    hist: [Complex32; 9],
    n: u64,
    sweep: Sweep,
    amp: f32,
    beacon: Option<Beacon>,
    sending: Vec<Complex32>,
    sent: usize,
}

impl Modulator {
    /// A modulator whose sweep spells `callsign` and that sends `beacon` (none: no slice).
    pub fn new(callsign: &str, beacon: Option<Beacon>) -> Modulator {
        Modulator {
            h: phy::rrc(SPS, 8, 0.25),
            hist: [Complex32::new(0.0, 0.0); 9],
            n: 0,
            sweep: Sweep::Cycle(sweep::id_cycle(callsign)),
            amp: (P_DATA * 0.1).sqrt() as f32,
            beacon,
            sending: Vec::new(),
            sent: 0,
        }
    }

    /// The modulator of the early format (appendix C of the description): its continuous up sweep at the level of its own
    /// data power, and no beacon.
    pub fn early() -> Modulator {
        Modulator {
            sweep: Sweep::Continuous(0.0),
            amp: (210.0 / 202.0 / 6.0 * 0.1f64).sqrt() as f32,
            ..Modulator::new("", None)
        }
    }

    /// Samples made so far.
    pub fn samples(&self) -> u64 {
        self.n
    }

    /// Appends the six samples of each symbol of `syms`.
    pub fn push(&mut self, syms: &[Complex32], out: &mut Vec<Complex32>) {
        for &s in syms {
            self.hist.copy_within(0..8, 1);
            self.hist[0] = s;
            for q in 0..SPS {
                let mut y = Complex32::new(0.0, 0.0);
                for m in 0..9 {
                    if let Some(&t) = self.h.get(SPS * m + q) {
                        y += self.hist[m] * t;
                    }
                }
                out.push(self.extra(y) * GAIN);
            }
        }
    }

    /// Adds the sweep and the beacon of the next sample to `y`.
    fn extra(&mut self, mut y: Complex32) -> Complex32 {
        let n = self.n;
        match &mut self.sweep {
            Sweep::Cycle(cycle) => {
                let p = (n / PERIOD as u64) as usize;
                let m = (n % PERIOD as u64) as i64;
                y += sweep::template(cycle[p % sweep::CYCLE], m) * self.amp;
            }
            Sweep::Continuous(acc) => {
                // The frequency of sample n joins the sum before its phase is taken; the sweep restarts every 0.5 s.
                let ph = (n % 24_000) as f64 / FS;
                *acc += -5000.0 + 10_000.0 * ph / 0.5;
                let a = TAU * (*acc / FS).rem_euclid(1.0);
                y += Complex32::new(a.cos() as f32, a.sin() as f32) * self.amp;
            }
        }
        if self.sent >= self.sending.len() {
            if let Some(b) = &self.beacon {
                let kind = if n.is_multiple_of(BEACON_FULL_EVERY) {
                    Some(b.text.is_empty())
                } else if n.is_multiple_of(BEACON_HEADER_EVERY) {
                    Some(true)
                } else {
                    None
                };
                if let Some(header_only) = kind {
                    self.sending = beacon::frame_bits(b, header_only)
                        .map(|bits| beacon::slice(&bits, FS))
                        .unwrap_or_default();
                    self.sent = 0;
                }
            }
        }
        if let Some(&v) = self.sending.get(self.sent) {
            y += v * self.amp;
            self.sent += 1;
        }
        self.n += 1;
        y
    }
}

/// The version 3 transmitter: 16 kHz speech in, 48 kHz samples out.
pub struct Transmitter {
    rate: V3Rate,
    enc: voice3::Encoder,
    pcm: Vec<i16>,
    words: Vec<Vec<u8>>,
    words_sent: u64,
    modu: Modulator,
    frame: Vec<u8>,
}

impl Transmitter {
    /// A transmitter at `rate` whose sweep spells `callsign` and that sends `beacon`.
    pub fn new(rate: V3Rate, callsign: &str, beacon: Option<Beacon>) -> Transmitter {
        let v = Variant::V3(rate);
        Transmitter {
            rate,
            enc: voice3::Encoder::new(v),
            pcm: Vec::new(),
            words: Vec::new(),
            words_sent: 0,
            modu: Modulator::new(callsign, beacon),
            frame: vec![0; v.frame_bytes()],
        }
    }

    fn code_frame(&mut self, pcm: &[i16]) {
        let _ = self.enc.encode(pcm, &mut self.frame);
        let mut bits = Vec::with_capacity(self.frame.len() * 8);
        crate::family::push_bytes(&mut bits, &self.frame);
        self.words.extend(words(&bits, self.rate));
    }

    fn flush_blocks(&mut self, out: &mut Vec<Complex32>) {
        while self.words.len() >= BLOCK {
            let b: Vec<Vec<u8>> = self.words.drain(..BLOCK).collect();
            self.words_sent += BLOCK as u64;
            self.modu.push(&block(&b), out);
        }
    }

    /// Takes 16 kHz speech and appends the samples of every block it completes.
    pub fn push(&mut self, pcm: &[i16], out: &mut Vec<Complex32>) {
        self.pcm.extend_from_slice(pcm);
        let n = self.pcm.len() / voice3::FRAME_SAMPLES * voice3::FRAME_SAMPLES;
        let take: Vec<i16> = self.pcm.drain(..n).collect();
        for f in take.chunks(voice3::FRAME_SAMPLES) {
            self.code_frame(f);
        }
        self.flush_blocks(out);
    }

    /// Ends the stream: the last partial frame padded with zeros, one frame of silence, more silent frames until the words
    /// fill whole blocks (at most 8), then 8 zero symbols.
    pub fn finish(&mut self, out: &mut Vec<Complex32>) {
        let silence = [0i16; voice3::FRAME_SAMPLES];
        if !self.pcm.is_empty() {
            let mut last = std::mem::take(&mut self.pcm);
            last.resize(voice3::FRAME_SAMPLES, 0);
            self.code_frame(&last);
        }
        self.code_frame(&silence);
        let mut extra = 0;
        while !self.words.len().is_multiple_of(BLOCK) && extra < 8 {
            self.code_frame(&silence);
            extra += 1;
        }
        self.flush_blocks(out);
        self.modu.push(&[Complex32::new(0.0, 0.0); 8], out);
    }

    /// Samples made so far at 48 kHz.
    pub fn samples(&self) -> u64 {
        self.modu.samples()
    }
}

/// Symbols of a frame of the early format.
pub const EARLY_FRAME: usize = 202;

/// The 202 symbols of an early-format frame that carries the 640 coded bits `bits`: the preamble, ten groups of 15 data
/// symbols and a pilot, then 10 data symbols.
pub fn early_frame(bits: &[u8]) -> Vec<Complex32> {
    let mut data = Vec::with_capacity(160);
    phy::map(phy::qam16(), bits, &mut data);
    let mut out = Vec::with_capacity(EARLY_FRAME);
    out.extend((0..32).map(preamble));
    let mut d = data.into_iter();
    for g in 0..11 {
        out.extend(d.by_ref().take(if g < 10 { 15 } else { 10 }));
        if g < 10 {
            out.push(pilot());
        }
    }
    out
}

/// The transmitter of the early format (appendix C of the description; no receiver accepts it): voice codec 1 at 16 kbit/s,
/// one full (640, 320) code word a codec frame, blocks of four words interleaved over 2560 bits, frames of 202 symbols and the
/// continuous up sweep.
pub struct EarlyTransmitter {
    enc: voice3::Encoder,
    pcm: Vec<i16>,
    words: Vec<[u8; N]>,
    modu: Modulator,
    frame: Vec<u8>,
}

impl Default for EarlyTransmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl EarlyTransmitter {
    /// A transmitter at the start of its stream.
    pub fn new() -> EarlyTransmitter {
        EarlyTransmitter {
            enc: voice3::Encoder::new(Variant::V1),
            pcm: Vec::new(),
            words: Vec::new(),
            modu: Modulator::early(),
            frame: vec![0; Variant::V1.frame_bytes()],
        }
    }

    fn word(&mut self, info: &[u8]) {
        let mut cw = [0u8; N];
        phy::encode(info, &mut cw);
        self.words.push(cw);
    }

    fn code_frame(&mut self, pcm: &[i16]) {
        let _ = self.enc.encode(pcm, &mut self.frame);
        let mut bits = Vec::with_capacity(phy::K);
        crate::family::push_bytes(&mut bits, &self.frame);
        self.word(&bits);
    }

    fn flush_blocks(&mut self, out: &mut Vec<Complex32>) {
        let p = phy::permutation(BLOCK * N);
        while self.words.len() >= BLOCK {
            let joined: Vec<u8> = self.words.drain(..BLOCK).flatten().collect();
            let inter: Vec<u8> = p.iter().map(|&i| joined[i as usize]).collect();
            let syms: Vec<Complex32> = inter.chunks(N).flat_map(early_frame).collect();
            self.modu.push(&syms, out);
        }
    }

    /// Takes 16 kHz speech and appends the samples of every block it completes.
    pub fn push(&mut self, pcm: &[i16], out: &mut Vec<Complex32>) {
        self.pcm.extend_from_slice(pcm);
        let n = self.pcm.len() / voice3::FRAME_SAMPLES * voice3::FRAME_SAMPLES;
        let take: Vec<i16> = self.pcm.drain(..n).collect();
        for f in take.chunks(voice3::FRAME_SAMPLES) {
            self.code_frame(f);
        }
        self.flush_blocks(out);
    }

    /// Ends the stream: the last partial frame padded with zeros, one frame of silence, all-zero words up to a whole block,
    /// then 8 zero symbols.
    pub fn finish(&mut self, out: &mut Vec<Complex32>) {
        if !self.pcm.is_empty() {
            let mut last = std::mem::take(&mut self.pcm);
            last.resize(voice3::FRAME_SAMPLES, 0);
            self.code_frame(&last);
        }
        self.code_frame(&[0i16; voice3::FRAME_SAMPLES]);
        while !self.words.len().is_multiple_of(BLOCK) {
            self.word(&[0u8; phy::K]);
        }
        self.flush_blocks(out);
        self.modu.push(&[Complex32::new(0.0, 0.0); 8], out);
    }
}

/// What the receiver reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Locked to the frames.
    pub locked: bool,
    /// The carrier offset at the notional rate, Hz.
    pub carrier_hz: f64,
    /// Data SNR, dB.
    pub snr_db: Option<f64>,
    /// The sweep level relative to the data, dB.
    pub sweep_db: Option<f64>,
    /// The bit rate read from the codec frames.
    pub rate: Option<V3Rate>,
    /// Words decoded.
    pub words_good: u64,
    /// Words that failed.
    pub words_bad: u64,
    /// The station identity of the sweep.
    pub sweep_id: Option<String>,
}

/// One output of the receiver.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// 320 samples of 16 kHz speech, and the codec frame they came from (`None`: concealed).
    Speech(Vec<i16>, Option<Vec<u8>>),
    /// A beacon frame.
    Beacon(Heard),
}

/// The search buffer, samples (1.25 s).
const ACQ: usize = 60_000;

/// The state after lock.
struct Lock {
    /// The position of the matched-filter sample of symbol 0 of the next frame, absolute.
    frame_at: f64,
    /// The start of the next sweep period, absolute.
    period_at: i64,
    /// Samples cleaned of the sweep up to this absolute index.
    clean_to: i64,
    /// Frames demodulated since lock.
    frames: u64,
    /// Frames with a weak preamble in a row.
    weak: u32,
    /// LLRs of the frames that wait for a block.
    llrs: Vec<Vec<f32>>,
    /// The frame index of the first block, once aligned.
    align: Option<u64>,
    /// Long-term noise variance.
    nv: f64,
    /// Words since lock, and the failures among them.
    words: (u64, u64),
    /// Samples that the frame clock moved against the sweep periods and that the periods have not followed yet.
    drift: f64,
}

/// The version 3 receiver at the notional 48 kHz.
pub struct Receiver {
    x: Vec<Complex32>,
    base: u64,
    n_in: u64,
    f_hz: f64,
    phase: f64,
    h: Vec<f32>,
    lock: Option<Lock>,
    dec: LdpcDecoder,
    last_k: usize,
    half: Option<(Vec<u8>, V3Rate)>,
    codec: Option<(V3Rate, voice3::Decoder)>,
    beacon: BeaconReceiver,
    id: IdReader,
    scanner: Scanner,
    tries_since: u64,
    /// The latest report.
    pub report: Report,
    sweep_power: f64,
    data_power: f64,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    /// A receiver that searches.
    pub fn new() -> Receiver {
        Receiver {
            x: Vec::new(),
            base: 0,
            n_in: 0,
            f_hz: 0.0,
            phase: 0.0,
            h: phy::rrc(SPS, 8, 0.25),
            lock: None,
            dec: phy::decoder(),
            last_k: 320,
            half: None,
            codec: None,
            beacon: BeaconReceiver::new(),
            id: IdReader::default(),
            scanner: Scanner::default(),
            tries_since: 0,
            report: Report::default(),
            sweep_power: 0.0,
            data_power: 0.0,
        }
    }

    /// Drops the lock and every half-received thing; keeps the counters.
    pub fn unlock(&mut self) {
        self.lock = None;
        self.half = None;
        self.report.locked = false;
        self.tries_since = 0;
    }

    /// Tells the receiver that the mixer in front of it moves by `df` Hz (at the notional rate) from the next sample on, its
    /// phase unbroken: the samples held are turned to the new carrier, and a lock that has decoded no word is dropped.
    pub fn carrier_step(&mut self, df: f64) {
        if self.lock.is_some() && !self.report.locked {
            self.unlock();
        }
        let n = self.x.len() as f64;
        for (i, v) in self.x.iter_mut().enumerate() {
            let a = TAU * (df * (i as f64 - n) / FS).fract();
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
        }
    }

    /// Forgets the counters and what was read.
    pub fn clear(&mut self) {
        let locked = self.report.locked;
        let carrier = self.report.carrier_hz;
        self.report = Report {
            locked,
            carrier_hz: carrier,
            ..Report::default()
        };
        self.id.clear();
        self.scanner.reset();
    }

    /// Ends the input: waits for the identity scan in progress.
    pub fn finish(&mut self) {
        self.scanner.finish();
        if let Some(c) = &self.scanner.callsign {
            self.report.sweep_id = Some(c.clone());
        }
    }

    /// Takes samples at the notional 48 kHz and appends the outputs.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Event>) {
        let start = self.x.len();
        for &v in input {
            let a = -TAU * self.phase;
            self.x.push(v * Complex32::new(a.cos() as f32, a.sin() as f32));
            self.phase = (self.phase + self.f_hz / FS).fract();
        }
        self.n_in += input.len() as u64;
        let mut heard = Vec::new();
        if self.lock.is_some() {
            self.beacon.process(&self.x[start..], &mut heard);
        }
        out.extend(heard.into_iter().map(Event::Beacon));
        if !self.report.locked {
            // The scanner reads the identity of a signal too weak to decode: it hears the input until a word decodes, also
            // through a lock that has decoded none.
            self.scanner.push(input);
            if let Some(c) = &self.scanner.callsign {
                self.report.sweep_id = Some(c.clone());
            }
        }
        if self.lock.is_none() {
            self.tries_since += input.len() as u64;
            if self.x.len() >= ACQ && self.tries_since >= 12_000 {
                self.tries_since = 0;
                self.acquire();
                if self.lock.is_none() {
                    let drop = self.x.len() - ACQ + 24_000;
                    self.drop_front(drop.min(self.x.len()));
                }
            }
        }
        if self.lock.is_some() {
            self.track(out);
            if self.lock.is_none() && self.x.len() > ACQ {
                // A lock that ended keeps only what the next search needs.
                let drop = self.x.len() - ACQ;
                self.drop_front(drop);
            }
        }
    }

    fn drop_front(&mut self, n: usize) {
        self.x.drain(..n);
        self.base += n as u64;
    }

    /// The matched-filter output at absolute sample `t` (the pulse centred on `t`).
    fn mf(&self, t: i64) -> Complex32 {
        let s = t - 24 - self.base as i64;
        if s < 0 || s as usize + self.h.len() > self.x.len() {
            return Complex32::new(0.0, 0.0);
        }
        let s = s as usize;
        self.x[s..s + self.h.len()]
            .iter()
            .zip(&self.h)
            .map(|(a, &b)| a * b)
            .sum()
    }

    /// The matched-filter output at a fractional absolute position.
    fn mf_at(&self, pos: f64) -> Complex32 {
        let fl = pos.floor() as i64;
        let w = [self.mf(fl - 1), self.mf(fl), self.mf(fl + 1), self.mf(fl + 2)];
        Reader::new(Cubic).at(&w, 1.0 + (pos - fl as f64))
    }

    /// Searches the buffer for frames: sets the lock when it finds them.
    fn acquire(&mut self) {
        for round in 0..3 {
            let Some((t0, f, metric)) = self.preamble_search() else {
                return;
            };
            let _ = metric;
            if f.abs() > 20.0 {
                // Search again after the move; the last move is taken as it is, and the fit below finds the rest.
                self.retune(f);
                if round < 2 {
                    continue;
                }
            }
            // Fine carrier from the known symbols of the frames in the buffer.
            let mut pts = Vec::new();
            let mut t = t0;
            while t >= 24.0 + self.base as f64 + FRAME_SAMPLES as f64 {
                t -= FRAME_SAMPLES as f64;
            }
            let end = (self.base + self.x.len() as u64) as f64 - 30.0;
            let mut k = 0;
            while t + (FRAME_SAMPLES as f64) < end {
                for i in 0..FRAME {
                    let known = if i < 32 {
                        preamble(i)
                    } else if is_pilot(i) {
                        pilot()
                    } else {
                        continue;
                    };
                    let y = self.mf_at(t + (SPS * i) as f64);
                    pts.push(((k * FRAME + i) as f64, y * known.conj()));
                }
                t += FRAME_SAMPLES as f64;
                k += 1;
            }
            let (w1, _) = phase_slope(&pts, 0.0, 0.03, 0.0005);
            let (w2, _) = phase_slope(&pts, w1, 0.0006, 0.00002);
            let df = w2 / TAU * SYMBOL_RATE;
            self.retune(df);
            self.lock_at(t0);
            return;
        }
    }

    /// Moves the mixer by `df` Hz: the buffer is mixed again and later input follows.
    fn retune(&mut self, df: f64) {
        for (i, v) in self.x.iter_mut().enumerate() {
            let a = -TAU * (df * i as f64 / FS).fract();
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
        }
        let off = (df * self.x.len() as f64 / FS).fract();
        self.phase = (self.phase + off).rem_euclid(1.0);
        self.f_hz += df;
        self.report.carrier_hz = self.f_hz;
    }

    /// The best preamble in the buffer: the absolute matched-filter position of symbol 0, the carrier estimate and the metric.
    fn preamble_search(&self) -> Option<(f64, f64, f64)> {
        let n = self.x.len();
        let z: Vec<Complex32> = (0..n as i64).map(|t| self.mf(t + self.base as i64)).collect();
        let pd: Vec<Complex32> = (0..31).map(|i| preamble(i + 1) * preamble(i).conj()).collect();
        let mut best = (0f64, 0usize, 0usize, Complex32::new(0.0, 0.0));
        for ph in 0..SPS {
            let y: Vec<Complex32> = z.iter().skip(ph).step_by(SPS).copied().collect();
            if y.len() < 40 {
                continue;
            }
            let d: Vec<Complex32> = y.windows(2).map(|w| w[1] * w[0].conj()).collect();
            for s in 0..d.len().saturating_sub(31) {
                let (mut c, mut e) = (Complex32::new(0.0, 0.0), 0f32);
                for i in 0..31 {
                    c += d[s + i] * pd[i].conj();
                    e += d[s + i].norm();
                }
                let m = f64::from(c.norm() / e.max(1e-20));
                if m > best.0 {
                    best = (m, ph, s, c);
                }
            }
        }
        if best.0 < 0.3 {
            return None;
        }
        let t0 = (best.1 + SPS * best.2) as f64 + self.base as f64;
        // Average the carrier over the frames 160 symbols apart that also show a preamble.
        let mut c = best.3;
        let y: Vec<Complex32> = z.iter().skip(best.1).step_by(SPS).copied().collect();
        let d: Vec<Complex32> = y.windows(2).map(|w| w[1] * w[0].conj()).collect();
        let mut s = best.2 as i64 - FRAME as i64;
        while s >= 0 {
            s -= FRAME as i64;
        }
        s += FRAME as i64;
        let mut support = 0;
        while s as usize + 31 <= d.len() {
            let (mut cc, mut e) = (Complex32::new(0.0, 0.0), 0f32);
            for i in 0..31 {
                cc += d[s as usize + i] * pd[i].conj();
                e += d[s as usize + i].norm();
            }
            if s as usize != best.2 && f64::from(cc.norm() / e.max(1e-20)) >= (best.0 / 2.0).max(0.55) {
                c += cc;
                support += 1;
            }
            s += FRAME as i64;
        }
        // A peak of noise has no preambles beside it; a signal has one every frame of the buffer.
        if support < 3 {
            return None;
        }
        let f = f64::from(c.arg()) / TAU * SYMBOL_RATE;
        Some((t0, f, best.0))
    }

    /// Finds the sweep periods and sets the lock with the frame at `t0`.
    fn lock_at(&mut self, t0: f64) {
        let first_frame = {
            let mut t = t0;
            let lo = self.base as f64 + 30.0;
            while t - FRAME_SAMPLES as f64 >= lo {
                t -= FRAME_SAMPLES as f64;
            }
            t
        };
        let mut g = self.sweep_search();
        // The fast variants are one waveform in either half of a period, so the search can land half a period off. Periods
        // and frames both start at sample 0 of the stream, and a period is 25.25 frames, so a period starts on a multiple of
        // 240 samples from the start of a frame (whose pulse peaks 24 samples after it): of the two halves, take that one.
        let r = (g - (t0 as i64 - 24)).rem_euclid(240);
        if (60..180).contains(&r) {
            g += sweep::HALF as i64;
        }
        // Start with the period that holds the first sample of the buffer: its part in the buffer is cleaned too, so the
        // frames of the whole buffer are demodulated at once.
        let mut period_at = g;
        while period_at > self.base as i64 {
            period_at -= PERIOD as i64;
        }
        // A cut period too short for the canceller to fit is skipped with its frames.
        if period_at + PERIOD as i64 - (self.base as i64) < (PERIOD / 4) as i64 {
            period_at += PERIOD as i64;
        }
        let mut frame_at = first_frame;
        while frame_at - 30.0 < period_at as f64 {
            frame_at += FRAME_SAMPLES as f64;
        }
        self.lock = Some(Lock {
            frame_at,
            period_at,
            clean_to: period_at,
            frames: 0,
            weak: 0,
            llrs: Vec::new(),
            align: None,
            nv: 0.0,
            words: (0, 0),
            drift: 0.0,
        });
        self.beacon.reset();
        let from = (period_at - self.base as i64).max(0) as usize;
        let mut heard = Vec::new();
        self.beacon.process(&self.x[from..], &mut heard);
    }

    /// The start (absolute, modulo a period) of the sweep periods in the buffer, by correlation with the four variants.
    fn sweep_search(&self) -> i64 {
        let n = self.x.len();
        let size = (n + PERIOD).next_power_of_two();
        let mut fx = vec![Complex32::new(0.0, 0.0); size];
        fx[..n].copy_from_slice(&self.x);
        Fft::forward(size).process(&mut fx);
        let mut fold = vec![0f64; PERIOD];
        let lags = n.saturating_sub(PERIOD) + 1;
        let mut best_per_lag = vec![0f64; lags];
        for v in 0..4u8 {
            let mut ft = vec![Complex32::new(0.0, 0.0); size];
            for m in 0..PERIOD {
                ft[m] = sweep::template(v, m as i64);
            }
            Fft::forward(size).process(&mut ft);
            for (a, b) in ft.iter_mut().zip(&fx) {
                *a = b * a.conj();
            }
            Fft::inverse(size).process(&mut ft);
            for l in 0..lags {
                best_per_lag[l] = best_per_lag[l].max(f64::from(ft[l].norm_sqr()));
            }
        }
        for l in 0..lags {
            fold[l % PERIOD] += best_per_lag[l];
        }
        let g = fold
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i);
        g as i64 + self.base as i64
    }

    /// Cleans whole sweep periods, then demodulates the frames whose samples are clean.
    fn track(&mut self, out: &mut Vec<Event>) {
        let end = (self.base + self.x.len() as u64) as i64;
        loop {
            let Some(l) = self.lock.as_mut() else {
                return;
            };
            // The periods run on the transmitter's clock as the frames do: they follow the frame clock in whole samples.
            if l.drift.abs() >= 0.75 {
                let step = l.drift.signum();
                l.period_at += step as i64;
                l.drift -= step;
            }
            let p = l.period_at;
            if p + PERIOD as i64 > end {
                break;
            }
            let base = self.base as i64;
            let fit = sweep::cancel_period(&mut self.x, p - base, 0, 3);
            let l = self.lock.as_mut().expect("locked");
            l.clean_to = p + PERIOD as i64;
            l.period_at += PERIOD as i64;
            if let Some(fit) = fit {
                let p = f64::from(fit.gain.norm_sqr());
                self.sweep_power = if self.sweep_power > 0.0 { 0.8 * self.sweep_power + 0.2 * p } else { p };
                self.id.push(&fit);
                if let Some(c) = &self.id.callsign {
                    self.report.sweep_id = Some(c.clone());
                }
            }
        }
        loop {
            let Some(l) = self.lock.as_ref() else {
                return;
            };
            let t = l.frame_at;
            if (t + (SPS * FRAME) as f64 + 30.0) as i64 > l.clean_to {
                break;
            }
            self.frame(t, out);
        }
        // Keep what the next period and frame need.
        if let Some(l) = &self.lock {
            let keep_from = (l.period_at.min(l.frame_at as i64 - 64) - 64).max(self.base as i64) as u64;
            let drop = (keep_from - self.base) as usize;
            if drop > 0 {
                self.drop_front(drop.min(self.x.len()));
            }
        }
    }

    /// Demodulates the frame whose symbol 0 is at `t`.
    fn frame(&mut self, t: f64, out: &mut Vec<Event>) {
        // Early-late on the preamble, half a sample each side: the peak of its correlation by a parabola, and half of the
        // error to the clock.
        let corr = |d: f64| -> f64 {
            let c: Complex32 = (0..32).map(|i| self.mf_at(t + d + (SPS * i) as f64) * preamble(i).conj()).sum();
            f64::from(c.norm())
        };
        let shift = 0.25 * crate::family::fit::parabola(corr(-0.5), corr(0.0), corr(0.5));
        let t = t + shift;
        let y: Vec<Complex32> = (0..FRAME).map(|i| self.mf_at(t + (SPS * i) as f64)).collect();
        let mut known = Vec::with_capacity(40);
        let (mut c, mut ey) = (Complex32::new(0.0, 0.0), 0f32);
        for i in 0..FRAME {
            let k = if i < 32 {
                preamble(i)
            } else if is_pilot(i) {
                pilot()
            } else {
                continue;
            };
            known.push((i as f64 - 79.5, y[i], k));
            if i < 32 {
                c += y[i] * k.conj();
                ey += y[i].norm();
            }
        }
        let metric = c.norm() / ey.max(1e-20);
        let (a, b) = line_gain(&known);
        let resid: f64 = known
            .iter()
            .map(|&(tt, yy, k)| f64::from((yy - (a + b * tt as f32) * k).norm_sqr()))
            .sum::<f64>()
            / 38.0
            / f64::from(a.norm_sqr()).max(1e-20);
        let l = self.lock.as_mut().expect("locked");
        l.frame_at = t + FRAME_SAMPLES as f64;
        l.drift += shift;
        l.frames += 1;
        l.weak = if metric < 0.3 { l.weak + 1 } else { 0 };
        l.nv = if l.frames == 1 { resid } else { 0.5 * l.nv + 0.5 * resid };
        let nv = l.nv.max(1e-4) as f32;
        let mut llr = Vec::with_capacity(FRAME_BITS);
        let mut eq = Vec::with_capacity(DATA);
        for i in 32..FRAME {
            if !is_pilot(i) {
                let g = a + b * (i as f32 - 79.5);
                eq.push(y[i] / if g.norm() > 1e-12 { g } else { Complex32::new(1.0, 0.0) });
            }
        }
        phy::demap(phy::qam16(), &eq, nv, &mut llr);
        l.llrs.push(llr);
        self.data_power = f64::from(a.norm_sqr());
        self.report.snr_db = Some(-10.0 * l.nv.max(1e-6).log10());
        if self.data_power > 0.0 {
            self.report.sweep_db = Some(10.0 * (self.sweep_power.max(1e-20) / (P_DATA * self.data_power)).log10());
        }
        // A small share of the phase slope goes to the mixer. The frames lag the mixer by up to a sweep period, so the share
        // is small enough for that delay.
        let slope = if a.norm() > 1e-12 { (b / a).im as f64 } else { 0.0 };
        let df = slope / TAU * SYMBOL_RATE;
        if metric >= 0.5 {
            self.f_hz += 0.01 * df.clamp(-20.0, 20.0);
        }
        self.report.carrier_hz = self.f_hz;
        let weak = l.weak;
        self.blocks(out);
        if weak >= 8 {
            self.unlock();
        }
    }

    /// Decodes whole interleaver blocks of the frames received.
    fn blocks(&mut self, out: &mut Vec<Event>) {
        let Some(l) = self.lock.as_mut() else {
            return;
        };
        if l.align.is_none() {
            if l.llrs.len() < 2 * BLOCK {
                return;
            }
            let mut best = (0usize, 0usize);
            for o in 0..BLOCK {
                let soft: Vec<f32> = l.llrs[o..o + BLOCK].iter().flatten().copied().collect();
                let good = deinterleave(&soft)
                    .iter()
                    .filter(|w| [320, 240].iter().any(|&k| decode_word(&mut self.dec, w, k, false).is_some()))
                    .count();
                if good > best.1 {
                    best = (o, good);
                }
            }
            if best.1 == 0 {
                l.llrs.drain(..BLOCK);
                return;
            }
            l.llrs.drain(..best.0);
            l.align = Some(0);
        }
        while self.lock.as_ref().is_some_and(|l| l.llrs.len() >= BLOCK) {
            let l = self.lock.as_mut().expect("locked");
            let soft: Vec<f32> = l.llrs.drain(..BLOCK).flatten().collect();
            for w in deinterleave(&soft) {
                self.word(&w, out);
            }
        }
        if let Some(l) = &self.lock {
            // A lock with no good word in 60 frames was false; a lock with more than 6 % bad words of 100 is lost.
            if (l.frames >= 60 && l.words.0 == l.words.1) || (l.words.0 >= 100 && l.words.1 * 100 > 6 * l.words.0) {
                self.unlock();
            }
        }
    }

    /// Decodes one 480-value word and turns it into speech.
    fn word(&mut self, w: &[f32], out: &mut Vec<Event>) {
        let first_k = match &self.half {
            Some((_, r)) => word_k(*r),
            None => self.last_k,
        };
        // The second half of a codec frame is all zeros in silence; a first word never is.
        let second = self.half.is_some();
        let mut got = decode_word(&mut self.dec, w, first_k, second).map(|b| (b, first_k));
        if got.is_none() && !second {
            let other = if first_k == 320 { 240 } else { 320 };
            got = decode_word(&mut self.dec, w, other, false).map(|b| (b, other));
        }
        if let Some(l) = self.lock.as_mut() {
            l.words.0 += 1;
            l.words.1 += u64::from(got.is_none());
        }
        let Some((bits, k)) = got else {
            self.report.words_bad += 1;
            let r = self.half.take().map(|h| h.1).or(self.report.rate);
            self.speech(None, r, out);
            return;
        };
        self.report.words_good += 1;
        self.report.locked = true;
        self.last_k = k;
        if let Some((first, r)) = self.half.take() {
            let mut all = first;
            all.extend_from_slice(&bits);
            self.speech(Some(&all), Some(r), out);
            return;
        }
        let rate = V3Rate::from_profile_id(((bits[0] << 1) | bits[1]) as u32);
        match (rate, k) {
            (Some(r @ (V3Rate::R12 | V3Rate::R16)), _) if word_k(r) == k => self.speech(Some(&bits), Some(r), out),
            (Some(r @ (V3Rate::R24 | V3Rate::R32)), _) if word_k(r) == k => self.half = Some((bits, r)),
            _ => self.speech(None, self.report.rate, out),
        }
    }

    /// Decodes a codec frame (or conceals one) into speech.
    fn speech(&mut self, bits: Option<&[u8]>, rate: Option<V3Rate>, out: &mut Vec<Event>) {
        let Some(rate) = rate else {
            return;
        };
        self.report.rate = Some(rate);
        if self.codec.as_ref().is_none_or(|c| c.0 != rate) {
            self.codec = Some((rate, voice3::Decoder::new(Variant::V3(rate))));
        }
        let bytes = bits.map(crate::family::pack);
        let mut pcm = vec![0i16; voice3::FRAME_SAMPLES];
        if let Some((_, d)) = self.codec.as_mut() {
            let _ = d.decode(bytes.as_deref(), &mut pcm);
        }
        out.push(Event::Speech(pcm, bytes));
    }
}

/// The four words of 480 LLRs of an interleaver block of 1920 LLRs.
pub fn deinterleave(soft: &[f32]) -> Vec<Vec<f32>> {
    let p = phy::permutation(BLOCK * FRAME_BITS);
    let mut d = vec![0f32; soft.len()];
    for (i, &pi) in p.iter().enumerate() {
        d[pi as usize] = soft[i];
    }
    d.chunks(FRAME_BITS).map(|c| c.to_vec()).collect()
}

/// Decodes a word of 480 LLRs that carries `k` information bits. An all-zero result counts only with `zero_ok`.
pub fn decode_word(dec: &mut LdpcDecoder, w: &[f32], k: usize, zero_ok: bool) -> Option<Vec<u8>> {
    let mean = w.iter().map(|v| v.abs()).sum::<f32>() / w.len().max(1) as f32;
    let scale = 12.0 / mean.max(1e-12);
    let mut l = [0i8; N];
    for i in 0..k {
        l[i] = phy::quantise(w[i], scale);
    }
    for j in 0..FRAME_BITS - k {
        l[phy::K + j] = phy::quantise(w[k + j], scale);
    }
    // An all-zero word is what a constant input decodes to; it counts only where the caller expects it (`zero_ok`).
    phy::decode(dec, &l, k, 40).filter(|b| zero_ok || b.iter().any(|&v| v != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preamble_and_the_frame() {
        let signs: String = (0..32)
            .map(|i| {
                let p = preamble(i);
                format!("{}{}", if p.re > 0.0 { '+' } else { '-' }, if p.im > 0.0 { '+' } else { '-' })
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(signs, "-+ +- -+ -+ +- -+ ++ -- -+ ++ -- ++ -+ -- ++ -+ +- -+ -+ +- -+ ++ -- -+ ++ -- ++ -+ -- ++ -+ +-");
        let f = frame(&[0u8; 480]);
        assert_eq!(f.len(), 160);
        assert!((f[47] - pilot()).norm() < 1e-6 && (f[159] - pilot()).norm() < 1e-6);
        let p: f32 = (0..32).map(|i| preamble(i).norm_sqr()).sum::<f32>() / 32.0;
        assert!((p - 1.0).abs() < 1e-6);
    }
}


