//! The DCR test signal (ARIB STD-T98 volume 3, as section 1.16 of the specification gives it): transmissions of frames
//! built from their fields, or the test frames of chapter 7 of the standard, as four-level FSK at 2400 symbols/s. The
//! symbols are shaped on frequency at 96 kHz with a raised cosine of roll-off 0.2 over +-4 symbols, 315 Hz a level; the
//! frequency track is read at the output rate with the symbol clock error and integrated to phase. The generator adds no
//! noise.

use super::link::{pich_bits, Message, Rich, TYPE_END, TYPE_IDLE, TYPE_VOICE};
use crate::nxdn::{scrambler, ChannelCoder, FACCH1, FSW, SACCH, SCRAMBLED};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Setting};
use sdrkit_util_bits::pack::push_u64;
use std::f64::consts::TAU;

/// The symbol rate, Hz.
pub const BAUD: f64 = 2400.0;
/// The deviation of one level, Hz (the inner symbols; the outer ones are at 3 levels, 945 Hz).
pub const HZ_PER_LEVEL: f64 = 315.0;
/// The rate of the frequency track, Hz: 40 samples a symbol.
const TRACK_RATE: f64 = 96_000.0;
/// Samples of the track in one symbol.
const TRACK_SPS: usize = 40;
/// Symbols on each side of a pulse's centre.
const PULSE_HALF: usize = 4;

/// The preamble after the three runs of +3 +3 -3 -3, as whitened symbol hex.
pub const PREAMBLE_HEX: &str = "5775FD";
/// The RICH of the synchronous burst of the test frames.
pub const RICH_BURST_HEX: &str = "5F7D";
/// The RICH of a voice frame of the test frames.
pub const RICH_VOICE_HEX: &str = "DDDD";
/// The RICH of the end frame of the test frames.
pub const RICH_END_HEX: &str = "DF5D";
/// The PICH of the test frames: call sign 100000001.
pub const PICH_HEX: &str = "82022028832C2AA08399EAAEF82838802E2B";
/// TCH2 of the synchronous burst of the test frames (bits the standard leaves undefined).
pub const BURST_TCH2_HEX: &str = "000A02822028822AAA202280A88A08A0AA02";

/// One test signal of chapter 7 of the standard: the whitened symbol hex of its parts and what it decodes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestFrames {
    /// The name.
    pub name: &'static str,
    /// The SACCH of the burst and of the voice frames.
    pub sacch: &'static str,
    /// The SACCH of the end frame.
    pub end_sacch: &'static str,
    /// TCH1 of the voice and end frames.
    pub tch1: &'static str,
    /// TCH2 of the voice and end frames.
    pub tch2: &'static str,
    /// The user code.
    pub user_code: u16,
    /// The call is privacy scrambled.
    pub privacy: bool,
}

/// The four test signals of chapter 7 of the standard.
pub const TEST_FRAMES: [TestFrames; 4] = [
    TestFrames {
        name: "user code test 1",
        sacch: "C60DB46E960168D",
        end_sacch: "8E4DB4AD8F1D6FC",
        tch1: "4CAADE8B26E4F28288C68A7429A4ECD00822",
        tch2: "CEA2FC018CECDA0AA0EE8A7E2B26CCF88A08",
        user_code: 1,
        privacy: false,
    },
    TestFrames {
        name: "user code test 2",
        sacch: "C60DB46E960168D",
        end_sacch: "8E4DB4AD8F1D6FC",
        tch1: "3BEAA15AEB53AA88E9B1CA0BF8695B880243",
        tch2: "B9E283D0415B8200C199CA01FAEB7BA08069",
        user_code: 1,
        privacy: false,
    },
    TestFrames {
        name: "user code test 3",
        sacch: "F70DF4DB9A4B280",
        end_sacch: "BF4DF41883572F1",
        tch1: "4CAADE8B26E4F28288C68A7429A4ECD00822",
        tch2: "CEA2FC018CECDA0AA0EE8A7E2B26CCF88A08",
        user_code: 511,
        privacy: false,
    },
    TestFrames {
        name: "private communication test",
        sacch: "FF0F705A9A1BBAB",
        end_sacch: "B74F70998307BDA",
        tch1: "EA8AACD526C46E1AAE6B36177F4F559B4B03",
        tch2: "C0A8A77326001E8E205EB25A2946C92D4193",
        user_code: 511,
        privacy: true,
    },
];

