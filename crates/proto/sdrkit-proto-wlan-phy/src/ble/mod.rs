//! The Bluetooth LE link layer (Bluetooth Core Specification v5.4, Vol 6 Part B): RF channels, the advertising access address,
//! data whitening, the CRC-24, the advertising PDUs (parse and build), the bits on the air of the LE 1M, LE 2M and LE Coded PHYs
//! ([`phy`]) and the receiver of one RF channel ([`BleReceiver`]).
//!
//! The pieces underneath are shared: the whitening register is [`sdrkit_util_bits::seq::ble_whitening`] and the CRC is
//! [`sdrkit_util_bits::crc::catalog::CRC24_BLE`]. The receiver is [`BleReceiver`]; [`AdvReceiver`] is its LE 1M advertising form. The GFSK waveform of a transmitter is
//! `sdrkit_algo_sim::modulate::Cpm::gfsk(0.5, 0.5, sps)` fed with [`air_bits`] as levels (bit 1 is the higher frequency).
//!
//! ```
//! use sdrkit_proto_wlan_phy::ble::{self, Addr, AdvPdu, PduType};
//! let mut pdu = Vec::new();
//! ble::build_adv(&mut pdu, PduType::AdvInd, &Addr::public([0x56, 0x34, 0x12, 0xEE, 0xFF, 0xC0]), &[2, 1, 6]).unwrap();
//! let mut bits = Vec::new();
//! ble::air_bits(&pdu, 37, &mut bits).unwrap();
//! assert_eq!(bits.len(), 8 + 32 + 8 * (pdu.len() + 3));
//! let p = AdvPdu::parse(&pdu).unwrap();
//! assert_eq!(p.header.pdu_type, PduType::AdvInd);
//! assert_eq!(p.adv_a().unwrap().text(), "C0:FF:EE:12:34:56");
//! ```

pub mod ad;
mod coded_rx;
pub mod csa;
pub mod ext;
pub mod ll;
pub mod phy;
mod rx;

pub use coded_rx::code as coded_fec_code;
pub use ext::{AuxPtr, SyncInfo};
pub use phy::{Coding, Link, LinkKind, Phy, Uncoded};
pub use rx::{AdvReceiver, BlePacket, BleReceiver, ChannelPlan, Phys, ReceiverStats, MAX_LINKS};

use crate::{Error, Malformation, Param, Result};
use sdrkit_util_bits::crc::catalog::CRC24_BLE;
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::{pack, seq, BitOrder};

/// The access address of every advertising physical channel packet (Vol 6 Part B clause 2.1.2).
pub const ADV_ACCESS_ADDRESS: u32 = 0x8E89_BED6;
/// The CRC preset of advertising physical channel PDUs (Vol 6 Part B clause 3.1.1): 0x555555.
pub const ADV_CRC_INIT: u32 = 0x55_5555;
/// The longest PDU payload of the LE 1M PHY, bytes (Vol 6 Part B clause 2.3: the length field is 8 bits).
pub const MAX_PAYLOAD: usize = 255;
/// The symbol (bit) rate of the LE 1M PHY.
pub const SYMBOL_RATE: f64 = 1e6;
/// The modulation index of the LE 1M PHY is 0.45 to 0.55 (Vol 6 Part A clause 3.1): the nominal peak deviation, Hz.
pub const DEVIATION_HZ: f64 = 250e3;
/// The bandwidth-time product of the Gaussian filter (Vol 6 Part A clause 3.1).
pub const BT: f64 = 0.5;

/// The centre frequency of RF channel index `channel` (Vol 6 Part B clause 1.4.1: 37, 38 and 39 advertising at 2402, 2426 and
/// 2480 MHz, and 0 to 36 data), in Hz. `None` above 39.
pub fn channel_hz(channel: u8) -> Option<f64> {
    let mhz = match channel {
        37 => 2402,
        38 => 2426,
        39 => 2480,
        0..=10 => 2404 + 2 * channel as u32,
        11..=36 => 2428 + 2 * (channel as u32 - 11),
        _ => return None,
    };
    Some(mhz as f64 * 1e6)
}

