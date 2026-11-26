//! The BLE test signal: a schedule of packets, each GFSK modulated (BT 0.5, modulation index 0.5) on the LE 1M, LE 2M or LE Coded
//! PHY and followed by silence. A packet on a secondary channel is sent at that channel's frequency, relative to channel 37 at
//! 0 Hz; one that does not fit the output rate is left out. The extended advertising part of the schedule places each auxiliary
//! packet in the window its AuxPtr gives. The framework adds noise and the receiver's impairments from the standard `gen_*`
//! settings.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part A clause 3.1 (modulation), Part B clauses 2.1, 2.2, 2.3.4 and 4.5.8.3.

use crate::testgen::GenError;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::{Burst, BurstSource, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_wlan_phy::ble::ad;
use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap};
use sdrkit_proto_wlan_phy::ble::phy::{coded_symbols, packet_bits};
use sdrkit_proto_wlan_phy::ble::{
    self, air_bits, build_adv, build_pdu, channel_hz, Addr, AuxPtr, Coding, ExtHeader, Header,
    Link, LinkKind, PduType, SyncInfo, Uncoded,
};

/// Silence before the first packet, s.
const LEAD_S: f64 = 2e-3;
/// Bits of unmodulated carrier before and after each packet.
const PAD_BITS: usize = 3;

/// The PHY a packet of the schedule is sent on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenPhy {
    /// LE 1M.
    Le1M,
    /// LE 2M.
    Le2M,
    /// LE Coded at S=2 or S=8.
    Coded(Coding),
}

/// One packet of the schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct GenPacket {
    /// Header and payload.
    pub pdu: Vec<u8>,
    /// Silence after it, s.
    pub gap_s: f64,
    /// When set, the next packet starts this long after this one starts (in place of `gap_s`), s.
    pub next_after_s: Option<f64>,
    /// The PHY.
    pub phy: GenPhy,
    /// The access address and CRC preset.
    pub link: Link,
    /// The channel (whitening and frequency); `None` for a primary packet whitened as the schedule's whitening says, at 0 Hz.
    pub channel: Option<u8>,
    /// A packet of a periodic train: its channel follows channel selection algorithm #2 over this channel map, with the
    /// schedule's round count as paEventCounter.
    pub hop: Option<[u8; 5]>,
    /// Amplitude scale of this transmitter (1 is full power).
    pub scale: f64,
    /// This transmitter's own carrier offset, Hz.
    pub offset_hz: f64,
    /// Send the last CRC bit inverted.
    pub bad_crc: bool,
}

impl GenPacket {
    /// A packet with a gap, scale 1 and no offset.
    pub fn new(pdu: Vec<u8>, gap_s: f64) -> GenPacket {
        GenPacket {
            pdu,
            gap_s,
            next_after_s: None,
            phy: GenPhy::Le1M,
            link: Link::ADVERTISING,
            channel: None,
            hop: None,
            scale: 1.0,
            offset_hz: 0.0,
            bad_crc: false,
        }
    }
    /// With an amplitude scale.
    pub fn scale(mut self, s: f64) -> Self {
        self.scale = s;
        self
    }
    /// With its own carrier offset.
    pub fn offset(mut self, hz: f64) -> Self {
        self.offset_hz = hz;
        self
    }
    /// With the last CRC bit inverted.
    pub fn bad_crc(mut self) -> Self {
        self.bad_crc = true;
        self
    }
    /// On `phy`.
    pub fn phy(mut self, phy: GenPhy) -> Self {
        self.phy = phy;
        self
    }
    /// On channel `c` (its whitening and its frequency).
    pub fn on(mut self, c: u8) -> Self {
        self.channel = Some(c);
        self
    }
    /// On `link`.
    pub fn link(mut self, link: Link) -> Self {
        self.link = link;
        self
    }
    /// A packet of a periodic train that hops over `chm`.
    pub fn hopping(mut self, chm: [u8; 5]) -> Self {
        self.hop = Some(chm);
        self
    }
    /// The next packet starts `s` after this one starts.
    pub fn next_after(mut self, s: f64) -> Self {
        self.next_after_s = Some(s);
        self
    }
    /// The frequency of the packet relative to channel 37, Hz (0 for a primary packet).
    pub fn rf_offset_hz(&self) -> f64 {
        match self.channel.and_then(channel_hz) {
            Some(f) if self.channel.is_some_and(|c| c < 37) => f - 2402e6,
            _ => 0.0,
        }
    }
}

/// The whitening of the packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whitening {
    /// Packet `i` of the schedule on channel 37 + i mod 3.
    Round,
    /// Every packet on one channel.
    Fixed(u8),
}

/// What the generator sends.
#[derive(Clone, Debug, PartialEq)]
pub struct BleGenParams {
    /// Output rate, Hz.
    pub rate_hz: Hz,
    /// Repeat the schedule.
    pub looped: bool,
    /// The whitening.
    pub whitening: Whitening,
    /// The schedule.
    pub packets: Vec<GenPacket>,
}

