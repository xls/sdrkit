//! LoRaWAN PHY payloads and the channel lists of a plan. Receive only and keyless: the MIC is shown, never checked.
//!
//! Standard: LoRaWAN L2 1.0.4 (TS001) chapters 4 and 5. The channels of a plan are settings of the host.

use super::error::{FrameType, ParseError, Part};
use sdrkit_util_ids::lorawan::DevAddr;
use sdrkit_util_ids::oui;
use serde::{Deserialize, Serialize};

/// MType names.
pub fn mtype_name(m: u8) -> &'static str {
    match m {
        0 => "Join request",
        1 => "Join accept",
        2 => "Unconfirmed up",
        3 => "Unconfirmed down",
        4 => "Confirmed up",
        5 => "Confirmed down",
        6 => "Rejoin request",
        _ => "Proprietary",
    }
}

/// A parsed PHY payload (`lorawan.frame`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LorawanFrame {
    /// MType.
    pub mtype: u8,
    /// Uplink.
    pub uplink: bool,
    /// Confirmed.
    pub confirmed: bool,
    /// DevAddr (data frames).
    pub devaddr: Option<u32>,
    /// DevEUI (join and rejoin requests).
    pub dev_eui: Option<u64>,
    /// JoinEUI.
    pub join_eui: Option<u64>,
    /// DevNonce.
    pub dev_nonce: Option<u16>,
    /// FCtrl.
    pub fctrl: u8,
    /// FCnt (16 bits on the air).
    pub fcnt: Option<u16>,
    /// FOpts length.
    pub fopts_len: u8,
    /// FPort.
    pub fport: Option<u8>,
    /// FRMPayload length.
    pub frm_len: usize,
    /// The set flags ("ADR, ACK").
    pub flags: String,
    /// MAC command names with their values.
    pub mac_commands: Vec<String>,
    /// MAC command names only.
    pub mac_names: Vec<String>,
    /// MIC, first byte in the high bits.
    pub mic: Option<u32>,
    /// Network name of the DevAddr.
    pub network: String,
    /// DevAddr type.
    pub devaddr_type: Option<u8>,
    /// NwkID.
    pub nwkid: Option<u32>,
    /// Vendor of the DevEUI.
    pub vendor: String,
    /// One-line summary.
    pub summary: String,
    /// Note ("payload encrypted", "encrypted", "proprietary").
    pub note: String,
    /// DevStatusAns battery byte.
    pub battery: Option<u8>,
    /// DevStatusAns margin.
    pub margin: Option<i8>,
    /// ADR flag.
    pub adr: bool,
}

/// An EUI as 16 upper-case hex digits, most significant first.
pub fn eui_text(e: u64) -> String {
    format!("{e:016X}")
}

/// Upper-case hex pairs separated by blanks.
pub fn hex_pairs(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64)
}

/// The maker of a DevEUI or JoinEUI from its IEEE OUI (the first three bytes, `sdrkit_util_ids::oui`). The block 70-B3-D5-7E-D of
/// The Things Network has its own name. Empty when the OUI is not known.
pub fn vendor(eui: u64) -> String {
    if eui >> 28 == 0x70B3D57ED {
        return "The Things Network block".into();
    }
    let b = eui.to_be_bytes();
    oui::vendor([b[0], b[1], b[2]]).unwrap_or("").to_string()
}

/// The network of a DevAddr: (NetID type, NwkID, operator). The layout is LoRaWAN Backend Interfaces TS002 clause 6.1 and the
/// operator names come from the LoRa Alliance NetID allocation list (`sdrkit_util_ids::lorawan`).
pub fn network(addr: u32) -> (Option<u8>, Option<u32>, String) {
    let d = DevAddr(addr);
    let Some(t) = d.net_type() else {
        return (None, None, "reserved DevAddr".into());
    };
    let nwk = d.nwk_id();
    let name = match (d.operator(), d.net_id()) {
        (Some(op), _) => op.to_string(),
        (None, Some(id)) if t <= 2 => format!("NetID 0x{:06X}", id.0),
        (None, _) => format!("NetID type {t}, NwkID 0x{:X}", nwk.unwrap_or(0)),
    };
    (Some(t), nwk, name)
}

