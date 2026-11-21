//! The packet modems AFSK 1200 and G3RUH 9600: discriminator audio in, AX.25 frames with a good FCS out, and the clean
//! transmit audio of frames. [`AfskReceiver`] is the AFSK receiver for any tone pair and speed on complex audio.
//!
//! Standard: AX.25 2.2 clause 3.5 (Bell 202 tones), J. Miller G3RUH (1988)

use crate::hdlc::{Line, Receiver, Transmitter, AX25_MAX, AX25_MIN};
use crate::{Error, Result};
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::fsk::{Analytic, ToneCorrelator};
use sdrkit_algo_modem::timing::EdgeClock;
use sdrkit_algo_sim::modulate::{Afsk, Linear};
use sdrkit_core::Complex32;
use sdrkit_util_bits::hdlc::FrameEnd;
use std::f64::consts::TAU;

/// Bell 202 mark tone (a 1), Hz.
pub const MARK_HZ: f64 = 1200.0;
/// Bell 202 space tone (a 0), Hz.
pub const SPACE_HZ: f64 = 2200.0;

/// The shared bit clock for `baud` at `rate`, with the error type of this crate.
fn bit_clock(baud: f64, rate: f64, search: f32, locked: f32) -> Result<EdgeClock> {
    EdgeClock::new(baud, rate, search, locked).map_err(Error::Modem)
}

/// A frame found by a demodulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found<'a> {
    /// The frame's bytes without the FCS.
    pub bytes: &'a [u8],
    /// The input sample at which its closing flag ended (counted from the first sample given).
    pub at: u64,
    /// The slicer that found it first (0 to 11 for AFSK, 0 and 1 for G3RUH).
    pub slicer: u8,
}

/// Counts kept by a demodulator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModemStats {
    /// Distinct frames reported.
    pub frames: u64,
    /// Copies of a frame found by more slicers, not reported.
    pub duplicates: u64,
}

/// The mean tone levels over a frame, from the AFSK tone correlators. Their scale is arbitrary. Their ratio is what matters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneLevels {
    /// The mean magnitude of the mark tone at the mark bits between two mark bits, at the decisions of the slicer that found
    /// the frame.
    pub mark: f32,
    /// The mean magnitude of the space tone at the space bits between two space bits.
    pub space: f32,
}

impl ToneLevels {
    /// The tone balance ("twist"): the mark level over the space level, dB. Positive when the mark tone is stronger.
    pub fn twist_db(&self) -> f32 {
        20.0 * (self.mark / self.space).log10()
    }
}

/// A frame found by [`Afsk1200::process_frames`] or [`G3ruh9600::process_frames`], good or bad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard<'a> {
    /// The frame's bytes without the FCS.
    pub bytes: &'a [u8],
    /// The bytes followed by the FCS as received (for [`crate::hdlc::repair`]).
    pub with_fcs: &'a [u8],
    /// Whether the FCS was right.
    pub fcs_ok: bool,
    /// The input sample at which its closing flag ended (counted from the first sample given).
    pub at: u64,
    /// The slicer that found it (0 to 11 for AFSK, 0 and 1 for G3RUH).
    pub slicer: u8,
    /// The mean tone levels over the frame (AFSK only).
    pub tones: Option<ToneLevels>,
}

/// A bad frame held until no good frame or other copy can follow.
struct Pending {
    with_fcs: Vec<u8>,
    /// The length without the FCS.
    len: usize,
    at: u64,
    slicer: u8,
    tones: Option<ToneLevels>,
    active: bool,
}

/// The frames of all slicers: copies of good frames dropped, one bad frame per failed transmission.
struct Frames {
    dedup: Dedup,
    pending: Pending,
    last_good: Option<u64>,
    bad: u64,
}

impl Frames {
    fn new(window: u64) -> Frames {
        Frames {
            dedup: Dedup::new(window),
            pending: Pending {
                with_fcs: Vec::with_capacity(AX25_MAX + 4),
                len: 0,
                at: 0,
                slicer: 0,
                tones: None,
                active: false,
            },
            last_good: None,
            bad: 0,
        }
    }

