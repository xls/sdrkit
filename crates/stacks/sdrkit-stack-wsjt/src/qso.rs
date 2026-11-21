//! The transmit side's logic: the QSO sequencer, the WSPR beacon schedule and the JS8 sender. Nothing here keys a
//! transmitter: each produces the message for the next transmit slot (stream kind `wsjt.tx`), and the host's transmit path
//! decides whether audio is sent.

use crate::mode::{Mode, Sub};
use crate::proto::js8msg::{self, Frame, Outgoing};
use crate::proto::msg77::PackError;
use crate::proto::wsprmsg;
use crate::rx::slot::Decode;
use serde::{Deserialize, Serialize};

/// A message to send (stream kind `wsjt.tx`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TxMessage {
    /// The mode.
    pub mode: Mode,
    /// The text (JS8: the frame's display text).
    pub text: String,
    /// Sub-mode index.
    pub submode: u8,
    /// Period, s.
    pub period: f64,
    /// Audio frequency of tone 0, Hz.
    pub hz: f64,
    /// The slot it is for, UTC (the audio starts at slot + t0).
    pub slot: f64,
    /// JS8: the 72 frame bits and the flags.
    pub js8: Option<(Vec<u8>, u8)>,
}

/// The sequencer's states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QsoState {
    /// Nothing to send.
    Idle,
    /// Calling CQ.
    Cq,
    /// Answering with my grid.
    Grid,
    /// Sending a report.
    Report,
    /// Sending R + report.
    RReport,
    /// Sending RR73 (RRR in JT65, JT9, JT4).
    Rr73,
    /// Sending 73.
    S73,
}

/// The sequencer's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct QsoSettings {
    /// My call.
    pub my_call: String,
    /// My grid (4 characters used).
    pub my_grid: String,
    /// The mode.
    pub mode: Mode,
    /// Transmit in the even slots.
    pub even: bool,
    /// Auto-sequence.
    pub auto_seq: bool,
    /// Keep the TX audio frequency when calling a station.
    pub hold_tx: bool,
    /// TX audio frequency, Hz.
    pub tx_hz: f64,
    /// Transmissions of one message without an answer.
    pub retries: u32,
    /// Seconds without a user action before halting.
    pub watchdog_s: f64,
}

impl Default for QsoSettings {
    fn default() -> Self {
        QsoSettings {
            my_call: String::new(),
            my_grid: String::new(),
            mode: Mode::Ft8,
            even: true,
            auto_seq: true,
            hold_tx: false,
            tx_hz: 1500.0,
            retries: 4,
            watchdog_s: 360.0,
        }
    }
}

/// A logged QSO.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QsoRecord {
    /// Their call.
    pub call: String,
    /// Their grid.
    pub grid: String,
    /// The mode.
    pub mode: Mode,
    /// Report sent.
    pub sent: String,
    /// Report received.
    pub received: String,
    /// My call.
    pub my_call: String,
    /// My grid.
    pub my_grid: String,
    /// Start, UTC.
    pub start: f64,
    /// End, UTC.
    pub end: f64,
}

/// What a received message is (the parser).
#[derive(Clone, Debug, PartialEq)]
pub enum Parsed {
    /// `CQ [modifier] CALL [GRID]`.
    Cq {
        /// Caller.
        call: String,
        /// Grid.
        grid: String,
    },
    /// TO FROM GRID.
    Grid {
        /// To.
        to: String,
        /// From.
        from: String,
        /// Grid.
        grid: String,
    },
    /// TO FROM -NN.
    Report {
        /// To.
        to: String,
        /// From.
        from: String,
        /// Report.
        rpt: String,
    },
    /// TO FROM R-NN.
    RReport {
        /// To.
        to: String,
        /// From.
        from: String,
        /// Report.
        rpt: String,
    },
    /// RR73 or RRR.
    Rr73 {
        /// To.
        to: String,
        /// From.
        from: String,
    },
    /// 73.
    S73 {
        /// To.
        to: String,
        /// From.
        from: String,
    },
}

fn bare(c: &str) -> String {
    c.trim_matches(|x| x == '<' || x == '>').to_string()
}

