//! The receivers of every BLE channel inside the capture (the `wlan.ble_adv` decoder): each channel is moved to 0 Hz and brought
//! to 4 Msps by the shared channelizer, then an LE receiver decodes it. The primary advertising channels run all the time and
//! take LE 1M and LE Coded. A secondary channel runs only when a packet points to it: an AuxPtr opens it for the window of the
//! auxiliary packet, and a periodic train that a SyncInfo described opens the channel of each of its events, which channel
//! selection algorithm #2 gives from the train's access address and event counter. A secondary channel takes all three PHYs.
//! The channels of a block are run primary first, so a pointer opens its channel for the same block.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part B clauses 1.4.1, 2.3.4.5, 2.3.4.6, 4.4.2.12 and 4.5.8.3.

use super::adv::{parse_ad, summary};
use super::conn::Conn;
use super::follow::start_to_aa_end_s;
use super::{AdvStats, Advert, HINT_TOLERANCE_HZ, RATE_HZ};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir;
use sdrkit_core::{Complex32, ConfigError, StageError, Values};
use sdrkit_graph::{no_values, Decoder, Format, Kind, Outbox, Port, RunContext, Timed};
use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap};
use sdrkit_proto_wlan_phy::ble::{
    advertising_channel_at, channel_hz, AdvPdu, BlePacket, BleReceiver, Body, ChannelPlan, Link,
    LinkKind, Phy, Phys, ReceiverStats, SyncInfo, MAX_LINKS,
};

/// The width each channel keeps, Hz: the LE 2M signal and its offset.
pub const CHAIN_WIDTH_HZ: f64 = 2.4e6;
/// Half the width a channel needs inside the capture, Hz.
const NEED_HALF_HZ: f64 = 0.8e6;
/// The part of the capture that counts as inside (the radio's filters roll off at the edges).
const USABLE: f64 = 0.9;
/// A periodic train not heard for this long is dropped, s.
const TRAIN_TIMEOUT_S: f64 = 30.0;
/// A secondary channel opens this long before the earliest start of the packet it waits for, s: the receiver's warm-up and
/// clock drift.
const OPEN_BEFORE_S: f64 = 0.3e-3;
/// A secondary channel stays open this long after the latest start of a train's packet, s: the longest packet (LE Coded S=8).
const OPEN_AFTER_S: f64 = 18e-3;

/// The longest packet on `phy` with a margin, s: how long a channel stays open after the latest start of an auxiliary packet.
fn longest_s(phy: Option<Phy>) -> f64 {
    match phy {
        Some(Phy::Le1M) => 2.2e-3,
        Some(Phy::Le2M) => 1.2e-3,
        _ => 17.2e-3,
    }
}
/// The clock drift allowed per second since a train's anchor, s/s (500 ppm on each side).
const DRIFT: f64 = 1e-3;

/// The level of one channel's own band: the mean power of the channel's samples through a lowpass of 650 kHz, taken on every
/// fourth sample (a mean power needs no more).
struct BandLevel {
    taps: Vec<f32>,
    sum: f64,
    n: u64,
}

impl BandLevel {
    fn new() -> BandLevel {
        BandLevel {
            taps: fir::lowpass(RATE_HZ, 0.65e6, 1.0e6, 40.0).unwrap_or_else(|_| vec![1.0]),
            sum: 0.0,
            n: 0,
        }
    }

    fn push(&mut self, x: &[Complex32]) {
        let l = self.taps.len();
        let mut k = l;
        while k <= x.len() {
            let y: Complex32 = self
                .taps
                .iter()
                .zip(x[k - l..k].iter().rev())
                .map(|(&t, &s)| s * t)
                .sum();
            self.sum += f64::from(y.norm_sqr());
            self.n += 1;
            k += 4;
        }
    }

    /// The mean power since the last call, dBFS; `None` before any sample.
    fn take_dbfs(&mut self) -> Option<f64> {
        let v = (self.n > 0).then(|| sdrkit_algo_dsp::meter::power_to_db(self.sum / self.n as f64));
        self.sum = 0.0;
        self.n = 0;
        v
    }
}