/// The primary advertising channel (37, 38 or 39) whose centre is within `tolerance_hz` of `hz`.
pub fn advertising_channel_at(hz: f64, tolerance_hz: f64) -> Option<u8> {
    [37u8, 38, 39]
        .into_iter()
        .find(|&c| channel_hz(c).is_some_and(|f| (hz - f).abs() <= tolerance_hz))
}

/// Whitens (or dewhitens: the same operation) bytes sent on RF channel `channel` (Vol 6 Part B clause 3.2), each byte least
/// significant bit first, starting at the first byte of the PDU header. Errors for a channel above 39.
pub fn whiten(data: &mut [u8], channel: u8) -> Result<()> {
    if channel > 39 {
        return Err(Error::BadParameter(Param::BleChannelIndex));
    }
    Additive::new(seq::ble_whitening(channel)).apply_bytes(data, BitOrder::Lsb);
    Ok(())
}

/// The CRC-24 of a PDU (header and payload) with the given preset (Vol 6 Part B clause 3.1.1), as the 24-bit value whose low byte
/// is sent first, each byte least significant bit first. The preset is [`ADV_CRC_INIT`] on the advertising channels and the
/// CRCInit of the connection elsewhere.
pub fn crc24(pdu: &[u8], preset: u32) -> u32 {
    if preset == ADV_CRC_INIT {
        return CRC24_BLE.checksum(pdu);
    }
    // The catalogue entry has the advertising preset. Another preset is the same register with another start. The register of
    // the reflected engine starts from the bit-reversed preset (the catalogue's init is in normal form).
    let spec = sdrkit_util_bits::crc::CrcSpec::new(
        "CRC-24/BLE connection",
        24,
        0x00065B,
        (preset & 0xFF_FFFF) as u64,
        true,
        true,
        0,
        0,
    );
    spec.compute(pdu) as u32
}

/// The three CRC bytes as sent after the PDU.
pub fn crc24_bytes(pdu: &[u8], preset: u32) -> [u8; 3] {
    let c = crc24(pdu, preset);
    [c as u8, (c >> 8) as u8, (c >> 16) as u8]
}

/// Whether `pdu_and_crc` (header, payload, three CRC bytes) carries a good advertising-channel CRC.
pub fn crc_ok(pdu_and_crc: &[u8]) -> bool {
    pdu_and_crc.len() >= 5 && CRC24_BLE.verify(pdu_and_crc, Trailer::Little)
}

/// The type of an advertising physical channel PDU (Vol 6 Part B clause 2.3, table 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PduType {
    /// ADV_IND (0): connectable and scannable undirected.
    AdvInd,
    /// ADV_DIRECT_IND (1): connectable directed.
    AdvDirectInd,
    /// ADV_NONCONN_IND (2): non-connectable, non-scannable.
    AdvNonconnInd,
    /// SCAN_REQ (3) (AUX_SCAN_REQ on the secondary channels).
    ScanReq,
    /// SCAN_RSP (4).
    ScanRsp,
    /// CONNECT_IND (5) (AUX_CONNECT_REQ on the secondary channels).
    ConnectInd,
    /// ADV_SCAN_IND (6): scannable undirected.
    AdvScanInd,
    /// ADV_EXT_IND (7) and the other common extended advertising payload formats.
    AdvExtInd,
    /// AUX_CONNECT_RSP (8).
    AuxConnectRsp,
    /// A reserved type (9 to 15).
    Reserved(u8),
}

impl PduType {
    /// The type of the 4-bit code.
    pub fn from_code(code: u8) -> PduType {
        match code & 0x0F {
            0 => PduType::AdvInd,
            1 => PduType::AdvDirectInd,
            2 => PduType::AdvNonconnInd,
            3 => PduType::ScanReq,
            4 => PduType::ScanRsp,
            5 => PduType::ConnectInd,
            6 => PduType::AdvScanInd,
            7 => PduType::AdvExtInd,
            8 => PduType::AuxConnectRsp,
            c => PduType::Reserved(c),
        }
    }

