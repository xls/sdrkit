//! FreeDV 700D and 700E: the OFDM waveforms (a BPSK pilot row, QPSK data rows, a unique word, text symbols and one interleaved
//! LDPC code word in each modem frame), their modulator and the demodulator component.
//! Standard: FreeDV project documentation.

use sdrkit_algo_dsp::fir::{ComplexFir, Fir};
use sdrkit_algo_fec::ldpc::tables::freedv::{FREEDV_700D_224_112, FREEDV_700E_112_56};
use sdrkit_algo_fec::ldpc::{BpDecoder, LdpcCode};
use sdrkit_core::Complex32;
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

/// The sample rate of the waveform layer.
pub const RATE: f64 = 8000.0;
/// Samples per symbol (body and cyclic prefix), both waveforms.
pub const S: usize = 160;
/// The timing metric a coarse search needs.
const TIMING_THRESHOLD: f32 = 0.30;
/// The Es/N0 (linear) that the soft decisions assume.
const SOFT_ES_N0: f32 = 3.0;
/// The largest magnitude of a soft decision.
const LLR_LIMIT: f32 = 50.0;
/// The gain of the frequency tracking.
const TRACK_GAIN: f32 = 0.1;
/// The most LDPC iterations.
const MAX_ITERATIONS: u32 = 100;

/// One OFDM waveform.
#[derive(Debug)]
pub struct Waveform {
    /// The name of the waveform.
    pub name: &'static str,
    /// Data carriers.
    pub nc: usize,
    /// Samples of the symbol body.
    pub m: usize,
    /// Samples of the cyclic prefix.
    pub ncp: usize,
    /// Symbols per modem frame, the pilot row included.
    pub ns: usize,
    /// The index of carrier 0: carrier `c` is at `(lowest + c) × 8000 / m` Hz.
    pub lowest: usize,
    /// The pilot values of the `nc + 2` carriers (0: the carrier is not sent).
    pub pilots: &'static [i8],
    /// Unique word bits (all zero).
    pub uw_bits: usize,
    /// Text bits per modem frame.
    pub text_bits: usize,
    /// The interleaver constant.
    pub interleave: usize,
    /// The data bits of each check of the LDPC code.
    pub checks: &'static [[u8; 3]],
    /// Candidates of the fine timing search.
    pub timing_window: usize,
    /// A frame is bad when its unique word has more errors than this.
    pub uw_limit: usize,
    /// The largest step of the frequency tracking, Hz.
    pub track_limit_hz: Option<f32>,
    /// The channel estimate averages over neighbour carriers and four pilot rows once synced.
    pub narrow_when_synced: bool,
    /// The sync rule.
    pub rule: SyncRule,
    /// The numbers of the transmit chain.
    pub tx: TxShape,
}

/// The numbers of a transmit chain ([`TxChain`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxShape {
    /// The amplitude scale, as a share of full scale: it puts the peak of the modulator output near half of full scale.
    pub scale: f32,
    /// The drive gain of the clipper.
    pub clip_gain: f32,
    /// The cut-off of the transmit filter around the centre, Hz.
    pub cut_hz: f64,
}

/// FreeDV 700D: 17 carriers of 55.6 Hz, frames of 160 ms, the (224,112) code.
pub static W700D: Waveform = Waveform {
    name: "700D",
    nc: 17,
    m: 144,
    ncp: 16,
    ns: 8,
    lowest: 18,
    pilots: &[-1, -1, 1, 1, -1, -1, -1, 1, -1, 1, -1, 1, 1, 1, 1, 1, 1, 1, 1],
    uw_bits: 10,
    text_bits: 4,
    interleave: 71,
    checks: &FREEDV_700D_224_112,
    timing_window: 32,
    uw_limit: 2,
    track_limit_hz: None,
    narrow_when_synced: true,
    rule: SyncRule::Counted,
    tx: TxShape {
        scale: 245_000.0 / 32_768.0,
        clip_gain: 2.5,
        cut_hz: 750.0,
    },
};

/// FreeDV 700E: 21 carriers of 71.4 Hz, frames of 80 ms, the (112,56) code.
pub static W700E: Waveform = Waveform {
    name: "700E",
    nc: 21,
    m: 112,
    ncp: 48,
    ns: 4,
    lowest: 10,
    pilots: &[0, -1, 1, 1, -1, -1, -1, 1, -1, 1, -1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, 1, 0],
    uw_bits: 12,
    text_bits: 2,
    interleave: 37,
    checks: &FREEDV_700E_112_56,
    timing_window: 80,
    uw_limit: 3,
    track_limit_hz: Some(1.0),
    narrow_when_synced: false,
    rule: SyncRule::FirstFrame,
    tx: TxShape {
        scale: 155_000.0 / 32_768.0,
        clip_gain: 3.0,
        cut_hz: 1000.0,
    },
};

impl Waveform {
    /// Samples per modem frame.
    pub const fn frame(&self) -> usize {
        self.ns * S
    }

    /// Data symbols per modem frame.
    pub const fn data_symbols(&self) -> usize {
        (self.ns - 1) * self.nc
    }

    /// Data bits per modem frame.
    pub const fn k(&self) -> usize {
        self.checks.len()
    }

    /// Seconds per modem frame.
    pub fn frame_s(&self) -> f64 {
        self.frame() as f64 / RATE
    }

    /// The frequency of carrier `c`, Hz above the dial frequency.
    pub fn carrier_hz(&self, c: usize) -> f64 {
        (self.lowest + c) as f64 * RATE / self.m as f64
    }

    /// The LDPC code of the waveform.
    pub fn code(&self) -> LdpcCode {
        let rows: Vec<&[u8]> = self.checks.iter().map(|r| r.as_slice()).collect();
        LdpcCode::from_accumulator(self.name, self.k(), &rows).expect("the table is a valid code")
    }