/// The MAC commands of `b` (uplink or downlink), names with values.
pub fn mac_commands(b: &[u8], uplink: bool) -> (Vec<String>, Vec<String>, Option<(u8, i8)>) {
    let mut out = Vec::new();
    let mut names = Vec::new();
    let mut status = None;
    let mut at = 0;
    while at < b.len() {
        let cid = b[at];
        at += 1;
        if cid >= 0x80 {
            let n = format!("proprietary 0x{cid:02X}");
            out.push(n.clone());
            names.push(n);
            break;
        }
        let (up, ul, down, dl) = match cid {
            0x01 => ("ResetInd", 1, "ResetConf", 1),
            0x02 => ("LinkCheckReq", 0, "LinkCheckAns", 2),
            0x03 => ("LinkADRAns", 1, "LinkADRReq", 4),
            0x04 => ("DutyCycleAns", 0, "DutyCycleReq", 1),
            0x05 => ("RXParamSetupAns", 1, "RXParamSetupReq", 4),
            0x06 => ("DevStatusAns", 2, "DevStatusReq", 0),
            0x07 => ("NewChannelAns", 1, "NewChannelReq", 5),
            0x08 => ("RXTimingSetupAns", 0, "RXTimingSetupReq", 1),
            0x09 => ("TxParamSetupAns", 0, "TxParamSetupReq", 1),
            0x0A => ("DlChannelAns", 1, "DlChannelReq", 4),
            0x0B => ("RekeyInd", 1, "RekeyConf", 1),
            0x0C => ("ADRParamSetupAns", 0, "ADRParamSetupReq", 1),
            0x0D => ("DeviceTimeReq", 0, "DeviceTimeAns", 5),
            0x0E if !uplink => ("", 0, "ForceRejoinReq", 2),
            0x0F => ("RejoinParamSetupAns", 1, "RejoinParamSetupReq", 1),
            0x10 => ("PingSlotInfoReq", 1, "PingSlotInfoAns", 0),
            0x11 => ("PingSlotChannelAns", 1, "PingSlotChannelReq", 4),
            0x12 => ("BeaconTimingReq", 0, "BeaconTimingAns", 3),
            0x13 => ("BeaconFreqAns", 1, "BeaconFreqReq", 3),
            0x20 => ("DeviceModeInd", 1, "DeviceModeConf", 1),
            _ => {
                let n = format!("CID 0x{cid:02X} (unknown: 1.1 encrypted FOpts?)");
                out.push(n.clone());
                names.push(n);
                break;
            }
        };
        let (name, len) = if uplink { (up, ul) } else { (down, dl) };
        names.push(name.to_string());
        if at + len > b.len() {
            out.push(format!("{name} (truncated)"));
            break;
        }
        let v = &b[at..at + len];
        at += len;
        let s = match name {
            "DevStatusAns" => {
                let bat = v[0];
                let m = ((v[1] & 0x3F) << 2) as i8 >> 2;
                status = Some((bat, m));
                let bt = match bat {
                    0 => "external power".to_string(),
                    255 => "battery unknown".to_string(),
                    b => format!("battery {} %", (b as f64 * 100.0 / 254.0).round() as u32),
                };
                format!("{name} ({bt}, margin {m} dB)")
            }
            "ResetInd" | "RekeyInd" | "ResetConf" | "RekeyConf" => {
                format!("{name} (LoRaWAN 1.{})", v[0] & 0x0F)
            }
            "LinkCheckAns" => format!("{name} (margin {} dB, {} gateways)", v[0], v[1]),
            "LinkADRReq" => format!(
                "{name} (DR{}, TX power {}, channel mask 0x{:04X})",
                v[0] >> 4,
                v[0] & 0x0F,
                u16::from_le_bytes([v[1], v[2]])
            ),
            "DeviceTimeAns" => format!(
                "{name} (GPS time {} s)",
                u32::from_le_bytes([v[0], v[1], v[2], v[3]])
            ),
            _ => name.to_string(),
        };
        out.push(s);
    }
    (out, names, status)
}