/// Parses a message for the sequencer.
pub fn parse(text: &str) -> Option<Parsed> {
    let w: Vec<&str> = text.split_whitespace().collect();
    if w.first() == Some(&"CQ") {
        let rest: Vec<&str> = w[1..].to_vec();
        let (call, grid) = match rest.as_slice() {
            [c] => (*c, ""),
            [m, c]
                if m.len() <= 4
                    && m.chars().all(|x| x.is_ascii_uppercase())
                    && !crate::proto::msg77::is_grid4(c) =>
            {
                (*c, "")
            }
            [c, g] if crate::proto::msg77::is_grid4(g) => (*c, *g),
            [_, c, g] => (*c, *g),
            _ => return None,
        };
        return Some(Parsed::Cq {
            call: bare(call),
            grid: grid.to_string(),
        });
    }
    if w.len() < 3 {
        return None;
    }
    let (to, from) = (bare(w[0]), bare(w[1]));
    let last = w[w.len() - 1];
    let is_rpt = |s: &str| {
        s.len() == 3
            && (s.starts_with('+') || s.starts_with('-'))
            && s[1..].chars().all(|c| c.is_ascii_digit())
    };
    Some(match last {
        "RR73" | "RRR" => Parsed::Rr73 { to, from },
        "73" => Parsed::S73 { to, from },
        s if is_rpt(s) => Parsed::Report {
            to,
            from,
            rpt: s.into(),
        },
        s if s.starts_with('R') && is_rpt(&s[1..]) => Parsed::RReport {
            to,
            from,
            rpt: s[1..].into(),
        },
        s if crate::proto::msg77::is_grid4(s) => Parsed::Grid {
            to,
            from,
            grid: s.into(),
        },
        _ => return None,
    })
}

/// A report as sent: the SNR clamped to -30..+32, signed, two digits.
pub fn report_of(snr: i32) -> String {
    let r = snr.clamp(-30, 32);
    if r >= 0 {
        format!("+{r:02}")
    } else {
        format!("-{:02}", -r)
    }
}

/// An event of the sequencer.
#[derive(Clone, Debug, PartialEq)]
pub enum QsoEvent {
    /// A QSO was logged.
    Logged(QsoRecord),
    /// The retry limit was reached.
    Retries,
    /// The watchdog halted.
    Watchdog,
}

/// The QSO sequencer.
#[derive(Clone, Debug)]
pub struct Sequencer {
    /// Settings.
    pub s: QsoSettings,
    /// State.
    pub state: QsoState,
    /// The other station.
    pub dx: String,
    /// Its grid.
    pub dx_grid: String,
    /// The report I send.
    pub sent: String,
    /// The report I received.
    pub received: String,
    from_cq: bool,
    tries: u32,
    last_action: f64,
    start: f64,
    logged: bool,
    /// The TX audio frequency.
    pub tx_hz: f64,
}