    /// What 10 log10 adds to Es/N0 for the SNR in 3000 Hz.
    fn snr_offset_db(&self) -> f32 {
        let band = self.nc as f32 * (RATE as f32 / self.m as f32) / 3000.0;
        10.0 * band.log10() + 10.0 * (S as f32 / self.m as f32).log10()
    }
}

/// Where the data symbols of a modem frame go, in data-symbol order (data row, then carrier).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The unique word symbols.
    pub uw: Vec<usize>,
    /// The payload symbols, in increasing order.
    pub payload: Vec<usize>,
    /// The text symbols (the last of the frame).
    pub text: Vec<usize>,
}

impl Layout {
    /// The layout of `w`: unique word symbol `i` is at `floor((i + 1) × step / 2)` with `step = nc + 1`, or `nc − 1` when the
    /// last one would not fit; the text symbols are the last; the payload takes the rest.
    pub fn of(w: &Waveform) -> Layout {
        let d = w.data_symbols();
        let n_uw = w.uw_bits / 2;
        let n_text = w.text_bits / 2;
        let step = if n_uw * (w.nc + 1) / 2 >= d { w.nc - 1 } else { w.nc + 1 };
        let uw: Vec<usize> = (0..n_uw).map(|i| (i + 1) * step / 2).collect();
        let text: Vec<usize> = (d - n_text..d).collect();
        let payload = (0..d).filter(|i| !uw.contains(i) && !text.contains(i)).collect();
        Layout { uw, payload, text }
    }
}

/// The QPSK point of the bit pair `(b0, b1)`: 00 is +1, 01 is +j, 10 is −j, 11 is −1.
pub fn qpsk(b0: u8, b1: u8) -> Complex32 {
    match (b0 & 1, b1 & 1) {
        (0, 0) => Complex32::new(1.0, 0.0),
        (0, _) => Complex32::new(0.0, 1.0),
        (_, 0) => Complex32::new(0.0, -1.0),
        _ => Complex32::new(-1.0, 0.0),
    }
}

/// The hard decision of a QPSK symbol: the bit pair of the nearest point.
pub fn hard_pair(s: Complex32) -> (u8, u8) {
    let u = s * Complex32::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2);
    ((u.im <= 0.0) as u8, (u.re <= 0.0) as u8)
}