/// The BLE channels (0 to 39) whose centres lie inside a capture of `rate_hz` around `centre_hz`, with their offsets from the
/// centre, primary channels first.
pub fn channels_in_capture(centre_hz: f64, rate_hz: f64) -> Vec<(u8, f64)> {
    let half = rate_hz / 2.0 * USABLE + 1e3;
    let mut v: Vec<(u8, f64)> = (0..40u8)
        .filter_map(|c| channel_hz(c).map(|f| (c, f - centre_hz)))
        .filter(|&(_, off)| off.abs() + NEED_HALF_HZ <= half)
        .collect();
    v.sort_by_key(|&(c, _)| (c < 37, c));
    v
}

/// One channel: its channelizer, its receiver and its time base.
struct Chain {
    ch: Channelizer,
    rx: BleReceiver,
    buf: Vec<Complex32>,
    /// The channel, or `None` when the centre is not known (a recording: one channel at the tuned frequency).
    channel: Option<u8>,
    /// Runs all the time (a primary channel).
    always: bool,
    /// The absolute input index of the receiver's sample 0, set when the chain starts running.
    start: Option<f64>,
    /// Input samples per output sample.
    ratio: f64,
    /// The chain ran up to the end of the last block.
    running: bool,
    /// The times (signal s) the chain is open, as (from, to).
    open: Vec<(f64, f64)>,
}

impl Chain {
    fn open_at(&self, from_s: f64, to_s: f64) -> bool {
        self.always || self.open.iter().any(|&(a, b)| a < to_s && b > from_s)
    }
}

/// A periodic train whose link the receivers hold.
#[derive(Clone, Copy, Debug)]
struct Train {
    link: Link,
    last_s: f64,
    /// The expected start of event `counter`, s.
    anchor_s: f64,
    counter: u16,
    interval_s: f64,
    map: Option<ChannelMap>,
}

impl Train {
    /// The (start, channel) of each event that may start in `from_s..to_s`.
    fn events(&self, from_s: f64, to_s: f64, out: &mut Vec<(f64, u8)>) {
        if self.interval_s <= 0.0 {
            return;
        }
        let Some(map) = self.map else {
            return;
        };
        let id = csa::channel_identifier(self.link.access_address);
        let k0 = ((from_s - OPEN_AFTER_S - self.anchor_s) / self.interval_s).ceil().max(0.0) as u32;
        let mut k = k0;
        loop {
            let t = self.anchor_s + k as f64 * self.interval_s;
            if t > to_s + OPEN_BEFORE_S {
                break;
            }
            let counter = self.counter.wrapping_add(k as u16);
            out.push((t, csa::csa2_channel(counter, id, &map)));
            k += 1;
            if k > k0 + 64 {
                break;
            }
        }
    }
}

/// The receivers of the channels inside the capture (module documentation).
pub struct AdvDecoder {
    chains: Vec<Chain>,
    rate: f64,
    tuned_hz: f64,
    centre_hz: f64,
    live: bool,
    input: u64,
    level: BandLevel,
    kept: ReceiverStats,
    trains: Vec<Train>,
    conns: Vec<Conn>,
    out: Vec<Timed<Advert>>,
    capture: u64,
    events: Vec<(f64, u8)>,
    conn_events: Vec<(f64, f64, u8)>,
}

impl Default for AdvDecoder {
    fn default() -> Self {
        AdvDecoder {
            chains: Vec::new(),
            rate: RATE_HZ,
            tuned_hz: 0.0,
            centre_hz: 0.0,
            live: false,
            input: 0,
            level: BandLevel::new(),
            kept: ReceiverStats::default(),
            trains: Vec::new(),
            conns: Vec::new(),
            out: Vec::new(),
            capture: 0,
            events: Vec::new(),
            conn_events: Vec::new(),
        }
    }
}

/// The plan of a channel: a primary channel tries the whitening of all three (a recording or a test signal may hold any of
/// them on one frequency), its own first when the source is live; another channel is known.
fn plan(channel: Option<u8>, live: bool) -> ChannelPlan {
    match channel {
        Some(c) if c < 37 => ChannelPlan::Known(c),
        Some(c) if live => ChannelPlan::Advertising { hint: Some(c) },
        _ => ChannelPlan::Advertising { hint: None },
    }
}