impl Sequencer {
    /// A sequencer, idle.
    pub fn new(s: QsoSettings) -> Sequencer {
        let tx = s.tx_hz;
        Sequencer {
            s,
            state: QsoState::Idle,
            dx: String::new(),
            dx_grid: String::new(),
            sent: String::new(),
            received: String::new(),
            from_cq: false,
            tries: 0,
            last_action: 0.0,
            start: 0.0,
            logged: false,
            tx_hz: tx,
        }
    }
    fn grid4(&self) -> String {
        self.s
            .my_grid
            .chars()
            .take(4)
            .collect::<String>()
            .to_ascii_uppercase()
    }
    fn rr(&self) -> &'static str {
        if matches!(self.s.mode, Mode::Jt65 | Mode::Jt9 | Mode::Jt4) {
            "RRR"
        } else {
            "RR73"
        }
    }
    /// The user starts calling CQ.
    pub fn call_cq(&mut self, now: f64) {
        self.state = QsoState::Cq;
        self.from_cq = true;
        self.dx.clear();
        self.tries = 0;
        self.last_action = now;
    }
    /// The user answers a decode (a CQ or a call): the reply goes in the parity opposite to the slot it was heard in.
    pub fn answer(&mut self, d: &Decode, period: f64, now: f64) {
        let Some(p) = parse(&d.text) else { return };
        let (call, grid) = match p {
            Parsed::Cq { call, grid } => (call, grid),
            Parsed::Grid { from, grid, .. } => (from, grid),
            Parsed::Report { from, .. }
            | Parsed::RReport { from, .. }
            | Parsed::Rr73 { from, .. }
            | Parsed::S73 { from, .. } => (from, String::new()),
        };
        self.dx = call;
        self.dx_grid = grid;
        self.sent = report_of(d.snr);
        self.from_cq = false;
        self.state = QsoState::Grid;
        self.tries = 0;
        self.logged = false;
        self.start = now;
        self.last_action = now;
        self.s.even = ((d.slot / period).round() as i64) % 2 != 0;
        if !self.s.hold_tx {
            self.tx_hz = d.hz;
        }
    }
    /// Halt: idle at once.
    pub fn halt(&mut self) {
        self.state = QsoState::Idle;
    }
    /// The message for the next transmission.
    pub fn message(&self) -> Option<String> {
        let me = &self.s.my_call;
        Some(match self.state {
            QsoState::Idle => return None,
            QsoState::Cq => format!("CQ {me} {}", self.grid4()),
            QsoState::Grid => format!("{} {me} {}", self.dx, self.grid4()),
            QsoState::Report => format!("{} {me} {}", self.dx, self.sent),
            QsoState::RReport => format!("{} {me} R{}", self.dx, self.sent),
            QsoState::Rr73 => format!("{} {me} {}", self.dx, self.rr()),
            QsoState::S73 => format!("{} {me} 73", self.dx),
        })
    }
    fn log(&mut self, now: f64, out: &mut Vec<QsoEvent>) {
        if self.logged {
            return;
        }
        self.logged = true;
        out.push(QsoEvent::Logged(QsoRecord {
            call: self.dx.clone(),
            grid: self.dx_grid.clone(),
            mode: self.s.mode,
            sent: self.sent.clone(),
            received: self.received.clone(),
            my_call: self.s.my_call.clone(),
            my_grid: self.grid4(),
            start: self.start,
            end: now,
        }));
    }
    fn done(&mut self) {
        self.state = if self.from_cq {
            QsoState::Cq
        } else {
            QsoState::Idle
        };
        if self.from_cq {
            self.dx.clear();
        }
        self.tries = 0;
    }
    fn go(&mut self, s: QsoState) {
        self.state = s;
        self.tries = 0;
    }
    /// Takes a decode; returns events.
    pub fn on_decode(&mut self, d: &Decode, now: f64) -> Vec<QsoEvent> {
        let mut ev = Vec::new();
        if !self.s.auto_seq || self.state == QsoState::Idle {
            return ev;
        }
        let Some(p) = parse(&d.text) else { return ev };
        let me = self.s.my_call.as_str();
        let (to, from) = match &p {
            Parsed::Cq { .. } => return ev,
            Parsed::Grid { to, from, .. }
            | Parsed::Report { to, from, .. }
            | Parsed::RReport { to, from, .. }
            | Parsed::Rr73 { to, from }
            | Parsed::S73 { to, from } => (to.clone(), from.clone()),
        };
        if to != me {
            return ev;
        }
        if self.state == QsoState::Cq {
            // A reply to my CQ: the replying station becomes the DX.
            self.dx = from.clone();
            self.dx_grid.clear();
            self.start = now;
            self.logged = false;
            self.sent = report_of(d.snr);
            if !self.s.hold_tx {
                self.tx_hz = d.hz;
            }
        } else if from != self.dx {
            return ev;
        }
        use QsoState::*;
        match (self.state, &p) {
            (Cq, Parsed::Grid { grid, .. }) => {
                self.dx_grid = grid.clone();
                self.go(Report)
            }
            (Cq, Parsed::Report { rpt, .. }) | (Grid, Parsed::Report { rpt, .. }) => {
                self.received = rpt.clone();
                self.go(RReport)
            }
            (Grid, Parsed::RReport { rpt, .. }) | (Report, Parsed::RReport { rpt, .. }) => {
                self.received = rpt.clone();
                self.go(Rr73);
                self.log(now, &mut ev);
            }
            (Grid, Parsed::Rr73 { .. }) | (RReport, Parsed::Rr73 { .. }) => {
                self.go(S73);
                self.log(now, &mut ev);
            }
            (Grid, Parsed::S73 { .. })
            | (Report, Parsed::Rr73 { .. })
            | (Report, Parsed::S73 { .. })
            | (RReport, Parsed::S73 { .. }) => {
                self.log(now, &mut ev);
                self.done();
            }
            (Rr73, Parsed::S73 { .. }) => self.done(),
            (Rr73, Parsed::RReport { .. }) => self.go(Rr73),
            _ => {}
        }
        ev
    }
    /// A transmission was sent (a sent 73 ends the QSO); counts retries.
    pub fn sent_one(&mut self, now: f64) -> Vec<QsoEvent> {
        let mut ev = Vec::new();
        if self.state == QsoState::S73 {
            self.done();
            return ev;
        }
        if self.state == QsoState::Idle {
            return ev;
        }
        self.tries += 1;
        if self.tries > self.s.retries && self.state != QsoState::Cq {
            ev.push(QsoEvent::Retries);
            if self.from_cq {
                self.state = QsoState::Cq;
                self.tries = 0;
            } else {
                self.state = QsoState::Idle;
            }
        }
        if now - self.last_action > self.s.watchdog_s {
            ev.push(QsoEvent::Watchdog);
            self.state = QsoState::Idle;
        }
        ev
    }
    /// Whether the slot (UTC, period) is one I transmit in.
    pub fn is_tx_slot(&self, slot: f64, period: f64) -> bool {
        let k = (slot / period).round() as i64;
        (k % 2 == 0) == self.s.even
    }
}