    /// The 4-bit code.
    pub fn code(self) -> u8 {
        match self {
            PduType::AdvInd => 0,
            PduType::AdvDirectInd => 1,
            PduType::AdvNonconnInd => 2,
            PduType::ScanReq => 3,
            PduType::ScanRsp => 4,
            PduType::ConnectInd => 5,
            PduType::AdvScanInd => 6,
            PduType::AdvExtInd => 7,
            PduType::AuxConnectRsp => 8,
            PduType::Reserved(c) => c & 0x0F,
        }
    }

    /// The name the specification provides it ("ADV_IND" and so on, "RESERVED" for 9 to 15).
    pub fn name(self) -> &'static str {
        match self {
            PduType::AdvInd => "ADV_IND",
            PduType::AdvDirectInd => "ADV_DIRECT_IND",
            PduType::AdvNonconnInd => "ADV_NONCONN_IND",
            PduType::ScanReq => "SCAN_REQ",
            PduType::ScanRsp => "SCAN_RSP",
            PduType::ConnectInd => "CONNECT_IND",
            PduType::AdvScanInd => "ADV_SCAN_IND",
            PduType::AdvExtInd => "ADV_EXT_IND",
            PduType::AuxConnectRsp => "AUX_CONNECT_RSP",
            PduType::Reserved(_) => "RESERVED",
        }
    }
}

/// The 16-bit header of an advertising physical channel PDU (Vol 6 Part B clause 2.3, figure 2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// PDU type.
    pub pdu_type: PduType,
    /// ChSel: the advertiser supports channel selection algorithm #2.
    pub ch_sel: bool,
    /// TxAdd: the transmitter's address is random.
    pub tx_add: bool,
    /// RxAdd: the receiver's address is random.
    pub rx_add: bool,
    /// Payload length, bytes.
    pub length: u8,
}

impl Header {
    /// Reads the two header bytes.
    pub fn parse(b: &[u8]) -> Option<Header> {
        let (&h, &length) = (b.first()?, b.get(1)?);
        Some(Header {
            pdu_type: PduType::from_code(h),
            ch_sel: h & 0x20 != 0,
            tx_add: h & 0x40 != 0,
            rx_add: h & 0x80 != 0,
            length,
        })
    }

    /// The two header bytes.
    pub fn bytes(&self) -> [u8; 2] {
        [
            self.pdu_type.code()
                | (self.ch_sel as u8) << 5
                | (self.tx_add as u8) << 6
                | (self.rx_add as u8) << 7,
            self.length,
        ]
    }
}

/// The kind of a device address (Vol 6 Part B clause 1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddrKind {
    /// A public device address (IEEE OUI in the top three bytes).
    Public,
    /// A random static address (the two most significant bits 11).
    RandomStatic,
    /// A resolvable private address (01).
    ResolvablePrivate,
    /// A non-resolvable private address (00).
    NonResolvablePrivate,
    /// A random address with the reserved top bits 10.
    RandomReserved,
}

/// A device address: the six bytes as sent (least significant byte first) and whether it is random (TxAdd or RxAdd).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Addr {
    /// The bytes in air order (least significant first).
    pub bytes: [u8; 6],
    /// A random address.
    pub random: bool,
}

impl Addr {
    /// A public address from its bytes in air order.
    pub const fn public(bytes: [u8; 6]) -> Addr {
        Addr {
            bytes,
            random: false,
        }
    }

    /// A random address from its bytes in air order.
    pub const fn random(bytes: [u8; 6]) -> Addr {
        Addr {
            bytes,
            random: true,
        }
    }

    /// The address at byte `at` of `b`.
    pub fn read(b: &[u8], at: usize, random: bool) -> Option<Addr> {
        let s = b.get(at..at.checked_add(6)?)?;
        let mut bytes = [0u8; 6];
        bytes.copy_from_slice(s);
        Some(Addr { bytes, random })
    }

