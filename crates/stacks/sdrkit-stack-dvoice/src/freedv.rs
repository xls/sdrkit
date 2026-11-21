//! FreeDV: the 1600 waveform (the FDMDV modulator and demodulator of sixteen DQPSK carriers and a BPSK pilot, and the frames
//! of 52 Codec 2 bits, 11 Golay parity bits and one text bit, read through the shared symbol layer), and the stack's part of
//! the OFDM waveforms 700D and 700E ([`crate::freedv_ofdm`]): which waveform is trusted, its text line and its speech frames.
//! The text channel of every waveform carries the PSK31 varicode.
//! Standard: FreeDV project documentation.

use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::freedv_ofdm::{OfdmDemod, RenderOptions, Step, SyncState, UpperSideband, Waveform, W700D, W700E};
use crate::gen::Prepared;
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::FreedvFields;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig};
use sdrkit_util_bits::pack::to_u64;
use sdrkit_util_text::varicode::psk as varicode;
use std::sync::LazyLock;

/// The demodulator's sample rate.
pub const AUDIO_RATE: f64 = 8000.0;
/// Samples per symbol.
pub const SPS: usize = 160;
/// The roll-off of the root raised cosine symbol shape, and its span in symbols.
pub const ROLL_OFF: f64 = 0.5;
const SPAN: usize = 6;
/// The taps of the symbol shape: a root raised cosine at [`SPS`] samples a symbol over [`SPAN`] symbols, unit energy. The
/// modulator shapes each carrier's symbols with it and the demodulator correlates each carrier with it (the matched filter).
pub static PULSE: LazyLock<Vec<f32>> = LazyLock::new(|| {
    sdrkit_algo_dsp::pulse::taps(
        sdrkit_algo_dsp::pulse::Shape::Rrc { beta: ROLL_OFF },
        SPS as f64,
        SPAN as f64,
        sdrkit_algo_dsp::pulse::Norm::Energy,
    )
    .expect("a valid pulse")
});
/// The samples of each carrier the demodulator keeps: one pulse and two symbols before it.
const RING: usize = SPAN * SPS + 1 + 2 * SPS;
/// A symbol timing at least this many samples from the one the timing search finds is beyond quick reach of the timing
/// loop: the timing jumps.
const FAR: u64 = 24;
/// The timing search tries a symbol boundary every this many samples.
const SEARCH_STEP: usize = 8;
const TIMINGS: usize = SPS / SEARCH_STEP;
/// The amplitude of a data carrier; the pilot has twice the power of one.
const DATA_AMP: f32 = 0.235_702_26; // 1/√18
const PILOT_AMP: f32 = DATA_AMP * std::f32::consts::SQRT_2;
/// Carriers: 16 data carriers and the pilot (index 8).
pub const CARRIERS: usize = 17;
pub const PILOT: usize = 8;
/// The marker the demodulator puts before each frame's bits (two-level).
pub const MARKER: &str = "1110010110000110";
/// The rate of the demodulator's output stream (bits and markers).
pub const STREAM_RATE: f64 = 2000.0;

/// Carrier frequency (Hz above the dial frequency).
pub fn carrier_hz(k: usize) -> f64 {
    1500.0 + (k as f64 - PILOT as f64) * 75.0
}

/// Dibit to DQPSK phase step (Gray: 00 0, 01 π/2, 11 π, 10 3π/2).
fn step_of(d: u8) -> f32 {
    std::f32::consts::FRAC_PI_2
        * match d {
            0 => 0.0,
            1 => 1.0,
            3 => 2.0,
            _ => 3.0,
        }
}

/// Golay (23,12) of the first 12 codec bits of a frame.
static GOLAY: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_23_12);

/// The FDMDV demodulator: IQ at 8 kHz in, the marker and soft bits of each frame out.
#[derive(Debug)]
pub struct FdmdvDemod {
    /// The matched filter output of each carrier at the last symbol.
    prev: [Complex32; CARRIERS],
    /// One oscillator per carrier, at minus its frequency (it brings the carrier to 0 Hz).
    carriers: [Nco; CARRIERS],
    /// The fine frequency correction.
    fine: Nco,
    /// The last [`RING`] samples of each carrier at 0 Hz, each written twice so that every window is in one piece.
    ring: Vec<Complex32>,
    /// Where the next sample goes, in `0..RING`.
    at: usize,
    n: usize,
    offset: i32,
    freq_corr: f64,
    half: Option<Vec<f32>>,
    /// The mean power of the pilot's matched filter output, over about ten symbols.
    level: f32,
    /// Samples taken.
    taken: u64,
    /// The score of each timing of the search, and its weight.
    score: [Complex32; TIMINGS],
    weight: [f32; TIMINGS],
    /// Frames demodulated.
    pub frames: u64,
}

impl Default for FdmdvDemod {
    fn default() -> Self {
        FdmdvDemod {
            prev: [Complex32::new(0.0, 0.0); CARRIERS],
            carriers: std::array::from_fn(|k| Nco::new(AUDIO_RATE, -carrier_hz(k))),
            fine: Nco::new(AUDIO_RATE, 0.0),
            ring: vec![Complex32::new(0.0, 0.0); CARRIERS * 2 * RING],
            at: 0,
            n: 0,
            offset: 0,
            freq_corr: 0.0,
            half: None,
            level: 0.0,
            taken: 0,
            score: [Complex32::new(0.0, 0.0); TIMINGS],
            weight: [0.0; TIMINGS],
            frames: 0,
        }
    }
}