impl Default for BleGenParams {
    fn default() -> Self {
        BleGenParams {
            rate_hz: 4e6,
            looped: true,
            whitening: Whitening::Round,
            packets: [test_schedule(), extended_schedule(), kinds_schedule()].concat(),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The choices of the test-signal setting `gen_channel`: the whitening of the packets.
    pub enum GenWhitening {
        /// Round the three channels.
        Round = ("round", "Round the three channels"),
        /// Every packet on channel 37.
        C37 = ("37", "All on 37"),
        /// Every packet on channel 38.
        C38 = ("38", "All on 38"),
        /// Every packet on channel 39.
        C39 = ("39", "All on 39"),
    }
}

/// The whitening that the `gen_channel` setting of `cfg` selects. A schema without the setting returns the round.
pub fn whitening_of(cfg: &GenConfig) -> Whitening {
    match crate::testgen::gen_setting(cfg, &crate::ble::GEN_CHANNEL) {
        Some(GenWhitening::C37) => Whitening::Fixed(37),
        Some(GenWhitening::C38) => Whitening::Fixed(38),
        Some(GenWhitening::C39) => Whitening::Fixed(39),
        Some(GenWhitening::Round) | None => Whitening::Round,
    }
}

/// An address from its text: 12 hexadecimal digits, most significant byte first, with or without `:` or `-` between the
/// bytes. `None` for other text.
pub fn parse_addr(text: &str, random: bool) -> Option<Addr> {
    let digits: Vec<u8> = text.bytes().filter(|c| !matches!(c, b':' | b'-')).collect();
    if digits.len() != 12 {
        return None;
    }
    let mut b = [0u8; 6];
    for (i, p) in digits.chunks(2).enumerate() {
        let d = |c: u8| (c as char).to_digit(16);
        b[5 - i] = (d(p[0])? * 16 + d(p[1])?) as u8;
    }
    Some(Addr { bytes: b, random })
}

/// An address from its text, as [`parse_addr`] reads it. Panics on bad text: the schedules use constants.
pub fn addr(text: &str, random: bool) -> Addr {
    parse_addr(text, random).expect("an address in the schedule")
}

fn hex(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    sdrkit_core::base64::unhex_into(text, &mut out).expect("hex in the schedule");
    out
}

fn adv(t: PduType, a: &Addr, data: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    build_adv(&mut p, t, a, data).expect("a PDU of the schedule");
    p
}

fn pdu(t: PduType, tx: bool, rx: bool, payload: &[u8]) -> Vec<u8> {
    let h = Header {
        pdu_type: t,
        ch_sel: false,
        tx_add: tx,
        rx_add: rx,
        length: payload.len() as u8,
    };
    let mut p = Vec::new();
    build_pdu(&mut p, h, payload).expect("a PDU of the schedule");
    p
}

fn push(d: &mut Vec<u8>, kind: u8, data: &[u8]) {
    ad::push(d, kind, data).expect("an AD structure of the schedule");
}

fn mfr(d: &mut Vec<u8>, company: u16, data: &[u8]) {
    ad::push_manufacturer(d, company, data).expect("manufacturer data of the schedule");
}

fn service(d: &mut Vec<u8>, uuid: u16, data: &[u8]) {
    ad::push_service_data16(d, uuid, data).expect("service data of the schedule");
}

fn uuids(d: &mut Vec<u8>, u: &[u16]) {
    ad::push_uuids16(d, u).expect("a UUID list of the schedule");
}

/// The eleven-packet schedule of the `ble` stack. Its devices are at 0, -2.5, -4.4, -6 and -8 dB.
pub fn test_schedule() -> Vec<GenPacket> {
    let a1 = addr("C0:FF:EE:12:34:56", true);
    let a2 = addr("00:0B:A1:5E:00:01", false);
    let scan = addr("66:55:44:33:22:11", true);
    let a5 = addr("3A:55:44:33:22:02", true);
    let a8 = addr("5B:12:34:56:78:9A", true);
    let mut v = Vec::new();
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    push(&mut d, ad::COMPLETE_NAME, b"TestDev");
    push(&mut d, ad::TX_POWER, &[4]);
    uuids(&mut d, &[0x180F, 0x180D]);
    mfr(&mut d, 0xFFFF, &[1, 2, 3, 4]);
    v.push(GenPacket::new(adv(PduType::AdvInd, &a1, &d), 3e-3));
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[4]);
    let mut b = vec![0x02, 0x15];
    b.extend(hex("E2C56DB5DFFB48D2B060D0F5A71096E0"));
    b.extend(100u16.to_be_bytes());
    b.extend(7u16.to_be_bytes());
    b.push((-59i8) as u8);
    mfr(&mut d, 0x004C, &b);
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a2, &d), 4e-3)
            .offset(20e3)
            .scale(0.75),
    );
    let mut p = scan.bytes.to_vec();
    p.extend_from_slice(&a1.bytes);
    v.push(
        GenPacket::new(pdu(PduType::ScanReq, true, true, &p), 0.2e-3)
            .offset(-15e3)
            .scale(0.6),
    );
    let mut d = Vec::new();
    push(&mut d, ad::COMPLETE_NAME, b"Test phone");
    v.push(GenPacket::new(adv(PduType::ScanRsp, &a1, &d), 3e-3));
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    uuids(&mut d, &[0xFEAA]);
    let mut url = vec![0x10, (-20i8) as u8, 3];
    url.extend(b"example");
    url.push(7);
    service(&mut d, 0xFEAA, &url);
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3)
            .offset(-30e3)
            .scale(0.5),
    );
    let mut d = Vec::new();
    uuids(&mut d, &[0xFEAA]);
    let mut tlm = vec![0x20, 0];
    tlm.extend(3000u16.to_be_bytes());
    tlm.extend(((21.5 * 256.0) as i16).to_be_bytes());
    tlm.extend(1234u32.to_be_bytes());
    tlm.extend(56789u32.to_be_bytes());
    service(&mut d, 0xFEAA, &tlm);
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3)
            .offset(-30e3)
            .scale(0.5),
    );
    let mut d = Vec::new();
    uuids(&mut d, &[0xFEAA]);
    let mut uid = vec![0x00, (-20i8) as u8];
    uid.extend(hex("EDD1EBEAC04E5DEFA017000000000001"));
    uid.extend([0, 0]);
    service(&mut d, 0xFEAA, &uid);
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3)
            .offset(-30e3)
            .scale(0.5),
    );
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[0x1A]);
    let mut rpi = hex("3C1A09775E42810DC62B94F06E13A558");
    rpi.extend([0x40, 0x08, 0x00, 0x00]);
    service(&mut d, 0xFD6F, &rpi);
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a8, &d), 4e-3)
            .offset(10e3)
            .scale(0.4),
    );
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[0x1A]);
    mfr(&mut d, 0x004C, &[0x10, 0x05, 0x01, 0x18, 0x44, 0x22, 0x11]);
    v.push(
        GenPacket::new(adv(PduType::AdvScanInd, &a8, &d), 3e-3)
            .offset(10e3)
            .scale(0.4),
    );
    let mut p = scan.bytes.to_vec();
    p.extend_from_slice(&a1.bytes);
    p.extend(hex("712A3B50123456020000180000004800FFFFFFFF1F05"));
    v.push(
        GenPacket::new(pdu(PduType::ConnectInd, true, true, &p), 4e-3)
            .offset(-15e3)
            .scale(0.6),
    );
    // ADV_EXT_IND with no AdvA: extended header length 6, AdvMode 0, flags ADI and AuxPtr, ADI, AuxPtr to channel 12, LE 2M.
    let aux: u32 = 12 | (100 + (1 << 13)) << 8;
    let ext = [
        0x06,
        0x18,
        0x23,
        0x21,
        aux as u8,
        (aux >> 8) as u8,
        (aux >> 16) as u8,
    ];
    v.push(GenPacket::new(
        pdu(PduType::AdvExtInd, false, false, &ext),
        5e-3,
    ));
    v
}

