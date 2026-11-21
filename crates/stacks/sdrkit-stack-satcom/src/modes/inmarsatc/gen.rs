//! The Inmarsat-C test signal: continuous 1200 symbol/s BPSK frames with a bulletin board, a signalling channel packet and up to
//! three EGC packets of a loop of five made-up messages, and extra LES TDM channels on request. The signal is clean: noise and
//! other impairments come from the standard `gen_*` settings.

use super::coding::{self, FRAME_SYMBOLS};
use super::egc::{ita2_encode, EgcPacket};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The test signal inverts the polarity.
pub const GEN_INVERT: Setting<field::Bool> =
    Setting::new("gen_invert", "Inverted polarity", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The number of the first frame.
pub const GEN_FIRST_FRAME: Setting<field::I64> =
    Setting::new("gen_first_frame", "First frame number", "", 1000)
        .range(0.0, 9999.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The extra channels of the test signal.
pub const GEN_EXTRA: Setting<field::I64> = Setting::new("gen_extra", "Extra channels", "", 0)
    .range(0.0, 3.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// Where in the first frame the test signal starts.
pub const GEN_START_SYMBOLS: Setting<field::I64> = Setting::new(
    "gen_start_symbols",
    "Start into the first frame",
    "symbols",
    0,
)
.range(0.0, 10367.0, 1.0)
.section(TEST_SIGNAL)
.advanced();

/// The made-up messages: (service, priority, number, presentation, text).
pub fn messages() -> Vec<(u8, u8, u16, u8, String)> {
    vec![
        (
            0x31,
            1,
            4596,
            0,
            "NAVAREA IX 231/26\r\nPERSIAN GULF.\r\nSTRAIT OF HORMUZ.\r\nTRAFFIC SEPARATION SCHEME OFF RAS AL KHAIMAH.\r\nDREDGING OPERATIONS IN \
             PROGRESS WITHIN 2 MILES OF 25-34.2N 056-24.6E. VESSELS REQUESTED TO KEEP CLEAR AND PASS AT SLOW SPEED.\r\nCANCEL THIS MESSAGE \
             15 DAYS AFTER DATE OF ISSUE.\r\n"
                .into(),
        ),
        (
            0x31,
            1,
            4891,
            0,
            "METAREA IX GALE WARNING 077/26\r\nNORTHERN ARABIAN SEA. NORTHWEST 8, LOCALLY 9 LATER. SEA VERY ROUGH.\r\n".into(),
        ),
        (
            0x34,
            2,
            1201,
            0,
            "SAR COORDINATION. VESSELS IN AREA 22N 060E TO 24N 062E REQUESTED TO KEEP SHARP LOOKOUT FOR LIFERAFT, REPORT TO MRCC.\r\n"
                .into(),
        ),
        (
            0x31,
            0,
            5151,
            0,
            "METAREA IX FORECAST FOR GULF OF OMAN AND ARABIAN SEA VALID 24 HOURS.\r\nSYNOPSIS: HIGH OVER IRAN, MONSOON TROUGH OVER \
             INDIA.\r\nGULF OF OMAN: NORTHWEST 3 TO 4. SEA SLIGHT TO MODERATE. VISIBILITY GOOD, MODERATE IN HAZE.\r\nNORTH ARABIAN SEA: \
             NORTHWEST 4 TO 5, OCCASIONALLY 6. SEA MODERATE TO ROUGH. SWELL SOUTHWEST 2 METRES.\r\nCENTRAL ARABIAN SEA: SOUTHWEST 5 TO 6. \
             SEA ROUGH. SWELL SOUTHWEST 3 METRES.\r\nOUTLOOK: LITTLE CHANGE.\r\n"
                .into(),
        ),
        (0x02, 0, 4277, 6, "FLEET 14 PORT: DUBAI, ETA 0630.".into()),
    ]
}

/// One channel's frame and message stream.
pub struct FrameStream {
    frame_number: u16,
    channel_type: u8,
    station: u8,
    rng: Rng,
    msgs: Vec<(u8, u8, u16, u8, Vec<u8>)>,
    msg: usize,
    part: usize,
    symbols: Vec<u8>,
    pos: usize,
    invert: bool,
}

impl FrameStream {
    pub fn new(
        first_frame: u16,
        channel_type: u8,
        station: u8,
        seed: u64,
        invert: bool,
        start: usize,
    ) -> Self {
        let mut rng = Rng::new(seed);
        let offset = rng.below(1000) as u16;
        let msgs = messages()
            .into_iter()
            .map(|(svc, pr, num, pres, text)| {
                let body = if pres == 6 {
                    ita2_encode(&text)
                } else {
                    text.into_bytes()
                };
                let num = if channel_type == 1 {
                    num
                } else {
                    num.wrapping_add(offset)
                };
                (svc, pr, num, pres, body)
            })
            .collect();
        let mut s = FrameStream {
            frame_number: first_frame,
            channel_type,
            station,
            rng,
            msgs,
            msg: 0,
            part: 0,
            symbols: Vec::new(),
            pos: start,
            invert,
        };
        s.symbols = s.next_frame();
        s
    }

    /// The packets of the next frame.
    pub fn frame_packets(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        // Bulletin board.
        let mut bb = vec![0x7Du8, 1, 0, 0, 0, 0, 0, 0, 0x70, 0xE0, 0x01, 8, 0, 0];
        bb[2..4].copy_from_slice(&self.frame_number.to_be_bytes());
        bb[4] = 0;
        bb[6] = self.channel_type << 5;
        bb[7] = (3 << 6) | self.station;
        coding::seal(&mut bb);
        out.extend_from_slice(&bb);
        // Signalling channel: 1626.8 MHz.
        let mut sc = vec![0x6Cu8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        sc[2..4].copy_from_slice(&6120u16.to_be_bytes());
        coding::seal(&mut sc);
        out.extend_from_slice(&sc);
        for _ in 0..3 {
            let (svc, pr, num, pres, body) = self.msgs[self.msg].clone();
            let chunks: Vec<&[u8]> = body.chunks(48).collect();
            let i = self.part;
            let alen = super::egc::service(svc).2;
            let mut addr = vec![(3 << 6) | self.station];
            for _ in 1..alen {
                addr.push(self.rng.next_u32() as u8);
            }
            let p = EgcPacket {
                descriptor: if i.is_multiple_of(2) { 0xB1 } else { 0xB2 },
                service: svc,
                continuation: i + 1 < chunks.len(),
                priority: pr,
                repetition: 1,
                message_number: num,
                packet_number: (i + 1) as u8,
                presentation: pres,
                address: addr,
                payload: chunks[i].to_vec(),
            };
            out.extend(p.build());
            self.part += 1;
            if self.part >= chunks.len() {
                self.part = 0;
                self.msg = (self.msg + 1) % self.msgs.len();
            }
        }
        out
    }

    fn next_frame(&mut self) -> Vec<u8> {
        let p = self.frame_packets();
        self.frame_number = self.frame_number.wrapping_add(1);
        coding::encode_frame(&p)
    }

    /// The next BPSK symbol (+-1).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Complex32 {
        if self.pos >= FRAME_SYMBOLS {
            self.symbols = self.next_frame();
            self.pos = 0;
        }
        let b = (self.symbols[self.pos] == 1) ^ self.invert;
        self.pos += 1;
        Complex32::new(if b { 1.0 } else { -1.0 }, 0.0)
    }
}

/// One channel of the test signal: its frames, the RRC modulator and the oscillator that puts it at its offset.
struct Channel {
    frames: FrameStream,
    modulator: Linear,
    nco: Nco,
    gain: f32,
    /// Samples made and not yet sent.
    held: Vec<Complex32>,
    sent: usize,
    syms: Vec<Complex32>,
}

impl Channel {
    fn fill(&mut self, out: &mut [Complex32]) {
        let mut i = 0;
        while i < out.len() {
            if self.sent == self.held.len() {
                self.held.clear();
                self.sent = 0;
                self.syms.clear();
                for _ in 0..64 {
                    self.syms.push(self.frames.next());
                }
                self.modulator.push(&self.syms, &mut self.held);
                continue;
            }
            let n = (self.held.len() - self.sent).min(out.len() - i);
            let piece = &mut self.held[self.sent..self.sent + n];
            self.nco.mix_in_place(piece);
            for (o, v) in out[i..i + n].iter_mut().zip(piece.iter()) {
                *o += v * self.gain;
            }
            self.sent += n;
            i += n;
        }
    }
}

/// The generator.
pub struct InmcGenerator {
    rate: f64,
    chans: Vec<Channel>,
}

/// Options.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    /// The main channel's place from the centre (by default the tune offset below it, where the engine puts the station).
    pub channel_hz: f64,
    pub invert: bool,
    pub first_frame: u16,
    pub more_channels: usize,
    pub start_symbols: usize,
    pub seed: u64,
}

impl GenParams {
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            channel_hz: -super::DESCRIPTOR.tuning.tune_offset_hz,
            invert: false,
            first_frame: 1000,
            more_channels: 0,
            start_symbols: 0,
            seed: 1,
        }
    }
}

