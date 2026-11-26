//! G-TOR monitor: the frames of the amateur hybrid-ARQ protocol. 2-FSK at 100, 200 or 300 Bd; a frame of `baud x 0.24` bytes (24,
//! 48 or 72) ends in a status byte and a CRC-16 (X.25: reflected 1021, start FFFF, complemented; high byte first). The frame is
//! sent as 12-bit groups, bit-interleaved (bit 11 of every group, then bit 10, down to bit 0), plain or with each group replaced
//! by its Golay (24,12) parity. Plain-text data frames give text; Huffman frames are shown as hex.
//!
//! Standard: "G-TOR: A Hybrid ARQ Protocol for Narrow Bandwidth HF Data Communication", Kantronics, ARRL/TAPR Digital
//! Communications Conference 1994 (a conference paper; no standards body).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [gtor: 8 kHz, 2-FSK front, frame search (4 forms, CRC)] -> text, frame events
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    FrontLock, FrontSpec, Layer, Sink, Symbols, ToneSeqGen, TtyDecoder, RATE,
};
use crate::typed::{GtorFrameEvent, GTOR_FRAME};
use sdrkit_algo_modem::fskauto::BinaryFskConfig;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Value,
    Values,
};
use sdrkit_graph::values_of;
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The speed.
    pub enum Speed {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// 100 Bd.
        B100 = ("100", "100 Bd"),
        /// 200 Bd.
        B200 = ("200", "200 Bd"),
        /// 300 Bd.
        B300 = ("300", "300 Bd"),
    }
}

impl Speed {
    fn baud(self) -> Option<f64> {
        match self {
            Speed::Auto => None,
            Speed::B100 => Some(100.0),
            Speed::B200 => Some(200.0),
            Speed::B300 => Some(300.0),
        }
    }
}