/// Bits of a step (a dibit, Gray coded): the first bit is one for steps π and 3π/2, the second for π/2 and π.
pub fn dibit_bits(step_quadrant: u8) -> (u8, u8) {
    match step_quadrant & 3 {
        0 => (0, 0),
        1 => (0, 1),
        2 => (1, 1),
        _ => (1, 0),
    }
}

impl FdmdvDemod {
    /// The matched filter output of carrier `k` for the pulse that ended `back` samples before the last sample taken.
    fn matched(&self, k: usize, back: usize) -> Complex32 {
        let g = &PULSE;
        let end = k * 2 * RING + self.at + RING - back;
        self.ring[end - g.len()..end]
            .iter()
            .zip(g.iter())
            .fold(Complex32::new(0.0, 0.0), |a, (x, &h)| a + x * h)
    }

    /// The timing search, when a symbol ends (the receiver can start anywhere in a symbol). For each timing it takes the
    /// pilot's matched filter output at the last two symbols of that timing. The pilot steps by π and by 0 in turn, so
    /// at the right timing the phase step, its sign turned on every other symbol, adds up; a timing between symbols gives
    /// small outputs and steps that do not add up. The score forgets with 0.9 a symbol. The timing found is the middle
    /// of the best run of [`FAR`] samples either side. When it is [`FAR`] samples or more from the present one, its score
    /// holds together (0.6 of its weight) and is 1.2 times the present one, it returns the change of the next symbol's
    /// length that makes it end on a boundary of the timing found.
    fn timing_search(&mut self) -> Option<i32> {
        if self.taken < RING as u64 {
            return None;
        }
        let sps = SPS as u64;
        let now = self.taken;
        for c in 0..TIMINGS {
            let tau = (c * SEARCH_STEP) as u64;
            // The last boundary of this timing, and how many symbols of this timing lie before it.
            let back = ((now + sps - tau) % sps) as usize;
            let count = (now - back as u64) / sps;
            let step = self.matched(PILOT, back) * self.matched(PILOT, back + SPS).conj();
            let sign = if count.is_multiple_of(2) { 1.0 } else { -1.0 };
            self.score[c] = self.score[c] * 0.9 + step * sign;
            self.weight[c] = self.weight[c] * 0.9 + step.norm();
        }
        let mag: [f32; TIMINGS] = std::array::from_fn(|c| self.score[c].norm());
        let reach = FAR as usize / SEARCH_STEP;
        let run = |c: usize| (0..=2 * reach).map(|j| mag[(c + TIMINGS + j - reach) % TIMINGS]).sum::<f32>();
        let best = (0..TIMINGS).fold(0, |b, c| if run(c) > run(b) { c } else { b });
        let phase = now % sps;
        let here = mag[(phase as usize + SEARCH_STEP / 2) / SEARCH_STEP % TIMINGS];
        let ahead = ((best * SEARCH_STEP) as u64 + sps - phase) % sps;
        let far = ahead.min(sps - ahead) >= FAR;
        if !far || mag[best] < 0.6 * self.weight[best] || mag[best] < 1.2 * here {
            return None;
        }
        // The frequency the pilot gave at the wrong timing is not to be trusted.
        self.freq_corr = 0.0;
        self.fine.set_freq(0.0);
        self.half = None;
        let mut len = ahead;
        if len < sps / 2 {
            len += sps;
        }
        Some(len as i32 - SPS as i32)
    }