    /// The kind of address.
    pub fn kind(&self) -> AddrKind {
        if !self.random {
            return AddrKind::Public;
        }
        match self.bytes[5] >> 6 {
            3 => AddrKind::RandomStatic,
            1 => AddrKind::ResolvablePrivate,
            0 => AddrKind::NonResolvablePrivate,
            _ => AddrKind::RandomReserved,
        }
    }

    /// The address as text, most significant byte first ("C0:FF:EE:12:34:56").
    pub fn text(&self) -> String {
        let b = &self.bytes;
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            b[5], b[4], b[3], b[2], b[1], b[0]
        )
    }

    /// The vendor of a public address from its OUI (`sdrkit_util_ids::oui`). `None` for a random address or an unknown OUI.
    pub fn vendor(&self) -> Option<&'static str> {
        if self.random {
            return None;
        }
        sdrkit_util_ids::oui::vendor([self.bytes[5], self.bytes[4], self.bytes[3]])
    }
}

/// The link-layer data of a CONNECT_IND (Vol 6 Part B clause 2.3.3.1, figure 2.12).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LlData {
    /// The connection's access address.
    pub access_address: u32,
    /// CRCInit.
    pub crc_init: u32,
    /// WinSize, units of 1.25 ms.
    pub win_size: u8,
    /// WinOffset, units of 1.25 ms.
    pub win_offset: u16,
    /// Interval, units of 1.25 ms.
    pub interval: u16,
    /// Latency, connection events.
    pub latency: u16,
    /// Timeout, units of 10 ms.
    pub timeout: u16,
    /// ChM, the channel map (bit n is data channel n).
    pub channel_map: [u8; 5],
    /// Hop increment (5 bits).
    pub hop: u8,
    /// Sleep clock accuracy code (3 bits).
    pub sca: u8,
}

/// The extended header of the common extended advertising payload format (Vol 6 Part B clause 2.3.4, figure 2.16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtHeader {
    /// AdvMode: bit 0 connectable, bit 1 scannable.
    pub adv_mode: u8,
    /// AdvA.
    pub adv_a: Option<Addr>,
    /// TargetA.
    pub target_a: Option<Addr>,
    /// CTEInfo.
    pub cte_info: Option<u8>,
    /// AdvDataInfo: (DID, SID).
    pub adi: Option<(u16, u8)>,
    /// AuxPtr.
    pub aux_ptr: Option<AuxPtr>,
    /// SyncInfo.
    pub sync_info: Option<SyncInfo>,
    /// TxPower, dBm.
    pub tx_power: Option<i8>,
}

/// A parsed advertising physical channel PDU, borrowing its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvPdu<'a> {
    /// The header.
    pub header: Header,
    /// The body by PDU type.
    pub body: Body<'a>,
}

/// The payload of an advertising PDU by type (Vol 6 Part B clause 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    /// ADV_IND, ADV_NONCONN_IND and ADV_SCAN_IND carry AdvA and AdvData. SCAN_RSP carries AdvA and ScanRspData.
    Adv {
        /// AdvA.
        adv_a: Addr,
        /// AdvData or ScanRspData (AD structures, [`ad`]).
        data: &'a [u8],
    },
    /// ADV_DIRECT_IND: AdvA and TargetA.
    Direct {
        /// AdvA.
        adv_a: Addr,
        /// TargetA.
        target_a: Addr,
    },
    /// SCAN_REQ: ScanA and AdvA.
    ScanReq {
        /// ScanA (TxAdd).
        scan_a: Addr,
        /// AdvA (RxAdd).
        adv_a: Addr,
    },
    /// CONNECT_IND: InitA, AdvA and LLData.
    Connect {
        /// InitA (TxAdd).
        init_a: Addr,
        /// AdvA (RxAdd).
        adv_a: Addr,
        /// LLData.
        ll: LlData,
    },
    /// The common extended advertising payload format (ADV_EXT_IND, AUX_ADV_IND, AUX_SCAN_RSP, AUX_SYNC_IND, AUX_CHAIN_IND,
    /// AUX_CONNECT_RSP): extended header and AdvData.
    Extended {
        /// The extended header.
        ext: ExtHeader,
        /// ACAD: the rest of the extended header after its fields (AD structures of the controller).
        acad: &'a [u8],
        /// AdvData after the extended header (AD structures).
        data: &'a [u8],
    },
    /// A reserved PDU type: the payload as received.
    Other(&'a [u8]),
}

