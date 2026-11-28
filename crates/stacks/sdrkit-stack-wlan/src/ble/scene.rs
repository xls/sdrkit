//! A busy band as a test signal: advertisers on the three primary channels and connections that hop over the data
//! channels, each transmitter with its own timing, so packets of different channels overlap in time as on the air. The
//! scene is centred on any frequency; a packet whose channel does not fit the output rate is left out.
//!
//! A connection is written from Bluetooth Core Specification v5.4 Vol 6 Part B: an optional CONNECT_IND on channel 37
//! (clause 2.3.3.1), the first event one transmit window delay and WinOffset after it (clause 4.5.3), then one event each
//! interval on the channel of algorithm #1 (clause 4.5.8.2, written out here) or #2 (clause 4.5.8.3). An event is the
//! central's packet and the peripheral's answer 150 µs after its end (T_IFS, clause 4.1.1): empty PDUs, L2CAP data, and for
//! an encrypted connection LL_START_ENC_REQ and then PDUs that are 4 bytes longer (the MIC, clause 2.4).

use super::gen::{packet_samples, GenPacket, GenPhy, PAD_BITS};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, BurstSource, Scene};
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};
use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap};
use sdrkit_proto_wlan_phy::ble::{
    self, build_adv, build_pdu, channel_hz, Addr, Header, Link, LinkKind, PduType,
};

/// The time between the end of a packet and the start of the answer, s.
const T_IFS_S: f64 = 150e-6;
/// 1.25 ms, s.
const UNIT_S: f64 = 1.25e-3;

/// One connection of the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneConn {
    /// The access address.
    pub aa: u32,
    /// The CRC preset.
    pub crc_init: u32,
    /// The interval, units of 1.25 ms.
    pub interval: u16,
    /// The hop increment of channel selection algorithm #1; `None` for algorithm #2.
    pub hop: Option<u8>,
    /// The channel map.
    pub map: [u8; 5],
    /// The connection is on LE 2M (from its first event when its start is not sent, else from event 6 by a PHY update).
    pub two_m: bool,
    /// The CONNECT_IND is sent, on channel 37.
    pub start: bool,
    /// When the CONNECT_IND starts (or the first event, without one), s.
    pub at_s: f64,
    /// The connection is encrypted from event 4 on.
    pub encrypted: bool,
    /// One event in this many carries data; the others are empty PDUs.
    pub data_every: u16,
    /// The events sent; 0 for no end.
    pub events: u32,
}

impl SceneConn {
    /// A connection with all data channels, LE 1M, in the clear, data in one event of four, without end.
    pub fn new(aa: u32, crc_init: u32, interval: u16, hop: Option<u8>, at_s: f64) -> SceneConn {
        SceneConn {
            aa,
            crc_init,
            interval,
            hop,
            map: [0xFF, 0xFF, 0xFF, 0xFF, 0x1F],
            two_m: false,
            start: false,
            at_s,
            encrypted: false,
            data_every: 4,
            events: 0,
        }
    }

    /// The data channel of connection event `counter` (0 the first).
    pub fn channel(&self, counter: u16) -> u8 {
        let Some(map) = ChannelMap::from_bytes(self.map) else {
            return 0;
        };
        match self.hop {
            None => csa::csa2_channel(counter, csa::channel_identifier(self.aa), &map),
            Some(hop) => {
                // unmappedChannel = (lastUnmappedChannel + hopIncrement) mod 37, from 0 before the first event; an unused
                // channel is replaced by the used channel at index unmappedChannel mod numUsedChannels.
                let unmapped = ((u32::from(counter) + 1) * u32::from(hop) % 37) as u8;
                let used: Vec<u8> = (0..37).filter(|&c| self.map[usize::from(c / 8)] >> (c % 8) & 1 == 1).collect();
                if used.contains(&unmapped) {
                    unmapped
                } else {
                    used[usize::from(unmapped) % used.len()]
                }
            }
        }
    }

    /// The PHY of event `counter`.
    fn phy(&self, counter: u32) -> GenPhy {
        if self.two_m && (!self.start || counter >= 6) {
            GenPhy::Le2M
        } else {
            GenPhy::Le1M
        }
    }
}

/// What the scene holds.
#[derive(Clone, Debug, PartialEq)]
pub struct BusyScene {
    /// The output rate, Hz.
    pub rate_hz: Hz,
    /// The frequency of the output's centre, Hz.
    pub centre_hz: Hz,
    /// The advertisers: each sends an ADV_IND on 37, 38 and 39 about every 100 ms.
    pub advertisers: usize,
    /// The connections.
    pub conns: Vec<SceneConn>,
}

