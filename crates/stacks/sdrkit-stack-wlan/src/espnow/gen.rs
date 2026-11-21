//! The ESP-NOW test signal: action frames built from IEEE Std 802.11-2020 clause 9 and the maker's public ESP-NOW guide,
//! sealed with CCMP when a sender has a key, and sent on the DSSS and non-HT OFDM PHYs of the Wi-Fi test signal.

use super::ccmp;
use crate::wifi::frames::{beacon, BssSpec, Sec};
use crate::wifi::gen::{GenFrame, Phy};
use crate::wifi::plan::Band;
use sdrkit_proto_wlan_phy::mac::{push_fcs, push_mgmt_header, MgmtSubtype};
use sdrkit_proto_wlan_phy::{dsss, ofdm};

/// The sender address of the test signal (an Espressif prefix).
pub const SENDER: [u8; 6] = [0x24, 0x6F, 0x28, 0x11, 0x22, 0x33];
/// The random value of the test signal.
pub const RANDOM: u32 = 0x5A17_C0DE;
/// The made-up key of the encrypted sender of the test signal: the bytes 00 to 0F.
pub const TEST_KEY: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
];

sdrkit_core::choice_enum! {
    /// The PHY and rate of a test frame.
    pub enum GenPhy {
        /// DSSS 1 Mbit/s, the ESP-NOW default.
        Dsss1 = ("dsss1", "DSSS 1 Mbit/s"),
        /// DSSS 2 Mbit/s with the long preamble.
        Dsss2Long = ("dsss2_long", "DSSS 2 Mbit/s, long preamble"),
        /// DSSS 2 Mbit/s with the short preamble.
        Dsss2Short = ("dsss2_short", "DSSS 2 Mbit/s, short preamble"),
        /// OFDM 6 Mbit/s.
        Ofdm6 = ("ofdm6", "OFDM 6 Mbit/s"),
        /// OFDM 12 Mbit/s.
        Ofdm12 = ("ofdm12", "OFDM 12 Mbit/s"),
    }
}

/// One ESP-NOW frame to build.
#[derive(Clone, Debug, PartialEq)]
pub struct EspNowSpec {
    /// Address 2, the sender.
    pub from: [u8; 6],
    /// Address 1, the destination.
    pub to: [u8; 6],
    /// The sequence number (12 bits).
    pub seq: u16,
    /// The random value.
    pub random: u32,
    /// 1 or 2.
    pub version: u8,
    /// The payload: at most 250 bytes in version 1, 1470 in version 2.
    pub payload: Vec<u8>,
    /// The CCMP key and packet number when the frame is sealed.
    pub sealed: Option<([u8; 16], u64)>,
}

impl EspNowSpec {
    /// A clear version 1 frame from `from` to `to` with `payload`, sequence 0 and the test random value.
    pub fn new(from: [u8; 6], to: [u8; 6], payload: &[u8]) -> EspNowSpec {
        EspNowSpec {
            from,
            to,
            seq: 0,
            random: RANDOM,
            version: 1,
            payload: payload.to_vec(),
            sealed: None,
        }
    }
}

/// The vendor content of a frame: category 127, the Espressif identifier, the random value, then elements of at most 250
/// body bytes; in version 2 every element but the last has "more data" set. Version 1 keeps the first 250 bytes.
fn vendor_content(s: &EspNowSpec) -> Vec<u8> {
    let mut out = vec![127, 0x18, 0xFE, 0x34];
    out.extend_from_slice(&s.random.to_be_bytes());
    let v = s.version & 0x0F;
    let payload: &[u8] = if v >= 2 {
        &s.payload[..s.payload.len().min(1470)]
    } else {
        &s.payload[..s.payload.len().min(250)]
    };
    let mut parts: Vec<&[u8]> = payload.chunks(250).collect();
    if parts.is_empty() {
        parts.push(&[]);
    }
    let last = parts.len() - 1;
    for (i, body) in parts.into_iter().enumerate() {
        let more = if v >= 2 && i < last { 0x10 } else { 0 };
        out.extend_from_slice(&[221, (5 + body.len()) as u8, 0x18, 0xFE, 0x34, 4, v | more]);
        out.extend_from_slice(body);
    }
    out
}

/// The frame of `s` with its FCS: a management action frame (frame control D0) with address 3 the broadcast address.
pub fn espnow_frame(s: &EspNowSpec) -> Vec<u8> {
    let mut f = Vec::with_capacity(64 + s.payload.len());
    push_mgmt_header(&mut f, MgmtSubtype::Action, s.to, s.from, [0xFF; 6], s.seq);
    let mut content = vendor_content(s);
    match &s.sealed {
        None => f.extend_from_slice(&content),
        Some((key, pn)) => {
            f[1] |= 0x40;
            let header: [u8; ccmp::HEADER_LEN] = f[..ccmp::HEADER_LEN]
                .try_into()
                .unwrap_or([0; ccmp::HEADER_LEN]);
            let c = ccmp::cipher(key);
            let mic = ccmp::seal(&c, ccmp::FLAG_MANAGEMENT, &header, *pn, &mut content)
                .unwrap_or_default();
            f.extend_from_slice(&ccmp::ccmp_header(*pn, 0));
            f.extend_from_slice(&content);
            f.extend_from_slice(&mic);
        }
    }
    push_fcs(&mut f);
    f
}