/// Parses a PHY payload. `Err` with the reason it is not LoRaWAN.
pub fn parse(b: &[u8]) -> Result<LorawanFrame, ParseError> {
    if b.is_empty() {
        return Err(ParseError::Empty);
    }
    let mhdr = b[0];
    let mtype = mhdr >> 5;
    if mhdr & 0x1F != 0 {
        return Err(ParseError::NotLorawanR1);
    }
    let mut f = LorawanFrame {
        mtype,
        uplink: matches!(mtype, 0 | 2 | 4 | 6),
        confirmed: matches!(mtype, 4 | 5),
        ..Default::default()
    };
    let n = b.len();
    let mic = |b: &[u8]| {
        u32::from_be_bytes(
            b[b.len() - 4..]
                .try_into()
                .expect("the range has the array's length"),
        )
    };
    match mtype {
        0 => {
            if n != 23 {
                return Err(ParseError::Length {
                    kind: FrameType::JoinRequest,
                    got: n,
                });
            }
            let je = le(&b[1..9]);
            let de = le(&b[9..17]);
            let dn = u16::from_le_bytes([b[17], b[18]]);
            f.join_eui = Some(je);
            f.dev_eui = Some(de);
            f.dev_nonce = Some(dn);
            let m = mic(b);
            f.mic = Some(m);
            f.vendor = vendor(de);
            f.summary = format!(
                "DevEUI {} ({}), JoinEUI {}, DevNonce {}, MIC {:08X}",
                eui_text(de),
                f.vendor,
                eui_text(je),
                dn,
                m
            );
        }
        1 => {
            if n != 17 && n != 33 {
                return Err(ParseError::Length {
                    kind: FrameType::JoinAccept,
                    got: n,
                });
            }
            f.frm_len = n - 1;
            f.summary = format!(
                "{} bytes encrypted{}",
                n - 1,
                if n == 33 {
                    " (with a channel list)"
                } else {
                    ""
                }
            );
            f.note = "encrypted".into();
        }
        6 => {
            if n < 2 {
                return Err(ParseError::Short(Part::RejoinRequest));
            }
            let t = b[1];
            let (ok, de, cnt) = match t {
                0 | 2 if n == 19 => (true, le(&b[5..13]), u16::from_le_bytes([b[13], b[14]])),
                1 if n == 24 => {
                    f.join_eui = Some(le(&b[2..10]));
                    (true, le(&b[10..18]), u16::from_le_bytes([b[18], b[19]]))
                }
                _ if n < 19 => return Err(ParseError::Short(Part::RejoinRequest)),
                _ => (false, 0, 0),
            };
            if !ok {
                return Err(ParseError::Length {
                    kind: FrameType::RejoinRequest,
                    got: n,
                });
            }
            f.dev_eui = Some(de);
            let m = mic(b);
            f.mic = Some(m);
            f.vendor = vendor(de);
            f.summary = format!(
                "type {t}, DevEUI {} ({}), RJcount {cnt}, MIC {:08X}",
                eui_text(de),
                f.vendor,
                m
            );
        }
        7 => {
            f.summary = format!("{} bytes, layout unknown", n - 1);
            f.note = "proprietary".into();
        }
        _ => {
            if n < 12 {
                return Err(ParseError::Length {
                    kind: FrameType::DataFrame,
                    got: n,
                });
            }
            let addr = u32::from_le_bytes(
                b[1..5]
                    .try_into()
                    .expect("the range has the array's length"),
            );
            let fctrl = b[5];
            let fcnt = u16::from_le_bytes([b[6], b[7]]);
            let fol = (fctrl & 0x0F) as usize;
            let rest = n - 8 - 4;
            if fol > rest {
                return Err(ParseError::FOptsLen { fopts: fol, got: n });
            }
            f.devaddr = Some(addr);
            f.fctrl = fctrl;
            f.fcnt = Some(fcnt);
            f.fopts_len = fol as u8;
            f.adr = fctrl & 0x80 != 0;
            let mut fl = Vec::new();
            if fctrl & 0x80 != 0 {
                fl.push("ADR");
            }
            if f.uplink {
                if fctrl & 0x40 != 0 {
                    fl.push("ADRACKReq");
                }
                if fctrl & 0x20 != 0 {
                    fl.push("ACK");
                }
                if fctrl & 0x10 != 0 {
                    fl.push("class B");
                }
            } else {
                if fctrl & 0x20 != 0 {
                    fl.push("ACK");
                }
                if fctrl & 0x10 != 0 {
                    fl.push("FPending");
                }
            }
            f.flags = fl.join(", ");
            let (cmds, names, status) = mac_commands(&b[8..8 + fol], f.uplink);
            f.mac_commands = cmds;
            f.mac_names = names;
            if let Some((bat, m)) = status {
                f.battery = Some(bat);
                f.margin = Some(m);
            }
            let after = 8 + fol;
            if n - after > 4 {
                f.fport = Some(b[after]);
                f.frm_len = n - after - 1 - 4;
            }
            f.mic = Some(mic(b));
            let (t, id, name) = network(addr);
            f.devaddr_type = t;
            f.nwkid = id;
            f.network = name;
            f.summary = match f.fport {
                Some(0) => format!("MAC commands in the payload, {} bytes encrypted", f.frm_len),
                Some(p) => format!("FPort {p}, {} bytes encrypted", f.frm_len),
                None => "no payload".into(),
            };
            f.note = if f.fport.is_some() {
                "payload encrypted".into()
            } else {
                "no payload".into()
            };
        }
    }
    Ok(f)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Channel lists.

/// A channel of a plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanChannel {
    /// Frequency, Hz.
    pub hz: f64,
    /// Bandwidth, Hz.
    pub bw: f64,
    /// Lowest SF.
    pub sf_min: u8,
    /// Highest SF.
    pub sf_max: u8,
    /// Gateways answer here (inverted IQ).
    pub downlink: bool,
    /// Devices send here.
    pub uplink: bool,
    /// A default (join) channel.
    pub default: bool,
}

