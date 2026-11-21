//! The channel bank of `horus` (`horus.channels`): above 2.4 Msps the shared decimator (a second-order CIC by the rate over
//! 2 Msps), then the dial channel and the channels that the span search opens, each with its tone estimator and its 4FSK and
//! RTTY demodulators. It publishes the present state of the channels, their table and the spectrum of the dial channel, and
//! sends each packet and a report of each tick to the payload table. A change of frequency clears it.

use super::demod::{Channel, Decoded, Found};
use super::span::SpanSearch;
use super::{Mode, State, MAX_CHANNELS as MAX_CHANNELS_SETTING, SPAN_SEARCH};
use crate::family::write_series;
use crate::typed::{
    HorusChannelClosedEvent, HorusChannelOpenedEvent, HorusChannelRow, HorusFields, HORUS_CHANNELS,
    HORUS_CHANNEL_CLOSED, HORUS_CHANNEL_OPENED, HORUS_SPECTRUM,
};
use sdrkit_algo_dsp::cic::Cic;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    ResultsDecl, SeriesId, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, TagKind,
    Timed,
};
use sdrkit_util_objects::commands as objcmd;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// Channels at most.
pub const MAX_CHANNELS: usize = 8;
/// Above this input rate the shared decimator runs.
const DECIMATE_ABOVE: f64 = 2.4e6;
/// The rate that the shared decimator aims at.
const DECIMATED_RATE: f64 = 2e6;
/// The span search runs from this input rate on.
const SEARCH_RATE: f64 = 40e3;
/// A signal this far from every channel opens one, Hz.
const CHANNEL_APART_HZ: f64 = 3500.0;
/// A searched channel closes when it decoded nothing for this long, s ...
const IDLE_S: f64 = 120.0;
/// ... and its signal was not seen for this long, s.
const UNSEEN_S: f64 = 30.0;
/// A channel decodes while a packet came within this time, s.
const DECODING_S: f64 = 30.0;
/// The best SNR is that of the packets of this time, s.
const SNR_S: f64 = 60.0;
/// Points of the spectrum of the dial channel.
const SPECTRUM_POINTS: usize = 256;

/// A packet of a channel, for the payload table.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The channel (0 is the dial).
    pub channel: usize,
    /// The middle of the tones, Hz from the dial.
    pub offset_hz: f64,
    /// The frequency of the middle of the tones, when the centre of the input is known.
    pub freq_hz: Option<f64>,
    /// The SNR in 3 kHz, dB.
    pub snr_db: f64,
    /// The tone spacing (4FSK) or shift (RTTY), Hz.
    pub spacing_hz: f64,
    /// The packet.
    pub decoded: Decoded,
}

impl Item for Heard {
    const KIND: &'static str = "horus.heard";
}

/// The state of the channels at a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BankReport {
    /// What the channels do.
    pub state: State,
    /// The best SNR of the last 60 s, dB.
    pub snr_db: Option<f64>,
    /// The offset from the dial of the last packet, Hz.
    pub offset_hz: Option<f64>,
    /// The events of the channels since the last report.
    pub events: Vec<Event>,
}

impl Item for BankReport {
    const KIND: &'static str = "horus.report";
}

sdrkit_core::settings! {
    /// The settings of [`ChannelBank`].
    pub enum BankSetting {
        /// Look for other balloons in the span.
        SpanSearch = SPAN_SEARCH,
        /// The channels at most.
        MaxChannels = MAX_CHANNELS_SETTING,
    }
}

sdrkit_core::commands! {
    /// The commands of [`ChannelBank`].
    pub enum BankCommand {
        /// Closes every channel but the dial channel and forgets what the channels measured.
        Clear = objcmd::CLEAR,
    }
}

/// One channel with what the bank knows of it.
struct Slot {
    ch: Channel,
    number: usize,
    searched: bool,
    opened_s: f64,
    last_seen_s: f64,
    last_binary_s: f64,
    last_rtty_s: f64,
    packets: u64,
    snr_db: Option<(f64, f64)>,
}

impl Slot {
    fn new(ch: Channel, number: usize, searched: bool, t: f64) -> Slot {
        Slot {
            ch,
            number,
            searched,
            opened_s: t,
            last_seen_s: t,
            last_binary_s: f64::NEG_INFINITY,
            last_rtty_s: f64::NEG_INFINITY,
            packets: 0,
            snr_db: None,
        }
    }

    fn last_packet_s(&self) -> f64 {
        self.last_binary_s.max(self.last_rtty_s)
    }