    /// Demodulates IQ at 8 kHz, appending the marker and the 64 soft values of each frame to `out` (forty values a symbol, so the
    /// stream keeps its time).
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        for &s in x {
            // Fine frequency correction, then each carrier at 0 Hz into its ring.
            let s = s * self.fine.next_sample();
            for k in 0..CARRIERS {
                let base = s * self.carriers[k].next_sample();
                let r = k * 2 * RING;
                self.ring[r + self.at] = base;
                self.ring[r + self.at + RING] = base;
            }
            self.at = (self.at + 1) % RING;
            self.taken += 1;
            self.n += 1;
            if self.n as i32 >= SPS as i32 + self.offset {
                // Each carrier through its matched filter: the symbol whose pulse ended now.
                let y: [Complex32; CARRIERS] = std::array::from_fn(|k| self.matched(k, 0));
                let mut bits = Vec::with_capacity(32);
                let pilot = y[PILOT] * self.prev[PILOT].conj();
                let flip = pilot.re < 0.0;
                let mut r = pilot.arg();
                if flip {
                    r = if r > 0.0 {
                        r - std::f32::consts::PI
                    } else {
                        r + std::f32::consts::PI
                    };
                }
                if pilot.norm() > 1e-9 && self.frames > 0 {
                    self.freq_corr +=
                        0.1 * (r as f64) / (std::f64::consts::TAU * SPS as f64 / AUDIO_RATE);
                    self.fine.set_freq(-self.freq_corr);
                }
                for k in 0..CARRIERS {
                    if k == PILOT {
                        continue;
                    }
                    let d = y[k] * self.prev[k].conj() * Complex32::from_polar(1.0, -r - std::f32::consts::FRAC_PI_4);
                    let m = d.norm().max(1e-9);
                    // Quadrants after the -pi/4 turn: step 0 -> (+,-), pi/2 -> (+,+), pi -> (-,+), 3pi/2 -> (-,-).
                    // First bit one for pi and 3pi/2: re < 0. Second bit one for pi/2 and pi: im > 0.
                    bits.push((-d.re / m * 1000.0).clamp(-1000.0, 1000.0));
                    bits.push((d.im / m * 1000.0).clamp(-1000.0, 1000.0));
                }
                // Timing: the timing search moves a timing that is far off at once. Else a Gardner detector on the pilot:
                // at a reversal, the matched filter output half a symbol back is zero on time and has the sign of the
                // new symbol when late.
                let mid = self.matched(PILOT, SPS / 2);
                let e = ((y[PILOT] - self.prev[PILOT]) * mid.conj()).re;
                let power = 0.5 * (y[PILOT].norm_sqr() + self.prev[PILOT].norm_sqr());
                let e = if power > 0.0 { e / power } else { 0.0 };
                self.offset = if let Some(jump) = self.timing_search() {
                    jump
                } else if e > 0.1 {
                    -1
                } else if e < -0.1 {
                    1
                } else {
                    0
                };
                self.prev = y;
                // A pilot 10 dB below its mean is no symbol (the end of a transmission: the shaped pulses have no
                // tails at the symbol instants), so no frame ends on it.
                let p = y[PILOT].norm_sqr();
                let weak = p < 0.1 * self.level;
                self.level = 0.9 * self.level + 0.1 * p;
                if weak {
                    self.half = None;
                    out.extend(std::iter::repeat_n(0.0, 40));
                    self.n = 0;
                    continue;
                }
                // Forty values per symbol, so the stream keeps its time: a frame (marker and 64 bits) after its second
                // symbol, nothing in between.
                if flip {
                    if self.half.replace(bits).is_some() {
                        out.extend(std::iter::repeat_n(0.0, 40));
                    }
                } else if let Some(first) = self.half.take() {
                    for c in MARKER.bytes() {
                        out.push(if c == b'1' { 1000.0 } else { -1000.0 });
                    }
                    out.extend_from_slice(&first);
                    out.extend_from_slice(&bits);
                    self.frames += 1;
                } else {
                    out.extend(std::iter::repeat_n(0.0, 40));
                }
                self.n = 0;
            }
        }
    }
}

/// The 52 codec bits of a 64-bit frame after the Golay (23,12) correction of its first 12 (FreeDV 1600 frame: 52 codec bits, 11
/// parity bits, one text bit), and the bits corrected. `None` when the word does not decode.
pub fn correct_frame(bits: &[u8; 64]) -> Option<([u8; 52], u32)> {
    let mut w = 0u128;
    for &b in bits[..12].iter().chain(&bits[52..63]) {
        w = (w << 1) | (b & 1) as u128;
    }
    let (cw, e) = GOLAY.decode(w)?;
    let info = GOLAY.info(cw);
    let mut codec = [0u8; 52];
    codec.copy_from_slice(&bits[..52]);
    for (i, c) in codec[..12].iter_mut().enumerate() {
        *c = (info >> (11 - i)) as u8 & 1;
    }
    Some((codec, e as u32))
}

/// The 64 bits of a 1600 frame: the 52 codec bits, the 11 Golay (23,12) parity bits of the first 12 (the first bit most
/// significant), and the text bit.
pub fn frame_bits(codec: &[u8; 52], text: u8) -> [u8; 64] {
    let mut bits = [0u8; 64];
    bits[..52].copy_from_slice(codec);
    let parity = GOLAY.encode(to_u64(&codec[..12]) as u128) as u64 & 0x7FF;
    for (i, b) in bits[52..63].iter_mut().enumerate() {
        *b = (parity >> (10 - i)) as u8 & 1;
    }
    bits[63] = text & 1;
    bits
}

/// The text bits of `text`: its characters and a carriage return in the PSK31 varicode, each code followed by "00". A
/// character outside the table ends the bits.
pub fn text_bits(text: &str) -> Vec<u8> {
    let mut bits = Vec::new();
    let _ = varicode::encode(format!("{text}\r").as_bytes(), &mut |b| bits.push(b));
    bits
}

/// The FDMDV modulator: 1600 frames in, the 8 kHz samples of their symbols out (complex, the upper sideband above the dial
/// frequency, unit power). Each carrier's symbols are shaped by [`PULSE`]; a data carrier has amplitude 1/√18 and the pilot
/// twice its power. The samples of a symbol's slot hold the pulses of that symbol and the six before it, so the pulse of a
/// symbol is complete [`SPAN`] slots after its own ([`FdmdvModulator::finish`]).
#[derive(Debug)]
pub struct FdmdvModulator {
    phase: [f32; CARRIERS],
    osc: [Nco; CARRIERS],
    /// The symbols of the last [`SPAN`] + 1 slots, the newest first.
    syms: [[Complex32; CARRIERS]; SPAN + 1],
}