/// A test frame of `mpdu` (FCS included) on `phy`, with a gap of 1.5 ms.
pub fn on_phy(mpdu: Vec<u8>, phy: GenPhy) -> GenFrame {
    let g = match phy {
        GenPhy::Dsss1 => GenFrame::new(mpdu, Phy::Dsss),
        GenPhy::Dsss2Long => {
            GenFrame::new(mpdu, Phy::Dsss).dsss(dsss::Rate::Dqpsk2, dsss::Preamble::Long)
        }
        GenPhy::Dsss2Short => {
            GenFrame::new(mpdu, Phy::Dsss).dsss(dsss::Rate::Dqpsk2, dsss::Preamble::Short)
        }
        GenPhy::Ofdm6 => GenFrame::new(mpdu, Phy::Ofdm),
        GenPhy::Ofdm12 => GenFrame::new(mpdu, Phy::Ofdm).ofdm_rate(ofdm::Rate::R12),
    };
    g.gap(1.5e-3)
}

/// The access point beacon of the scene, which is not ESP-NOW.
pub fn ap_beacon() -> Vec<u8> {
    beacon(&BssSpec::new(
        [0x00, 0x04, 0x0E, 0x12, 0x34, 0x56],
        "Test-Home",
        Band::G2,
        1,
        Sec::Wpa2,
    ))
}

/// The mixed scene: `first` (the frame the options choose) on `phy`, then a unicast binary frame at 1 Mbit/s, frames at
/// 2 Mbit/s with the long and the short preamble, at OFDM 6 and 12 Mbit/s, a 600-byte version 2 frame in three elements, a
/// unicast frame sealed with [`TEST_KEY`], and an access point beacon. Five senders.
pub fn scene(first: &EspNowSpec, phy: GenPhy) -> Vec<GenFrame> {
    let peer = [0x84, 0xCC, 0xA8, 0x01, 0x02, 0x03];
    let n2 = [0x30, 0xAE, 0xA4, 0x10, 0x20, 0x30];
    let n3 = [0x7C, 0x9E, 0xBD, 0x40, 0x50, 0x60];
    let n4 = [0xA4, 0xCF, 0x12, 0x70, 0x80, 0x90];
    let n5 = [0x24, 0x0A, 0xC4, 0xAA, 0xBB, 0xCC];
    let unicast = EspNowSpec {
        seq: 291,
        random: 0xA1B2_C3D4,
        ..EspNowSpec::new(
            SENDER,
            peer,
            &[
                0x01, 0x02, 0x03, 0x04, 0x2A, 0x00, 0x00, 0x00, 0x10, 0x27, 0x00, 0x00,
            ],
        )
    };
    let long600: Vec<u8> = (0..600u32).map(|i| (i * 7 + 3) as u8).collect();
    let v2 = EspNowSpec {
        version: 2,
        seq: 7,
        ..EspNowSpec::new(n4, [0xFF; 6], &long600)
    };
    let sealed = EspNowSpec {
        seq: 40,
        sealed: Some((TEST_KEY, 1)),
        ..EspNowSpec::new(n5, SENDER, b"secret door code 4711")
    };
    let to_all = [0xFF; 6];
    vec![
        on_phy(espnow_frame(first), phy),
        on_phy(espnow_frame(&unicast), GenPhy::Dsss1),
        on_phy(
            espnow_frame(&EspNowSpec {
                seq: 1,
                ..EspNowSpec::new(n2, to_all, b"node 2 long preamble")
            }),
            GenPhy::Dsss2Long,
        ),
        on_phy(
            espnow_frame(&EspNowSpec {
                seq: 2,
                ..EspNowSpec::new(n3, to_all, b"node 3 short preamble")
            }),
            GenPhy::Dsss2Short,
        )
        .scale(0.8)
        .offset(10e3),
        on_phy(
            espnow_frame(&EspNowSpec {
                seq: 3,
                ..EspNowSpec::new(n4, to_all, b"node 4 at 6 Mbit/s")
            }),
            GenPhy::Ofdm6,
        ),
        on_phy(espnow_frame(&v2), GenPhy::Ofdm12),
        on_phy(espnow_frame(&sealed), GenPhy::Dsss1).scale(0.7),
        on_phy(ap_beacon(), GenPhy::Dsss1).gap(3e-3),
    ]
}

/// Six bytes from "AA:BB:CC:DD:EE:FF" (':' or '-' between the bytes, or none) or "broadcast"; `None` for any other text.
pub fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("broadcast") {
        return Some([0xFF; 6]);
    }
    let digits: Vec<u8> = t.bytes().filter(|&c| c != b':' && c != b'-').collect();
    if digits.len() != 12 || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut a = [0u8; 6];
    for (i, pair) in digits.chunks(2).enumerate() {
        a[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(a)
}