/// The interval of the test signal's periodic train, s: the schedule repeats at it.
pub const TRAIN_INTERVAL_S: f64 = 0.08;

/// The PDU with the paEventCounter of its SyncInfo set to `counter`, when it is an extended PDU with a SyncInfo.
fn with_sync_counter(pdu: &[u8], counter: u16) -> Option<Vec<u8>> {
    let p = ble::AdvPdu::parse(pdu).ok()?;
    let ble::Body::Extended { ext, .. } = p.body else {
        return None;
    };
    ext.sync_info?;
    let flags = *pdu.get(3)?;
    // The fields before SyncInfo in flag order: AdvA, TargetA, CTEInfo, ADI, AuxPtr.
    let before: usize = [(0x01, 6), (0x02, 6), (0x04, 1), (0x08, 2), (0x10, 3)]
        .iter()
        .filter(|(f, _)| flags & f != 0)
        .map(|(_, n)| n)
        .sum();
    let at = 4 + before + 16;
    let mut v = pdu.to_vec();
    v.get_mut(at..at + 2)?.copy_from_slice(&counter.to_le_bytes());
    Some(v)
}

/// The access address of the test signal's periodic train.
pub const TRAIN_AA: u32 = 0x5065_17A3;
/// The CRC preset of the test signal's periodic train.
pub const TRAIN_CRC: u32 = 0x11_2233;

fn ext(t: PduType, h: &ExtHeader, acad: &[u8], data: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    ble::build_ext(&mut p, t, h, acad, data).expect("an extended PDU of the schedule");
    p
}

fn aux(channel: u8, offset_30us: u16, phy_code: u8) -> AuxPtr {
    AuxPtr {
        channel,
        clock_50ppm: true,
        units_300us: false,
        offset: offset_30us,
        phy_code,
    }
}

/// The start of an auxiliary packet after the packet that points to it: 10 us into its window.
fn after(a: &AuxPtr) -> f64 {
    a.offset_s() + 10e-6
}

