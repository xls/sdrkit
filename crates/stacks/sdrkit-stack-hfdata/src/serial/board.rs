//! What a serial-tone modem publishes: the received text (bytes least significant bit first, printable bytes as themselves,
//! others as `[XX]`, the last 6000 characters), the D_PDUs found in the bits, its events and its status.

use crate::typed::{DpduRow, SerialFields, DPDUS};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{Clock, Event, LockState, ResultsDecl, Status};
use sdrkit_graph::{Format, Kind, Outbox, Port, RateFrom};
use sdrkit_proto_link::s5066::{type_name, DPdu, DPduFinder};
use std::collections::VecDeque;

/// The characters of text kept.
pub const TEXT_CAP: usize = 6000;
/// The most bytes of a message kept for its message event.
const MESSAGE_CAP: usize = 65_536;
/// The bytes of a message that wait before they are shown.
const HELD: usize = 4;

/// The events of a modem stack.
pub struct Layout {
    /// The declaration of the two events.
    pub events: fn(ResultsDecl) -> ResultsDecl,
    /// A status event (sync, end of message, loss) at a time and a frequency, with its text.
    pub event: fn(f64, f64, &str) -> Event,
    /// A message at a time and a frequency, with its text and its number of bytes.
    pub message: fn(f64, f64, &str, i64) -> Event,
}

impl Layout {
    /// The declaration of the results.
    pub fn declaration(&self) -> ResultsDecl {
        (self.events)(
            ResultsDecl::new()
                .with_fields(SerialFields::SHAPE.fields)
                .with_table(&DPDUS),
        )
    }
}

/// How a byte is shown: printable ASCII, LF and CR as themselves, any other as `[XX]`.
pub fn show_byte(b: u8, s: &mut String) {
    if (0x20..0x7F).contains(&b) || b == b'\n' || b == b'\r' {
        s.push(b as char);
    } else {
        use std::fmt::Write;
        let _ = write!(s, "[{b:02X}]");
    }
}

/// The hex digits of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s
}

/// The results of a modem: text, counters, D_PDUs and status.
pub struct Board {
    layout: &'static Layout,
    clock: Option<Clock>,
    utc0: Option<f64>,
    dial_hz: f64,
    text: String,
    message: Vec<u8>,
    acc: u8,
    nbits: u8,
    finder: DPduFinder,
    rows: VecDeque<(f64, DPdu)>,
    new_rows: usize,
    serial: u64,
    cleared: bool,
    /// Messages ended.
    pub messages: i64,
    /// Interleaver blocks decoded.
    pub blocks: i64,
    /// D_PDUs with a good header CRC.
    pub dpdus: i64,
    /// The rate and interleaver.
    pub waveform: String,
    /// The SNR, dB.
    pub snr_db: Option<f64>,
    /// The frequency error, Hz.
    pub offset_hz: Option<f64>,
    /// The status shown.
    pub status: Status,
}

impl Board {
    /// An empty board.
    pub fn new(layout: &'static Layout) -> Board {
        Board {
            layout,
            clock: None,
            utc0: None,
            dial_hz: 0.0,
            text: String::new(),
            message: Vec::new(),
            acc: 0,
            nbits: 0,
            finder: DPduFinder::new(),
            rows: VecDeque::new(),
            new_rows: 0,
            serial: 0,
            cleared: false,
            messages: 0,
            blocks: 0,
            dpdus: 0,
            waveform: String::new(),
            snr_db: None,
            offset_hz: None,
            status: Status::default(),
        }
    }

    /// Takes the host's clock.
    pub fn attach(&mut self, clock: &Clock) {
        self.clock = Some(clock.clone());
    }

    /// Takes the dial of the input.
    pub fn configure(&mut self, input: &Format) {
        self.dial_hz = input.tuned_hz;
    }

    /// Notes that signal time `t` (seconds from the first sample) came now.
    pub fn saw(&mut self, t: f64) {
        if self.utc0.is_none() {
            let now = self.clock.as_ref().map_or(0.0, Clock::now);
            self.utc0 = Some(now - t);
        }
    }

    /// The UTC of signal time `t`.
    pub fn utc(&self, t: f64) -> f64 {
        self.utc0.unwrap_or(0.0) + t
    }

    /// A status event at signal time `t`.
    pub fn event(&self, out: &mut Outbox, t: f64, text: &str) {
        out.messages(0)
            .push((self.layout.event)(self.utc(t), self.dial_hz, text));
    }

    /// Starts a message: the byte alignment and the D_PDU finder start again.
    pub fn start_message(&mut self) {
        self.message.clear();
        self.acc = 0;
        self.nbits = 0;
        self.finder.reset();
    }