/// The WSPR beacon schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct Beacon {
    /// Call.
    pub call: String,
    /// Grid.
    pub grid: String,
    /// Power, dBm.
    pub dbm: i32,
    /// One slot in N.
    pub every: u32,
    /// Which of the N.
    pub offset: u32,
    /// Audio frequency (clamped to 1400..1600).
    pub hz: f64,
    /// Transmissions left without operator action.
    pub left: u32,
}

impl Beacon {
    /// Transmissions without operator action.
    pub const MAX_TX: u32 = 30;
    /// A beacon.
    pub fn new(call: &str, grid: &str, dbm: i32, every: u32, offset: u32, hz: f64) -> Beacon {
        Beacon {
            call: call.to_ascii_uppercase(),
            grid: grid
                .chars()
                .take(4)
                .collect::<String>()
                .to_ascii_uppercase(),
            dbm,
            every: every.max(1),
            offset,
            hz: hz.clamp(1400.0, 1600.0),
            left: Self::MAX_TX,
        }
    }
    /// The message, or `None` when the call, grid or power is invalid.
    pub fn message(&self) -> Option<String> {
        let t = format!("{} {} {}", self.call, self.grid, self.dbm);
        wsprmsg::pack50(&t).ok().map(|_| t)
    }
    /// The operator touched a control.
    pub fn touched(&mut self) {
        self.left = Self::MAX_TX;
    }
    /// Whether this slot (UTC of a 120 s slot) is a beacon slot; counts it. `None` after "maxtx".
    pub fn slot(&mut self, utc: f64) -> Option<TxMessage> {
        let k = (utc / 120.0).round() as i64;
        if k.rem_euclid(self.every as i64) != self.offset as i64 % self.every as i64 {
            return None;
        }
        if self.left == 0 {
            return None;
        }
        let text = self.message()?;
        self.left -= 1;
        Some(TxMessage {
            mode: Mode::Wspr,
            text,
            submode: 0,
            period: 120.0,
            hz: self.hz,
            slot: utc,
            js8: None,
        })
    }
}

/// The JS8 sender: messages become frames, one per slot of the chosen sub-mode.
#[derive(Clone, Debug, Default)]
pub struct Js8Sender {
    queue: std::collections::VecDeque<(Vec<u8>, u8, String)>,
    /// Sub-mode.
    pub submode: u8,
    /// Audio frequency.
    pub hz: f64,
}

impl Js8Sender {
    /// Queues a message.
    pub fn send(&mut self, m: &Outgoing) -> Result<usize, PackError> {
        let frames = js8msg::frames_of(m)?;
        let n = frames.len();
        for (f, flags) in frames {
            let bits = f.pack()?;
            self.queue.push_back((bits, flags, f.display()));
        }
        Ok(n)
    }
    /// The frame for a slot of the sub-mode.
    pub fn slot(&mut self, utc: f64) -> Option<TxMessage> {
        let (bits, flags, text) = self.queue.pop_front()?;
        let sub = Sub::new(Mode::Js8).with_sub(self.submode);
        Some(TxMessage {
            mode: Mode::Js8,
            text,
            submode: self.submode,
            period: sub.period,
            hz: self.hz,
            slot: utc,
            js8: Some((bits, flags)),
        })
    }
    /// Drops the frames waiting.
    pub fn clear(&mut self) {
        self.queue.clear();
    }
    /// Frames waiting.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

/// Reassembles queued frames as the receiver does (for tests and previews): the texts of the frames joined.
pub fn reassemble(frames: &[(Frame, u8)]) -> String {
    frames.iter().map(|(f, _)| f.display()).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str, snr: i32) -> Decode {
        Decode {
            slot: 15.0,
            mode: Mode::Ft8,
            label: "FT8".into(),
            snr,
            dt: 0.0,
            hz: 1200.0,
            text: text.into(),
            mirrored: false,
            cq: text.starts_with("CQ"),
            call: String::new(),
            grid: String::new(),
            dbm: -1,
            pass: 0,
            dial: 0.0,
            deep: false,
            how: Default::default(),
            early: false,
            band: None,
        }
    }