/// The extended advertising part of the `ble` schedule, on channel 37 and the secondary channel 0 (2404 MHz, 2 MHz above
/// channel 37): an ADV_EXT_IND on LE 1M points to an AUX_ADV_IND on LE 2M that chains to an AUX_CHAIN_IND; an ADV_EXT_IND on
/// LE Coded S=8 points to an AUX_ADV_IND on LE Coded S=2 with a SyncInfo; the train's AUX_SYNC_IND hops over channels 0 and 1;
/// an ADV_EXT_IND points to channel 20, outside a capture around channels 37 and 0.
pub fn extended_schedule() -> Vec<GenPacket> {
    let a1 = addr("E5:01:02:03:04:05", true);
    let a2 = addr("C3:11:22:33:44:55", true);
    let a3 = addr("D7:66:55:44:33:22", true);
    let mut v = Vec::new();
    // Set 2 of a1: LE 1M primary, LE 2M secondary, two packets of data.
    let p1 = aux(0, 20, 1);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adi: Some((0x101, 2)),
                    aux_ptr: Some(p1),
                    ..Default::default()
                },
                &[],
                &[],
            ),
            0.0,
        )
        .next_after(after(&p1)),
    );
    let p2 = aux(0, 20, 1);
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    push(&mut d, ad::COMPLETE_NAME, b"Ext sensor");
    uuids(&mut d, &[0x181A]);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adv_a: Some(a1),
                    adi: Some((0x101, 2)),
                    aux_ptr: Some(p2),
                    tx_power: Some(-4),
                    ..Default::default()
                },
                &[],
                &d,
            ),
            0.0,
        )
        .phy(GenPhy::Le2M)
        .on(0)
        .next_after(after(&p2)),
    );
    let mut d = Vec::new();
    mfr(&mut d, 0xFFFF, &[0xE7, 0x7E, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adi: Some((0x101, 2)),
                    ..Default::default()
                },
                &[],
                &d,
            ),
            3e-3,
        )
        .phy(GenPhy::Le2M)
        .on(0),
    );
    // Set 5 of a2: LE Coded S=8 primary, S=2 secondary, with a periodic train on channels 0 and 1.
    let p3 = aux(0, 50, 2);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adi: Some((0x055, 5)),
                    aux_ptr: Some(p3),
                    ..Default::default()
                },
                &[],
                &[],
            ),
            0.0,
        )
        .phy(GenPhy::Coded(Coding::S8))
        .next_after(after(&p3)),
    );
    // The train's packet comes 21 ms after the AUX_ADV_IND, after the window its AuxPtr opened: only the train's own hop
    // opens the channel for it.
    let sync = SyncInfo {
        offset_base: 700,
        units_300us: false,
        offset_adjust: false,
        interval: (TRAIN_INTERVAL_S / 1.25e-3).round() as u16,
        channel_map: [0x03, 0, 0, 0, 0],
        sca: 5,
        access_address: TRAIN_AA,
        crc_init: TRAIN_CRC,
        event_counter: 0,
    };
    let mut d = Vec::new();
    push(&mut d, ad::COMPLETE_NAME, b"Coded beacon");
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adv_a: Some(a2),
                    adi: Some((0x055, 5)),
                    sync_info: Some(sync),
                    tx_power: Some(8),
                    ..Default::default()
                },
                &[],
                &d,
            ),
            0.0,
        )
        .phy(GenPhy::Coded(Coding::S2))
        .on(0)
        .next_after(sync.offset_s() + 10e-6),
    );
    let mut d = Vec::new();
    service(&mut d, 0x181A, &[0x10, 0x27]);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adi: Some((0x055, 5)),
                    ..Default::default()
                },
                &[],
                &d,
            ),
            3e-3,
        )
        .phy(GenPhy::Coded(Coding::S2))
        .link(Link::new(TRAIN_AA, TRAIN_CRC, LinkKind::Periodic))
        .hopping(sync.channel_map),
    );
    // a3: its data is on channel 20.
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adv_a: Some(a3),
                    adi: Some((0x0A0, 1)),
                    aux_ptr: Some(aux(20, 40, 0)),
                    tx_power: Some(0),
                    ..Default::default()
                },
                &[],
                &[],
            ),
            4e-3,
        )
        .offset(12e3),
    );
    v
}

/// One device of each kind that an Appearance declares, a Find My tracker and one device that declares nothing, at levels from
/// -3 to -30 dB: a spread for a view that places devices by level. No names, so the named devices stay those of
/// [`test_schedule`].
pub fn kinds_schedule() -> Vec<GenPacket> {
    // Appearance values by category (Assigned Numbers clause 2.6): phone, wearable audio, watch, computer, media player,
    // thermometer, tag.
    let kinds: [(&str, Option<u16>, f64); 7] = [
        ("D0:4B:00:00:00:01", Some(0x0040), -3.0),
        ("D0:4B:00:00:00:02", Some(0x0941), -6.0),
        ("D0:4B:00:00:00:03", Some(0x00C1), -9.0),
        ("D0:4B:00:00:00:04", Some(0x0080), -12.0),
        ("D0:4B:00:00:00:05", Some(0x0280), -15.0),
        ("D0:4B:00:00:00:06", Some(0x0300), -18.0),
        ("D0:4B:00:00:00:07", Some(0x0200), -21.0),
    ];
    let scale = |db: f64| 10f64.powf(db / 20.0);
    let mut v = Vec::new();
    for (i, &(a, appearance, db)) in kinds.iter().enumerate() {
        let mut d = Vec::new();
        push(&mut d, ad::FLAGS, &[6]);
        if let Some(x) = appearance {
            push(&mut d, ad::APPEARANCE, &x.to_le_bytes());
        }
        v.push(
            GenPacket::new(adv(PduType::AdvInd, &addr(a, true), &d), 3e-3)
                .scale(scale(db))
                .offset(-25e3 + 7e3 * i as f64),
        );
    }
    // The offline finding frame of Find My: type 0x12, length 25, status, 22 bytes of key, key bits and hint.
    let mut m = vec![0x12, 0x19, 0x10];
    m.extend((0..22).map(|i| 0x21u8.wrapping_add(5 * i)));
    m.extend([0x02, 0x00]);
    let mut d = Vec::new();
    mfr(&mut d, 0x004C, &m);
    v.push(
        GenPacket::new(
            adv(PduType::AdvNonconnInd, &addr("D0:4B:00:00:00:08", true), &d),
            3e-3,
        )
        .scale(scale(-24.0))
        .offset(6e3),
    );
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    mfr(&mut d, 0xFFFF, &[9, 8, 7]);
    v.push(
        GenPacket::new(
            adv(PduType::AdvNonconnInd, &addr("D0:4B:00:00:00:09", true), &d),
            3e-3,
        )
        .scale(scale(-30.0))
        .offset(-6e3),
    );
    v
}