/// The test-frame data bits: `x0 = 1`, `x(i+1) = (1103515245 x(i) + 12345) mod 32768`, bit `i` is one when
/// `x(i+1) > 16384`. The same `k` bits in every frame.
pub fn test_frame_bits(k: usize) -> Vec<u8> {
    let mut x = 1u64;
    (0..k)
        .map(|_| {
            x = (1_103_515_245 * x + 12_345) % 32_768;
            (x > 16_384) as u8
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------------
// Modulator.

/// The modulator of one waveform.
#[derive(Debug)]
pub struct Modulator {
    w: &'static Waveform,
    layout: Layout,
    code: LdpcCode,
}

impl Modulator {
    /// The modulator of `w`.
    pub fn new(w: &'static Waveform) -> Modulator {
        Modulator {
            w,
            layout: Layout::of(w),
            code: w.code(),
        }
    }

    /// The data symbols of one modem frame in data-symbol order: the unique word (+1), the code word of `data` (`k` bits)
    /// mapped to QPSK with code-word symbol `i` at payload position `(b × i) mod N`, and the symbols of `text`.
    pub fn frame_symbols(&self, data: &[u8], text: &[u8]) -> Vec<Complex32> {
        let w = self.w;
        let k = w.k();
        let mut cw = vec![0u8; 2 * k];
        self.code
            .encode(&data[..k], &mut cw)
            .expect("the code has an encoder");
        let mut out = vec![Complex32::new(1.0, 0.0); w.data_symbols()];
        for i in 0..k {
            out[self.layout.payload[(w.interleave * i) % k]] = qpsk(cw[2 * i], cw[2 * i + 1]);
        }
        for (j, &at) in self.layout.text.iter().enumerate() {
            let b = |i: usize| text.get(i).copied().unwrap_or(0);
            out[at] = qpsk(b(2 * j), b(2 * j + 1));
        }
        out
    }

    /// Appends the samples of one modem frame: the pilot row, then the data rows of `symbols`. A symbol is
    /// `x[n] = (1/M) Σ X[c] exp(j 2π (L + c) n / M)` with its last `ncp` samples copied in front. The output is complex with
    /// positive frequencies only; its real part is the audio.
    pub fn synthesize(&self, symbols: &[Complex32], out: &mut Vec<Complex32>) {
        let w = self.w;
        let mut row = vec![Complex32::new(0.0, 0.0); w.nc + 2];
        for r in 0..w.ns {
            for (c, x) in row.iter_mut().enumerate() {
                *x = if r == 0 {
                    Complex32::new(w.pilots[c] as f32, 0.0)
                } else if c >= 1 && c <= w.nc {
                    symbols[(r - 1) * w.nc + c - 1]
                } else {
                    Complex32::new(0.0, 0.0)
                };
            }
            push_symbol(w, &row, out);
        }
    }

    /// Appends one pilot row alone: the end of a transmission, so the receiver has the pilots after the last frame.
    pub fn pilot_row(&self, out: &mut Vec<Complex32>) {
        let row: Vec<Complex32> = self.w.pilots.iter().map(|&p| Complex32::new(p as f32, 0.0)).collect();
        push_symbol(self.w, &row, out);
    }
}

/// Appends the symbol with the carrier values `row`: its body with the cyclic prefix in front.
fn push_symbol(w: &Waveform, row: &[Complex32], out: &mut Vec<Complex32>) {
    let start = out.len();
    out.resize(start + S, Complex32::new(0.0, 0.0));
    let body = synth_body(w, row);
    out[start + w.ncp..start + S].copy_from_slice(&body);
    out[start..start + w.ncp].copy_from_slice(&body[w.m - w.ncp..]);
}

/// The `m` body samples of the symbol with the carrier values `row`.
fn synth_body(w: &Waveform, row: &[Complex32]) -> Vec<Complex32> {
    (0..w.m)
        .map(|n| {
            let mut v = Complex32::new(0.0, 0.0);
            for (c, &x) in row.iter().enumerate() {
                let turn = ((w.lowest + c) * n % w.m) as f32 / w.m as f32;
                v += x * Complex32::from_polar(1.0, TAU * turn);
            }
            v / w.m as f32
        })
        .collect()
}

/// The magnitude that the clipper of the transmit chain limits to: half of full scale.
pub const TX_LIMIT: f32 = 0.5;

/// The transmit chain of a waveform, sample by sample: the amplitude scale ([`TxShape::scale`]); with the clipper the drive
/// gain and a limit to [`TX_LIMIT`]; with the transmit filter a band-pass around 1500 Hz (a low-pass with its −6 dB point at
/// [`TxShape::cut_hz`], moved up); with the clipper again a gain of 0.8 and the limit. The filter delays the signal by
/// [`TxChain::delay`] samples.
pub struct TxChain {
    shape: TxShape,
    clip: bool,
    filter: Option<ComplexFir>,
    delay: usize,
}

impl TxChain {
    /// The chain of `shape` with the clipper and the transmit filter on or off.
    pub fn new(shape: TxShape, clip: bool, filter: bool) -> TxChain {
        let (filter, delay) = if filter {
            let taps = sdrkit_algo_dsp::fir::lowpass(RATE, shape.cut_hz - 125.0, shape.cut_hz + 125.0, 50.0)
                .expect("a valid filter");
            let d = taps.len() / 2;
            let shifted: Vec<Complex32> = taps
                .iter()
                .enumerate()
                .map(|(k, &h)| {
                    let turn = (1500.0 * (k as f64 - d as f64) / RATE).rem_euclid(1.0);
                    Complex32::from_polar(h, TAU * turn as f32)
                })
                .collect();
            (Some(ComplexFir::new(&shifted)), d)
        } else {
            (None, 0)
        };
        TxChain {
            shape,
            clip,
            filter,
            delay,
        }
    }

    /// The nominal peak of the output: [`TX_LIMIT`], times 0.8 with the clipper.
    pub fn peak(&self) -> f32 {
        if self.clip {
            0.8 * TX_LIMIT
        } else {
            TX_LIMIT
        }
    }

    /// The delay of the transmit filter, samples (0 without it).
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Appends the output of the chain for the modulator samples `x` to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        let limit = |s: Complex32| {
            let m = s.norm();
            if m > TX_LIMIT {
                s * (TX_LIMIT / m)
            } else {
                s
            }
        };
        let shape = self.shape;
        for &s in x {
            let mut y = s * shape.scale;
            if self.clip {
                y = limit(y * shape.clip_gain);
            }
            if let Some(f) = self.filter.as_mut() {
                y = f.process_sample(y);
            }
            if self.clip {
                y = limit(y * 0.8);
            }
            out.push(y);
        }
    }
}

/// The clipper and the transmit filter on `x` in place ([`TxChain`] with both on, its delay removed).
pub fn clip(w: &'static Waveform, x: &mut [Complex32]) {
    let mut chain = TxChain::new(w.tx, true, true);
    let d = chain.delay();
    let mut y = Vec::with_capacity(x.len() + d);
    chain.process(x, &mut y);
    chain.process(&vec![Complex32::new(0.0, 0.0); d], &mut y);
    x.copy_from_slice(&y[d..]);
}

/// What a test signal holds.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions<'a> {
    /// The text of the text channel; a carriage return follows it and it repeats.
    pub text: &'a str,
    /// Seconds of frames.
    pub seconds: f64,
    /// The clipper and transmit filter are on.
    pub clip: bool,
}

/// The test signal of `w` at 8 kHz (complex, the upper sideband above the dial frequency, unit power while it sends): 0.3 s
/// of silence, modem frames with the test-frame data bits and the text, 0.5 s of silence.
pub fn render(w: &'static Waveform, o: &RenderOptions) -> Vec<Complex32> {
    let m = Modulator::new(w);
    let data = test_frame_bits(w.k());
    let mut text_bits = crate::freedv::text_bits(o.text);
    if text_bits.is_empty() {
        text_bits.push(0);
    }
    let frames = (o.seconds / w.frame_s()).round().max(1.0) as usize;
    let mut sig = Vec::with_capacity(frames * w.frame());
    let mut at = 0;
    for _ in 0..frames {
        let text: Vec<u8> = (0..w.text_bits)
            .map(|i| text_bits[(at + i) % text_bits.len()])
            .collect();
        at += w.text_bits;
        m.synthesize(&m.frame_symbols(&data, &text), &mut sig);
    }
    if o.clip {
        clip(w, &mut sig);
    }
    let power = sig.iter().map(|s| s.norm_sqr()).sum::<f32>() / sig.len() as f32;
    let gain = 1.0 / power.sqrt();
    let mut out = vec![Complex32::new(0.0, 0.0); (0.3 * RATE) as usize];
    out.extend(sig.iter().map(|s| s * gain));
    out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), (0.5 * RATE) as usize));
    out
}

// ---------------------------------------------------------------------------------------------------------------------------
// Sync.