fn le(b: &[u8], at: usize, n: usize) -> u32 {
    (0..n).fold(0u32, |a, i| a | (b[at + i] as u32) << (8 * i))
}

impl<'a> AdvPdu<'a> {
    /// Parses a PDU (header and payload). Bytes after the payload are ignored. Errors when the bytes are shorter than the
    /// header says, or the length does not fit the type (Vol 6 Part B clause 2.3.1).
    pub fn parse(b: &'a [u8]) -> Result<AdvPdu<'a>> {
        let header = Header::parse(b).ok_or(Error::InputTooShort)?;
        let len = header.length as usize;
        let p = b.get(2..2 + len).ok_or(Error::InputTooShort)?;
        let bad = |m| Err(Error::Malformed(m));
        let body = match header.pdu_type {
            PduType::AdvInd | PduType::AdvNonconnInd | PduType::AdvScanInd | PduType::ScanRsp => {
                if !(6..=37).contains(&len) {
                    return bad(Malformation::AdvPayloadLength);
                }
                Body::Adv {
                    adv_a: Addr::read(p, 0, header.tx_add).ok_or(Error::InputTooShort)?,
                    data: &p[6..],
                }
            }
            PduType::AdvDirectInd => {
                if len != 12 {
                    return bad(Malformation::AdvDirectLength);
                }
                Body::Direct {
                    adv_a: Addr::read(p, 0, header.tx_add).ok_or(Error::InputTooShort)?,
                    target_a: Addr::read(p, 6, header.rx_add).ok_or(Error::InputTooShort)?,
                }
            }
            PduType::ScanReq => {
                if len != 12 {
                    return bad(Malformation::ScanReqLength);
                }
                Body::ScanReq {
                    scan_a: Addr::read(p, 0, header.tx_add).ok_or(Error::InputTooShort)?,
                    adv_a: Addr::read(p, 6, header.rx_add).ok_or(Error::InputTooShort)?,
                }
            }
            PduType::ConnectInd => {
                if len != 34 {
                    return bad(Malformation::ConnectIndLength);
                }
                let mut channel_map = [0u8; 5];
                channel_map.copy_from_slice(&p[28..33]);
                Body::Connect {
                    init_a: Addr::read(p, 0, header.tx_add).ok_or(Error::InputTooShort)?,
                    adv_a: Addr::read(p, 6, header.rx_add).ok_or(Error::InputTooShort)?,
                    ll: LlData {
                        access_address: le(p, 12, 4),
                        crc_init: le(p, 16, 3),
                        win_size: p[19],
                        win_offset: le(p, 20, 2) as u16,
                        interval: le(p, 22, 2) as u16,
                        latency: le(p, 24, 2) as u16,
                        timeout: le(p, 26, 2) as u16,
                        channel_map,
                        hop: p[33] & 0x1F,
                        sca: p[33] >> 5,
                    },
                }
            }
            PduType::AdvExtInd | PduType::AuxConnectRsp => {
                let (ext, acad, data) = parse_ext(p, header.tx_add, header.rx_add)?;
                Body::Extended { ext, acad, data }
            }
            PduType::Reserved(_) => Body::Other(p),
        };
        Ok(AdvPdu { header, body })
    }

    /// The advertiser's address, when the PDU carries one.
    pub fn adv_a(&self) -> Option<Addr> {
        match self.body {
            Body::Adv { adv_a, .. }
            | Body::Direct { adv_a, .. }
            | Body::ScanReq { adv_a, .. }
            | Body::Connect { adv_a, .. } => Some(adv_a),
            Body::Extended { ext, .. } => ext.adv_a,
            Body::Other(_) => None,
        }
    }

    /// The advertising data (AD structures), when the PDU carries some.
    pub fn data(&self) -> Option<&'a [u8]> {
        match self.body {
            Body::Adv { data, .. } | Body::Extended { data, .. } => Some(data),
            _ => None,
        }
    }
}