/// The most channels a list holds.
pub const MAX_CHANNELS: usize = 256;

/// One entry of a channel list: `Hz/bandwidth Hz/SF/flags`. `None` if it does not parse.
fn parse_channel(e: &str) -> Option<PlanChannel> {
    let mut f = e.split('/').map(str::trim);
    let hz: f64 = f.next()?.parse().ok()?;
    let bw: f64 = f.next()?.parse().ok()?;
    let sfs = f.next()?;
    let (lo, hi) = sfs.split_once('-').unwrap_or((sfs, sfs));
    let (sf_min, sf_max): (u8, u8) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
    let flags = f.next().unwrap_or("ud");
    if f.next().is_some()
        || !(hz.is_finite() && hz > 0.0 && bw.is_finite() && bw > 0.0)
        || !(5..=12).contains(&sf_min)
        || !(sf_min..=12).contains(&sf_max)
        || flags.chars().any(|c| !matches!(c, 'u' | 'd' | 'j'))
    {
        return None;
    }
    Some(PlanChannel {
        hz: hz.round(),
        bw,
        sf_min,
        sf_max,
        downlink: flags.contains('d'),
        uplink: flags.contains('u'),
        default: flags.contains('j'),
    })
}

/// The channels of the text of the setting `lw_channels`, ascending by frequency then bandwidth. The entries are separated by
/// ";"; each is the frequency in Hz, "/", the bandwidth in Hz, "/", the spreading factors ("7-12" or "7"), "/" and the flags:
/// `u` devices send there, `d` gateways answer there, `j` a default (join) channel. Without flags a channel is `ud`. `None`
/// when an entry does not parse or there are more than [`MAX_CHANNELS`].
pub fn parse_channels(text: &str) -> Option<Vec<PlanChannel>> {
    let mut v = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(parse_channel)
        .collect::<Option<Vec<_>>>()?;
    if v.len() > MAX_CHANNELS {
        return None;
    }
    v.sort_by(|a, b| a.hz.total_cmp(&b.hz).then(a.bw.total_cmp(&b.bw)));
    Some(v)
}