impl BusyScene {
    /// Twenty advertisers and five connections: one that starts with its CONNECT_IND (algorithm #1), and four in
    /// progress: algorithm #1 and #2, LE 1M and LE 2M, one of them encrypted.
    pub fn busy(rate_hz: Hz, centre_hz: Hz) -> BusyScene {
        let mut heard = SceneConn::new(0x5A3C_9617, 0x0A_BCDE, 8, Some(7), 0.0213);
        heard.start = true;
        let mut b = SceneConn::new(0x7193_A2C5, 0x5D_1E07, 12, None, 0.0041);
        b.two_m = true;
        let mut c = SceneConn::new(0x2B65_D8E3, 0xC3_5A11, 24, None, 0.0127);
        c.encrypted = true;
        let mut d = SceneConn::new(0x9C4E_3165, 0x31_F00D, 6, Some(11), 0.0069);
        d.two_m = true;
        BusyScene {
            rate_hz,
            centre_hz,
            advertisers: 20,
            conns: vec![heard, SceneConn::new(0xAF9A_B4D1, 0x12_3456, 16, Some(13), 0.0017), b, c, d],
        }
    }
}

/// The packet of `pdu` on `link`, `phy` and `channel` as a burst whose first bit is on the air at `at_s`, at `level_db`.
/// `None` when the channel does not fit the output.
fn burst(s: &BusyScene, pdu: Vec<u8>, link: Link, phy: GenPhy, channel: u8, at_s: f64, level_db: f64) -> Option<Burst> {
    let offset_hz = channel_hz(channel)? - s.centre_hz;
    if offset_hz.abs() + 1.2e6 > s.rate_hz / 2.0 {
        return None;
    }
    let p = GenPacket::new(pdu, 0.0).link(link).phy(phy).on(channel);
    let samples = packet_samples(&p, channel, s.rate_hz).ok()?;
    let symbol_rate = if phy == GenPhy::Le2M { 2e6 } else { 1e6 };
    Some(Burst {
        start_s: (at_s - PAD_BITS as f64 / symbol_rate).max(0.0),
        offset_hz,
        level_db,
        samples,
    })
}

/// The time on the air of a packet with `pdu_len` PDU bytes, s (clause 2.1: preamble, access address, PDU, CRC).
fn air_s(pdu_len: usize, phy: GenPhy) -> f64 {
    match phy {
        GenPhy::Le2M => (2 + 4 + pdu_len + 3) as f64 * 4e-6,
        _ => (1 + 4 + pdu_len + 3) as f64 * 8e-6,
    }
}

/// Advertiser `i`: its events, each an ADV_IND on channels 37, 38 and 39.
struct Advertiser {
    s: BusyScene,
    pdu: Vec<u8>,
    rng: Rng,
    event_s: f64,
    step: u8,
    level_db: f64,
}

impl BurstSource for Advertiser {
    fn next_burst(&mut self) -> Option<Burst> {
        for _ in 0..6 {
            let (at, ch) = (self.event_s + f64::from(self.step) * 0.6e-3, 37 + self.step);
            self.step += 1;
            if self.step == 3 {
                self.step = 0;
                // advInterval and the advDelay of 0 to 10 ms (clause 4.4.2.2.1).
                self.event_s += 0.1 + 0.01 * self.rng.uniform();
            }
            if let Some(b) = burst(&self.s, self.pdu.clone(), Link::ADVERTISING, GenPhy::Le1M, ch, at, self.level_db) {
                return Some(b);
            }
        }
        None
    }
}

/// A connection: its CONNECT_IND, then the two packets of each event.
struct Connection {
    s: BusyScene,
    c: SceneConn,
    rng: Rng,
    /// The start of the first event, s.
    first_s: f64,
    connect: Option<Vec<u8>>,
    counter: u32,
    /// The end of the central's packet of this event, when the peripheral's is due.
    answer_at: Option<f64>,
}