/// The access address of the periodic train of [`broadcast_schedule`].
pub const BROADCAST_AA: u32 = 0x4A1B_2C3D;

/// An LE Audio broadcast source on channel 37 and the secondary channel 0: an ADV_EXT_IND points to an AUX_ADV_IND with the
/// Broadcast Audio Announcement (Broadcast_ID 123456), the Broadcast_Name "Gate 7", a Public Broadcast Announcement (high
/// quality, program "Boarding") and the SyncInfo of a train on channel 0. The train's AUX_SYNC_IND holds the BIGInfo (two BIS,
/// 10 ms, LE 2M, encrypted when `encrypted`) in its ACAD and the BASE (40 ms, LC3 at 48 kHz, 10 ms frames of 100 octets,
/// English, left and right) in its AdvData. The source is D5:01:02:03:04:05.
pub fn broadcast_schedule(encrypted: bool) -> Vec<GenPacket> {
    use sdrkit_proto_wlan_phy::ble::audio::{BigInfo, BASIC_AUDIO, BIGINFO, BROADCAST_AUDIO, BROADCAST_NAME, PUBLIC_BROADCAST};
    let a = addr("D5:01:02:03:04:05", true);
    let p = aux(0, 50, 0);
    let mut v = vec![GenPacket::new(
        ext(
            PduType::AdvExtInd,
            &ExtHeader {
                adi: Some((0x0B0, 3)),
                aux_ptr: Some(p),
                ..Default::default()
            },
            &[],
            &[],
        ),
        0.0,
    )
    .next_after(after(&p))];
    let sync = SyncInfo {
        offset_base: 300,
        units_300us: false,
        offset_adjust: false,
        interval: 16,
        channel_map: [0x01, 0, 0, 0, 0],
        sca: 5,
        access_address: BROADCAST_AA,
        crc_init: 0x00_5A5A,
        event_counter: 0,
    };
    let mut d = Vec::new();
    service(&mut d, BROADCAST_AUDIO, &[0x56, 0x34, 0x12]);
    push(&mut d, BROADCAST_NAME, b"Gate 7");
    let mut pba = vec![if encrypted { 0x05 } else { 0x04 }, 10, 9, 0x03];
    pba.extend(b"Boarding");
    service(&mut d, PUBLIC_BROADCAST, &pba);
    v.push(
        GenPacket::new(
            ext(
                PduType::AdvExtInd,
                &ExtHeader {
                    adv_a: Some(a),
                    adi: Some((0x0B0, 3)),
                    sync_info: Some(sync),
                    ..Default::default()
                },
                &[],
                &d,
            ),
            0.0,
        )
        .on(0)
        .next_after(sync.offset_s() + 10e-6),
    );
    let big = BigInfo {
        iso_interval: 8,
        num_bis: 2,
        sdu_interval_us: 10_000,
        max_sdu: 100,
        max_pdu: 100,
        phy: 1,
        framed: false,
        encrypted,
    }
    .build();
    let mut acad = vec![big.len() as u8 + 1, BIGINFO];
    acad.extend(&big);
    let base: [u8; 43] = [
        0x40, 0x9C, 0x00, 1, 2, 0x06, 0, 0, 0, 0, 10, 2, 0x01, 0x08, 2, 0x02, 0x01, 3, 0x04, 100, 0, 5, 4, 0x04, b'e', b'n',
        b'g', 1, 6, 5, 0x03, 1, 0, 0, 0, 2, 6, 5, 0x03, 2, 0, 0, 0,
    ];
    let mut d = Vec::new();
    service(&mut d, BASIC_AUDIO, &base);
    v.push(
        GenPacket::new(ext(PduType::AdvExtInd, &ExtHeader::default(), &acad, &d), 20e-3)
            .link(Link::new(BROADCAST_AA, 0x00_5A5A, LinkKind::Periodic))
            .hopping(sync.channel_map),
    );
    v
}

/// The access address of the connection of [`connection_schedule`].
pub const CONN_AA: u32 = 0x5A3C_9617;
/// The CRC preset of that connection.
pub const CONN_CRC: u32 = 0x0A_BCDE;
/// Its channel map: data channels 0 to 8.
pub const CONN_MAP: [u8; 5] = [0xFF, 0x01, 0, 0, 0];
/// Its interval, units of 1.25 ms (10 ms).
pub const CONN_INTERVAL: u16 = 8;
/// Its hop increment (channel selection algorithm #1).
pub const CONN_HOP: u8 = 7;