/// The sync state of a waveform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    /// Looking for two pilots one frame apart.
    Search,
    /// A timing was found; the unique word decides.
    Trial,
    /// The unique word confirmed the frames.
    Synced,
}

impl SyncState {
    /// The name the results show.
    pub fn key(self) -> &'static str {
        match self {
            SyncState::Search => "search",
            SyncState::Trial => "trial",
            SyncState::Synced => "synced",
        }
    }
}

/// How the unique word moves the sync state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncRule {
    /// 700D. Trial: two bad frames go back to search, four good frames in a row sync. Synced: more than six bad frames in a
    /// row go back to search.
    Counted,
    /// 700E. Trial: the first frame decides. Synced: six bad frames in a row go back to search.
    FirstFrame,
}

/// The sync state machine: a function of the unique word error count of each frame.
#[derive(Clone, Copy, Debug)]
pub struct SyncMachine {
    rule: SyncRule,
    uw_limit: usize,
    state: SyncState,
    good: u32,
    bad: u32,
}

impl SyncMachine {
    /// The machine of `w`, searching.
    pub fn new(w: &Waveform) -> SyncMachine {
        SyncMachine {
            rule: w.rule,
            uw_limit: w.uw_limit,
            state: SyncState::Search,
            good: 0,
            bad: 0,
        }
    }

    /// The state.
    pub fn state(&self) -> SyncState {
        self.state
    }

    /// A coarse search found a valid timing: search becomes trial.
    pub fn found(&mut self) {
        self.state = SyncState::Trial;
        self.good = 0;
        self.bad = 0;
    }

    /// One frame in trial or synced with `uw_errors` bit errors in its unique word. Returns the new state.
    pub fn frame(&mut self, uw_errors: usize) -> SyncState {
        let is_bad = uw_errors > self.uw_limit;
        match (self.rule, self.state) {
            (_, SyncState::Search) => {}
            (SyncRule::Counted, SyncState::Trial) => {
                self.good += 1;
                if is_bad {
                    self.bad += 1;
                    self.good = 0;
                }
                if self.bad >= 2 {
                    self.state = SyncState::Search;
                } else if self.good >= 4 {
                    self.state = SyncState::Synced;
                }
            }
            (SyncRule::Counted, SyncState::Synced) => {
                self.bad = if is_bad { self.bad + 1 } else { 0 };
                if self.bad > 6 {
                    self.state = SyncState::Search;
                }
            }
            (SyncRule::FirstFrame, SyncState::Trial) => {
                self.bad = 0;
                self.state = if is_bad { SyncState::Search } else { SyncState::Synced };
            }
            (SyncRule::FirstFrame, SyncState::Synced) => {
                self.bad = if is_bad { self.bad + 1 } else { 0 };
                if self.bad >= 6 {
                    self.state = SyncState::Search;
                }
            }
        }
        self.state
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Demodulator.

/// Keeps the upper sideband of the 8 kHz channel (0 Hz is the dial frequency): 400 Hz to 3600 Hz pass, the lower sideband
/// does not. The demodulators take its output, which is the audio of an upper-sideband receiver as a complex signal.
#[derive(Debug)]
pub struct UpperSideband {
    fir: Fir<Complex32>,
    n: usize,
}

impl Default for UpperSideband {
    fn default() -> Self {
        let taps = sdrkit_algo_dsp::fir::lowpass(RATE, 1600.0, 2000.0, 50.0).expect("a valid filter");
        UpperSideband {
            fir: Fir::new(&taps),
            n: 0,
        }
    }
}

impl UpperSideband {
    /// Appends the upper sideband of `x` to `out`: the band is moved down by 2000 Hz, low-pass filtered and moved back.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        const J: [Complex32; 4] = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, 1.0),
            Complex32::new(-1.0, 0.0),
            Complex32::new(0.0, -1.0),
        ];
        for &s in x {
            let up = J[self.n];
            out.push(self.fir.process_sample(s * up.conj()) * up);
            self.n = (self.n + 1) & 3;
        }
    }
}

/// One step of a demodulator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The sync state after the step.
    pub sync: SyncState,
    /// The step demodulated a modem frame ([`OfdmDemod::frame`]).
    pub frame: bool,
    /// The input samples the step took.
    pub samples: usize,
}

/// A demodulated modem frame.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// The data bits after the LDPC decoder.
    pub data: Vec<u8>,
    /// The text bits.
    pub text: Vec<u8>,
    /// Bit errors in the unique word.
    pub uw_errors: usize,
    /// The LDPC decoder satisfied every check.
    pub ldpc_ok: bool,
    /// Code-word bits the decoder changed.
    pub corrected: usize,
}

/// The demodulator component of one waveform: coarse search over timing and −60 Hz to +59 Hz, fine timing, frequency
/// tracking, the channel estimate from the pilots, soft decisions, de-interleaving, the LDPC decoder, the unique word and the
/// sync state machine. Input: the upper sideband at 8 kHz as a complex signal ([`UpperSideband`]); one modem frame a step.
pub struct OfdmDemod {
    w: &'static Waveform,
    layout: Layout,
    bp: BpDecoder,
    machine: SyncMachine,
    /// One pilot symbol with a zero prefix, and its energy.
    template: Vec<Complex32>,
    template_energy: f32,
    /// `exp(−j 2π (L + c) n / M)` for each carrier `c` and body sample `n`.
    dft: Vec<Complex32>,
    /// The latest three frames and three symbols.
    buf: Vec<Complex32>,
    pending: Vec<Complex32>,
    /// The samples the next step takes.
    nin: usize,
    shifted: Vec<Complex32>,
    window: Vec<Complex32>,
    /// The pilot rows one frame before, of this frame, one and two frames after.
    pilots: [Vec<Complex32>; 4],
    rows: Vec<Complex32>,
    h: Vec<Complex32>,
    sym: Vec<Complex32>,
    amp: Vec<f32>,
    llr: Vec<f32>,
    word: Vec<u8>,
    frame: Frame,
    freq_hz: f32,
    timing: i32,
    sample_point: i32,
    mean_amp: f32,
    snr_db: f32,
    timing_metric: f32,
    drift: i64,
    drift_frames: u64,
    /// Modem frames demodulated with sync.
    pub frames: u64,
    /// Unique word bit errors, and bits, since the last reset of the counts.
    pub uw_errors: u64,
    pub uw_bits: u64,
    /// Frames whose LDPC decoder did not satisfy every check.
    pub ldpc_failed: u64,
}