/// The speed.
pub const BAUD: Setting<field::Choice<Speed>> = Setting::choice("baud", "Speed", Speed::Auto);
/// The shift; 0 finds it.
pub const SHIFT: Setting<field::Hz> = Setting::new("shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 1200.0, 1.0)
    .help("0 finds it from the signal.");
/// The speed of the test signal.
pub const GEN_BAUD: Setting<field::Choice<Speed>> =
    Setting::choice("gen_baud", "Speed", Speed::B100)
        .help("Automatic sends 100 Bd.")
        .section(GEN_SECTION)
        .advanced();
/// The test frames in the Golay form.
pub const GEN_GOLAY: Setting<field::Bool> = Setting::new("gen_golay", "Golay form", "", false)
    .section(GEN_SECTION)
    .advanced();
/// The shift of the test signal.
pub const GEN_SHIFT_HZ: Setting<field::Hz> = Setting::new("gen_shift_hz", "Shift", "Hz", 170.0)
    .range(60.0, 1100.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1500.0)
    .range(400.0, 3500.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the G-TOR receiver.
    pub enum GtorRxSetting {
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
    }
}

sdrkit_core::settings! {
    /// The settings of the G-TOR stack.
    pub enum GtorSetting {
        /// Speed.
        Baud = BAUD,
        /// Shift.
        Shift = SHIFT,
        /// Speed of the test signal.
        GenBaud = GEN_BAUD,
        /// Golay form of the test signal.
        GenGolay = GEN_GOLAY,
        /// Shift of the test signal.
        GenShiftHz = GEN_SHIFT_HZ,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "G-TOR is described by a conference paper, not by a standards body. Not confirmed by a public \
document: the shift (found from the signal), the 1C escape of plain text (1C 7E for a data byte 1E), the coding of the call \
field of connect frames (shown raw), and the Huffman table (Huffman frames are shown as hex). The acknowledgements between the \
frames are not read.";

/// The Golay (24,12) parity rows, the row of bit 11 first.
pub const GOLAY_ROWS: [u16; 12] = [
    0xDC5, 0xB8B, 0x717, 0xE2D, 0xC5B, 0x8B7, 0x16F, 0x2DD, 0x5B9, 0xB71, 0x6E3, 0xFFE,
];

/// The Golay parity of a 12-bit value: the XOR of the rows of its 1 bits. The map is its own inverse.
pub fn golay_parity(v: u16) -> u16 {
    (0..12)
        .filter(|&i| v >> (11 - i) & 1 == 1)
        .fold(0, |p, i| p ^ GOLAY_ROWS[i])
}

/// The bytes of a frame at `baud`.
pub fn frame_len(baud: f64) -> usize {
    (baud * 0.24).round() as usize
}

/// The CRC of the bytes of a frame before it.
pub fn crc(bytes: &[u8]) -> u16 {
    CRC16_IBM_SDLC.checksum(bytes)
}

/// The 12-bit groups of `bytes` (a multiple of 3).
pub fn groups(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks(3)
        .flat_map(|c| {
            let (b0, b1, b2) = (u16::from(c[0]), u16::from(c[1]), u16::from(c[2]));
            [(b0 << 4) | (b1 >> 4), ((b1 & 0xF) << 8) | b2]
        })
        .collect()
}

/// The bytes of 12-bit groups.
pub fn bytes_of(groups: &[u16]) -> Vec<u8> {
    groups
        .chunks(2)
        .flat_map(|g| {
            let (a, b) = (g[0], g.get(1).copied().unwrap_or(0));
            [
                (a >> 4) as u8,
                (((a & 0xF) << 4) | (b >> 8)) as u8,
                (b & 0xFF) as u8,
            ]
        })
        .collect()
}

/// The bits of `groups` in sending order: bit 11 of every group, then bit 10, down to bit 0.
pub fn interleave(groups: &[u16]) -> Vec<u8> {
    (0..12)
        .rev()
        .flat_map(|b| groups.iter().map(move |g| ((g >> b) & 1) as u8))
        .collect()
}

/// The groups of `bits` sent in the interleaved order.
pub fn deinterleave(bits: &[u8], ngroups: usize) -> Vec<u16> {
    let mut g = vec![0u16; ngroups];
    for (i, &b) in bits.iter().take(12 * ngroups).enumerate() {
        let level = 11 - i / ngroups;
        g[i % ngroups] |= u16::from(b & 1) << level;
    }
    g
}

/// A data frame of `data` (at most `len - 3` bytes, the rest filled with 1E), with `status`.
pub fn data_frame(data: &[u8], len: usize, status: u8) -> Vec<u8> {
    let mut f: Vec<u8> = data.iter().copied().take(len - 3).collect();
    f.resize(len - 3, 0x1E);
    f.push(status);
    let c = crc(&f);
    f.push((c >> 8) as u8);
    f.push((c & 0xFF) as u8);
    f
}

/// The plain text of the data bytes `data`: up to the first 1E, with 1C 7E for 1E.
pub fn plain_text(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            0x1E => break,
            0x1C if data.get(i + 1) == Some(&0x7E) => {
                out.push(0x1E);
                i += 2;
                continue;
            }
            b => out.push(b),
        }
        i += 1;
    }
    out
}

/// The G-TOR layer.
pub struct Gtor {
    baud_set: Speed,
    shift: f64,
    baud: f64,
    bits: Vec<u8>,
    quality: Vec<f32>,
    since_frame: u64,
    frames: u64,
}

impl Default for Gtor {
    fn default() -> Self {
        Self::new()
    }
}

impl Gtor {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Gtor {
            baud_set: Speed::Auto,
            shift: 0.0,
            baud: 0.0,
            bits: Vec::new(),
            quality: Vec::new(),
            since_frame: u64::MAX / 2,
            frames: 0,
        }
    }

    /// The frame of the window, if one of the four forms passes its CRC: the bytes, Golay form, inverted.
    fn try_window(&self, w: &[u8], len: usize) -> Option<(Vec<u8>, bool, bool)> {
        let ng = len * 2 / 3;
        for inv in [false, true] {
            let bits: Vec<u8> = w.iter().map(|&b| b ^ u8::from(inv)).collect();
            let g = deinterleave(&bits, ng);
            for golay in [false, true] {
                let g: Vec<u16> = if golay {
                    g.iter().map(|&v| golay_parity(v)).collect()
                } else {
                    g.clone()
                };
                let bytes = bytes_of(&g);
                let c = crc(&bytes[..len - 2]);
                let status = bytes[len - 3];
                if c == (u16::from(bytes[len - 2]) << 8 | u16::from(bytes[len - 1]))
                    && (status >> 2) & 3 != 3
                {
                    return Some((bytes, golay, inv));
                }
            }
        }
        None
    }

    fn frame(&mut self, bytes: &[u8], golay: bool, inv: bool, sink: &mut Sink) {
        let len = bytes.len();
        let status = bytes[len - 3];
        let command = ["data", "changeover", "disconnect", "connect"][usize::from(status >> 6)];
        let comp = ["none", "Huffman", "swapped Huffman", "none"][usize::from((status >> 2) & 3)];
        let block = i64::from(status & 3);
        let data = &bytes[..len - 3];
        let mut text = format!("G-TOR {:.0} Bd {command} block {block}", self.baud);
        if golay {
            text.push_str(" (Golay form)");
        }
        if inv {
            text.push_str(" (inverted)");
        }
        if comp != "none" {
            text.push_str(&format!(" {comp} (not expanded): "));
            text.extend(data.iter().map(|b| format!("{b:02X}")));
        } else if status >> 6 == 0 {
            let t = plain_text(data);
            for &b in &t {
                let c = char::from(b);
                if c == '\n' || c == '\r' || (' '..='~').contains(&c) {
                    sink.chars.push(c);
                }
            }
        }
        sink.good += 1;
        let frame = |text: &str| {
            GTOR_FRAME.event(
                sink.time,
                sink.dial,
                &GtorFrameEvent {
                    text,
                    baud: self.baud,
                    command,
                    block,
                    form: if golay { "Golay" } else { "plain" },
                    inverted: inv,
                    compression: comp,
                },
            )
        };
        let e = frame(&text);
        sink.events.push(e);
        if status >> 6 >= 2 {
            let call: String = data
                .iter()
                .take(20)
                .map(|&b| char::from(b))
                .filter(|c| (' '..='~').contains(c))
                .collect();
            let e = frame(&format!("G-TOR {command}: call field \"{call}\" (raw)"));
            sink.events.push(e);
        }
        if self.frames == 0 || self.since_frame > (self.baud * 12.0) as u64 {
            sink.lock(
                format!("G-TOR frames: {:.0} Bd", self.baud),
                format!("{:.0} Bd", self.baud),
            );
        }
        self.frames += 1;
        self.since_frame = 0;
    }
}