/// What a packet asks of the other channels.
enum Ask {
    /// Open `channel` from `from_s` to `to_s`.
    Open(u8, f64, f64),
    /// A SyncInfo in a packet that started at the given time.
    Train(SyncInfo, f64),
    /// A packet of a train's link that started at the given time.
    Heard(u32, f64),
    /// A CONNECT_IND or AUX_CONNECT_REQ: the connection to follow.
    Connect(Box<Conn>),
}

/// What packet `p` (an advert at `time_s`) asks of the other channels.
fn asks(a: &Advert, out: &mut Vec<Ask>) {
    let start = a.time_s - start_to_aa_end_s(a.phy);
    if a.kind == LinkKind::Periodic {
        out.push(Ask::Heard(a.access_address, start));
    }
    if let Some(AdvPdu {
        header,
        body: Body::Connect { init_a, adv_a, ll },
    }) = a.parsed()
    {
        // The packet ends after the PDU and the CRC (8 µs a byte on LE 1M, 4 on LE 2M, 64 on LE Coded S=8).
        let per_byte = match a.phy {
            Phy::Le1M => 8e-6,
            Phy::Le2M => 4e-6,
            Phy::LeCoded => 64e-6,
        };
        let end = a.time_s + (a.pdu.len() + 3) as f64 * per_byte;
        let aux = a.channel < 37;
        if let Some(c) = Conn::new(init_a, adv_a, &ll, end, a.phy, aux, header.ch_sel) {
            out.push(Ask::Connect(Box::new(c)));
        }
    }
    let Some(AdvPdu {
        body: Body::Extended { ext, .. },
        ..
    }) = a.parsed()
    else {
        return;
    };
    if let Some(ap) = ext.aux_ptr {
        if ap.offset > 0 {
            let at = start + ap.offset_s();
            out.push(Ask::Open(
                ap.channel,
                at - OPEN_BEFORE_S,
                at + ap.window_s() + longest_s(ap.phy()),
            ));
        }
    }
    if let Some(s) = ext.sync_info {
        out.push(Ask::Train(s, start));
    }
}

impl AdvDecoder {
    /// The advertising channel the tuned frequency is on, for a live source only (0 when not known).
    pub fn hint(&self) -> u8 {
        if self.live {
            advertising_channel_at(self.tuned_hz, HINT_TOLERANCE_HZ).unwrap_or(0)
        } else {
            0
        }
    }

    /// The channels inside the capture, bit `c` for channel `c` (0 when the centre is not known).
    pub fn capture(&self) -> u64 {
        self.capture
    }

    fn stats(&self) -> ReceiverStats {
        let mut s = self.kept;
        for c in &self.chains {
            let r = c.rx.stats();
            s.aa_hits += r.aa_hits;
            s.crc_failures += r.crc_failures;
            s.packets += r.packets;
        }
        s
    }