/// A connection from its CONNECT_IND on channel 37 to ten connection events on data channels 0 to 8: version exchange, a
/// primary service discovery (GAP, then Battery at handles 6 to 9), a read and a notification of handle 8 in the clear,
/// then the encryption start and four events of ciphertext. The central is C1:..., the peripheral D2:.... `ch_sel` sets the
/// ChSel bit of the CONNECT_IND: the connection then uses channel selection algorithm #2.
pub fn connection_schedule(ch_sel: bool) -> Vec<GenPacket> {
    connection_schedule_with(&ConnSchedule {
        ch_sel,
        ..ConnSchedule::default()
    })
}

/// A connection update that [`connection_schedule_with`] sends: LL_CONNECTION_UPDATE_IND in the central's packet of event 3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnUpdate {
    /// The event at which it applies (after 3).
    pub instant: u16,
    /// The new interval, units of 1.25 ms.
    pub interval: u16,
    /// WinOffset, units of 1.25 ms.
    pub win_offset: u16,
}

/// The choices of [`connection_schedule_with`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnSchedule {
    /// The ChSel bit of the CONNECT_IND.
    pub ch_sel: bool,
    /// The channel map.
    pub map: [u8; 5],
    /// A connection update.
    pub update: Option<ConnUpdate>,
}

impl Default for ConnSchedule {
    fn default() -> Self {
        ConnSchedule {
            ch_sel: false,
            map: CONN_MAP,
            update: None,
        }
    }
}

/// The data channel of event `counter` of the schedule's connection.
pub fn conn_channel(c: &ConnSchedule, counter: u16) -> u8 {
    let map = ChannelMap::from_bytes(c.map).expect("the channel map of the schedule");
    if c.ch_sel {
        csa::csa2_channel(counter, csa::channel_identifier(CONN_AA), &map)
    } else {
        csa::Csa1::channel_of(CONN_HOP, counter, &map)
    }
}

/// As [`connection_schedule`] with the channel map and the update of `c`.
pub fn connection_schedule_with(c: &ConnSchedule) -> Vec<GenPacket> {
    let ch_sel = c.ch_sel;
    let central = addr("C1:23:45:67:89:AB", true);
    let peripheral = addr("D2:34:56:78:9A:BC", true);
    let (win_size, win_offset, hop) = (2u8, 1u16, CONN_HOP);
    let mut ll = Vec::new();
    ll.extend(CONN_AA.to_le_bytes());
    ll.extend(&CONN_CRC.to_le_bytes()[..3]);
    ll.push(win_size);
    ll.extend(win_offset.to_le_bytes());
    ll.extend(CONN_INTERVAL.to_le_bytes());
    ll.extend(0u16.to_le_bytes());
    ll.extend(100u16.to_le_bytes());
    ll.extend(c.map);
    ll.push(hop);
    let mut payload = central.bytes.to_vec();
    payload.extend_from_slice(&peripheral.bytes);
    payload.extend(&ll);
    let h = Header {
        pdu_type: PduType::ConnectInd,
        ch_sel,
        tx_add: true,
        rx_add: true,
        length: payload.len() as u8,
    };
    let mut connect = Vec::new();
    build_pdu(&mut connect, h, &payload).expect("the CONNECT_IND of the schedule");
    // A packet of `n` PDU bytes lasts (n + 8) bytes of 8 µs on LE 1M, with the generator's carrier before and after.
    let air = |n: usize| ((n + 8) * 8 + 2 * PAD_BITS) as f64 * 1e-6;
    let interval = f64::from(CONN_INTERVAL) * 1.25e-3;
    // The first central packet 0.5 ms into the transmit window: 1.25 ms after the CONNECT_IND, then WinOffset.
    let first = air(connect.len()) + 1.25e-3 + f64::from(win_offset) * 1.25e-3 + 0.5e-3;
    let link = Link::new(CONN_AA, CONN_CRC, LinkKind::Data);
    let l2 = |att: &[u8]| {
        let mut v = (att.len() as u16).to_le_bytes().to_vec();
        v.extend(4u16.to_le_bytes());
        v.extend_from_slice(att);
        v
    };
    let data = |llid: u8, p: &[u8]| {
        let mut v = vec![llid, p.len() as u8];
        v.extend_from_slice(p);
        v
    };
    let cipher = |n: usize, seed: u8| -> Vec<u8> {
        (0..n)
            .map(|i| seed.wrapping_mul(31).wrapping_add((i as u8).wrapping_mul(17)))
            .collect()
    };
    let mut rsp = vec![0x11, 6];
    for (a, b, u) in [(1u16, 5u16, 0x1800u16), (6, 9, 0x180F)] {
        rsp.extend(a.to_le_bytes());
        rsp.extend(b.to_le_bytes());
        rsp.extend(u.to_le_bytes());
    }
    let mut enc_req = vec![0x03];
    enc_req.extend(cipher(22, 3));
    let mut enc_rsp = vec![0x04];
    enc_rsp.extend(cipher(12, 4));
    let mut events: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (
            data(3, &[0x0C, 0x0D, 0x5F, 0x00, 0x01, 0x00]),
            data(3, &[0x0C, 0x0D, 0x59, 0x00, 0x02, 0x00]),
        ),
        (
            data(2, &l2(&[0x10, 0x01, 0x00, 0xFF, 0xFF, 0x00, 0x28])),
            data(2, &l2(&rsp)),
        ),
        (data(2, &l2(&[0x0A, 0x08, 0x00])), data(2, &l2(&[0x0B, 0x5A]))),
        (
            match c.update {
                Some(u) => {
                    let mut p = vec![0x00, 1];
                    p.extend(u.win_offset.to_le_bytes());
                    p.extend(u.interval.to_le_bytes());
                    p.extend(0u16.to_le_bytes());
                    p.extend(100u16.to_le_bytes());
                    p.extend(u.instant.to_le_bytes());
                    data(3, &p)
                }
                None => data(1, &[]),
            },
            data(2, &l2(&[0x1B, 0x08, 0x00, 0x59])),
        ),
        (data(3, &enc_req), data(3, &enc_rsp)),
        // The peripheral starts the encryption in the clear; both LL_START_ENC_RSP come encrypted (with their MIC).
        (data(1, &[]), data(3, &[0x05])),
        (data(3, &cipher(5, 6)), data(3, &cipher(5, 7))),
    ];
    for k in 0..3u8 {
        events.push((data(2, &cipher(12, 10 + k)), data(2, &cipher(12, 20 + k))));
    }
    // The start of each event's central packet after the CONNECT_IND starts: the update moves the events from its instant
    // on, 0.3 ms into its new transmit window.
    let mut starts = Vec::with_capacity(events.len() + 1);
    let mut t = first;
    for k in 0..=events.len() as u16 {
        starts.push(t);
        let next = k + 1;
        t = match c.update {
            Some(u) if next == u.instant => t + interval + f64::from(u.win_offset) * 1.25e-3 + 0.3e-3,
            Some(u) if next > u.instant => t + f64::from(u.interval) * 1.25e-3,
            _ => t + interval,
        };
    }
    let mut v = vec![GenPacket::new(connect, 0.0).next_after(first)];
    for (k, (cp, pp)) in events.into_iter().enumerate() {
        let ch = conn_channel(c, k as u16);
        let c_air = air(cp.len());
        v.push(GenPacket::new(cp, 0.0).on(ch).link(link).next_after(c_air + 150e-6));
        v.push(
            GenPacket::new(pp, 0.0)
                .on(ch)
                .link(link)
                .next_after(starts[k + 1] - starts[k] - c_air - 150e-6),
        );
    }
    if let Some(last) = v.last_mut() {
        *last = last.clone().next_after(0.05);
    }
    v
}