/// Voice frames of a test transmission of chapter 7.
pub const TEST_VOICE_FRAMES: usize = 24;

/// A call: a synchronous burst with the call sign, voice frames with random voice bits, and the end frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSpec {
    /// The call type: 0 normal, 1 privacy scrambled.
    pub call_type: u8,
    /// The user code, 0 to 511.
    pub user_code: u16,
    /// The manufacturer number, 0 to 127.
    pub manufacturer: u8,
    /// The call sign, 9 digits.
    pub call_sign: String,
    /// Voice frames (four voice frames of 20 ms each).
    pub frames: usize,
}

/// A manufacturer message: one unit per frame in idle frames (M = 5), the whole message sent `repeats` times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManufacturerSpec {
    /// The format, 1 to 18 (the message type is the format plus 1).
    pub format: u8,
    /// The call type.
    pub call_type: u8,
    /// The user code.
    pub user_code: u16,
    /// The manufacturer number.
    pub manufacturer: u8,
    /// The extra bits of the units after the first, 18 a unit, at most 54.
    pub extra: Vec<u8>,
    /// How often the message is sent.
    pub repeats: usize,
}

/// Data frames (F = 1, M = 4): an idle message in the SACCH and the same two coded 80-bit blocks in each frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSpec {
    /// The user code of the idle message.
    pub user_code: u16,
    /// The 80 bits of TCH1 and of TCH2.
    pub blocks: [Vec<u8>; 2],
    /// Frames.
    pub frames: usize,
}

/// One transmission: a preamble, then frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transmission {
    /// A test signal of chapter 7 of the standard.
    TestFrames(TestFrames),
    /// A call built from its fields.
    Call(CallSpec),
    /// A manufacturer message.
    Manufacturer(ManufacturerSpec),
    /// Data frames.
    Data(DataSpec),
}

/// What the test signal holds and how it is sent.
#[derive(Clone, Debug, PartialEq)]
pub struct GenOptions {
    /// The output rate, Hz.
    pub rate_hz: f64,
    /// The place of the channel from the centre, Hz.
    pub channel_hz: f64,
    /// The frequency error of the transmitter, Hz.
    pub carrier_hz: f64,
    /// The error of the symbol clock, ppm (positive: fast).
    pub clock_ppm: f64,
    /// The amplitude of the carrier.
    pub amplitude: f64,
    /// The sign of the frequency is reversed.
    pub inverted: bool,
    /// The outer deviation over 945 Hz.
    pub deviation_scale: f64,
    /// The seed of the random voice bits and the undefined bits.
    pub seed: u64,
    /// The transmissions, in order.
    pub content: Vec<Transmission>,
    /// Silence after each transmission but the last, s.
    pub gap_s: f64,
    /// Silence before the first transmission, s.
    pub lead_s: f64,
    /// Silence after the last transmission, s.
    pub tail_s: f64,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            rate_hz: 250_000.0,
            channel_hz: 0.0,
            carrier_hz: 0.0,
            clock_ppm: 0.0,
            amplitude: 0.05,
            inverted: false,
            deviation_scale: 1.0,
            seed: 1,
            content: vec![Transmission::TestFrames(TEST_FRAMES[0])],
            gap_s: 0.6,
            lead_s: 0.3,
            tail_s: 2.0,
        }
    }
}

sdrkit_core::choice_enum! {
    /// What the test signal of the stack sends.
    pub enum Content {
        /// The first user code test of chapter 7 of the standard.
        UserCode1 = ("user_code_1", "User code test 1"),
        /// The second user code test.
        UserCode2 = ("user_code_2", "User code test 2"),
        /// The third user code test.
        UserCode3 = ("user_code_3", "User code test 3"),
        /// The private communication test.
        Privacy = ("privacy", "Private communication test"),
        /// Eight calls built from their fields.
        Sweep = ("sweep", "Eight calls"),
        /// Three manufacturer messages, data frames and a call sign.
        Messages = ("messages", "Messages and data"),
    }
}