impl OfdmDemod {
    /// The demodulator of `w`, searching.
    pub fn new(w: &'static Waveform) -> OfdmDemod {
        let code = w.code();
        let carriers = w.nc + 2;
        let row: Vec<Complex32> = w
            .pilots
            .iter()
            .map(|&p| Complex32::new(p as f32, 0.0))
            .collect();
        let mut template = vec![Complex32::new(0.0, 0.0); S];
        template[w.ncp..].copy_from_slice(&synth_body(w, &row));
        let template_energy = template.iter().map(|s| s.norm_sqr()).sum();
        let mut dft = Vec::with_capacity(carriers * w.m);
        for c in 0..carriers {
            for n in 0..w.m {
                let turn = ((w.lowest + c) * n % w.m) as f32 / w.m as f32;
                dft.push(Complex32::from_polar(1.0, -TAU * turn));
            }
        }
        let f = w.frame();
        let zero = Complex32::new(0.0, 0.0);
        OfdmDemod {
            w,
            layout: Layout::of(w),
            bp: BpDecoder::new(&code),
            machine: SyncMachine::new(w),
            template,
            template_energy,
            dft,
            buf: vec![zero; 3 * f + 3 * S],
            pending: Vec::with_capacity(4 * f),
            nin: f,
            shifted: vec![zero; S],
            window: vec![zero; w.m],
            pilots: std::array::from_fn(|_| vec![zero; carriers]),
            rows: vec![zero; (w.ns - 1) * carriers],
            h: vec![zero; carriers],
            sym: vec![zero; w.data_symbols()],
            amp: vec![0.0; w.data_symbols()],
            llr: vec![0.0; 2 * w.k()],
            word: vec![0; 2 * w.k()],
            frame: Frame {
                data: vec![0; w.k()],
                text: vec![0; w.text_bits],
                ..Frame::default()
            },
            freq_hz: 0.0,
            timing: 0,
            sample_point: 0,
            mean_amp: 0.0,
            snr_db: -5.0,
            timing_metric: 0.0,
            drift: 0,
            drift_frames: 0,
            frames: 0,
            uw_errors: 0,
            uw_bits: 0,
            ldpc_failed: 0,
        }
    }

