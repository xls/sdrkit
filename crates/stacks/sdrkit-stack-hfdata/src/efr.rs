//! EFR teleswitch: the long-wave load-control telegrams of the EFR transmitters. 2-FSK at 200 Bd with a 340 Hz shift; 11-bit
//! characters (start 0, eight data bits least significant first, even parity, stop 1) carry FT1.2 frames of variable length
//! (`68 L L 68`, the user bytes, the checksum, `16`). The first three user bytes are read as the message number and two
//! addresses; the payload is shown as hex.
//!
//! Standard: the FT1.2 frame of IEC 60870-5-1 and IEC 60870-5-2 (DIN 19244); those documents are sold, so no part is confirmed
//! by a public document. The payloads of the two load-control systems are not read.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [efr: 8 kHz, 2-FSK front (200 Bd, 340 Hz), character lock, FT1.2 reader] -> telegrams, text
//! ```

use crate::receiver::GEN_SECTION;
use crate::teleprinter::{
    FrontLock, FrontSpec, Layer, Polarity, Sink, Symbols, ToneSeqGen, TtyDecoder, POLARITY, RATE,
};
use crate::typed::{EfrTelegramEvent, EfrTelegramRow, EFR_TELEGRAM, EFR_TELEGRAMS};
use sdrkit_algo_modem::fskauto::BinaryFskConfig;
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Value,
    Values,
};
use sdrkit_graph::values_of;
use sdrkit_proto_link::ft12::{self, CharRead, Ft12Event, Ft12Reader};
use std::sync::Arc;

/// The shift. 0 takes 340 Hz.
pub const SHIFT: Setting<field::Hz> = Setting::new("shift_hz", "Shift", "Hz", 0.0)
    .range(0.0, 600.0, 1.0)
    .help("0 takes 340 Hz.");
/// The speed.
pub const BAUD: Setting<field::F64> = Setting::new("baud", "Speed", "Bd", 200.0)
    .range(50.0, 600.0, 0.1)
    .help("Not confirmed by a public document.")
    .advanced();
/// The most user bytes a frame may have.
pub const MAX_LEN: Setting<field::I64> = Setting::new("max_len", "Longest frame", "B", 40)
    .range(3.0, 255.0, 1.0)
    .help("Not confirmed by a public document.")
    .advanced();
/// The centre of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1500.0)
    .range(400.0, 3500.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the EFR receiver.
    pub enum EfrRxSetting {
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Speed.
        Baud = BAUD,
        /// Longest frame.
        MaxLen = MAX_LEN,
    }
}

