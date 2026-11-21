//! The BLE test signal: a schedule of advertising packets, each GFSK modulated (BT 0.5, modulation index 0.5) and followed by
//! silence. The framework adds noise and the receiver's impairments from the standard `gen_*` settings.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part A clause 3.1 (modulation) and Part B clause 2.1 (packet).

use crate::testgen::GenError;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::{Burst, BurstSource, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_wlan_phy::ble::ad;
use sdrkit_proto_wlan_phy::ble::{self, air_bits, build_adv, build_pdu, Addr, Header, PduType};

/// Silence before the first packet, s.
const LEAD_S: f64 = 2e-3;
/// Bits of unmodulated carrier before and after each packet.
const PAD_BITS: usize = 3;

/// One packet of the schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct GenPacket {
    /// Header and payload.
    pub pdu: Vec<u8>,
    /// Silence after it, s.
    pub gap_s: f64,
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
            packets: test_schedule(),
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

/// An address from its text (most significant byte first). Panics on bad text: the schedules use constants.
pub fn addr(text: &str, random: bool) -> Addr {
    let mut b = [0u8; 6];
    for (i, p) in text.split(':').enumerate().take(6) {
        b[5 - i] = u8::from_str_radix(p, 16).expect("an address in the schedule");
    }
    Addr { bytes: b, random }
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

/// The eleven-packet schedule of the `ble` stack.
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
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a2, &d), 4e-3).offset(20e3));
    let mut p = scan.bytes.to_vec();
    p.extend_from_slice(&a1.bytes);
    v.push(GenPacket::new(pdu(PduType::ScanReq, true, true, &p), 0.2e-3).offset(-15e3));
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
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3).offset(-30e3));
    let mut d = Vec::new();
    uuids(&mut d, &[0xFEAA]);
    let mut tlm = vec![0x20, 0];
    tlm.extend(3000u16.to_be_bytes());
    tlm.extend(((21.5 * 256.0) as i16).to_be_bytes());
    tlm.extend(1234u32.to_be_bytes());
    tlm.extend(56789u32.to_be_bytes());
    service(&mut d, 0xFEAA, &tlm);
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3).offset(-30e3));
    let mut d = Vec::new();
    uuids(&mut d, &[0xFEAA]);
    let mut uid = vec![0x00, (-20i8) as u8];
    uid.extend(hex("EDD1EBEAC04E5DEFA017000000000001"));
    uid.extend([0, 0]);
    service(&mut d, 0xFEAA, &uid);
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a5, &d), 3e-3).offset(-30e3));
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[0x1A]);
    let mut rpi = hex("3C1A09775E42810DC62B94F06E13A558");
    rpi.extend([0x40, 0x08, 0x00, 0x00]);
    service(&mut d, 0xFD6F, &rpi);
    v.push(GenPacket::new(adv(PduType::AdvNonconnInd, &a8, &d), 4e-3).offset(10e3));
    let mut d = Vec::new();
    push(&mut d, ad::FLAGS, &[0x1A]);
    mfr(&mut d, 0x004C, &[0x10, 0x05, 0x01, 0x18, 0x44, 0x22, 0x11]);
    v.push(GenPacket::new(adv(PduType::AdvScanInd, &a8, &d), 3e-3).offset(10e3));
    let mut p = scan.bytes.to_vec();
    p.extend_from_slice(&a1.bytes);
    p.extend(hex("712A3B50123456020000180000004800FFFFFFFF1F05"));
    v.push(GenPacket::new(pdu(PduType::ConnectInd, true, true, &p), 4e-3).offset(-15e3));
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
    let mut bits = Vec::with_capacity(8 * (p.pdu.len() + 10));
    air_bits(&p.pdu, channel, &mut bits)?;
    if p.bad_crc {
        if let Some(b) = bits.last_mut() {
            *b ^= 1;
        }
    }
    let mut levels = vec![0.0f32; PAD_BITS];
    levels.extend(bits.iter().map(|&b| if b == 1 { 1.0 } else { -1.0 }));
    levels.extend([0.0; PAD_BITS]);
    let mut m = Cpm::gfsk(ble::BT, 0.5, rate_hz / ble::SYMBOL_RATE)?;
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
        }
        let i = self.index;
        self.index += 1;
        let pk = &self.p.packets[i];
        let ch = match self.p.whitening {
            Whitening::Round => 37 + (i % 3) as u8,
            Whitening::Fixed(c) => c,
        };
        let samples = packet_samples(pk, ch, self.p.rate_hz).ok()?;
        let start_s = self.at_s;
        self.at_s += samples.len() as f64 / self.p.rate_hz + pk.gap_s;
        Some(Burst {
            start_s,
            offset_hz: pk.offset_hz,
            level_db: 20.0 * pk.scale.max(1e-9).log10(),
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