    /// The waveform.
    pub fn waveform(&self) -> &'static Waveform {
        self.w
    }

    /// The sync state.
    pub fn sync(&self) -> SyncState {
        self.machine.state()
    }

    /// The frame of the last step that demodulated one.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The SNR in 3000 Hz, dB: −5 while searching.
    pub fn snr_db(&self) -> f32 {
        self.snr_db
    }

    /// The frequency estimate, Hz, relative to the 1500 Hz centre.
    pub fn freq_offset_hz(&self) -> f32 {
        self.freq_hz
    }

    /// The last timing metric (0 to about 1).
    pub fn timing_metric(&self) -> f32 {
        self.timing_metric
    }

    /// The sample clock offset since the last coarse search, parts per million.
    pub fn clock_offset_ppm(&self) -> f32 {
        if self.drift_frames == 0 {
            return 0.0;
        }
        (self.drift as f64 / (self.drift_frames as f64 * self.w.frame() as f64) * 1e6) as f32
    }

    /// Sets the counts of frames and unique word errors to zero.
    pub fn reset_counts(&mut self) {
        self.frames = 0;
        self.uw_errors = 0;
        self.uw_bits = 0;
        self.ldpc_failed = 0;
    }

    /// Adds input samples.
    pub fn feed(&mut self, x: &[Complex32]) {
        self.pending.extend_from_slice(x);
    }

    /// Runs one step when enough input waits: a coarse search while searching, else one modem frame. `None` when the
    /// component needs more input.
    pub fn step(&mut self) -> Option<Step> {
        let n = self.nin;
        if self.pending.len() < n {
            return None;
        }
        let len = self.buf.len();
        self.buf.copy_within(n.., 0);
        self.buf[len - n..].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        let frame = if self.machine.state() == SyncState::Search {
            self.search();
            false
        } else {
            self.demodulate();
            true
        };
        Some(Step {
            sync: self.machine.state(),
            frame,
            samples: n,
        })
    }

    /// The conjugate template moved up by `hz`, in `shifted`.
    fn shift_template(&mut self, hz: f32) {
        let step = Complex32::from_polar(1.0, -TAU * hz / RATE as f32);
        let mut turn = Complex32::new(1.0, 0.0);
        for (o, t) in self.shifted.iter_mut().zip(&self.template) {
            *o = t.conj() * turn;
            turn *= step;
        }
    }

    /// The correlation of the input at `at` with the shifted template.
    fn correlate(&self, at: usize) -> Complex32 {
        let ncp = self.w.ncp;
        let x = &self.buf[at + ncp..at + S];
        let t = &self.shifted[ncp..];
        let mut acc = Complex32::new(0.0, 0.0);
        for (a, b) in x.iter().zip(t) {
            acc += a * b;
        }
        acc
    }

    /// `|C(at)| + |C(at + F)|`: two pilots one frame apart against the shifted template.
    fn pair(&self, at: usize) -> f32 {
        self.correlate(at).norm() + self.correlate(at + self.w.frame()).norm()
    }

    /// The scale that makes [`OfdmDemod::pair`] the timing metric, from the mean power of `buf[from..from + len]`. The input
    /// is one-sided, so its real part (the audio) has half the power and half the correlation. 0 when there is no input.
    fn metric_scale(&self, from: usize, len: usize) -> f32 {
        let power = self.buf[from..from + len].iter().map(|s| s.norm_sqr()).sum::<f32>() / len as f32;
        let d = 2.0 * (2.0 * S as f32 * self.template_energy * power).sqrt();
        if d > 0.0 && d.is_finite() {
            1.0 / d
        } else {
            0.0
        }
    }

    /// The coarse search: the best timing in steps of 2 samples over one frame for the offsets −40, 0 and +40 Hz, then the
    /// frequency in steps of 1 Hz within −20 Hz to +19 Hz of the best. A valid timing starts the trial.
    fn search(&mut self) {
        let f = self.w.frame();
        let base = S + f;
        self.snr_db = -5.0;
        self.nin = f;
        let scale = self.metric_scale(base, 2 * f + S);
        let mut best = (0.0f32, 0usize, 0.0f32);
        if scale > 0.0 {
            for hz in [-40.0f32, 0.0, 40.0] {
                self.shift_template(hz);
                for t in (0..f).step_by(2) {
                    let v = self.pair(base + t);
                    if v > best.0 {
                        best = (v, t, hz);
                    }
                }
            }
        }
        let (peak, t, coarse) = best;
        self.timing_metric = peak * scale;
        let valid = self.timing_metric > TIMING_THRESHOLD && self.buf[base + t].norm_sqr() > 0.0;
        if !valid {
            return;
        }
        let mut fine = (0.0f32, 0.0f32);
        for d in -20..20 {
            let hz = coarse + d as f32;
            self.shift_template(hz);
            let v = self.pair(base + t);
            if v > fine.0 {
                fine = (v, hz);
            }
        }
        self.nin = t;
        self.timing = 0;
        self.sample_point = 0;
        self.freq_hz = fine.1;
        self.drift = 0;
        self.drift_frames = 0;
        self.machine.found();
    }

    /// The fine timing: the best of `timing_window` starts around the timing estimate, which then moves there.
    fn fine_timing(&mut self) {
        let w = self.w;
        let f = w.frame();
        let centre = w.timing_window.div_ceil(2) as i32 - 1;
        let start = ((S + f) as i32 + self.timing - centre) as usize;
        self.shift_template(self.freq_hz);
        let scale = self.metric_scale(start, f + S + w.timing_window - 1);
        let mut best = (0.0f32, centre);
        for j in 0..w.timing_window {
            let v = self.pair(start + j);
            if v > best.0 {
                best = (v, j as i32);
            }
        }
        self.timing_metric = best.0 * scale;
        let moved = best.1 - centre;
        self.timing += moved;
        self.drift += moved as i64;
        self.drift_frames += 1;
        self.sample_point = self
            .sample_point
            .clamp(self.timing + 4, self.timing + w.ncp as i32 - 4);
    }

    /// The carrier values of the symbol whose prefix starts at `at`, frequency corrected, into `out`.
    fn carriers(&mut self, at: usize, row: Row) {
        let w = self.w;
        let from = (at as i32 + self.sample_point) as usize;
        let step = Complex32::from_polar(1.0, -TAU * self.freq_hz / RATE as f32);
        let mut turn = Complex32::from_polar(
            1.0,
            -TAU * (self.freq_hz * from as f32 / RATE as f32).fract(),
        );
        for (o, s) in self.window.iter_mut().zip(&self.buf[from..from + w.m]) {
            *o = s * turn;
            turn *= step;
        }
        let carriers = w.nc + 2;
        let out = match row {
            Row::Pilot(i) => &mut self.pilots[i][..],
            Row::Data(r) => &mut self.rows[r * carriers..(r + 1) * carriers],
        };
        for (c, o) in out.iter_mut().enumerate() {
            let table = &self.dft[c * w.m..(c + 1) * w.m];
            let mut acc = Complex32::new(0.0, 0.0);
            for (a, b) in self.window.iter().zip(table) {
                acc += a * b;
            }
            *o = acc;
        }
    }

    /// One modem frame: timing, the carrier values, the channel estimate, the symbols, the unique word, the text, the soft
    /// decisions, the LDPC decoder, then the sync state, the SNR, the frequency tracking and the sample clock.
    fn demodulate(&mut self) {
        let w = self.w;
        let f = w.frame();
        let carriers = w.nc + 2;
        self.fine_timing();
        for (i, at) in [S, S + f, S + 2 * f, S + 3 * f].into_iter().enumerate() {
            self.carriers(at, Row::Pilot(i));
        }
        for r in 0..w.ns - 1 {
            self.carriers(S + f + (r + 1) * S, Row::Data(r));
        }
        // The channel of each data carrier.
        let narrow = w.narrow_when_synced && self.machine.state() == SyncState::Synced;
        let pilot = |row: &[Complex32], c: usize| row[c] * w.pilots[c] as f32;
        for c in 1..=w.nc {
            self.h[c] = if narrow {
                let mut acc = Complex32::new(0.0, 0.0);
                for row in &self.pilots {
                    acc += pilot(row, c - 1) + pilot(row, c) + pilot(row, c + 1);
                }
                acc / 12.0
            } else {
                (pilot(&self.pilots[1], c) + pilot(&self.pilots[2], c)) / 2.0
            };
        }
        // Phase correction and amplitudes.
        let mut amp_sum = 0.0;
        for r in 0..w.ns - 1 {
            for k in 0..w.nc {
                let h = self.h[k + 1];
                let a = h.norm();
                let y = self.rows[r * carriers + k + 1];
                let s = r * w.nc + k;
                self.sym[s] = if a > 0.0 { y * h.conj() / a } else { Complex32::new(0.0, 0.0) };
                self.amp[s] = a;
                amp_sum += a;
            }
        }
        let mean = amp_sum / w.data_symbols() as f32;
        // The first frame sets the mean amplitude; later frames move it slowly.
        self.mean_amp = if self.mean_amp > 0.0 {
            0.9 * self.mean_amp + 0.1 * mean
        } else {
            mean
        };
        // Unique word and text.
        let mut uw_errors = 0;
        for &at in &self.layout.uw {
            let (a, b) = hard_pair(self.sym[at]);
            uw_errors += (a + b) as usize;
        }
        for (j, &at) in self.layout.text.iter().enumerate() {
            let (a, b) = hard_pair(self.sym[at]);
            self.frame.text[2 * j] = a;
            self.frame.text[2 * j + 1] = b;
        }
        self.frame.uw_errors = uw_errors;
        self.decode();
        self.frames += 1;
        self.uw_errors += uw_errors as u64;
        self.uw_bits += w.uw_bits as u64;
        if !self.frame.ldpc_ok {
            self.ldpc_failed += 1;
        }
        self.measure_snr();
        // Frequency tracking from the turn between this pilot row and the next.
        let mut turn = Complex32::new(0.0, 0.0);
        for (a, b) in self.pilots[1].iter().zip(&self.pilots[2]) {
            turn += a.conj() * b;
        }
        if turn.norm_sqr() > 0.0 {
            let mut e = turn.arg() * (RATE as f32 / w.m as f32) / (TAU * w.ns as f32);
            if let Some(limit) = w.track_limit_hz {
                e = e.clamp(-limit, limit);
            }
            self.freq_hz += TRACK_GAIN * e;
        }
        if self.machine.frame(uw_errors) == SyncState::Search {
            self.snr_db = -5.0;
        }
        // The sample clock: a timing estimate past an eighth of a symbol moves the next step by a quarter symbol.
        let quarter = (S / 4) as i32;
        self.nin = f;
        if self.timing > quarter / 2 {
            self.nin = f + quarter as usize;
            self.timing -= quarter;
            self.sample_point -= quarter;
        } else if self.timing < -quarter / 2 {
            self.nin = f - quarter as usize;
            self.timing += quarter;
            self.sample_point += quarter;
        }
    }

    /// Soft decisions of the de-interleaved payload symbols, and the LDPC decoder. A frame without energy fails.
    fn decode(&mut self) {
        let w = self.w;
        let k = w.k();
        let m = self.mean_amp;
        let points = [qpsk(0, 0), qpsk(0, 1), qpsk(1, 0), qpsk(1, 1)];
        let mut energy = false;
        for i in 0..k {
            let at = self.layout.payload[(w.interleave * i) % k];
            let (r, a) = (self.sym[at], self.amp[at]);
            let metric = points.map(|p| {
                let d = if m > 0.0 { (r - p * a) / m } else { Complex32::new(0.0, 0.0) };
                -SOFT_ES_N0 * d.norm_sqr()
            });
            let l0 = max_star(metric[0], metric[1]) - max_star(metric[2], metric[3]);
            let l1 = max_star(metric[0], metric[2]) - max_star(metric[1], metric[3]);
            for (j, l) in [l0, l1].into_iter().enumerate() {
                let l = if l.is_finite() { l.clamp(-LLR_LIMIT, LLR_LIMIT) } else { 0.0 };
                energy |= l != 0.0;
                self.llr[2 * i + j] = l;
                self.word[2 * i + j] = (l < 0.0) as u8;
            }
        }
        let decoded = self.bp.decode(&self.llr, MAX_ITERATIONS, &mut self.word);
        self.frame.ldpc_ok = energy && decoded.is_ok();
        self.frame.corrected = decoded.map_or(0, |d| d.corrected as usize);
        self.frame.data.copy_from_slice(&self.word[..k]);
    }

    /// The SNR in 3000 Hz from the scatter of the strong symbols across their decision axis. A higher value replaces the held
    /// one at once; a lower value moves it by a tenth.
    fn measure_snr(&mut self) {
        let n = self.sym.len() as f32;
        let signal = self.sym.iter().map(|s| s.norm_sqr()).sum::<f32>() / n;
        let sigma = signal.sqrt();
        let (mut count, mut sum, mut squares) = (0.0f32, 0.0f32, 0.0f32);
        for s in &self.sym {
            if s.norm() > sigma {
                let x = if s.re.abs() > s.im.abs() { s.im } else { s.re };
                count += 1.0;
                sum += x;
                squares += x * x;
            }
        }
        let noise = if count > 1.0 {
            2.0 * (count * squares - sum * sum) / (count * (count - 1.0))
        } else {
            signal
        };
        let es_n0 = 10.0 * (signal / noise).log10();
        let new = es_n0 + self.w.snr_offset_db();
        // No energy, or no measurable noise, reads as the search value.
        let new = if new.is_finite() { new } else { -5.0 };
        self.snr_db = if new > self.snr_db {
            new
        } else {
            0.9 * self.snr_db + 0.1 * new
        };
    }
}