impl Default for FdmdvModulator {
    fn default() -> Self {
        // Each carrier starts one sample into its cycle.
        let osc = std::array::from_fn(|k| {
            let mut n = Nco::new(AUDIO_RATE, carrier_hz(k));
            n.set_phase_cycles(carrier_hz(k) / AUDIO_RATE);
            n
        });
        FdmdvModulator {
            phase: [0.0; CARRIERS],
            osc,
            syms: [[Complex32::new(0.0, 0.0); CARRIERS]; SPAN + 1],
        }
    }
}

impl FdmdvModulator {
    /// Appends the first symbol of a transmission, from which the first frame steps: every carrier at its present phase.
    pub fn first_symbol(&mut self, out: &mut Vec<Complex32>) {
        self.symbol(&[0.0; CARRIERS], out);
    }

    /// Appends the two symbols of the frame `bits`. Symbol `h` carries bits `32h` to `32h + 31`, two to each data carrier
    /// from the lowest (a DQPSK step, Gray coded); the pilot steps by π on the first symbol and stays on the second.
    pub fn frame(&mut self, bits: &[u8; 64], out: &mut Vec<Complex32>) {
        for half in 0..2 {
            let mut st = [0f32; CARRIERS];
            st[PILOT] = if half == 0 { std::f32::consts::PI } else { 0.0 };
            let mut j = 0;
            for (k, x) in st.iter_mut().enumerate() {
                if k == PILOT {
                    continue;
                }
                let b = &bits[half * 32 + 2 * j..half * 32 + 2 * j + 2];
                *x = step_of((b[0] << 1) | b[1]);
                j += 1;
            }
            self.symbol(&st, out);
        }
    }

    /// Appends the [`SPAN`] slots in which the pulses of the last symbols end: the end of a transmission.
    pub fn finish(&mut self, out: &mut Vec<Complex32>) {
        for _ in 0..SPAN {
            self.slot([Complex32::new(0.0, 0.0); CARRIERS], out);
        }
    }

    /// Appends the slot of one symbol whose carriers step by `steps` from the last one.
    fn symbol(&mut self, steps: &[f32; CARRIERS], out: &mut Vec<Complex32>) {
        let sym = std::array::from_fn(|k| {
            self.phase[k] += steps[k];
            let amp = if k == PILOT { PILOT_AMP } else { DATA_AMP };
            Complex32::from_polar(amp, self.phase[k])
        });
        self.slot(sym, out);
    }

    /// Appends the [`SPS`] samples of the slot of `sym`: each carrier carries the pulses of the last [`SPAN`] + 1 symbols.
    fn slot(&mut self, sym: [Complex32; CARRIERS], out: &mut Vec<Complex32>) {
        self.syms.rotate_right(1);
        self.syms[0] = sym;
        let g = &PULSE;
        let scale = (SPS as f32).sqrt();
        for t in 0..SPS {
            let mut v = Complex32::new(0.0, 0.0);
            for k in 0..CARRIERS {
                let mut c = Complex32::new(0.0, 0.0);
                for (i, s) in self.syms.iter().enumerate() {
                    if let Some(&h) = g.get(i * SPS + t) {
                        c += s[k] * h;
                    }
                }
                v += c * scale * self.osc[k].next_sample();
            }
            out.push(v);
        }
    }
}

/// The line of a text channel: varicode bits in, finished lines out.
#[derive(Debug, Default)]
pub(crate) struct TextLine {
    vari: varicode::Decoder,
    line: String,
    /// The line in progress started after a line end, so it is whole when it ends.
    whole: bool,
}

impl TextLine {
    /// Drops the line in progress; the next line that ends is not whole.
    pub(crate) fn clear(&mut self) {
        self.vari.reset();
        self.line.clear();
        self.whole = false;
    }

    /// The line in progress.
    pub(crate) fn current(&self) -> &str {
        &self.line
    }

    /// Takes one bit. A carriage return, a line feed or a NUL ends the line, and so does a full line: the line, trimmed,
    /// and whether it was whole.
    pub(crate) fn push(&mut self, bit: u8) -> Option<(String, bool)> {
        let Some(Ok(c)) = self.vari.push(bit) else {
            return None;
        };
        if c.is_ascii_graphic() || c == b' ' {
            self.line.push(c as char);
        }
        if !matches!(c, b'\r' | b'\n' | 0) && self.line.len() < LINE_LEN {
            return None;
        }
        let t = self.line.trim().to_string();
        self.line.clear();
        Some((t, std::mem::replace(&mut self.whole, true)))
    }
}

/// Which waveforms the stack listens for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Submode {
    /// 700D, 700E and 1600 at the same time.
    #[default]
    Auto,
    /// Only 1600.
    W1600,
    /// Only 700D.
    W700D,
    /// Only 700E.
    W700E,
    /// RADE: listed, not received.
    Rade,
}

impl Submode {
    /// The submode of the id a host names; none, or an id the stack does not list, is the automatic one.
    pub fn of(id: Option<&str>) -> Submode {
        match id {
            Some("1600") => Submode::W1600,
            Some("700D") => Submode::W700D,
            Some("700E") => Submode::W700E,
            Some("rade") => Submode::Rade,
            _ => Submode::Auto,
        }
    }