    fn rebuild(&mut self) -> Result<(), ConfigError> {
        self.kept = self.stats();
        self.chains.clear();
        let rate = self.rate;
        let list: Vec<(Option<u8>, f64)> = if self.centre_hz > 0.0 {
            channels_in_capture(self.centre_hz, rate)
                .into_iter()
                .map(|(c, o)| (Some(c), o))
                .collect()
        } else {
            Vec::new()
        };
        let list = if list.is_empty() {
            // The centre is not known, or no channel fits: one channel at the tuned frequency.
            let off = if self.centre_hz > 0.0 {
                self.tuned_hz - self.centre_hz
            } else {
                0.0
            };
            let ch = advertising_channel_at(self.tuned_hz, HINT_TOLERANCE_HZ)
                .filter(|_| self.centre_hz > 0.0);
            vec![(ch, off)]
        } else {
            list
        };
        self.capture = list
            .iter()
            .filter_map(|&(c, _)| c)
            .fold(0u64, |m, c| m | 1 << c);
        for (channel, off) in list {
            let mut cfg = ChannelConfig::new(rate, off, CHAIN_WIDTH_HZ.min(rate * 0.6), RATE_HZ);
            cfg.exact_rate = true;
            let ch = Channelizer::new(cfg).map_err(ConfigError::from)?;
            let secondary = channel.is_some_and(|c| c < 37);
            let phys = if secondary { Phys::ALL } else { Phys::PRIMARY };
            let mut rx = BleReceiver::new((RATE_HZ / 1e6) as usize, phys)
                .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
            let _ = rx.set_channels(plan(channel, self.live));
            for t in &self.trains {
                let _ = rx.add_link(t.link);
            }
            for c in &self.conns {
                let _ = rx.add_link(Link::new(c.aa, c.crc_init, LinkKind::Data));
            }
            self.chains.push(Chain {
                ratio: rate / ch.output_rate(),
                ch,
                rx,
                buf: Vec::new(),
                channel,
                always: !secondary,
                start: None,
                running: false,
                open: Vec::new(),
            });
        }
        Ok(())
    }

    /// Opens the chain of `channel` from `from_s` to `to_s`.
    fn open(&mut self, channel: u8, from_s: f64, to_s: f64) {
        if let Some(c) = self.chains.iter_mut().find(|c| c.channel == Some(channel)) {
            let known = c
                .open
                .iter()
                .any(|&(a, b)| (a - from_s).abs() < 1e-6 && (b - to_s).abs() < 1e-6);
            if !c.always && !known && c.open.len() < 64 {
                c.open.push((from_s, to_s));
            }
        }
    }

    /// Adds the link of a periodic train that a SyncInfo described, or refreshes it.
    fn follow_train(&mut self, s: SyncInfo, start_s: f64) {
        let link = Link::new(s.access_address, s.crc_init, LinkKind::Periodic);
        let anchor = (s.offset_base > 0).then(|| start_s + s.offset_s());
        if let Some(t) = self
            .trains
            .iter_mut()
            .find(|t| t.link.access_address == link.access_address)
        {
            t.last_s = start_s;
            if let Some(a) = anchor {
                t.anchor_s = a;
                t.counter = s.event_counter;
            }
            return;
        }
        if self.trains.len() + 1 >= MAX_LINKS {
            return;
        }
        self.trains.push(Train {
            link,
            last_s: start_s,
            anchor_s: anchor.unwrap_or(f64::NAN),
            counter: s.event_counter,
            interval_s: s.interval_s(),
            map: ChannelMap::from_bytes(s.channel_map),
        });
        for c in &mut self.chains {
            let _ = c.rx.add_link(link);
        }
    }

    /// Follows connection `c`, or starts it again when its access address is followed already.
    fn follow_conn(&mut self, c: Conn) {
        if let Some(old) = self.conns.iter_mut().find(|o| o.aa == c.aa) {
            *old = c;
            return;
        }
        if self.trains.len() + self.conns.len() + 1 >= MAX_LINKS {
            return;
        }
        let link = Link::new(c.aa, c.crc_init, LinkKind::Data);
        for ch in &mut self.chains {
            let _ = ch.rx.add_link(link);
        }
        self.conns.push(c);
    }