/// The extra LES TDM channels: (place from the centre, frequency error, station id, start symbol).
const EXTRAS: [(f64, f64, u8, usize); 3] = [
    (-20e3, 1200.0, 2, 3_000),
    (-90e3, -2300.0, 3, 6_000),
    (30e3, 700.0, 4, 8_000),
];

impl InmcGenerator {
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.invert = s.get(&GEN_INVERT);
        p.first_frame = u16::try_from(s.get(&GEN_FIRST_FRAME)).unwrap_or(p.first_frame);
        p.more_channels = usize::try_from(s.get(&GEN_EXTRA)).unwrap_or(0);
        p.start_symbols = usize::try_from(s.get(&GEN_START_SYMBOLS)).unwrap_or(0);
        p.seed = cfg.seed;
        Self::new(p)
    }

    pub fn new(p: GenParams) -> Self {
        let n = 1 + p.more_channels.min(3);
        // Each channel carries 1/n of the power, so the sum has unit power.
        let gain = (1.0 / n as f64).sqrt() as f32;
        let channel = |frames: FrameStream, at_hz: f64| Channel {
            frames,
            modulator: Linear::rrc(1.0, 8.0, p.rate / coding::SYMBOL_RATE)
                .expect("a valid RRC modulator"),
            nco: Nco::new(p.rate, at_hz),
            gain,
            held: Vec::new(),
            sent: 0,
            syms: Vec::new(),
        };
        let mut chans = vec![channel(
            FrameStream::new(p.first_frame, 1, 44, p.seed, p.invert, p.start_symbols),
            p.channel_hz,
        )];
        for (k, &(f, off, st, start)) in EXTRAS.iter().enumerate().take(n - 1) {
            chans.push(channel(
                FrameStream::new(
                    2000 + 100 * k as u16,
                    2,
                    st,
                    p.seed + 10 + k as u64,
                    false,
                    start,
                ),
                f + off,
            ));
        }
        InmcGenerator {
            rate: p.rate,
            chans,
        }
    }
}

impl Generator for InmcGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::new(0.0, 0.0));
        for c in &mut self.chans {
            c.fill(out);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // One channel: 2400 Hz wide (RRC roll-off 1), 1200 symbol/s, 5120 information bits in each frame of 10 368 symbols, at
        // 1/n of the power.
        Some(
            SignalInfo::new(2400.0)
                .with_symbol_rate(coding::SYMBOL_RATE)
                .with_bit_rate(
                    coding::SYMBOL_RATE * coding::FRAME_BITS as f64 / FRAME_SYMBOLS as f64,
                )
                .with_power(1.0 / self.chans.len() as f64),
        )
    }
}