    /// The OFDM waveforms of the submode, in the order they take the output.
    fn ofdm(self) -> &'static [&'static Waveform] {
        static BOTH: [&Waveform; 2] = [&W700D, &W700E];
        match self {
            Submode::Auto => &BOTH,
            Submode::W700D => &BOTH[..1],
            Submode::W700E => &BOTH[1..],
            Submode::W1600 | Submode::Rade => &[],
        }
    }

    fn listens_1600(self) -> bool {
        matches!(self, Submode::Auto | Submode::W1600)
    }
}

/// An OFDM waveform is trusted after it held sync at [`SNR_FLOOR_DB`] or more for this long, samples (1.5 s).
const CONFIRM_SAMPLES: usize = 12_000;
/// The SNR in 3000 Hz that every frame of a trusted waveform has, dB.
const SNR_FLOOR_DB: f32 = 3.0;
/// The active waveform stays active until it has not been trusted for this long, samples (2 s).
const HOLD_SAMPLES: usize = 16_000;
/// The call ends when the active waveform has not been trusted for this long, samples (1 s).
const CALL_END_SAMPLES: usize = 8_000;
/// The mean power of a piece of the channel below which it is silence (-100 dBFS).
const SILENCE_POWER: f32 = 1e-10;
/// The longest text line, characters.
const LINE_LEN: usize = 80;
/// Bits of one Codec 2 700C frame.
const CODEC_BITS: usize = 28;

/// One OFDM waveform of the stack: its demodulator, whether it is trusted, and its text line.
struct Lane {
    demod: OfdmDemod,
    /// Input samples the demodulator took.
    at: u64,
    /// Samples of sync at the SNR floor or above, in a row.
    run: usize,
    confirmed: bool,
    /// Samples since it was last trusted.
    untrusted: usize,
    text: TextLine,
    last_line: String,
}

impl Lane {
    fn new(w: &'static Waveform) -> Lane {
        Lane {
            demod: OfdmDemod::new(w),
            at: 0,
            run: 0,
            confirmed: false,
            untrusted: HOLD_SAMPLES + 1,
            text: TextLine::default(),
            last_line: String::with_capacity(LINE_LEN),
        }
    }

    fn clear_text(&mut self) {
        self.text.clear();
        self.last_line.clear();
    }

    /// Counts one step towards the trust: 1.5 s of frames with sync at the SNR floor or above. Any other step removes the
    /// trust and the text at once.
    fn trust(&mut self, step: Step) {
        let good = step.frame && step.sync != SyncState::Search && self.demod.snr_db() >= SNR_FLOOR_DB;
        if good {
            self.run += step.samples;
            if !self.confirmed && self.run >= CONFIRM_SAMPLES {
                self.confirmed = true;
                self.clear_text();
            }
        } else {
            self.run = 0;
            self.confirmed = false;
            self.clear_text();
        }
        self.untrusted = if self.confirmed {
            0
        } else {
            self.untrusted.saturating_add(step.samples)
        };
    }

    /// The line the results show: the one in progress, or the last finished one.
    fn shown(&self) -> &str {
        match self.text.current() {
            "" => &self.last_line,
            t => t,
        }
    }
}

/// The FreeDV protocol: the 1600 frames of 64 bits after each marker, and the OFDM waveforms on the 8 kHz channel. Only the
/// active waveform gives speech, text and a call: the first trusted one in the order 700D, 700E, 1600.
pub struct FreeDv {
    submode: Submode,
    buf: Vec<Sym>,
    reading: bool,
    /// The frame being read decoded, so its text bit counts.
    frame_ok: bool,
    good_run: u32,
    text: TextLine,
    candidate: String,
    callsign: String,
    frames: u64,
    sideband: UpperSideband,
    upper: Vec<Complex32>,
    lanes: Vec<Lane>,
    /// The active OFDM waveform, an index of `lanes`.
    active: Option<usize>,
    ofdm_frames: u64,
    speech_frames: u64,
}

impl Default for FreeDv {
    fn default() -> Self {
        FreeDv::new(Submode::Auto)
    }
}

const KEY: u32 = 1;

impl FreeDv {
    /// The protocol of `submode`.
    pub fn new(submode: Submode) -> FreeDv {
        FreeDv {
            submode,
            buf: Vec::with_capacity(64),
            reading: false,
            frame_ok: false,
            good_run: 0,
            text: TextLine::default(),
            candidate: String::new(),
            callsign: String::new(),
            frames: 0,
            sideband: UpperSideband::default(),
            upper: Vec::new(),
            lanes: submode.ofdm().iter().map(|w| Lane::new(w)).collect(),
            active: None,
            ofdm_frames: 0,
            speech_frames: 0,
        }
    }

    /// The speech frame of a waveform whose frames last 40 ms.
    fn speech_spec(ctx: &Ctx, codec_mode: Option<sdrkit_graph::Codec2Mode>) -> crate::voice::FrameSpec {
        crate::voice::FrameSpec {
            codec: Codec::Codec2,
            packaging: sdrkit_graph::SpeechPackaging::Raw,
            codec_mode,
            duration_ms: 40,
            stack: "freedv",
            carrier_hz: ctx.carrier_hz,
            slot: 0,
        }
    }