/// Parses the common extended advertising payload format: the extended header length and AdvMode byte, the flags, the fields
/// present in flag order, the ACAD after them, and the AdvData after the header.
fn parse_ext(p: &[u8], tx_add: bool, rx_add: bool) -> Result<(ExtHeader, &[u8], &[u8])> {
    let first = *p
        .first()
        .ok_or(Error::Malformed(Malformation::ExtendedNoHeader))?;
    let hl = (first & 0x3F) as usize;
    if 1 + hl > p.len() {
        return Err(Error::Malformed(Malformation::ExtendedHeaderLong));
    }
    let mut e = ExtHeader {
        adv_mode: first >> 6,
        ..Default::default()
    };
    if hl > 0 {
        let hdr = &p[1..1 + hl];
        let flags = hdr[0];
        let mut at = 1usize;
        let mut take = |n: usize| -> Result<usize> {
            if at + n > hdr.len() {
                return Err(Error::Malformed(Malformation::ExtendedFieldPastEnd));
            }
            at += n;
            Ok(at - n)
        };
        if flags & 0x01 != 0 {
            e.adv_a = Addr::read(hdr, take(6)?, tx_add);
        }
        if flags & 0x02 != 0 {
            e.target_a = Addr::read(hdr, take(6)?, rx_add);
        }
        if flags & 0x04 != 0 {
            e.cte_info = Some(hdr[take(1)?]);
        }
        if flags & 0x08 != 0 {
            let v = le(hdr, take(2)?, 2) as u16;
            e.adi = Some((v & 0x0FFF, (v >> 12) as u8));
        }
        if flags & 0x10 != 0 {
            let i = take(3)?;
            e.aux_ptr = Some(AuxPtr::read([hdr[i], hdr[i + 1], hdr[i + 2]]));
        }
        if flags & 0x20 != 0 {
            let i = take(SyncInfo::LEN)?;
            let mut b = [0u8; SyncInfo::LEN];
            b.copy_from_slice(&hdr[i..i + SyncInfo::LEN]);
            e.sync_info = Some(SyncInfo::read(&b));
        }
        if flags & 0x40 != 0 {
            e.tx_power = Some(hdr[take(1)?] as i8);
        }
        let acad = take(0)?;
        return Ok((e, &hdr[acad..], &p[1 + hl..]));
    }
    Ok((e, &[], &p[1 + hl..]))
}

/// Appends a non-extended advertising PDU (header, AdvA, data) for ADV_IND, ADV_NONCONN_IND, ADV_SCAN_IND or SCAN_RSP: TxAdd follows
/// the address. Errors for another type or data over 31 bytes.
pub fn build_adv(out: &mut Vec<u8>, pdu_type: PduType, adv_a: &Addr, data: &[u8]) -> Result<()> {
    if !matches!(
        pdu_type,
        PduType::AdvInd | PduType::AdvNonconnInd | PduType::AdvScanInd | PduType::ScanRsp
    ) {
        return Err(Error::BadParameter(Param::BleAdvType));
    }
    if data.len() > 31 {
        return Err(Error::TooLong);
    }
    let h = Header {
        pdu_type,
        ch_sel: false,
        tx_add: adv_a.random,
        rx_add: false,
        length: (6 + data.len()) as u8,
    };
    out.extend_from_slice(&h.bytes());
    out.extend_from_slice(&adv_a.bytes);
    out.extend_from_slice(data);
    Ok(())
}