    fn reset(&mut self) {
        self.pending.active = false;
        self.last_good = None;
    }

    /// Returns the held bad frame.
    fn emit_pending(&mut self, on: &mut impl FnMut(Heard<'_>)) {
        let p = &mut self.pending;
        if !p.active {
            return;
        }
        p.active = false;
        self.bad += 1;
        on(Heard {
            bytes: &p.with_fcs[..p.len.min(p.with_fcs.len())],
            with_fcs: &p.with_fcs,
            fcs_ok: false,
            at: p.at,
            slicer: p.slicer,
            tones: p.tones,
        });
    }

    /// Returns the held bad frame once its time is over.
    #[inline]
    fn tick(&mut self, n: u64, on: &mut impl FnMut(Heard<'_>)) {
        if self.pending.active && n >= self.pending.at + self.dedup.window {
            self.emit_pending(on);
        }
    }

    /// A frame end of a slicer's receiver.
    fn end(
        &mut self,
        rx: &Receiver,
        end: FrameEnd,
        at: u64,
        slicer: u8,
        tones: impl FnOnce(usize) -> Option<ToneLevels>,
        on: &mut impl FnMut(Heard<'_>),
    ) {
        let near = |t: u64, w: u64| at.saturating_sub(t) < w;
        let w = self.dedup.window;
        if end.fcs_ok {
            self.last_good = Some(at);
            if self.pending.active && near(self.pending.at, w) {
                self.pending.active = false;
            }
            if self.dedup.fresh(rx.frame(), at) {
                on(Heard {
                    bytes: rx.frame(),
                    with_fcs: rx.frame_with_fcs(),
                    fcs_ok: true,
                    at,
                    slicer,
                    tones: tones(end.len),
                });
            }
            return;
        }
        if self.last_good.is_some_and(|g| near(g, w))
            || (self.pending.active && near(self.pending.at, w))
        {
            return;
        }
        self.emit_pending(on);
        let p = &mut self.pending;
        p.with_fcs.clear();
        p.with_fcs.extend_from_slice(rx.frame_with_fcs());
        p.len = end.len;
        p.at = at;
        p.slicer = slicer;
        p.tones = tones(end.len);
        p.active = true;
    }
}

struct Slicer {
    clock: EdgeClock,
    rx: Receiver,
    /// The tone levels at this slicer's bit decisions (AFSK only).
    sums: Option<ToneSums>,
}

/// The tone magnitudes at one slicer's bit decisions, with a snapshot every 8 decisions, so the levels over a frame come from
/// two snapshots.
struct ToneSums {
    /// Running sums: mark magnitude and count at mark bits, space magnitude and count at space bits.
    now: [f64; 4],
    ring: Vec<[f64; 4]>,
    head: usize,
    filled: usize,
    /// Decisions since the last snapshot.
    bits: u8,
    /// The last two decisions: tone (true for mark) and the two magnitudes.
    last: [(Option<bool>, f32, f32); 2],
}

impl ToneSums {
    fn new() -> ToneSums {
        ToneSums {
            now: [0.0; 4],
            ring: vec![[0.0; 4]; AX25_MAX + 8],
            head: 0,
            filled: 0,
            bits: 0,
            last: [(None, 0.0, 0.0); 2],
        }
    }

    fn reset(&mut self) {
        self.now = [0.0; 4];
        self.head = 0;
        self.filled = 0;
        self.bits = 0;
        self.last = [(None, 0.0, 0.0); 2];
    }

    /// Takes the tone magnitudes at a bit decision. A decision counts when the bits before and after it have the same tone:
    /// the correlator window then lies in that tone even if the decision is a few samples off the bit centre, so the other
    /// tone does not leak in.
    #[inline]
    fn add(&mut self, mark: bool, m: f32, s: f32) {
        let [(b0, _, _), (b1, m1, s1)] = self.last;
        if b0 == b1 && b1 == Some(mark) {
            if mark {
                self.now[0] += m1 as f64;
                self.now[1] += 1.0;
            } else {
                self.now[2] += s1 as f64;
                self.now[3] += 1.0;
            }
        }
        self.last = [self.last[1], (Some(mark), m, s)];
        self.bits += 1;
        if self.bits == 8 {
            self.bits = 0;
            self.ring[self.head] = self.now;
            self.head = (self.head + 1) % self.ring.len();
            self.filled = (self.filled + 1).min(self.ring.len());
        }
    }

    /// The levels over a frame of `len` bytes (without its 2 FCS bytes) that ended at the last decision: from the snapshots
    /// inside its data and FCS, leaving out the closing flag.
    fn levels(&self, len: usize) -> Option<ToneLevels> {
        let span = len + 1;
        if self.filled < span + 2 {
            return None;
        }
        let at = |back: usize| self.ring[(self.head + self.ring.len() - back) % self.ring.len()];
        let (end, start) = (at(2), at(2 + span));
        let (nm, ns) = (end[1] - start[1], end[3] - start[3]);
        if nm < 1.0 || ns < 1.0 {
            return None;
        }
        Some(ToneLevels {
            mark: ((end[0] - start[0]) / nm) as f32,
            space: ((end[2] - start[2]) / ns) as f32,
        })
    }
}

/// Suppresses copies of a frame found by several slicers.
struct Dedup {
    last: Vec<u8>,
    at: u64,
    window: u64,
    stats: ModemStats,
}

impl Dedup {
    fn new(window: u64) -> Dedup {
        Dedup {
            last: Vec::with_capacity(AX25_MAX),
            at: 0,
            window,
            stats: ModemStats::default(),
        }
    }

    /// Whether `bytes` ending at `at` is new. It is then remembered.
    fn fresh(&mut self, bytes: &[u8], at: u64) -> bool {
        if bytes == self.last.as_slice() && at.saturating_sub(self.at) < self.window {
            self.stats.duplicates += 1;
            return false;
        }
        self.last.clear();
        self.last.extend_from_slice(bytes);
        self.at = at;
        self.stats.frames += 1;
        true
    }
}

/// Twist weights of the five fixed slicers, on tone magnitude (mark against weight times space).
const WEIGHTS: [f32; 5] = [1.0, 0.71, 1.41, 0.5, 2.0];

struct Correlator {
    tones: ToneCorrelator,
    env_m: f32,
    env_s: f32,
    slicers: Vec<Slicer>,
}

/// The AFSK 1200 demodulator: two tone correlators (one bit and 0.8 of a bit long), each with six slicers for tilted tones, and
/// one bit clock and NRZI HDLC receiver per slicer. A frame that several slicers decode is reported once. It allocates in
/// [`Afsk1200::new`] and nothing after.
///
/// ```
/// use sdrkit_proto_link::modem::{afsk1200_audio, Afsk1200};
/// let frame = b"\x82\xa0\xa4\xa6@@`\x9c`\x86\x82\x98\x98a\x03\xf0hello";
/// let mut audio = Vec::new();
/// afsk1200_audio(&[frame], 30, 48_000.0, &mut audio).unwrap();
/// let mut rx = Afsk1200::new(48_000.0).unwrap();
/// let mut got = Vec::new();
/// rx.process(&audio, |f| got.push(f.bytes.to_vec()));
/// assert_eq!(got, [frame.to_vec()]);
/// ```
pub struct Afsk1200 {
    analytic: Analytic,
    corr: Vec<Correlator>,
    decay: f32,
    n: u64,
    frames: Frames,
}

impl Afsk1200 {
    /// The lowest audio rate in samples per second.
    pub const MIN_RATE: f64 = 9_600.0;
    /// The highest audio rate in samples per second.
    pub const MAX_RATE: f64 = 192_000.0;

    /// A demodulator for audio at `rate` samples per second ([`Self::MIN_RATE`] to [`Self::MAX_RATE`]).
    pub fn new(rate: f64) -> Result<Afsk1200> {
        if !(Self::MIN_RATE..=Self::MAX_RATE).contains(&rate) {
            return Err(Error::Rate {
                min: Self::MIN_RATE,
                max: Self::MAX_RATE,
                got: rate,
            });
        }
        let spb = rate / 1200.0;
        let analytic = Analytic::for_band(rate, 900.0).map_err(Error::Modem)?;
        let freqs = [MARK_HZ / rate, SPACE_HZ / rate];
        let mut corr = Vec::new();
        for w in [spb, 0.8 * spb] {
            let tones =
                ToneCorrelator::new(&freqs, w.round().max(2.0) as usize).map_err(Error::Modem)?;
            let mut slicers = Vec::new();
            for _ in 0..6 {
                slicers.push(Slicer {
                    clock: bit_clock(1200.0, rate, 0.74, 0.84)?,
                    rx: Receiver::ax25(Line::Nrzi).deliver_bad(true),
                    sums: Some(ToneSums::new()),
                });
            }
            corr.push(Correlator {
                tones,
                env_m: 0.0,
                env_s: 0.0,
                slicers,
            });
        }
        Ok(Afsk1200 {
            analytic,
            corr,
            // The envelope falls by a factor e over about six bits.
            decay: (1.0 - 1.0 / (6.0 * spb)) as f32,
            n: 0,
            frames: Frames::new((32.0 * spb) as u64),
        })
    }

    /// Feeds discriminator audio (any scale) and calls `on_frame` for each new frame with a good FCS.
    pub fn process(&mut self, audio: &[f32], mut on_frame: impl FnMut(Found<'_>)) {
        self.run(audio, false, &mut |h: Heard<'_>| {
            on_frame(Found {
                bytes: h.bytes,
                at: h.at,
                slicer: h.slicer,
            })
        });
    }

    /// Feeds discriminator audio (any scale) and calls `on_frame` for each new frame with a good FCS, as
    /// [`Afsk1200::process`], and also once for each failed transmission with `fcs_ok` false. A bad frame is held for 32 bits
    /// and dropped if a slicer decodes another frame in that time. Each frame carries its tone levels.
    pub fn process_frames(&mut self, audio: &[f32], mut on_frame: impl FnMut(Heard<'_>)) {
        self.run(audio, true, &mut on_frame);
    }

    /// Returns a bad frame still held (at the end of a recording).
    pub fn flush_frames(&mut self, mut on_frame: impl FnMut(Heard<'_>)) {
        self.frames.emit_pending(&mut on_frame);
    }

    fn run(&mut self, audio: &[f32], bad: bool, on: &mut impl FnMut(Heard<'_>)) {
        let Afsk1200 {
            analytic,
            corr,
            decay,
            n,
            frames,
        } = self;
        for &x in audio {
            let z = analytic.push(x);
            for (ci, c) in corr.iter_mut().enumerate() {
                c.tones.push(z);
                let s = c.tones.sums();
                let (m, sp) = (s[0].norm(), s[1].norm());
                c.env_m = if m > c.env_m { m } else { c.env_m * *decay };
                c.env_s = if sp > c.env_s { sp } else { c.env_s * *decay };
                let balanced = m * c.env_s > sp * c.env_m;
                for (k, sl) in c.slicers.iter_mut().enumerate() {
                    let level = if k < 5 { m > WEIGHTS[k] * sp } else { balanced };
                    if let Some(bit) = sl.clock.sample(level as u8) {
                        if let Some(t) = &mut sl.sums {
                            t.add(bit == 1, m, sp);
                        }
                        if let Some(end) = sl.rx.push(bit) {
                            if end.fcs_ok {
                                // Hold the clock through back-to-back frames.
                                sl.clock.hold(96);
                            } else if !bad {
                                continue;
                            }
                            frames.end(
                                &sl.rx,
                                end,
                                *n,
                                (ci * 6 + k) as u8,
                                |len| sl.sums.as_ref().and_then(|t| t.levels(len)),
                                on,
                            );
                        }
                    }
                }
            }
            if bad {
                frames.tick(*n, on);
            }
            *n += 1;
        }
    }

    /// The bad frames given by [`Afsk1200::process_frames`] (one per failed transmission).
    pub fn bad_frames(&self) -> u64 {
        self.frames.bad
    }

    /// The counts of frames and duplicates.
    pub fn stats(&self) -> ModemStats {
        self.frames.dedup.stats
    }

    /// Forgets all state (after a retune). A bad frame still held is dropped.
    pub fn reset(&mut self) {
        self.analytic.reset();
        self.frames.reset();
        for c in &mut self.corr {
            c.tones.reset();
            c.env_m = 0.0;
            c.env_s = 0.0;
            for s in &mut c.slicers {
                s.rx.reset();
                if let Some(t) = &mut s.sums {
                    t.reset();
                }
            }
        }
    }
}

/// The G3RUH 9600 demodulator: a low-pass filter, a slicer at the middle of the tracked peaks, and two bit clocks with
/// different inertia, each with an HDLC receiver and the G3RUH descrambler. It allocates in [`G3ruh9600::new`] and nothing
/// after.
pub struct G3ruh9600 {
    lp: Fir<f32>,
    hi: f32,
    lo: f32,
    decay: f32,
    slicers: [Slicer; 2],
    n: u64,
    frames: Frames,
}

impl G3ruh9600 {
    /// The lowest audio rate in samples per second.
    pub const MIN_RATE: f64 = 38_400.0;
    /// The highest audio rate in samples per second.
    pub const MAX_RATE: f64 = 384_000.0;

    /// A demodulator for discriminator audio at `rate` samples per second ([`Self::MIN_RATE`] to [`Self::MAX_RATE`]).
    pub fn new(rate: f64) -> Result<G3ruh9600> {
        if !(Self::MIN_RATE..=Self::MAX_RATE).contains(&rate) {
            return Err(Error::Rate {
                min: Self::MIN_RATE,
                max: Self::MAX_RATE,
                got: rate,
            });
        }
        let spb = rate / 9600.0;
        Ok(G3ruh9600 {
            lp: Fir::new(&fir::lowpass(rate, 5_500.0, 9_000.0, 40.0).map_err(Error::Dsp)?),
            hi: 0.0,
            lo: 0.0,
            decay: (1.0 - 1.0 / (400.0 * spb)) as f32,
            slicers: [
                Slicer {
                    clock: bit_clock(9600.0, rate, 0.70, 0.80)?,
                    rx: Receiver::ax25(Line::G3ruh).deliver_bad(true),
                    sums: None,
                },
                Slicer {
                    clock: bit_clock(9600.0, rate, 0.75, 0.85)?,
                    rx: Receiver::ax25(Line::G3ruh).deliver_bad(true),
                    sums: None,
                },
            ],
            n: 0,
            frames: Frames::new((32.0 * spb) as u64),
        })
    }

    /// Feeds discriminator audio (any scale) and calls `on_frame` for each new frame with a good FCS.
    pub fn process(&mut self, audio: &[f32], mut on_frame: impl FnMut(Found<'_>)) {
        self.run(audio, false, &mut |h: Heard<'_>| {
            on_frame(Found {
                bytes: h.bytes,
                at: h.at,
                slicer: h.slicer,
            })
        });
    }

    /// As [`Afsk1200::process_frames`]: good frames, and one bad frame per failed transmission, without tone levels.
    pub fn process_frames(&mut self, audio: &[f32], mut on_frame: impl FnMut(Heard<'_>)) {
        self.run(audio, true, &mut on_frame);
    }

    /// Returns a bad frame still held (at the end of a recording).
    pub fn flush_frames(&mut self, mut on_frame: impl FnMut(Heard<'_>)) {
        self.frames.emit_pending(&mut on_frame);
    }

    /// The bad frames given by [`G3ruh9600::process_frames`] (one per failed transmission).
    pub fn bad_frames(&self) -> u64 {
        self.frames.bad
    }

    fn run(&mut self, audio: &[f32], bad: bool, on: &mut impl FnMut(Heard<'_>)) {
        for &x in audio {
            self.lp.push(x);
            let y = self.lp.output();
            // Peak trackers: each follows its extreme at once and relaxes toward the other.
            self.hi = if y > self.hi {
                y
            } else {
                self.lo + (self.hi - self.lo) * self.decay
            };
            self.lo = if y < self.lo {
                y
            } else {
                self.hi - (self.hi - self.lo) * self.decay
            };
            let level = (y > 0.5 * (self.hi + self.lo)) as u8;
            for (k, sl) in self.slicers.iter_mut().enumerate() {
                if let Some(bit) = sl.clock.sample(level) {
                    if let Some(end) = sl.rx.push(bit) {
                        if end.fcs_ok {
                            sl.clock.hold(96);
                        } else if !bad {
                            continue;
                        }
                        self.frames.end(&sl.rx, end, self.n, k as u8, |_| None, on);
                    }
                }
            }
            if bad {
                self.frames.tick(self.n, on);
            }
            self.n += 1;
        }
    }

    /// The counts of frames and duplicates.
    pub fn stats(&self) -> ModemStats {
        self.frames.dedup.stats
    }

    /// Forgets all state (after a retune). A bad frame still held is dropped.
    pub fn reset(&mut self) {
        self.lp.reset();
        self.frames.reset();
        self.hi = 0.0;
        self.lo = 0.0;
        for s in &mut self.slicers {
            s.rx.reset();
        }
    }
}

/// The tuning gain of [`AfskReceiver`] outside a frame, per sample.
const GAIN_IDLE: f64 = 4e-4;
/// The tuning gain inside a frame, per sample.
const GAIN_FRAME: f64 = 5e-5;
/// The least tone power of a signal block.
const TONE_MIN: f64 = 1e-7;
/// The least tone to rest ratio of a signal block, dB.
const TONE_SNR_MIN_DB: f64 = 3.0;

/// The AFSK receiver: complex audio (the suppressed carrier at 0 Hz, an audio tone `f` at `+f` Hz) to HDLC frames. The tone pair
/// and the symbol rate are free.
pub struct AfskReceiver {
    rate: f64,
    sps: f64,
    shift: f64,
    corr: ToneCorrelator,
    window: usize,
    /// The tuning correction, Hz, and the phase of its oscillator, cycles.
    tune_hz: f64,
    phase: f64,
    prev: [Complex32; 2],
    level: u8,
    steady: u32,
    clock: f64,
    flag_seen: bool,
    last_level: u8,
    recent: u8,
    ones: u32,
    hdlc: Receiver,
    blk_n: u32,
    blk_tone: f64,
    blk_all: f64,
    signal_blocks: u64,
    signal_now: bool,
    quality_db: Option<f64>,
}

/// What [`AfskReceiver::process`] reports.
pub enum AfskHeard<'a> {
    /// A frame with a good FCS: its bytes without the FCS.
    Frame(&'a [u8]),
    /// A frame with a bad FCS, while the tones are there.
    Bad,
}

impl AfskReceiver {
    /// A receiver for complex audio at `rate` with `baud` symbols a second, line level 1 at `mark_hz` and 0 at `space_hz`.
    ///
    /// # Errors
    ///
    /// [`Error::Rate`] if the rate holds fewer than 4 samples for a symbol, [`Error::Modem`] if a tone is above half the rate.
    pub fn new(rate: f64, baud: f64, mark_hz: f64, space_hz: f64) -> Result<Self> {
        let sps = rate / baud;
        if !(sps >= 4.0 && sps.is_finite()) {
            return Err(Error::Rate {
                min: 4.0 * baud,
                max: f64::MAX,
                got: rate,
            });
        }
        let window = sps.round() as usize;
        let corr = ToneCorrelator::new(&[mark_hz / rate, space_hz / rate], window)
            .map_err(Error::Modem)?;
        Ok(AfskReceiver {
            rate,
            sps,
            shift: (space_hz - mark_hz).abs(),
            corr,
            window,
            tune_hz: 0.0,
            phase: 0.0,
            prev: [Complex32::default(); 2],
            level: 0,
            steady: 0,
            clock: 0.0,
            flag_seen: false,
            last_level: 0,
            recent: 0,
            ones: 0,
            hdlc: Receiver::ax25(Line::Nrzi).deliver_bad(true),
            blk_n: 0,
            blk_tone: 0.0,
            blk_all: 0.0,
            signal_blocks: 0,
            signal_now: false,
            quality_db: None,
        })
    }

    /// The tuning correction, Hz.
    pub fn tone_offset_hz(&self) -> f64 {
        self.tune_hz
    }

    /// Blocks of 0.1 s that held the tones.
    pub fn signal_blocks(&self) -> u64 {
        self.signal_blocks
    }

    /// The smoothed ratio of the tone power to the rest, dB. `None` before the first block with the tones.
    pub fn quality_db(&self) -> Option<f64> {
        self.quality_db
    }

    /// Forgets the signal: the frame in progress, the tuning and the tone measure.
    pub fn reset(&mut self) {
        self.corr.reset();
        self.hdlc.reset();
        self.tune_hz = 0.0;
        self.steady = 0;
        self.clock = 0.0;
        self.flag_seen = false;
        self.blk_n = 0;
        self.blk_tone = 0.0;
        self.blk_all = 0.0;
        self.signal_blocks = 0;
        self.signal_now = false;
        self.quality_db = None;
    }

    /// Takes one line bit: the flag watch for the tuning gain and the clock pull, then the HDLC receiver.
    fn bit(&mut self, on: &mut impl FnMut(AfskHeard<'_>)) {
        let data = u8::from(self.level == self.last_level);
        self.last_level = self.level;
        self.recent = (self.recent >> 1) | (data << 7);
        self.ones = if data == 1 { self.ones + 1 } else { 0 };
        if self.recent == 0x7E {
            self.flag_seen = true;
        } else if self.ones >= 7 {
            self.flag_seen = false;
        }
        if let Some(end) = self.hdlc.push(self.level) {
            if end.fcs_ok {
                on(AfskHeard::Frame(self.hdlc.frame()));
            } else if self.signal_now {
                on(AfskHeard::Bad);
            }
        }
    }

    /// Ends a block of 0.1 s of the tone measure.
    fn block(&mut self) {
        let n = f64::from(self.blk_n);
        let tone = self.blk_tone / n;
        let all = self.blk_all / n;
        self.blk_n = 0;
        self.blk_tone = 0.0;
        self.blk_all = 0.0;
        let rest = (all - tone).max(1e-3 * all).max(1e-30);
        let snr = 10.0 * (tone / rest).max(1e-30).log10();
        self.signal_now = tone > TONE_MIN && snr > TONE_SNR_MIN_DB;
        if self.signal_now {
            self.signal_blocks += 1;
            self.quality_db = Some(self.quality_db.map_or(snr, |q| 0.8 * q + 0.2 * snr));
        }
    }

    /// Decodes `x` and calls `on` for each frame that ends.
    pub fn process(&mut self, x: &[Complex32], mut on: impl FnMut(AfskHeard<'_>)) {
        let w2 = (self.window * self.window) as f64;
        let block = (0.1 * self.rate).round() as u32;
        let limit = self.shift / 3.0;
        for &s in x {
            let (sin, cos) = (TAU * self.phase).sin_cos();
            self.phase = (self.phase - self.tune_hz / self.rate).rem_euclid(1.0);
            self.corr.push(s * Complex32::new(cos as f32, sin as f32));
            let sums = self.corr.sums();
            let (mark, space) = (sums[0], sums[1]);
            let (em, es) = (f64::from(mark.norm_sqr()), f64::from(space.norm_sqr()));
            let level = u8::from(em > es);
            let strong = if level == 1 { mark } else { space };
            if level == self.level {
                self.steady += 1;
            } else {
                self.level = level;
                self.steady = 0;
                self.clock *= if self.flag_seen { 0.85 } else { 0.7 };
            }
            // The sum of the stronger tone turns by the tuning error from one sample to the next.
            let before = self.prev[usize::from(level)];
            if f64::from(self.steady) > self.sps && before.norm_sqr() > 0.0 {
                let turn = strong * before.conj();
                let err = f64::from(turn.im.atan2(turn.re)) / TAU * self.rate;
                if err.abs() < self.shift / 2.0 {
                    let gain = if self.flag_seen {
                        GAIN_FRAME
                    } else {
                        GAIN_IDLE
                    };
                    self.tune_hz = (self.tune_hz + gain * err).clamp(-limit, limit);
                }
            }
            self.prev = [space, mark];
            self.blk_tone += (em + es) / w2;
            self.blk_all += f64::from(s.norm_sqr());
            self.blk_n += 1;
            if self.blk_n == block {
                self.block();
            }
            self.clock += 1.0 / self.sps;
            if self.clock >= 0.5 {
                self.clock -= 1.0;
                self.bit(&mut on);
            }
        }
    }
}

/// Appends the AFSK 1200 audio of `frames` (bytes without the FCS) at `rate`: `flags` flags of preamble (TXDELAY), the frames,
/// and a closing flag. The audio is phase continuous with peak amplitude 1.
pub fn afsk1200_audio(frames: &[&[u8]], flags: usize, rate: f64, out: &mut Vec<f32>) -> Result<()> {
    check_lengths(frames)?;
    let mut bits = Vec::new();
    let mut tx = Transmitter::new(Line::Nrzi);
    tx.encode(frames, flags, &mut bits);
    tx.flags(2, &mut bits);
    let mut m = Afsk::new(rate, 1200.0, MARK_HZ, SPACE_HZ).map_err(Error::Sim)?;
    m.push(&bits, out);
    m.flush(out);
    Ok(())
}

/// Appends the G3RUH 9600 baseband of `frames` (bytes without the FCS) at `rate`: `flags` flags of preamble, the frames, and
/// closing flags, scrambled NRZI as levels of about ±1 with a raised-cosine pulse (roll-off 0.5).
pub fn g3ruh9600_audio(
    frames: &[&[u8]],
    flags: usize,
    rate: f64,
    out: &mut Vec<f32>,
) -> Result<()> {
    check_lengths(frames)?;
    let mut bits = Vec::new();
    let mut tx = Transmitter::new(Line::G3ruh);
    tx.encode(frames, flags, &mut bits);
    tx.flags(4, &mut bits);
    let mut m = Linear::new(Shape::Rc { beta: 0.5 }, 8.0, rate / 9600.0).map_err(Error::Sim)?;
    let symbols: Vec<Complex32> = bits
        .iter()
        .map(|&b| Complex32::new(if b == 1 { 1.0 } else { -1.0 }, 0.0))
        .collect();
    let mut iq = Vec::new();
    m.push(&symbols, &mut iq);
    m.flush(&mut iq);
    out.extend(iq.iter().map(|z| z.re));
    Ok(())
}

fn check_lengths(frames: &[&[u8]]) -> Result<()> {
    for f in frames {
        if f.len() < AX25_MIN {
            return Err(Error::Short);
        }
        if f.len() > AX25_MAX {
            return Err(Error::TooLong);
        }
    }
    Ok(())
}