    fn start_call(ctx: &mut Ctx) {
        let rec = CallRecord {
            kind: CallKind::Voice,
            src: Ident::None,
            encryption: Encryption::clear(),
            ..Default::default()
        };
        ctx.start_call(KEY, rec, false);
        ctx.set_encryption(KEY, Encryption::clear());
    }

    /// A finished text line of the call: its source and a message, when the line is new.
    fn line_done(ctx: &mut Ctx, t: String) {
        ctx.update_call(KEY, |c| {
            c.src = Ident::Text(t.clone());
            c.text = t.clone();
        });
        ctx.message(MessageRecord {
            src: Ident::Text(t.clone()),
            format: MessageFormat::FreedvText,
            data_format: None,
            text: t,
            crc_ok: true,
            time: ctx.now,
            ..Default::default()
        });
    }

    /// One step of the OFDM waveform `i`: the trust, which waveform is active, the call, and for a frame of the active
    /// trusted waveform its text and its speech frames.
    fn ofdm_step(&mut self, i: usize, step: Step, ctx: &mut Ctx) {
        let lane = &mut self.lanes[i];
        lane.at += step.samples as u64;
        let now = lane.at as f64 / AUDIO_RATE;
        lane.trust(step);
        let w = lane.demod.waveform();
        if step.frame {
            self.ofdm_frames += 1;
        }
        if step.frame && (step.sync == SyncState::Synced || lane.confirmed) {
            let f = lane.demod.frame();
            ctx.now = now;
            ctx.identity(f.uw_errors <= w.uw_limit);
            ctx.block(f.ldpc_ok, 2 * w.k(), f.corrected);
        }
        if let Some(a) = self.active {
            let untrusted = self.lanes[a].untrusted;
            if untrusted > CALL_END_SAMPLES && ctx.has_call(KEY) {
                ctx.now = now;
                ctx.end_call(KEY, false);
            }
            if untrusted > HOLD_SAMPLES {
                self.active = None;
            }
        }
        if self.active.is_none() && !ctx.has_call(KEY) {
            self.active = self.lanes.iter().position(|l| l.confirmed);
        }
        if self.active != Some(i) || !self.lanes[i].confirmed || !step.frame {
            return;
        }
        ctx.now = now;
        if !ctx.has_call(KEY) {
            Self::start_call(ctx);
            self.lanes[i].demod.reset_counts();
            self.good_run = 0;
        }
        let lane = &mut self.lanes[i];
        for b in 0..w.text_bits {
            let bit = lane.demod.frame().text[b];
            let Some((t, whole)) = lane.text.push(bit) else {
                continue;
            };
            // The first line after the waveform became trusted can lack its start: it is shown, not recorded.
            let record = whole && !t.is_empty() && t != lane.last_line;
            if !t.is_empty() {
                lane.last_line.clone_from(&t);
            }
            if record {
                Self::line_done(ctx, t);
            }
        }
        // Speech: the data bits are Codec 2 700C frames of 28 bits, the first bit most significant, in 4 bytes each. 700C has
        // no value in the media format, so the frames name no codec mode.
        let f = lane.demod.frame();
        let spec = Self::speech_spec(ctx, None);
        for (j, bits) in f.data.as_chunks::<CODEC_BITS>().0.iter().enumerate() {
            let mut data = [0u8; 4];
            for (n, &b) in bits.iter().enumerate() {
                data[n / 8] |= (b & 1) << (7 - n % 8);
            }
            ctx.update_call(KEY, |c| c.voice_frames += 1);
            ctx.speech(KEY, spec, &data, now + 0.04 * j as f64, !f.ldpc_ok, None);
            self.speech_frames += 1;
        }
    }

    /// The first 63 bits of a frame (the codec and parity bits): the Golay check, the counts, the call and the speech. The
    /// text bit follows with the next stream values ([`FreeDv::text_bit`]). Whether the frame decoded.
    fn frame(&mut self, ctx: &mut Ctx) -> bool {
        // An active OFDM waveform keeps the output.
        if self.active.is_some() {
            self.good_run = 0;
            return false;
        }
        let bits: [u8; 64] = std::array::from_fn(|i| self.buf.get(i).is_some_and(|s| s.v > 0.0) as u8);
        let (codec, e) = correct_frame(&bits).unwrap_or(([0; 52], 4));
        // The Golay code is perfect (every word is within 3 of a code word): a frame counts as good with at most one repair.
        let ok = e <= 1;
        ctx.identity(ok);
        ctx.block(ok, 23, e as usize);
        self.frames += 1;
        if !ok {
            // A lost frame loses a text bit, so the line in progress is not whole.
            self.good_run = 0;
            self.text.clear();
            return false;
        }
        self.good_run += 1;
        if !ctx.has_call(KEY) && self.good_run >= 3 {
            Self::start_call(ctx);
        }
        if ctx.has_call(KEY) {
            ctx.update_call(KEY, |c| c.voice_frames += 1);
            let mut data = [0u8; 7];
            for (i, &b) in codec.iter().enumerate() {
                data[i / 8] |= b << (7 - i % 8);
            }
            let spec = Self::speech_spec(ctx, Some(sdrkit_graph::Codec2Mode::M1300));
            self.speech_frames += 1;
            ctx.speech(KEY, spec, &data, ctx.now, false, None);
        }
        true
    }