/// Appends a PDU of any type from its header fields and payload (the length is set from the payload). Errors for a payload over
/// [`MAX_PAYLOAD`] bytes.
pub fn build_pdu(out: &mut Vec<u8>, header: Header, payload: &[u8]) -> Result<()> {
    if payload.len() > MAX_PAYLOAD {
        return Err(Error::TooLong);
    }
    let h = Header {
        length: payload.len() as u8,
        ..header
    };
    out.extend_from_slice(&h.bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// Appends a PDU of the common extended advertising payload format (Vol 6 Part B clause 2.3.4): the header of `pdu_type`
/// (TxAdd from AdvA, RxAdd from TargetA), the extended header length and AdvMode, the flags and fields `ext` holds in flag
/// order, `acad`, then `data`. Errors for an extended header over 63 bytes or a payload over [`MAX_PAYLOAD`] bytes.
pub fn build_ext(
    out: &mut Vec<u8>,
    pdu_type: PduType,
    ext: &ExtHeader,
    acad: &[u8],
    data: &[u8],
) -> Result<()> {
    let mut hdr = vec![0u8];
    let mut flags = 0u8;
    if let Some(a) = ext.adv_a {
        flags |= 0x01;
        hdr.extend_from_slice(&a.bytes);
    }
    if let Some(a) = ext.target_a {
        flags |= 0x02;
        hdr.extend_from_slice(&a.bytes);
    }
    if let Some(c) = ext.cte_info {
        flags |= 0x04;
        hdr.push(c);
    }
    if let Some((did, sid)) = ext.adi {
        flags |= 0x08;
        hdr.extend_from_slice(&((did & 0x0FFF) | (sid as u16 & 0xF) << 12).to_le_bytes());
    }
    if let Some(a) = ext.aux_ptr {
        flags |= 0x10;
        hdr.extend_from_slice(&a.bytes());
    }
    if let Some(si) = ext.sync_info {
        flags |= 0x20;
        hdr.extend_from_slice(&si.bytes());
    }
    if let Some(t) = ext.tx_power {
        flags |= 0x40;
        hdr.push(t as u8);
    }
    hdr[0] = flags;
    hdr.extend_from_slice(acad);
    let len = if hdr.len() == 1 && acad.is_empty() {
        0
    } else {
        hdr.len()
    };
    if len > 63 {
        return Err(Error::TooLong);
    }
    let mut payload = Vec::with_capacity(1 + len + data.len());
    payload.push(len as u8 | (ext.adv_mode & 3) << 6);
    payload.extend_from_slice(&hdr[..len]);
    payload.extend_from_slice(data);
    let h = Header {
        pdu_type,
        ch_sel: false,
        tx_add: ext.adv_a.is_some_and(|a| a.random),
        rx_add: ext.target_a.is_some_and(|a| a.random),
        length: 0,
    };
    build_pdu(out, h, &payload)
}

/// The preamble byte of an access address (Vol 6 Part B clause 2.1.1): 0xAA when its least significant bit is 0 (so the preamble
/// alternates into it), else 0x55. Sent least significant bit first.
pub fn preamble(access_address: u32) -> u8 {
    if access_address & 1 == 0 {
        0xAA
    } else {
        0x55
    }
}

/// Appends the bits on the air of an advertising packet on the LE 1M PHY (Vol 6 Part B clause 2.1, clause 3): the preamble, the
/// access address, then the PDU and its CRC, whitened for `channel`, every byte least significant bit first. Errors for a PDU
/// shorter than its header or longer than 2 + [`MAX_PAYLOAD`] bytes, or a channel above 39.
pub fn air_bits(pdu: &[u8], channel: u8, out: &mut Vec<u8>) -> Result<()> {
    if pdu.len() < 2 {
        return Err(Error::InputTooShort);
    }
    if pdu.len() > 2 + MAX_PAYLOAD {
        return Err(Error::TooLong);
    }
    if channel > 39 {
        return Err(Error::BadParameter(Param::BleChannelIndex));
    }
    let aa = ADV_ACCESS_ADDRESS;
    pack::push_u64_lsb(out, preamble(aa) as u64, 8);
    pack::push_u64_lsb(out, aa as u64, 32);
    let crc = crc24(pdu, ADV_CRC_INIT);
    let mut w = Additive::new(seq::ble_whitening(channel));
    let start = out.len();
    pack::unpack(pdu, BitOrder::Lsb, out);
    pack::push_u64_lsb(out, crc as u64, 24);
    w.apply_bits(&mut out[start..]);
    Ok(())
}