sdrkit_core::settings! {
    /// The settings of the EFR stack.
    pub enum EfrSetting {
        /// Shift.
        Shift = SHIFT,
        /// Polarity.
        Polarity = POLARITY,
        /// Speed.
        Baud = BAUD,
        /// Longest frame.
        MaxLen = MAX_LEN,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "IEC 60870-5-1, IEC 60870-5-2 and DIN 19244 are sold, so nothing here is confirmed by a public \
document: the FT1.2 frame and character, the 200 Bd speed and the 340 Hz shift (settings), the reading of the first three user \
bytes as a message number and two addresses, and the limit of 40 user bytes (a setting). The payloads are shown as hex.";

/// The bits before a character lock is tried.
const LOCK_BITS: usize = 200;

#[derive(Clone, Copy, Debug)]
struct Row {
    time: f64,
    good: bool,
}

/// The EFR layer.
pub struct Efr {
    shift: f64,
    polarity: Polarity,
    baud: f64,
    max_len: i64,
    bits: Vec<u8>,
    locked: Option<bool>,
    err: f64,
    reader: Ft12Reader,
    rows: Vec<(Row, Vec<u8>)>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
}

impl Default for Efr {
    fn default() -> Self {
        Self::new()
    }
}

impl Efr {
    /// The layer at the default settings.
    pub fn new() -> Self {
        Efr {
            shift: 0.0,
            polarity: Polarity::Auto,
            baud: 200.0,
            max_len: 40,
            bits: Vec::new(),
            locked: None,
            err: 0.0,
            reader: Ft12Reader::new(3, 40),
            rows: Vec::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
        }
    }

    fn try_lock(&mut self) -> Option<(bool, usize)> {
        let mut best = (false, 0usize, 0.0f64);
        for &inv in self.polarity.tries() {
            for off in 0..11 {
                let mut ok = 0;
                for c in 0..9 {
                    let s = off + 11 * c;
                    let b = |i: usize| self.bits[s + i] ^ u8::from(inv);
                    if b(0) == 0 && b(10) == 1 {
                        ok += 1;
                    }
                }
                let f = f64::from(ok) / 9.0;
                if f > best.2 {
                    best = (inv, off, f);
                }
            }
        }
        (best.2 >= 0.9).then_some((best.0, best.1))
    }

    fn frame(&mut self, ev: Ft12Event, sink: &mut Sink) {
        let (good, user) = match ev {
            Ft12Event::Frame(u) => (true, u),
            Ft12Event::BadChecksum(u) => (false, u),
        };
        let at = |i: usize| i64::from(user.get(i).copied().unwrap_or(0));
        let payload: Vec<u8> = user.iter().skip(3).copied().collect();
        let hex: String = payload.iter().map(|b| format!("{b:02X}")).collect();
        let text = if good {
            sink.good += 1;
            sink.chars.push_str(&hex);
            sink.chars.push('\n');
            format!(
                "EFR telegram ok: message {}, address {} / {}, {} data bytes: {hex}",
                at(0),
                at(1),
                at(2),
                payload.len()
            )
        } else {
            sink.bad += 1;
            format!("EFR telegram: bad checksum ({} user bytes)", user.len())
        };
        let check = if good { "good" } else { "bad" };
        sink.events.push(EFR_TELEGRAM.event(
            sink.time,
            sink.dial,
            &EfrTelegramEvent {
                text: &text,
                checksum: check,
                message: at(0),
                address1: at(1),
                address2: at(2),
                payload_bytes: payload.len() as i64,
                payload: &hex,
            },
        ));
        self.rows.push((
            Row {
                time: sink.time,
                good,
            },
            user,
        ));
        self.new_rows += 1;
        if self.rows.len() > 200 {
            self.rows.remove(0);
        }
    }
}

impl Layer for Efr {
    fn name(&self) -> &'static str {
        "hfdata.efr"
    }
    fn schema(&self) -> Arc<Schema> {
        EfrRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (EfrRxSetting::Shift, Value::Hz(self.shift)),
            (EfrRxSetting::Polarity, Value::Choice(self.polarity.index())),
            (EfrRxSetting::Baud, Value::F64(self.baud)),
            (EfrRxSetting::MaxLen, Value::I64(self.max_len)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<bool, SettingError> {
        match EfrRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EfrRxSetting::Shift => self.shift = SHIFT.read(id, value)?,
            EfrRxSetting::Polarity => {
                self.polarity = POLARITY.read(id, value)?;
                return Ok(false);
            }
            EfrRxSetting::Baud => self.baud = BAUD.read(id, value)?,
            EfrRxSetting::MaxLen => {
                self.max_len = MAX_LEN.read(id, value)?;
                self.reader = Ft12Reader::new(3, self.max_len.clamp(3, 255) as u8);
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn front(&self) -> FrontSpec {
        let mut c = BinaryFskConfig::new(RATE, &[]);
        c.baud = Some(self.baud);
        c.shift_hz = Some(if self.shift > 0.0 { self.shift } else { 340.0 });
        c.shift_tol_hz = 40.0;
        FrontSpec::Fsk(c)
    }
    fn declare(&self, d: ResultsDecl) -> ResultsDecl {
        d.with_table(&EFR_TELEGRAMS).event(&EFR_TELEGRAM)
    }
    fn note(&self) -> &'static str {
        NOTE_TEXT
    }
    fn front_locked(&mut self, _lock: FrontLock) {}
    fn symbols(&mut self, s: Symbols<'_>, sink: &mut Sink) {
        let Symbols::Soft(soft) = s else { return };
        self.bits.extend(soft.iter().map(|&v| u8::from(v > 0.0)));
        loop {
            match self.locked {
                None => {
                    if self.bits.len() < LOCK_BITS {
                        return;
                    }
                    match self.try_lock() {
                        Some((inv, off)) => {
                            self.locked = Some(inv);
                            self.bits.drain(..off);
                            self.err = 0.0;
                            self.reader.reset();
                            sink.lock(
                                format!("EFR character sync: {:.0} Bd", self.baud),
                                format!("{:.0} Bd, DIN 19244 frames", self.baud),
                            );
                        }
                        None => {
                            self.bits.drain(..11);
                        }
                    }
                }
                Some(inv) => {
                    if self.bits.len() < 11 {
                        return;
                    }
                    let ch: Vec<u8> = self.bits.drain(..11).map(|b| b ^ u8::from(inv)).collect();
                    match ft12::read_char(&ch) {
                        CharRead::Byte(b) => {
                            self.err *= 0.9;
                            if let Some(ev) = self.reader.push(b) {
                                self.frame(ev, sink);
                            }
                        }
                        CharRead::Parity(_) => {
                            self.err *= 0.9;
                            sink.bad += 1;
                            self.reader.reset();
                        }
                        CharRead::Framing => {
                            self.err = 0.9 * self.err + 1.0;
                            if self.err > 3.0 {
                                self.locked = None;
                                self.reader.reset();
                                sink.lose("character sync lost");
                            }
                        }
                    }
                }
            }
        }
    }
    fn decoding(&self) -> bool {
        self.locked.is_some()
    }
    fn detail(&self) -> String {
        format!("{:.0} Bd, DIN 19244 frames", self.baud)
    }
    fn reset(&mut self) {
        self.bits.clear();
        self.locked = None;
        self.err = 0.0;
        self.reader.reset();
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.new_rows = 0;
        self.cleared = true;
    }
    fn report(&mut self, res: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            res.clear_rows(&EFR_TELEGRAMS);
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        for (k, (r, user)) in self.rows.iter().skip(start).enumerate() {
            let key = format!("{}", self.serial + k as u64);
            let at = |i: usize| i64::from(user.get(i).copied().unwrap_or(0));
            let payload: String = user.iter().skip(3).map(|b| format!("{b:02X}")).collect();
            EFR_TELEGRAMS.put(
                res,
                &key,
                &EfrTelegramRow {
                    time: Time(r.time),
                    checksum: if r.good { "good" } else { "bad" },
                    message: at(0),
                    address1: at(1),
                    address2: at(2),
                    payload_bytes: user.len().saturating_sub(3) as i64,
                    payload: &payload,
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
    }
}

/// The user bytes of the test telegram: message 37, addresses 12 and 34, the first 16 bytes of `text`.
pub fn test_user_bytes(text: &str) -> Vec<u8> {
    let mut u = vec![37, 12, 34];
    u.extend(text.bytes().take(16));
    u
}

/// The bits of a test telegram cycle: 30 mark bits, the frame of `user`, 120 mark bits, twice.
pub fn test_bits(user: &[u8]) -> Vec<u8> {
    let mut bits = Vec::new();
    for _ in 0..2 {
        bits.extend(std::iter::repeat_n(1u8, 30));
        for b in ft12::frame(user) {
            bits.extend(ft12::char_bits(b));
        }
        bits.extend(std::iter::repeat_n(1u8, 120));
    }
    bits
}

/// The test signal: [`test_bits`] of the test text at 200 Bd, mark on the higher tone, 340 Hz shift around the centre.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let c = cfg.settings.get(&GEN_CENTRE_HZ);
    let bits = test_bits(&test_user_bytes(crate::serial::TEST_TEXT));
    let seq: Vec<(f64, Option<f64>)> = bits
        .iter()
        .map(|&b| {
            (
                1.0 / 200.0,
                Some(if b == 1 { c + 170.0 } else { c - 170.0 }),
            )
        })
        .collect();
    Ok(Box::new(ToneSeqGen::new(cfg.rate_hz, &seq, 600.0)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    EfrSetting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("efr"),
        "EFR",
        TtyDecoder::new(Efr::new()),
        cfg,
    )
}

/// EFR teleswitch.
pub static EFR: StackDescriptor = StackDescriptor {
    id: "efr",
    name: "EFR teleswitch",
    group: "HF utility",
    blurb: "Long-wave load-control telegrams: 2-FSK at 200 Bd, 11-bit characters, FT1.2 frames with a checksum; the message number, two addresses and the payload as hex.",
    tuning: crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ),
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::EFR),
    benchmarks: Some(crate::benchmarks),
};