    #[test]
    fn a_qso_from_my_cq() {
        let mut s = Sequencer::new(QsoSettings {
            my_call: "K1ABC".into(),
            my_grid: "FN42".into(),
            ..Default::default()
        });
        s.call_cq(0.0);
        assert_eq!(s.message().unwrap(), "CQ K1ABC FN42");
        s.on_decode(&dec("K1ABC W9XYZ EN37", -12), 1.0);
        assert_eq!(s.state, QsoState::Report);
        assert_eq!(s.message().unwrap(), "W9XYZ K1ABC -12");
        let ev = s.on_decode(&dec("K1ABC W9XYZ R-05", -10), 2.0);
        assert_eq!(s.state, QsoState::Rr73);
        assert!(
            matches!(&ev[0], QsoEvent::Logged(r) if r.call == "W9XYZ" && r.received == "-05" && r.grid == "EN37")
        );
        s.on_decode(&dec("K1ABC W9XYZ 73", -10), 3.0);
        assert_eq!(s.state, QsoState::Cq);
    }

    #[test]
    fn answering_and_retries() {
        let mut s = Sequencer::new(QsoSettings {
            my_call: "W9XYZ".into(),
            my_grid: "EN37".into(),
            mode: Mode::Jt65,
            ..Default::default()
        });
        s.answer(&dec("CQ K1ABC FN42", -40), 60.0, 0.0);
        assert_eq!(s.message().unwrap(), "K1ABC W9XYZ EN37");
        assert_eq!(s.sent, "-30");
        s.on_decode(&dec("W9XYZ K1ABC -15", -12), 1.0);
        assert_eq!(s.message().unwrap(), "K1ABC W9XYZ R-30");
        let ev = s.on_decode(&dec("W9XYZ K1ABC RRR", -12), 2.0);
        assert_eq!(s.state, QsoState::S73);
        assert!(matches!(ev[0], QsoEvent::Logged(_)));
        s.sent_one(3.0);
        assert_eq!(s.state, QsoState::Idle);
        s.answer(&dec("CQ G4ABC IO91", 0), 60.0, 10.0);
        let mut retries = false;
        for _ in 0..6 {
            retries |= s.sent_one(11.0).contains(&QsoEvent::Retries);
        }
        assert!(retries);
        assert_eq!(s.state, QsoState::Idle);
    }

    #[test]
    fn beacon_schedule() {
        let mut b = Beacon::new("k1abc", "FN42ab", 37, 5, 2, 1700.0);
        assert_eq!(b.hz, 1600.0);
        assert_eq!(b.message().unwrap(), "K1ABC FN42 37");
        let sent: Vec<i64> = (0..20)
            .filter(|&k| b.slot(k as f64 * 120.0).is_some())
            .collect();
        assert_eq!(sent, vec![2, 7, 12, 17]);
        assert!(Beacon::new("K1ABC", "FN42", 38, 1, 0, 1500.0)
            .message()
            .is_none());
        let mut b = Beacon::new("K1ABC", "FN42", 37, 1, 0, 1500.0);
        let n = (0..40)
            .filter(|&k| b.slot(k as f64 * 120.0).is_some())
            .count();
        assert_eq!(n, 30);
    }

    #[test]
    fn js8_sender_frames() {
        let mut s = Js8Sender::default();
        assert_eq!(
            s.send(&Outgoing::Text {
                from: "G4ABC".into(),
                to: "DL1XYZ".into(),
                text: "HELLO FROM LONDON. HOW COPY?".into()
            })
            .unwrap(),
            4
        );
        let f = js8msg::frames_of(&Outgoing::Relay {
            from: "EA3XYZ".into(),
            via: "OK1XX".into(),
            dest: "SP5AB".into(),
            text: "PLEASE RELAY".into(),
        })
        .unwrap();
        assert_eq!(reassemble(&f), "EA3XYZ: OK1XX>SP5AB PLEASE RELAY");
        let f = js8msg::frames_of(&Outgoing::Text {
            from: "G4ABC".into(),
            to: "DL1XYZ".into(),
            text: "HELLO FROM LONDON. HOW COPY?".into(),
        })
        .unwrap();
        assert_eq!(reassemble(&f), "G4ABC: DL1XYZ HELLO FROM LONDON. HOW COPY?");
        assert!(s.slot(0.0).is_some());
        assert_eq!(s.pending(), 3);
        assert!(js8msg::frames_of(&Outgoing::Heartbeat {
            call: "TOOLONGCALLSIGN".into(),
            grid: "KO24".into(),
            cq: None
        })
        .is_err());
    }
}