    fn mode(&self, t: f64) -> Mode {
        if t - self.last_packet_s() > DECODING_S {
            Mode::Listening
        } else if self.last_binary_s >= self.last_rtty_s {
            Mode::Binary
        } else {
            Mode::Rtty
        }
    }
}

/// The channel bank component.
pub struct ChannelBank {
    rate: f64,
    centre: f64,
    dial: f64,
    front: Option<Cic<Complex>>,
    front_rate: f64,
    buf: Vec<Complex>,
    slots: Vec<Slot>,
    span: Option<SpanSearch>,
    seen: Vec<f64>,
    span_on: bool,
    max: usize,
    t: f64,
    found: Vec<Found>,
    best: VecDeque<(f64, f64)>,
    last_offset: Option<f64>,
    syncs_closed: u64,
    failed_closed: u64,
    events: Vec<Event>,
    published: Published,
    spectrum: SeriesId,
    key: String,
    text: String,
}

type Complex = sdrkit_core::Complex32;

/// What the channel bank publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(HorusFields::SHAPE.fields)
        .with_table(HORUS_CHANNELS.def())
        .with_series(&HORUS_SPECTRUM)
}

impl Default for ChannelBank {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let spectrum = decl
            .series_id(HORUS_SPECTRUM.key)
            .expect("the declaration holds the spectrum");
        ChannelBank {
            rate: 0.0,
            centre: 0.0,
            dial: 0.0,
            front: None,
            front_rate: 0.0,
            buf: Vec::new(),
            slots: Vec::new(),
            span: None,
            seen: Vec::new(),
            span_on: SPAN_SEARCH.default,
            max: MAX_CHANNELS_SETTING.default as usize,
            t: 0.0,
            found: Vec::new(),
            best: VecDeque::new(),
            last_offset: None,
            syncs_closed: 0,
            failed_closed: 0,
            events: Vec::new(),
            published: Published::from_decl(decl),
            spectrum,
            key: String::new(),
            text: String::new(),
        }
    }
}

impl ChannelBank {
    /// The channels: the place from the centre of the input, Hz, and whether the span search opened it.
    pub fn channels(&self) -> Vec<(f64, bool)> {
        self.slots
            .iter()
            .map(|s| (s.ch.offset_hz, s.searched))
            .collect()
    }

    /// The span search is on for this input.
    fn searching(&self) -> bool {
        self.span_on && self.max > 1 && self.rate >= SEARCH_RATE
    }

    /// Makes the decimator, the dial channel and the span search for the input of now, and forgets the rest.
    fn reset(&mut self) {
        let d = if self.rate > DECIMATE_ABOVE {
            (self.rate / DECIMATED_RATE).floor() as usize
        } else {
            1
        };
        self.front = (d > 1).then(|| Cic::new(2, d).ok()).flatten();
        self.front_rate = if self.front.is_some() {
            self.rate / d as f64
        } else {
            self.rate
        };
        self.slots.clear();
        if let Some(ch) = Channel::new(self.front_rate, self.dial) {
            self.slots.push(Slot::new(ch, 0, false, self.t));
        }
        self.span = if self.searching() {
            SpanSearch::new(self.front_rate, self.front.is_some(), self.dial.abs() > 1.0)
        } else {
            None
        };
        self.best.clear();
        self.last_offset = None;
        self.syncs_closed = 0;
        self.failed_closed = 0;
        self.published.clear_rows(HORUS_CHANNELS.def());
    }