/// What the test signal sends.
pub const GEN_CONTENT: Setting<field::Choice<Content>> = Setting::choice("gen_content", "Content", Content::UserCode1)
    .section("Test signal")
    .help("A test signal of chapter 7 of the standard, eight calls, or manufacturer messages, data frames and a call sign.");
/// The outer deviation of the test signal over 945 Hz.
pub const GEN_DEVIATION_SCALE: Setting<field::F64> =
    Setting::new("gen_deviation_scale", "Deviation scale", "", 1.0)
        .range(0.5, 1.5, 0.0)
        .section("Test signal")
        .help("The outer deviation over 945 Hz.");

/// The user codes, manufacturers and call types of the eight calls.
pub const SWEEP_USER_CODES: [u16; 8] = [1, 0, 511, 256, 255, 100, 2, 384];
/// The manufacturer numbers of the eight calls.
pub const SWEEP_MANUFACTURERS: [u8; 8] = [0, 5, 127, 64, 0, 33, 1, 0];

/// The eight calls: every third one privacy scrambled, `frames` voice frames each, call signs from `seed`.
pub fn sweep(frames: usize, seed: u64) -> Vec<Transmission> {
    let mut rng = Rng::new(seed.wrapping_add(17));
    (0..8)
        .map(|i| {
            let mut sign = String::from("1");
            for _ in 0..8 {
                sign.push(char::from(b'0' + rng.below(10) as u8));
            }
            Transmission::Call(CallSpec {
                call_type: u8::from(i % 3 == 2),
                user_code: SWEEP_USER_CODES[i],
                manufacturer: SWEEP_MANUFACTURERS[i],
                call_sign: sign,
                frames,
            })
        })
        .collect()
}

/// Bits of `n` from `seed`, for the extra bits and the data blocks of the test signal.
pub fn bits(n: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::new(seed);
    let mut b = vec![0u8; n];
    rng.fill_bits(&mut b);
    b
}

/// The manufacturer messages (format 1 with 1 unit, format 4 with 2 units, format 8 with 4 units), two data blocks and a
/// call sign, of user code 7 and manufacturer 5.
pub fn messages() -> Vec<Transmission> {
    let mfr = |format: u8, extra: usize, seed: u64| {
        Transmission::Manufacturer(ManufacturerSpec {
            format,
            call_type: 0,
            user_code: 7,
            manufacturer: 5,
            extra: bits(extra, seed),
            repeats: 3,
        })
    };
    vec![
        mfr(1, 0, 1),
        mfr(4, 18, 2),
        mfr(8, 54, 3),
        Transmission::Data(DataSpec {
            user_code: 7,
            blocks: [bits(80, 4), bits(80, 5)],
            frames: 3,
        }),
        Transmission::Call(CallSpec {
            call_type: 0,
            user_code: 7,
            manufacturer: 5,
            call_sign: "234567890".into(),
            frames: 0,
        }),
    ]
}

/// The levels of whitened symbol hex: two bits a symbol, 01 +3, 00 +1, 10 -1, 11 -3.
pub fn levels_of_hex(hex: &str) -> Vec<f32> {
    let mut out = Vec::with_capacity(hex.len() * 2);
    for c in hex.chars() {
        let v = c.to_digit(16).unwrap_or(0) as u8;
        out.push(f32::from(sdrkit_algo_fec::conv::tcm::dibit_symbol(v >> 2)));
        out.push(f32::from(sdrkit_algo_fec::conv::tcm::dibit_symbol(v & 3)));
    }
    out
}

/// The coders of the test signal.
struct Coders {
    sacch: ChannelCoder,
    facch1: ChannelCoder,
    pn: [u8; SCRAMBLED],
}