    /// Drops the connections that are over at `now_s`.
    fn drop_old_conns(&mut self, now_s: f64) {
        let mut i = 0;
        while i < self.conns.len() {
            if self.conns[i].over(now_s) {
                let aa = self.conns[i].aa;
                for c in &mut self.chains {
                    c.rx.remove_link(aa);
                }
                self.conns.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Opens the channels of the connection events that may start in this block.
    fn open_conn_events(&mut self, from_s: f64, to_s: f64) {
        self.conn_events.clear();
        for c in &self.conns {
            c.events(from_s, to_s, &mut self.conn_events);
        }
        for i in 0..self.conn_events.len() {
            let (a, b, ch) = self.conn_events[i];
            self.open(ch, a, b);
        }
    }

    /// Places the packets of the followed connections in this block's output, in time order.
    fn place_conn_packets(&mut self) {
        for t in &mut self.out {
            let a = &mut t.value;
            if a.kind != LinkKind::Data {
                continue;
            }
            if let Some(c) = self.conns.iter_mut().find(|c| c.aa == a.access_address) {
                let start = a.time_s - start_to_aa_end_s(a.phy);
                a.conn = c.heard(start, a.channel, &a.pdu);
            }
        }
    }

    /// A packet of a train started at `start_s`: the train's events are counted from it.
    fn heard_train(&mut self, aa: u32, start_s: f64) {
        if let Some(t) = self
            .trains
            .iter_mut()
            .find(|t| t.link.access_address == aa)
        {
            t.last_s = start_s;
            if t.anchor_s.is_finite() && t.interval_s > 0.0 {
                let k = ((start_s - t.anchor_s) / t.interval_s).round();
                if k >= 0.0 && (start_s - t.anchor_s - k * t.interval_s).abs() < 2e-3 {
                    t.counter = t.counter.wrapping_add(k as u16);
                    t.anchor_s = start_s;
                }
            } else {
                t.anchor_s = start_s;
            }
        }
    }

    fn drop_old_trains(&mut self, now_s: f64) {
        let mut i = 0;
        while i < self.trains.len() {
            if now_s - self.trains[i].last_s > TRAIN_TIMEOUT_S {
                let aa = self.trains[i].link.access_address;
                for c in &mut self.chains {
                    c.rx.remove_link(aa);
                }
                self.trains.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Opens the channels of the train events that may start in this block.
    fn open_train_events(&mut self, from_s: f64, to_s: f64) {
        self.events.clear();
        for t in &self.trains {
            if t.anchor_s.is_finite() {
                let before = self.events.len();
                t.events(from_s, to_s, &mut self.events);
                let drift = (from_s - t.anchor_s).abs() * DRIFT;
                for e in &mut self.events[before..] {
                    e.0 -= drift;
                }
            }
        }
        for i in 0..self.events.len() {
            let (t, ch) = self.events[i];
            self.open(ch, t - OPEN_BEFORE_S, t + OPEN_AFTER_S);
        }
    }

    /// Runs chain `i` over the part of `x` (which starts at absolute index `first`) it is open for. Returns whether it ran.
    fn run_chain(&mut self, i: usize, x: &[Complex32], first: u64, asks_out: &mut Vec<Ask>) -> bool {
        let rate = self.rate;
        let (b0, b1) = (first as f64 / rate, (first + x.len() as u64) as f64 / rate);
        let c = &mut self.chains[i];
        if !c.open_at(b0, b1) {
            c.running = false;
            return false;
        }
        // A chain that slept starts again where its first window opens.
        let from = if c.always || c.running {
            0
        } else {
            let t = c
                .open
                .iter()
                .filter(|&&(a, b)| a < b1 && b > b0)
                .map(|&(a, _)| a)
                .fold(f64::INFINITY, f64::min);
            (((t - b0) * rate).max(0.0) as usize).min(x.len())
        };
        if !c.always && !c.running {
            c.ch.reset();
            c.rx.reset();
            c.start = None;
        }
        let slice = &x[from..];
        let abs = (first + from as u64) as f64;
        c.buf.clear();
        c.ch.process(slice, &mut c.buf);
        let consumed = c.rx.samples() as f64 * c.ratio;
        let start = *c.start.get_or_insert(abs - consumed);
        let (ratio, delay) = (c.ratio, c.ch.delay());
        if i == 0 {
            // The level counts the first channel's own band only (a primary channel when the capture holds one).
            self.level.push(&c.buf);
        }
        let out = &mut self.out;
        c.rx.process(&c.buf, |p| {
            let at = start + p.sample as f64 * ratio - delay;
            let time_s = at.max(0.0) / rate;
            if let Some(a) = advert(p, time_s) {
                asks(&a, asks_out);
                out.push(Timed {
                    time: time_s,
                    value: a,
                });
            }
        });
        c.running = true;
        true
    }
}

/// The advert of a good packet at signal time `time_s`.
fn advert(p: &BlePacket<'_>, time_s: f64) -> Option<Advert> {
    let (info, summary) = if p.link.kind == LinkKind::Data {
        (Default::default(), super::conn::summary(p.pdu))
    } else {
        let pdu = AdvPdu::parse(p.pdu).ok()?;
        let info = parse_ad(pdu.data().unwrap_or(&[]));
        let summary = summary(&pdu, &info);
        (info, summary)
    };
    Some(Advert {
        time_s,
        channel: p.channel,
        level_dbfs: p.level_dbfs.max(-200.0),
        offset_hz: p.offset_hz,
        deviation_hz: p.deviation_hz,
        snr_db: p.snr_db,
        pdu: p.pdu.to_vec(),
        info,
        summary,
        phy: p.phy,
        coding: p.coding,
        kind: p.link.kind,
        access_address: p.link.access_address,
        conn: None,
    })
}

impl Decoder for AdvDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.ble_adv"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<Advert>("adverts"),
            Port::typed::<AdvStats>("stats"),
        ]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.tuned_hz = input.tuned_hz;
        self.centre_hz = input.centre_hz;
        self.input = 0;
        self.level = BandLevel::new();
        self.kept = ReceiverStats::default();
        self.trains.clear();
        self.chains.clear();
        self.rebuild()?;
        Ok(vec![
            Format::new(Kind::typed::<Advert>(), 0.0),
            Format::new(Kind::typed::<AdvStats>(), 0.0),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.tuned_hz = input.tuned_hz;
        self.centre_hz = input.centre_hz;
        let _ = self.rebuild();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        let live = self.live;
        for c in &mut self.chains {
            let _ = c.rx.set_channels(plan(c.channel, live));
        }
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let rate = self.rate;
        let (b0, b1) = (first as f64 / rate, (first + x.len() as u64) as f64 / rate);
        self.out.clear();
        self.open_train_events(b0, b1);
        self.open_conn_events(b0, b1);
        let mut asks_v: Vec<Ask> = Vec::new();
        // At most 40 chains: bit `i` is chain `i`.
        let mut done = 0u64;
        // Primary channels first, then every secondary channel a packet opened, until none opens another.
        loop {
            let mut ran = false;
            for i in 0..self.chains.len() {
                if done & (1 << i) != 0 {
                    continue;
                }
                let always = self.chains[i].always;
                if !always && !self.chains[i].open_at(b0, b1) {
                    continue;
                }
                done |= 1 << i;
                ran |= self.run_chain(i, x, first, &mut asks_v);
            }
            if asks_v.is_empty() && !ran {
                break;
            }
            let pending = std::mem::take(&mut asks_v);
            if pending.is_empty() {
                break;
            }
            for a in pending {
                match a {
                    Ask::Open(ch, from, to) => self.open(ch, from, to),
                    Ask::Train(s, t) => self.follow_train(s, t),
                    Ask::Heard(aa, t) => self.heard_train(aa, t),
                    Ask::Connect(c) => self.follow_conn(*c),
                }
            }
            self.open_train_events(b0, b1);
            self.open_conn_events(b0, b1);
        }
        for (i, c) in self.chains.iter_mut().enumerate() {
            if done & (1 << i) == 0 {
                c.running = false;
            }
            c.open.retain(|&(_, to)| to > b1);
        }
        self.input = first + x.len() as u64;
        let end_s = self.input as f64 / rate;
        self.drop_old_trains(end_s);
        self.out.sort_by(|a, b| a.time.total_cmp(&b.time));
        self.place_conn_packets();
        self.drop_old_conns(end_s);
        out.items::<Advert>(0).append(&mut self.out);
        let s = self.stats();
        let stats = AdvStats {
            time_s: end_s,
            aa_hits: s.aa_hits,
            crc_failures: s.crc_failures,
            packets: s.packets,
            channel: self.hint(),
            level_dbfs: self.level.take_dbfs(),
            capture: self.capture,
            trains: self.trains.len() as u32,
            connections: self.conns.len() as u32,
        };
        out.items::<AdvStats>(1).push(Timed {
            time: end_s,
            value: stats,
        });
    }
}