/// The schedule of the `bletracker` stack: all random static addresses.
pub fn tracker_schedule() -> Vec<GenPacket> {
    let mut v = Vec::new();
    let a = addr("E9:2E:77:C4:31:5A", true);
    let mut m = vec![0x12, 0x19, 0x10];
    m.extend((0..22).map(|i| 0x40u8.wrapping_add(7 * i)));
    m.extend([0x01, 0x00]);
    let mut d = Vec::new();
    mfr(&mut d, 0x004C, &m);
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a, &d), 4e-3).offset(5e3));
    let a = addr("CB:A9:87:65:43:21", true);
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    uuids(&mut d, &[0xFEED]);
    service(
        &mut d,
        0xFEED,
        &[0x02, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77],
    );
    v.push(
        GenPacket::new(adv(PduType::AdvInd, &a, &d), 6e-3)
            .scale(0.6)
            .offset(-10e3),
    );
    let a = addr("F6:55:44:33:22:11", true);
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    uuids(&mut d, &[0xFE33]);
    v.push(
        GenPacket::new(adv(PduType::AdvInd, &a, &d), 7e-3)
            .scale(0.35)
            .offset(15e3),
    );
    let a = addr("D1:22:33:44:55:66", true);
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[6]);
    service(
        &mut d,
        0xFD5A,
        &[0x01, 0x40, 0x8A, 0x13, 0x77, 0x02, 0x55, 0x61],
    );
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a, &d), 6e-3)
            .scale(0.8)
            .offset(-5e3),
    );
    let a = addr("CF:0E:0D:0C:0B:0A", true);
    let mut d = Vec::new();
    service(
        &mut d,
        0xFEAA,
        &[0x41, 0xC2, 0x31, 0x77, 0x08, 0x19, 0xAB, 0x4E, 0x02],
    );
    v.push(
        GenPacket::new(adv(PduType::AdvNonconnInd, &a, &d), 8e-3)
            .scale(0.5)
            .offset(8e3),
    );
    let a = addr("CC:BB:AA:99:88:77", true);
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[0x1A]);
    mfr(&mut d, 0x004C, &[0x10, 0x05, 0x01, 0x18, 0x44, 0x22, 0x11]);
    v.push(GenPacket::new(adv(PduType::AdvInd, &a, &d), 4e-3).scale(0.9));
    v
}