impl Coders {
    fn new() -> Coders {
        Coders {
            sacch: ChannelCoder::new(SACCH),
            facch1: ChannelCoder::new(FACCH1),
            pn: scrambler(),
        }
    }

    /// The levels of one frame: the sync, then the RICH (outer symbols), the coded SACCH unit `unit` and the two traffic
    /// halves of 144 bits, whitened.
    fn frame(&self, rich: Rich, unit: &[u8], tch1: &[u8], tch2: &[u8]) -> Vec<f32> {
        let mut b = Vec::with_capacity(384);
        push_u64(&mut b, u64::from_str_radix(FSW, 16).unwrap_or(0), 20);
        let r = rich.byte();
        let mut body = Vec::with_capacity(364);
        for i in 0..8 {
            body.push((r >> (7 - i)) & 1);
            body.push(1);
        }
        body.extend(self.sacch.encode(unit));
        body.extend_from_slice(&tch1[..144]);
        body.extend_from_slice(&tch2[..144]);
        for k in 0..SCRAMBLED {
            body[2 * k] ^= self.pn[k];
        }
        b.extend(body);
        crate::gen::bits_to_levels(&b)
    }

    /// The levels of transmission `t`, its preamble first.
    fn transmission(&self, t: &Transmission, rng: &mut Rng) -> Vec<f32> {
        let mut out: Vec<f32> = [3.0, 3.0, -3.0, -3.0].repeat(3);
        out.extend(levels_of_hex(PREAMBLE_HEX));
        let random = |rng: &mut Rng| {
            let mut v = vec![0u8; 144];
            rng.fill_bits(&mut v);
            v
        };
        match t {
            Transmission::TestFrames(f) => {
                let frame = |rich: &str, sacch: &str, tch1: &str, tch2: &str| {
                    levels_of_hex(&format!("{FSW}{rich}{sacch}{tch1}{tch2}"))
                };
                out.extend(frame(RICH_BURST_HEX, f.sacch, PICH_HEX, BURST_TCH2_HEX));
                for _ in 0..TEST_VOICE_FRAMES {
                    out.extend(frame(RICH_VOICE_HEX, f.sacch, f.tch1, f.tch2));
                }
                out.extend(frame(RICH_END_HEX, f.end_sacch, f.tch1, f.tch2));
            }
            Transmission::Call(c) => {
                let voice = Message::new(TYPE_VOICE, c.call_type, c.user_code, c.manufacturer).units();
                let end = Message::new(TYPE_END, c.call_type, c.user_code, c.manufacturer).units();
                let pich = self.facch1.encode(&pich_bits(&c.call_sign));
                let burst = Rich { service: false, layout: 4, relay: false };
                let tch2 = random(rng);
                out.extend(self.frame(burst, &voice[0], &pich, &tch2));
                let traffic = Rich { service: true, layout: 3, relay: false };
                for _ in 0..c.frames {
                    let (a, b) = (random(rng), random(rng));
                    out.extend(self.frame(traffic, &voice[0], &a, &b));
                }
                let idle = Rich { service: true, layout: 5, relay: false };
                let (a, b) = (random(rng), random(rng));
                out.extend(self.frame(idle, &end[0], &a, &b));
            }
            Transmission::Manufacturer(m) => {
                let mut msg = Message::new(m.format + 1, m.call_type, m.user_code, m.manufacturer);
                msg.extra = m.extra.clone();
                let idle = Rich { service: true, layout: 5, relay: false };
                for _ in 0..m.repeats {
                    for u in msg.units() {
                        let (a, b) = (random(rng), random(rng));
                        out.extend(self.frame(idle, &u, &a, &b));
                    }
                }
            }
            Transmission::Data(d) => {
                let unit = &Message::new(TYPE_IDLE, 0, d.user_code, 0).units()[0];
                let data = Rich { service: true, layout: 4, relay: false };
                let (a, b) = (self.facch1.encode(&d.blocks[0]), self.facch1.encode(&d.blocks[1]));
                for _ in 0..d.frames {
                    out.extend(self.frame(data, unit, &a, &b));
                }
            }
        }
        out
    }
}