    /// Opens a channel on each new signal of the search and closes the searched channels that went quiet.
    fn follow_search(&mut self) {
        let t = self.t;
        let last: Vec<f64> = self
            .span
            .as_ref()
            .map(|s| s.last().to_vec())
            .unwrap_or_default();
        for s in &mut self.slots {
            if last
                .iter()
                .any(|f| (f - s.ch.offset_hz).abs() <= CHANNEL_APART_HZ)
            {
                s.last_seen_s = t;
            }
        }
        for f in std::mem::take(&mut self.seen) {
            let free = self
                .slots
                .iter()
                .all(|s| (f - s.ch.offset_hz).abs() > CHANNEL_APART_HZ);
            if !free || self.slots.len() >= self.max {
                continue;
            }
            let Some(number) =
                (1..MAX_CHANNELS).find(|n| self.slots.iter().all(|s| s.number != *n))
            else {
                continue;
            };
            let Some(ch) = Channel::new(self.front_rate, f) else {
                continue;
            };
            self.slots.push(Slot::new(ch, number, true, t));
            let off = f - self.dial;
            self.text.clear();
            let _ = write!(self.text, "a signal {:+.1} kHz from the dial", off / 1e3);
            let ev = HORUS_CHANNEL_OPENED.event(
                t,
                self.freq(f),
                &HorusChannelOpenedEvent {
                    text: &self.text,
                    channel: number as i64,
                    offset: Hertz(off),
                },
            );
            self.events.push(ev);
        }
        let mut i = 0;
        while i < self.slots.len() {
            let s = &self.slots[i];
            let idle =
                t - s.opened_s.max(s.last_packet_s()) > IDLE_S && t - s.last_seen_s > UNSEEN_S;
            if !(s.searched && idle) {
                i += 1;
                continue;
            }
            let s = self.slots.remove(i);
            self.syncs_closed += s.ch.fsk.syncs;
            self.failed_closed += s.ch.fsk.failed;
            let off = s.ch.offset_hz - self.dial;
            self.text.clear();
            let _ = write!(
                self.text,
                "the signal {:+.1} kHz from the dial is gone",
                off / 1e3
            );
            let ev = HORUS_CHANNEL_CLOSED.event(
                t,
                self.freq(s.ch.offset_hz),
                &HorusChannelClosedEvent {
                    text: &self.text,
                    channel: s.number as i64,
                    offset: Hertz(off),
                },
            );
            self.events.push(ev);
            self.key.clear();
            let _ = write!(self.key, "{}", s.number);
            self.published.remove_row(HORUS_CHANNELS.def(), &self.key);
        }
    }

    /// The frequency `offset_hz` from the centre of the input, or the offset when the centre is not known.
    fn freq(&self, offset_hz: f64) -> f64 {
        self.centre + offset_hz
    }

    fn state(&self) -> State {
        if self
            .slots
            .iter()
            .any(|s| self.t - s.last_packet_s() <= DECODING_S)
        {
            State::Decoding
        } else if self
            .slots
            .iter()
            .any(|s| s.ch.fsk.tones().is_some() || s.ch.rtty.tones().is_some())
        {
            State::Listening
        } else {
            State::Waiting
        }
    }

    fn publish(&mut self) {
        let t = self.t;
        while self.best.front().is_some_and(|(at, _)| t - at > SNR_S) {
            self.best.pop_front();
        }
        let snr = self
            .best
            .iter()
            .map(|b| b.1)
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
        let state = self.state();
        let (mut syncs, mut failed) = (self.syncs_closed, self.failed_closed);
        for s in &self.slots {
            syncs += s.ch.fsk.syncs;
            failed += s.ch.fsk.failed;
        }
        let p = &mut self.published;
        p.put(&HorusFields {
            state,
            channels: self.slots.len() as i64,
            syncs: syncs as i64,
            failed: failed as i64,
            snr_db: snr,
        });
        for s in &self.slots {
            let tones = s.ch.fsk.tones();
            let rtty = s.ch.rtty.tones();
            let tone = |k: usize| tones.map(|f| Hertz(f[k]));
            self.key.clear();
            let _ = write!(self.key, "{}", s.number);
            HORUS_CHANNELS.put(
                p,
                &self.key,
                &HorusChannelRow {
                    offset: Hertz(s.ch.offset_hz - self.dial),
                    decoding: s.mode(t),
                    tone_1: tone(0),
                    tone_2: tone(1),
                    tone_3: tone(2),
                    tone_4: tone(3),
                    rtty_low: rtty.map(|f| Hertz(f[0])),
                    rtty_high: rtty.map(|f| Hertz(f[1])),
                    spacing: tones.map(|f| Hertz((f[3] - f[0]) / 3.0)),
                    snr_db: s.snr_db.filter(|(at, _)| t - at <= SNR_S).map(|(_, v)| v),
                    packets: s.packets as i64,
                    syncs: s.ch.fsk.syncs as i64,
                    failed: s.ch.fsk.failed as i64,
                    searched: s.searched,
                },
            );
        }
        let spectrum = self
            .slots
            .first()
            .and_then(|s| s.ch.est.spectrum_db(SPECTRUM_POINTS));
        let r = p.results_mut();
        match spectrum {
            Some((start, step, v)) => write_series(
                r,
                self.spectrum,
                true,
                v.iter()
                    .enumerate()
                    .map(|(i, &d)| [(start + i as f64 * step) as f32, d]),
            ),
            None => write_series(r, self.spectrum, true, std::iter::empty()),
        }
    }