    /// Takes a decoded bit of the message at signal time `t`.
    pub fn bit(&mut self, out: &mut Outbox, t: f64, bit: u8) {
        if let Some(d) = self.finder.push_bit(bit) {
            self.dpdus += 1;
            let text = format!(
                "D_PDU {} from {} to {}, {} bytes, data CRC {}",
                type_name(d.kind),
                d.source,
                d.destination,
                d.segment.len(),
                d.data_crc.name()
            );
            self.event(out, t, &text);
            self.rows.push_back((self.utc(t), d));
            self.new_rows += 1;
            while self.rows.len() > DPDUS.cap {
                self.rows.pop_front();
            }
        }
        self.acc |= (bit & 1) << self.nbits;
        self.nbits += 1;
        if self.nbits == 8 {
            let b = self.acc;
            self.acc = 0;
            self.nbits = 0;
            self.message.push(b);
            if self.message.len() > MESSAGE_CAP {
                // A broadcast without an end: the oldest bytes go (they were shown already).
                self.message.drain(..MESSAGE_CAP / 2);
            }
            // The last 4 bytes wait: they may be the end-of-message pattern.
            if self.message.len() > HELD {
                let b = self.message[self.message.len() - 1 - HELD];
                self.show(b);
            }
        }
    }

    fn show(&mut self, b: u8) {
        show_byte(b, &mut self.text);
        if self.text.len() > TEXT_CAP + 512 {
            let cut = self.text.len() - TEXT_CAP;
            let cut = (cut..self.text.len())
                .find(|&i| self.text.is_char_boundary(i))
                .unwrap_or(self.text.len());
            self.text.drain(..cut);
        }
    }

    /// Ends a message at signal time `t`, without its last `drop_bytes` bytes (the end-of-message pattern): a message event
    /// with its text.
    pub fn end_message(&mut self, out: &mut Outbox, t: f64, drop_bytes: usize) {
        let n = self.message.len().saturating_sub(drop_bytes);
        let mut s = String::new();
        for &b in &self.message[..n] {
            show_byte(b, &mut s);
        }
        for i in self.message.len().saturating_sub(HELD)..n {
            let b = self.message[i];
            self.show(b);
        }
        self.messages += 1;
        self.text.push('\n');
        let e = (self.layout.message)(self.utc(t), self.dial_hz, &s, n as i64);
        out.messages(0).push(e);
        self.start_message();
    }

    /// The bytes of the message so far.
    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// The text kept.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the lock state and the summary.
    pub fn set_state(&mut self, state: LockState, summary: std::fmt::Arguments<'_>) {
        use std::fmt::Write;
        self.status.lock_state = state;
        self.status.summary.clear();
        let _ = self.status.summary.write_fmt(summary);
    }

    /// Writes the fields, the new D_PDU rows, the note and the status.
    pub fn report(&mut self, p: &mut Published, note: &str) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&DPDUS);
        }
        let start = self.rows.len().saturating_sub(self.new_rows);
        for (k, (utc, d)) in self.rows.iter().skip(start).enumerate() {
            let key = format!("{}", self.serial + k as u64);
            DPDUS.put(
                p,
                &key,
                &DpduRow {
                    utc: Time(*utc),
                    r#type: type_name(d.kind),
                    eot: i64::from(d.eot),
                    destination: &d.destination,
                    source: &d.source,
                    header_bytes: d.header.len() as i64,
                    payload_bytes: d.segment.len() as i64,
                    data_crc: d.data_crc.name(),
                    payload: &hex(&d.segment[..d.segment.len().min(32)]),
                },
            );
        }
        self.serial += self.new_rows as u64;
        self.new_rows = 0;
        p.put(&SerialFields {
            note,
            text: &self.text,
            waveform: &self.waveform,
            messages_total: self.messages,
            blocks_total: self.blocks,
            dpdus_good: self.dpdus,
            snr_db: self.snr_db.filter(|v| v.is_finite()),
            offset_hz: self.offset_hz.filter(|v| v.is_finite()).map(Hertz),
        });
        self.status.snr_db = self.snr_db.filter(|v| v.is_finite()).map(|v| v as f32);
        self.status.offset_hz = self.offset_hz.filter(|v| v.is_finite());
        p.results_mut().set_status(&self.status);
    }

    /// Forgets the text, the counters and the rows. The table is emptied at the next report.
    pub fn clear(&mut self) {
        self.text.clear();
        self.rows.clear();
        self.new_rows = 0;
        self.messages = 0;
        self.blocks = 0;
        self.dpdus = 0;
        self.cleared = true;
    }
}

/// The outputs of a modem decoder: the events.
pub fn outputs() -> Vec<Port> {
    vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
}

/// The formats of [`outputs`].
pub fn formats() -> Vec<Format> {
    vec![Format::new(Kind::Messages, 0.0)]
}