impl Connection {
    fn new(s: &BusyScene, c: SceneConn, seed: u64) -> Connection {
        let (win_size, win_offset) = (2u8, 1u16);
        let mut ll = Vec::new();
        ll.extend(c.aa.to_le_bytes());
        ll.extend(&c.crc_init.to_le_bytes()[..3]);
        ll.push(win_size);
        ll.extend(win_offset.to_le_bytes());
        ll.extend(c.interval.to_le_bytes());
        ll.extend(0u16.to_le_bytes());
        ll.extend(200u16.to_le_bytes());
        ll.extend(c.map);
        ll.push(c.hop.unwrap_or(9));
        let mut payload = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0xC6, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xC7];
        payload.extend(&ll);
        let h = Header {
            pdu_type: PduType::ConnectInd,
            ch_sel: c.hop.is_none(),
            tx_add: true,
            rx_add: true,
            length: payload.len() as u8,
        };
        let mut connect = Vec::new();
        let ok = c.start && build_pdu(&mut connect, h, &payload).is_ok();
        // The first event starts 0.3 ms into the transmit window: transmitWindowDelay 1.25 ms after the CONNECT_IND ends,
        // then WinOffset.
        let first_s = if ok {
            c.at_s + air_s(connect.len(), GenPhy::Le1M) + UNIT_S + f64::from(win_offset) * UNIT_S + 0.3e-3
        } else {
            c.at_s
        };
        Connection {
            s: s.clone(),
            c,
            rng: Rng::new(seed),
            first_s,
            connect: ok.then_some(connect),
            counter: 0,
            answer_at: None,
        }
    }

    /// A data channel PDU of event `k`: `central` sends it.
    fn pdu(&mut self, k: u32, central: bool) -> Vec<u8> {
        let c = self.c;
        // SN and NESN step with every event: each packet is new and acknowledges the last.
        let bits = (k as u8 & 1) << 3 | ((k as u8 ^ u8::from(central)) & 1) << 2;
        let mic = if c.encrypted && k >= 4 { 4 } else { 0 };
        let mut random = |n: usize| (0..n).map(|_| self.rng.next_u32() as u8).collect::<Vec<u8>>();
        let (llid, body): (u8, Vec<u8>) = if c.start && c.two_m && k == 1 && central {
            // LL_PHY_UPDATE_IND: both directions on LE 2M at instant 6.
            (3, vec![0x18, 2, 2, 6, 0])
        } else if c.encrypted && k == 3 && !central {
            // LL_START_ENC_REQ: the last LL control PDU sent in the clear.
            (3, vec![0x05])
        } else if c.encrypted && k >= 4 && k.is_multiple_of(6) {
            // LL control continues under encryption (keepalives, channel reports): the opcode byte and the CtrData are
            // ciphertext and the MIC follows, so the PDU is longer than any control PDU sent in the clear.
            (3, random(5 + mic))
        } else if c.data_every > 0 && k % u32::from(c.data_every) == 1 {
            let n = 5 + (k as usize * 7 + usize::from(central)) % 20;
            if mic > 0 {
                (2, random(n + mic))
            } else {
                // An L2CAP frame on a dynamic channel.
                let mut v = ((n - 4) as u16).to_le_bytes().to_vec();
                v.extend(0x0041u16.to_le_bytes());
                v.extend(random(n - 4));
                (2, v)
            }
        } else {
            (1, Vec::new())
        };
        let mut v = vec![llid | bits, body.len() as u8];
        v.extend(body);
        v
    }
}

impl BurstSource for Connection {
    fn next_burst(&mut self) -> Option<Burst> {
        if let Some(connect) = self.connect.take() {
            if let Some(b) = burst(&self.s, connect, Link::ADVERTISING, GenPhy::Le1M, 37, self.c.at_s, -1.0) {
                return Some(b);
            }
        }
        let link = Link::new(self.c.aa, self.c.crc_init, LinkKind::Data);
        for _ in 0..4000 {
            let k = self.counter;
            if self.c.events > 0 && k >= self.c.events {
                return None;
            }
            let (phy, ch) = (self.c.phy(k), self.c.channel(k as u16));
            let anchor = self.first_s + f64::from(k) * f64::from(self.c.interval) * UNIT_S;
            let (at, central) = match self.answer_at.take() {
                Some(end) => {
                    self.counter += 1;
                    (end + T_IFS_S, false)
                }
                None => (anchor, true),
            };
            let pdu = self.pdu(k, central);
            if central {
                self.answer_at = Some(at + air_s(pdu.len(), phy));
            }
            let level = if central { -3.0 } else { -6.0 };
            if let Some(b) = burst(&self.s, pdu, link, phy, ch, at, level) {
                return Some(b);
            }
        }
        None
    }
}

/// The generator of a [`BusyScene`].
pub struct SceneGenerator {
    rate_hz: f64,
    scene: Option<Scene>,
}

impl SceneGenerator {
    /// The generator of `s`; `seed` sets the advertisers' delays and the payload bytes.
    pub fn new(s: &BusyScene, seed: u64) -> SceneGenerator {
        let scene = Scene::new(s.rate_hz).ok().map(|mut scene| {
            for i in 0..s.advertisers {
                let mut d = vec![2, 1, 6, 5, 9, b'd', b'e', b'v', b'a' + (i % 26) as u8];
                d.extend([3, 3, 0x0F, 0x18]);
                let a = Addr::random([i as u8, 0x10, 0x32, 0x54, 0x76, 0xC0 | (i as u8 & 0x3F)]);
                let mut pdu = Vec::new();
                if build_adv(&mut pdu, PduType::AdvInd, &a, &d).is_ok() {
                    scene.add_source(Box::new(Advertiser {
                        s: s.clone(),
                        pdu,
                        rng: Rng::stream(seed, i as u64),
                        event_s: 3e-3 + 4.7e-3 * i as f64,
                        step: 0,
                        level_db: -2.0 - (i % 7) as f64 * 3.0,
                    }));
                }
            }
            for (i, &c) in s.conns.iter().enumerate() {
                scene.add_source(Box::new(Connection::new(s, c, seed ^ (0x100 + i as u64))));
            }
            scene
        });
        SceneGenerator {
            rate_hz: s.rate_hz,
            scene,
        }
    }
}

impl Generator for SceneGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        match &mut self.scene {
            Some(s) => s.generate(out),
            None => 0,
        }
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            bandwidth_hz: 1e6,
            symbol_rate_hz: Some(ble::SYMBOL_RATE),
            bit_rate_hz: Some(ble::SYMBOL_RATE),
            power: 1.0,
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