    /// Runs the channels and the search over `x`, at the rate after the shared decimator.
    fn run(&mut self, x: &[Complex], heard: &mut Vec<Timed<Heard>>) {
        for s in &mut self.slots {
            self.found.clear();
            s.ch.process(x, &mut self.found);
            for f in self.found.drain(..) {
                let offset = s.ch.offset_hz - self.dial + f.centre_hz;
                let snr = if f.snr_db.is_finite() {
                    f.snr_db
                } else {
                    -10.0
                };
                match f.decoded {
                    Decoded::Binary { .. } => s.last_binary_s = self.t,
                    Decoded::Rtty { .. } => s.last_rtty_s = self.t,
                }
                s.packets += 1;
                s.snr_db = Some((self.t, snr));
                self.best.push_back((self.t, snr));
                self.last_offset = Some(offset);
                heard.push(Timed {
                    time: self.t,
                    value: Heard {
                        channel: s.number,
                        offset_hz: offset,
                        freq_hz: (self.centre != 0.0)
                            .then_some(self.centre + s.ch.offset_hz + f.centre_hz),
                        snr_db: snr,
                        spacing_hz: f.spacing_hz,
                        decoded: f.decoded,
                    },
                });
            }
        }
        if let Some(span) = self.span.as_mut() {
            if span.push(x, &mut self.seen) {
                self.follow_search();
            }
        }
    }
}

impl Component for ChannelBank {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "horus.channels",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Heard>("packets").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BankReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .manual_tags()
        .with_settings(BankSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.dial = if inputs[0].tuned_hz != 0.0 {
            inputs[0].tuned_hz - inputs[0].centre_hz
        } else {
            0.0
        };
        self.t = 0.0;
        self.reset();
        self.publish();
        Ok(vec![
            Format::new(Kind::typed::<Heard>(), 0.0),
            Format::new(Kind::typed::<BankReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.iq(0);
        let mut retune = None;
        for t in io.input(0).tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                retune = Some((t.at, centre_hz, tuned_hz));
            }
        }
        let mut heard = Vec::new();
        let mut pos = 0usize;
        if let Some((at, centre, tuned)) = retune {
            let end = (at.saturating_sub(first) as usize).min(x.len());
            self.feed(&x[..end], first, &mut heard);
            pos = end;
            self.centre = centre;
            self.dial = if tuned != 0.0 { tuned - centre } else { 0.0 };
            self.reset();
        }
        self.feed(&x[pos..], first + pos as u64, &mut heard);
        self.t = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        io.items_out::<Heard>(0).append(&mut heard);
        if io.tick() {
            self.publish();
            let report = BankReport {
                state: self.state(),
                snr_db: self.best.iter().map(|b| b.1).reduce(f64::max),
                offset_hz: self.last_offset,
                events: std::mem::take(&mut self.events),
            };
            io.items_out::<BankReport>(1).push(Timed {
                time: self.t,
                value: report,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::SpanSearch => self.span_on = SPAN_SEARCH.read(id, v)?,
            BankSetting::MaxChannels => self.max = MAX_CHANNELS_SETTING.read(id, v)? as usize,
        }
        if self.rate > 0.0 {
            let searching = self.searching();
            if searching != self.span.is_some() {
                self.span = if searching {
                    SpanSearch::new(self.front_rate, self.front.is_some(), self.dial.abs() > 1.0)
                } else {
                    None
                };
            }
            while self.slots.len() > self.max.max(1) {
                self.slots.pop();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BankSetting::SpanSearch, Value::Bool(self.span_on)),
            (BankSetting::MaxChannels, Value::I64(self.max as i64)),
        ])
    }
    fn commands(&self) -> &Commands {
        BankCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match BankCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BankCommand::Clear => {
                self.reset();
                self.events.clear();
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

impl ChannelBank {
    /// Runs `x` (input samples from sample `first` on) through the shared decimator, the channels and the search.
    fn feed(&mut self, x: &[Complex], first: u64, heard: &mut Vec<Timed<Heard>>) {
        if x.is_empty() {
            return;
        }
        self.t = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        match self.front.as_mut() {
            Some(c) => {
                let mut buf = std::mem::take(&mut self.buf);
                buf.clear();
                c.process(x, &mut buf);
                self.run(&buf, heard);
                self.buf = buf;
            }
            None => self.run(x, heard),
        }
    }
}