impl Layer for Gtor {
    fn name(&self) -> &'static str {
        "hfdata.gtor"
    }
    fn schema(&self) -> Arc<Schema> {
        GtorRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (GtorRxSetting::Baud, Value::Choice(self.baud_set.index())),
            (GtorRxSetting::Shift, Value::Hz(self.shift)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match GtorRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GtorRxSetting::Baud => self.baud_set = BAUD.read(id, value)?,
            GtorRxSetting::Shift => self.shift = SHIFT.read(id, value)?,
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let mut c = BinaryFskConfig::new(RATE, &[100.0, 200.0, 300.0]);
        c.baud = self.baud_set.baud();
        c.shift_hz = (self.shift > 0.0).then_some(self.shift);
        FrontSpec::Fsk(c)
    }
    fn declare(&self, d: ResultsDecl) -> ResultsDecl {
        d.event(&GTOR_FRAME)
    }
    fn note(&self) -> &'static str {
        NOTE_TEXT
    }
    fn front_locked(&mut self, lock: FrontLock) {
        self.baud = lock.baud();
    }
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink) {
        let Symbols::Soft(soft) = s else { return };
        let len = frame_len(self.baud);
        if len < 6 {
            return;
        }
        let w = (self.baud * 1.92).round() as usize;
        for &v in soft {
            self.bits.push(u8::from(v > 0.0));
            self.quality.push(v.abs());
            let was_on = self.since_frame <= (self.baud * 12.0) as u64;
            self.since_frame = self.since_frame.saturating_add(1);
            if was_on && self.since_frame > (self.baud * 12.0) as u64 {
                sink.lose("no frame for 12 s");
            }
            if self.bits.len() < w {
                continue;
            }
            // Noise gives soft values spread evenly over [-1, 1]; a frame gives few near 0.
            let weak = self.quality.iter().filter(|&&q| q < 0.15).count();
            let found = if weak * 20 <= w {
                self.try_window(&self.bits, len)
            } else {
                None
            };
            match found {
                Some((bytes, golay, inv)) => {
                    self.frame(&bytes, golay, inv, sink);
                    self.bits.clear();
                    self.quality.clear();
                }
                None => {
                    self.bits.remove(0);
                    self.quality.remove(0);
                }
            }
        }
    }
    fn decoding(&self) -> bool {
        self.frames > 0 && self.since_frame <= (self.baud * 12.0) as u64
    }
    fn detail(&self) -> String {
        format!("{:.0} Bd", self.baud)
    }
    fn reset(&mut self) {
        self.bits.clear();
        self.quality.clear();
        self.since_frame = u64::MAX / 2;
        self.frames = 0;
    }
}

/// The test frames of `text` at `baud`: data frames filled with 1E (at least two), block number = frame number modulo 4.
pub fn test_frames(text: &str, baud: f64) -> Vec<Vec<u8>> {
    let len = frame_len(baud);
    let per = len - 3;
    let data = text.as_bytes();
    let n = data.len().div_ceil(per).max(2);
    (0..n)
        .map(|i| {
            let part = &data[(i * per).min(data.len())..((i + 1) * per).min(data.len())];
            data_frame(part, len, (i % 4) as u8)
        })
        .collect()
}

/// The test signal: the frames of the test text, the higher tone for 1, each followed by 0.48 s of silence.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let baud = s.get(&GEN_BAUD).baud().unwrap_or(100.0);
    let golay = s.get(&GEN_GOLAY);
    let shift = s.get(&GEN_SHIFT_HZ);
    let c = s.get(&GEN_CENTRE_HZ);
    let mut seq = Vec::new();
    for f in test_frames(crate::serial::TEST_TEXT, baud) {
        let mut g = groups(&f);
        if golay {
            g.iter_mut().for_each(|v| *v = golay_parity(*v));
        }
        for b in interleave(&g) {
            let f = if b == 1 {
                c + shift / 2.0
            } else {
                c - shift / 2.0
            };
            seq.push((1.0 / baud, Some(f)));
        }
        seq.push((0.48, None));
    }
    Ok(Box::new(ToneSeqGen::new(
        cfg.rate_hz,
        &seq,
        3.0 * baud + shift,
    )))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    GtorSetting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("gtor"),
        "G-TOR",
        TtyDecoder::new(Gtor::new()),
        cfg,
    )
}

/// G-TOR monitor.
pub static GTOR: StackDescriptor = StackDescriptor {
    id: "gtor",
    name: "G-TOR monitor",
    group: "HF utility",
    blurb: "G-TOR frames at 100, 200 or 300 Bd, plain or in the Golay form, checked by their CRC: plain text, and Huffman frames as hex.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::GTOR),
    benchmarks: Some(crate::benchmarks),
};