/// The channel that a stack uses when its list is empty: 125 kHz, SF7 to SF12, uplinks and downlinks, at `hz`.
pub fn default_channel(hz: f64) -> PlanChannel {
    PlanChannel {
        hz,
        bw: 125e3,
        sf_min: 7,
        sf_max: 12,
        downlink: true,
        uplink: true,
        default: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::meshtastic::hex_decode;

    #[test]
    fn channel_lists() {
        let v = parse_channels(
            "100500000/125000/7-12/udj; 100100000/125000/7-12/ud;100300000/250000/7",
        )
        .unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!((v[0].hz, v[0].default, v[0].uplink), (100.1e6, false, true));
        assert_eq!((v[1].bw, v[1].sf_min, v[1].sf_max), (250e3, 7, 7));
        assert!(v[2].default && v[2].downlink);
        assert!(parse_channels("").unwrap().is_empty());
        assert!(parse_channels("100.1/125").is_none());
        assert!(parse_channels("100100000/125000/7-13").is_none());
        assert!(parse_channels("100100000/125000/7/x").is_none());
    }

    #[test]
    fn parsing() {
        let b = hex_decode("8034120b26c40700064f0a020a01020355aa55aa").unwrap();
        let f = parse(&b).unwrap();
        assert_eq!(mtype_name(f.mtype), "Confirmed up");
        assert_eq!(f.devaddr, Some(0x260B1234));
        assert_eq!(f.fcnt, Some(7));
        assert_eq!(f.fport, Some(10));
        assert_eq!(f.frm_len, 3);
        assert_eq!(f.mic, Some(0x55AA55AA));
        assert_eq!(f.flags, "ADR, ADRACKReq");
        assert_eq!(
            f.mac_commands,
            ["DevStatusAns (battery 31 %, margin 10 dB)", "LinkCheckReq"]
        );
        assert_eq!((f.devaddr_type, f.nwkid), (Some(0), Some(0x13)));
        assert_eq!(f.network, "The Things Network");
        let j =
            parse(&hex_decode("00010000d07ed5b370563412d07ed5b3702b4a12345678").unwrap()).unwrap();
        assert_eq!(j.dev_nonce, Some(0x4A2B));
        assert_eq!(eui_text(j.dev_eui.unwrap()), "70B3D57ED0123456");
        assert_eq!(eui_text(j.join_eui.unwrap()), "70B3D57ED0000001");
        assert_eq!(j.mic, Some(0x12345678));
        assert_eq!(j.vendor, "The Things Network block");
        let ja = parse(&[0x20; 33]).unwrap();
        assert!(!ja.uplink && ja.summary.contains("channel list"));
        assert!(parse(&[0x41, 0, 0]).is_err());
        assert!(parse(&hex_decode("40341200260f0100aabbccdd").unwrap()).is_err());
        let (c, _, _) = mac_commands(&[0x03, 0x52, 0x07, 0x00, 0x01], false);
        assert_eq!(c[0], "LinkADRReq (DR5, TX power 2, channel mask 0x0007)");
        let (c, _, _) = mac_commands(&[0x03, 0x07], true);
        assert_eq!(c[0], "LinkADRAns");
        let (c, _, _) = mac_commands(&[0x90], true);
        assert_eq!(c[0], "proprietary 0x90");
    }
}