/// The clean samples of one packet at `rate_hz`: a few bits of carrier, the GFSK packet, a few bits of carrier. Errors for a
/// PDU the air format refuses.
pub fn packet_samples(
    p: &GenPacket,
    channel: u8,
    rate_hz: f64,
) -> Result<Vec<Complex32>, GenError> {
    let channel = p.channel.unwrap_or(channel);
    let mut bits = Vec::with_capacity(8 * (p.pdu.len() + 10));
    let symbol_rate = match p.phy {
        GenPhy::Le1M if p.link == Link::ADVERTISING => {
            air_bits(&p.pdu, channel, &mut bits)?;
            ble::SYMBOL_RATE
        }
        GenPhy::Le1M => {
            packet_bits(&p.pdu, &p.link, channel, Uncoded::Le1M, &mut bits)?;
            ble::SYMBOL_RATE
        }
        GenPhy::Le2M => {
            packet_bits(&p.pdu, &p.link, channel, Uncoded::Le2M, &mut bits)?;
            2.0 * ble::SYMBOL_RATE
        }
        GenPhy::Coded(c) => {
            coded_symbols(&p.pdu, &p.link, channel, c, &mut bits)?;
            ble::SYMBOL_RATE
        }
    };
    if p.bad_crc {
        if let Some(b) = bits.last_mut() {
            *b ^= 1;
        }
    }
    let mut levels = vec![0.0f32; PAD_BITS];
    levels.extend(bits.iter().map(|&b| if b == 1 { 1.0 } else { -1.0 }));
    levels.extend([0.0; PAD_BITS]);
    let mut m = Cpm::gfsk(ble::BT, 0.5, rate_hz / symbol_rate)?;
    let mut out = Vec::with_capacity((levels.len() as f64 * rate_hz / 1e6) as usize + 64);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// Makes the bursts of the schedule when they are due.
struct Schedule {
    p: BleGenParams,
    index: usize,
    at_s: f64,
    done: bool,
    /// Rounds of the schedule sent: the event counter of its periodic train.
    round: u16,
    /// The start of the current round, s.
    loop_start: f64,
}

impl BurstSource for Schedule {
    fn next_burst(&mut self) -> Option<Burst> {
        if self.done || self.p.packets.is_empty() {
            return None;
        }
        if self.index >= self.p.packets.len() {
            if !self.p.looped {
                self.done = true;
                return None;
            }
            self.index = 0;
            self.round = self.round.wrapping_add(1);
            // A schedule with a periodic train repeats at the train's interval, so its events keep their times.
            if self.p.packets.iter().any(|p| p.hop.is_some()) {
                self.loop_start += TRAIN_INTERVAL_S;
                self.at_s = self.at_s.max(self.loop_start);
            }
        }
        let i = self.index;
        self.index += 1;
        let pk = &self.p.packets[i];
        let ch = match self.p.whitening {
            Whitening::Round => 37 + (i % 3) as u8,
            Whitening::Fixed(c) => c,
        };
        let mut hopped;
        let counted;
        let pk = match with_sync_counter(&pk.pdu, self.round) {
            Some(pdu) => {
                counted = GenPacket { pdu, ..pk.clone() };
                &counted
            }
            None => pk,
        };
        let pk = match pk.hop.and_then(ChannelMap::from_bytes) {
            Some(map) => {
                hopped = pk.clone();
                let id = csa::channel_identifier(pk.link.access_address);
                hopped.channel = Some(csa::csa2_channel(self.round, id, &map));
                &hopped
            }
            None => pk,
        };
        let samples = packet_samples(pk, ch, self.p.rate_hz).ok()?;
        let start_s = self.at_s;
        self.at_s += match pk.next_after_s {
            Some(n) => n,
            None => samples.len() as f64 / self.p.rate_hz + pk.gap_s,
        };
        let rf = pk.rf_offset_hz();
        // A packet whose channel does not fit the output is left out; the schedule's timing stays.
        let fits = rf.abs() + 1.2e6 <= self.p.rate_hz / 2.0;
        Some(Burst {
            start_s,
            offset_hz: pk.offset_hz + if fits { rf } else { 0.0 },
            level_db: if fits {
                20.0 * pk.scale.max(1e-9).log10()
            } else {
                -300.0
            },
            samples,
        })
    }
}

/// The BLE test-signal generator.
pub struct BleGenerator {
    rate_hz: f64,
    scene: Option<Scene>,
}

impl BleGenerator {
    /// A generator of `p`.
    pub fn new(p: BleGenParams) -> BleGenerator {
        let rate_hz = p.rate_hz;
        let scene = Scene::new(rate_hz).ok().map(|mut s| {
            s.add_source(Box::new(Schedule {
                p,
                index: 0,
                at_s: LEAD_S,
                done: false,
                round: 0,
                loop_start: LEAD_S,
            }));
            s
        });
        BleGenerator { rate_hz, scene }
    }

    /// The `ble` test signal at the rate and whitening that `cfg` specifies.
    pub fn from_config(cfg: &GenConfig) -> BleGenerator {
        BleGenerator::new(BleGenParams {
            rate_hz: cfg.rate_hz,
            whitening: whitening_of(cfg),
            ..Default::default()
        })
    }
}

impl Generator for BleGenerator {
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