    /// The text bit of a frame that decoded: one bit per frame. A line counts when it was read whole or twice in a row.
    fn text_bit(&mut self, bit: u8, ctx: &mut Ctx) {
        if let Some((t, whole)) = self.text.push(bit) {
            if !t.is_empty() && (whole || t == self.candidate) && t != self.callsign {
                self.callsign.clone_from(&t);
                Self::line_done(ctx, t);
            } else {
                self.candidate = t;
            }
        }
    }
}

impl Protocol for FreeDv {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "freedv",
            name: "FreeDV",
            baud: STREAM_RATE,
            two_level: true,
            channel_hz: 3_000.0,
            calls: &crate::results::CALLS_300,
            message_cap: 100,
            polarity_rule: false,
            clock_loop: false,
            extra_baud: None,
            stream_rate: STREAM_RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        vec![SyncPattern::from_bits(0, MARKER, 0)
            .with_max_err(0)
            .no_auto_invert()]
    }

    fn on_sync(
        &mut self,
        _id: u8,
        _inv: bool,
        _pre: &[Sym],
        _at: u64,
        _tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        if !self.submode.listens_1600() {
            return SyncAnswer::Ignore;
        }
        // A frame whose text bit did not come loses its text bit.
        if self.reading && self.frame_ok {
            self.text.clear();
        }
        self.buf.clear();
        self.reading = true;
        self.frame_ok = false;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        if !self.reading {
            return SymAnswer::Continue;
        }
        self.buf.push(s);
        // The codec bits are read before the text bit: the symbol layer gives a symbol only when the next stream value
        // has come, and the last bit of a frame waits for the next frame.
        if self.buf.len() == 63 {
            self.frame_ok = self.frame(ctx);
        } else if self.buf.len() == 64 {
            if self.frame_ok {
                self.text_bit((s.v > 0.0) as u8, ctx);
            }
            self.reading = false;
            self.frame_ok = false;
        }
        SymAnswer::Continue
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        if ctx.has_call(KEY) && ctx.now - ctx.last_frame_s > 1.0 {
            ctx.end_call(KEY, false);
            self.good_run = 0;
            self.callsign.clear();
        }
    }

    fn on_reset(&mut self, _ctx: &mut Ctx) {
        *self = FreeDv::new(self.submode);
    }

    fn on_clear(&mut self) {
        self.callsign.clear();
        for l in self.lanes.iter_mut() {
            l.clear_text();
        }
    }

    fn on_channel(&mut self, x: &[Complex32], ctx: &mut Ctx) {
        if self.lanes.is_empty() {
            return;
        }
        self.upper.clear();
        // A channel below -100 dBFS is silence. The demodulators do not depend on the level, so they get zeros and not the
        // tails of the filters before them.
        let power = x.iter().map(|s| s.norm_sqr()).sum::<f32>() / x.len() as f32;
        if power < SILENCE_POWER {
            self.upper.resize(x.len(), Complex32::new(0.0, 0.0));
        } else {
            self.sideband.process(x, &mut self.upper);
        }
        for i in 0..self.lanes.len() {
            self.lanes[i].demod.feed(&self.upper);
            while let Some(step) = self.lanes[i].demod.step() {
                self.ofdm_step(i, step, ctx);
            }
        }
    }

    fn measured(&self) -> Option<(f32, f32)> {
        let d = &self.lanes[self.active?].demod;
        Some((d.snr_db(), d.freq_offset_hz()))
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <FreedvFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        let sync = |w: &Waveform| {
            self.lanes
                .iter()
                .find(|l| std::ptr::eq(l.demod.waveform(), w))
                .map_or("", |l| l.demod.sync().key())
        };
        let synced_1600 = self.good_run >= 3;
        let lane = self.active.map(|a| &self.lanes[a]);
        let demod = lane.map(|l| &l.demod);
        r.put(&FreedvFields {
            submode: match lane {
                Some(l) => l.demod.waveform().name,
                None if synced_1600 => "1600",
                None => "",
            },
            sync_700d: sync(&W700D),
            sync_700e: sync(&W700E),
            sync_1600: match (self.submode.listens_1600(), synced_1600) {
                (false, _) => "",
                (true, false) => "search",
                (true, true) => "synced",
            },
            confirmed: lane.map_or(synced_1600, |l| l.confirmed),
            freq_offset_hz: demod.map_or(0.0, |d| f64::from(d.freq_offset_hz())),
            clock_offset_ppm: demod.map_or(0.0, |d| f64::from(d.clock_offset_ppm())),
            timing_metric: demod.map_or(0.0, |d| f64::from(d.timing_metric())),
            uw_ber_pct: demod.map_or(0.0, |d| 100.0 * d.uw_errors as f64 / d.uw_bits.max(1) as f64),
            ldpc_failed_frames: demod.map_or(0, |d| d.ldpc_failed as i64),
            freedv_frames: (self.frames + self.ofdm_frames) as i64,
            speech_frames: self.speech_frames as i64,
            callsign: lane.map_or(&self.callsign, |l| l.shown()),
        });
    }

    fn codec(&self) -> Codec {
        Codec::Codec2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

/// The FreeDV 1600 test signal at 8 kHz (complex, the upper sideband above the dial frequency): 0.3 s of silence, the
/// first symbol, `seconds` of frames of random codec bits with `text` in the text channel, and 0.5 s of silence.
pub fn render(seed: u64, text: &str, seconds: f64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut out = vec![Complex32::new(0.0, 0.0); (0.3 * AUDIO_RATE) as usize];
    let frames = (seconds / 0.04) as usize;
    let mut text_bits = text_bits(text);
    if text_bits.is_empty() {
        text_bits.push(0);
    }
    let mut m = FdmdvModulator::default();
    m.first_symbol(&mut out);
    for f in 0..frames {
        let mut codec = [0u8; 52];
        rng.fill_bits(&mut codec);
        m.frame(&frame_bits(&codec, text_bits[f % text_bits.len()]), &mut out);
    }
    m.finish(&mut out);
    out.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (0.5 * AUDIO_RATE) as usize,
    ));
    out
}