/// The frequency track of `levels` at 96 kHz in level units: raised cosine pulses of roll-off 0.2 over +-4 symbols. The
/// centre of symbol `k` is at sample `(k + 4) * 40`.
fn track(levels: &[f32], pulse: &[f32]) -> Vec<f32> {
    let mut t = vec![0f32; (levels.len() + 2 * PULSE_HALF) * TRACK_SPS + 1];
    for (k, &a) in levels.iter().enumerate() {
        let at = k * TRACK_SPS;
        for (i, &p) in pulse.iter().enumerate() {
            t[at + i] += a * p;
        }
    }
    t
}

/// The test signal of `o`: silence, the transmissions with their gaps, silence. The carrier has the amplitude of `o`
/// while a transmission is on and is off in the silences.
pub fn render(o: &GenOptions) -> Vec<Complex32> {
    let coders = Coders::new();
    let mut rng = Rng::new(o.seed);
    let pulse = taps(Shape::Rc { beta: 0.2 }, TRACK_SPS as f64, (2 * PULSE_HALF) as f64, Norm::Peak)
        .unwrap_or_else(|_| vec![1.0]);
    let silence = |s: f64, out: &mut Vec<Complex32>| {
        out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), (s * o.rate_hz).round() as usize));
    };
    let sign = if o.inverted { -1.0 } else { 1.0 };
    let hz = HZ_PER_LEVEL * o.deviation_scale * sign;
    let speed = TRACK_RATE * (1.0 + o.clock_ppm * 1e-6) / o.rate_hz;
    let mut out = Vec::new();
    let mut phase = 0.0f64;
    silence(o.lead_s, &mut out);
    for (i, t) in o.content.iter().enumerate() {
        let tr = track(&coders.transmission(t, &mut rng), &pulse);
        let n = ((tr.len() - 1) as f64 / speed).floor() as usize;
        out.reserve(n);
        for j in 0..n {
            let pos = j as f64 * speed;
            let k = pos.floor() as usize;
            let f = pos - k as f64;
            let v = f64::from(tr[k]) * (1.0 - f) + f64::from(tr[(k + 1).min(tr.len() - 1)]) * f;
            phase = (phase + TAU * (v * hz + o.carrier_hz + o.channel_hz) / o.rate_hz).rem_euclid(TAU);
            out.push(Complex32::from_polar(o.amplitude as f32, phase as f32));
        }
        let after = if i + 1 == o.content.len() { o.tail_s } else { o.gap_s };
        silence(after, &mut out);
    }
    out
}

/// The content of the setting `gen_content`.
pub fn content(c: Content, seed: u64) -> Vec<Transmission> {
    match c {
        Content::UserCode1 => vec![Transmission::TestFrames(TEST_FRAMES[0])],
        Content::UserCode2 => vec![Transmission::TestFrames(TEST_FRAMES[1])],
        Content::UserCode3 => vec![Transmission::TestFrames(TEST_FRAMES[2])],
        Content::Privacy => vec![Transmission::TestFrames(TEST_FRAMES[3])],
        Content::Sweep => sweep(6, seed),
        Content::Messages => messages(),
    }
}

/// The generator of the stack: the content of `gen_content` at unit amplitude, with the outer deviation of
/// `gen_deviation_scale`, on the tuned frequency, repeated. Noise, offsets, clock errors and the inverted spectrum come
/// from the standard impairments.
pub fn generator(cfg: &GenConfig) -> crate::gen::Prepared {
    let o = GenOptions {
        rate_hz: cfg.rate_hz,
        amplitude: 1.0,
        deviation_scale: cfg.settings.get(&GEN_DEVIATION_SCALE),
        seed: cfg.seed,
        content: content(cfg.settings.get(&GEN_CONTENT), cfg.seed),
        tail_s: 0.6,
        ..GenOptions::default()
    };
    crate::gen::Prepared::with_source_rate(render(&o), cfg.rate_hz, cfg.rate_hz, true).with_info(6_250.0, BAUD, 2.0 * BAUD)
}