/// Which carrier row a transform fills.
#[derive(Clone, Copy)]
enum Row {
    Pilot(usize),
    Data(usize),
}

/// `ln(e^x + e^y)` by the linear correction of the larger value.
fn max_star(x: f32, y: f32) -> f32 {
    let d = (x - y).abs();
    let m = x.max(y);
    if d < 2.506_817_4 {
        m + 0.249_041_63 * (2.506_817_4 - d)
    } else {
        m
    }
}

/// Whether the test signal has the clipper and the transmit filter on.
pub const GEN_CLIP: sdrkit_core::Setting<sdrkit_core::publish::field::Bool> =
    sdrkit_core::Setting::new("gen_clip", "Clipper", "", false)
        .section("Test signal")
        .advanced()
        .help("The 700D and 700E test signals pass the clipper and the transmit filter.");

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS_700D: [&str; 7] = [
        "-j -j -1 -1 -j +1 +j -j -j +1 +j -j +1 -1 -j +j -j",
        "-j +1 +1 +1 +j +j -j +1 +j +j +1 -j -j -j +j -j -1",
        "-j -j +1 +j -j +1 -1 -j -j -j -1 +1 +j +j -j -j -1",
        "-1 -j -1 -1 -1 -j -1 +j +1 -1 +1 +1 +j -j +j +j +j",
        "-1 -j +j +1 -j -1 +1 -1 -j +j -j +j +j -j -j -j +j",
        "+j -1 -1 +j -j -j -1 +j +j -j +j -1 +j +1 +1 -j +j",
        "+j -j -1 -j +1 +j -1 +j +1 +1 -j +1 +1 -j +j -j -1",
    ];
    const ROWS_700E: [&str; 3] = [
        "-j -j -j +j -1 +j +1 +j -1 +j +1 +1 +j -j +j -j +1 -j +j -1 +1",
        "+j +j -1 +j -1 +j +j +1 +1 +1 +1 -j +1 +1 -j -j -j -1 +j +1 -j",
        "-1 +1 -j +1 +j -j -j +1 +1 -j -j +j +1 -1 +1 -1 +j +j +1 +1 -j",
    ];

    fn points(rows: &[&str]) -> Vec<Complex32> {
        rows.iter()
            .flat_map(|r| r.split_whitespace())
            .map(|s| match s {
                "+1" => qpsk(0, 0),
                "+j" => qpsk(0, 1),
                "-j" => qpsk(1, 0),
                _ => qpsk(1, 1),
            })
            .collect()
    }

    #[test]
    fn the_layouts_are_those_of_the_specification() {
        let d = Layout::of(&W700D);
        assert_eq!(d.uw, [9, 18, 27, 36, 45]);
        assert_eq!(d.text, [117, 118]);
        assert_eq!(d.payload.len(), 112);
        let e = Layout::of(&W700E);
        assert_eq!(e.uw, [10, 20, 30, 40, 50, 60]);
        assert_eq!(e.text, [62]);
        assert_eq!(e.payload.len(), 56);
        assert_eq!((W700D.frame(), W700E.frame()), (1280, 640));
        assert_eq!((W700D.pilots.len(), W700E.pilots.len()), (19, 23));
    }

    #[test]
    fn the_modulator_gives_the_worked_frames_symbol_for_symbol() {
        for (w, rows, text) in [
            (&W700D, &ROWS_700D[..], &[1u8, 0, 1, 1][..]),
            (&W700E, &ROWS_700E[..], &[1, 0][..]),
        ] {
            let m = Modulator::new(w);
            let got = m.frame_symbols(&test_frame_bits(w.k()), text);
            assert_eq!(got, points(rows), "{}", w.name);
        }
    }

    #[test]
    fn hard_decisions_invert_the_mapping() {
        for b in 0..4u8 {
            assert_eq!(hard_pair(qpsk(b >> 1, b & 1)), (b >> 1, b & 1));
        }
    }

    #[test]
    fn the_sync_machine_of_700d_counts_as_specified() {
        let mut m = SyncMachine::new(&W700D);
        m.found();
        // Three good frames, a bad one, then four good frames in a row.
        for e in [0, 2, 1, 3, 0, 0, 0] {
            assert_eq!(m.frame(e), SyncState::Trial);
        }
        assert_eq!(m.frame(0), SyncState::Synced);
        // The bad frame of the trial still counts: five more keep sync, and a good frame clears the count.
        for _ in 0..5 {
            assert_eq!(m.frame(3), SyncState::Synced);
        }
        assert_eq!(m.frame(0), SyncState::Synced);
        // Six bad frames keep sync, the seventh in a row loses it.
        for _ in 0..6 {
            assert_eq!(m.frame(3), SyncState::Synced);
        }
        assert_eq!(m.frame(2), SyncState::Synced);
        for _ in 0..6 {
            assert_eq!(m.frame(3), SyncState::Synced);
        }
        assert_eq!(m.frame(3), SyncState::Search);
        // Two bad frames in trial go back to search.
        m.found();
        assert_eq!(m.frame(3), SyncState::Trial);
        assert_eq!(m.frame(0), SyncState::Trial);
        assert_eq!(m.frame(10), SyncState::Search);
    }

    #[test]
    fn the_sync_machine_of_700e_counts_as_specified() {
        let mut m = SyncMachine::new(&W700E);
        m.found();
        assert_eq!(m.frame(4), SyncState::Search);
        m.found();
        assert_eq!(m.frame(3), SyncState::Synced);
        // Three errors of twelve are a good frame.
        for _ in 0..20 {
            assert_eq!(m.frame(3), SyncState::Synced);
        }
        for _ in 0..5 {
            assert_eq!(m.frame(4), SyncState::Synced);
        }
        assert_eq!(m.frame(4), SyncState::Search);
    }
}