/// The text of the test signals.
const GEN_TEXT: &str = "N0CALL";

/// The FreeDV generator from the settings of the stack: the 700D or the 700E test signal for those submodes
/// ([`crate::freedv_ofdm::GEN_CLIP`] turns their clipper on), else the 1600 test signal.
pub fn generator(cfg: &GenConfig) -> Prepared {
    let w = match Submode::of(cfg.submode.as_deref()) {
        Submode::W700D => &W700D,
        Submode::W700E => &W700E,
        _ => {
            return Prepared::with_source_rate(
                render(cfg.seed, GEN_TEXT, 7.0),
                AUDIO_RATE,
                cfg.rate_hz,
                true,
            )
            .with_info(2_400.0, 800.0, 1600.0)
        }
    };
    let o = RenderOptions {
        text: GEN_TEXT,
        seconds: 10.0,
        clip: cfg.settings.get(&crate::freedv_ofdm::GEN_CLIP),
    };
    Prepared::with_source_rate(crate::freedv_ofdm::render(w, &o), AUDIO_RATE, cfg.rate_hz, true)
        .with_info(3_000.0, 50.0 * w.nc as f64, 700.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_bits_agree() {
        for d in 0..4u8 {
            let q = (step_of(d) / std::f32::consts::FRAC_PI_2).round() as u8;
            let (a, b) = dibit_bits(q);
            assert_eq!((a << 1) | b, d);
        }
    }

    #[test]
    fn the_codec_bits_are_the_golay_corrected_ones() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
        let mut bits = [0u8; 64];
        rng.fill_bits(&mut bits[..52]);
        let g = GOLAY.encode(to_u64(&bits[..12]) as u128);
        for i in 0..11 {
            bits[52 + i] = (g >> (10 - i)) as u8 & 1;
        }
        let sent = bits;
        bits[3] ^= 1;
        bits[10] ^= 1;
        let (codec, e) = correct_frame(&bits).unwrap();
        assert_eq!(e, 2);
        assert_eq!(codec[..], sent[..52]);
    }

    /// The parity bits of a frame are those of the worked Golay examples: information 0x555 gives 0x686, 0x001 gives 0x475.
    #[test]
    fn the_frame_carries_the_golay_parity_of_the_worked_examples() {
        for (info, parity) in [(0x555u64, 0x686u64), (0x001, 0x475)] {
            let mut codec = [0u8; 52];
            for (i, b) in codec[..12].iter_mut().enumerate() {
                *b = (info >> (11 - i)) as u8 & 1;
            }
            let bits = frame_bits(&codec, 1);
            assert_eq!(to_u64(&bits[52..63]), parity, "{info:#x}");
            assert_eq!(bits[63], 1);
            assert_eq!(correct_frame(&bits), Some((codec, 0)));
        }
    }

    /// From any start in a symbol (starts 10 samples apart), the demodulator finds the symbol timing: every frame from the
    /// 13th on is bit for bit.
    #[test]
    fn the_demodulator_finds_the_symbol_timing_from_any_start() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let mut m = FdmdvModulator::default();
        let mut x0 = Vec::new();
        m.first_symbol(&mut x0);
        let mut sent = Vec::new();
        for _ in 0..100 {
            let mut codec = [0u8; 52];
            rng.fill_bits(&mut codec);
            let bits = frame_bits(&codec, 0);
            m.frame(&bits, &mut x0);
            sent.push(bits);
        }
        m.finish(&mut x0);
        for lead in (0..SPS).step_by(10) {
            let mut x = vec![Complex32::new(0.0, 0.0); 2400 + lead];
            x.extend_from_slice(&x0);
            let mut d = FdmdvDemod::default();
            let mut out = Vec::new();
            d.process(&x, &mut out);
            let mut got = Vec::new();
            let mut i = 0;
            while i + 80 <= out.len() {
                if out[i] == 0.0 {
                    i += 40;
                    continue;
                }
                let bits: [u8; 64] = std::array::from_fn(|j| (out[i + 16 + j] > 0.0) as u8);
                got.push(sent.iter().position(|s| *s == bits));
                i += 80;
            }
            let at = got.iter().position(|g| *g == Some(12));
            let rest = at.map(|a| &got[a..(a + sent.len() - 12).min(got.len())]);
            assert!(
                rest.is_some_and(|r| r.iter().copied().eq((12..sent.len()).map(Some))),
                "start {lead}: {got:?}"
            );
        }
    }
}
