//! The Aero signal unit layer: SU types and what is decoded of them, the system facts, log-ons, and the ISU and SSU reassembly of
//! user data into ACARS blocks, which `sdrkit-proto-aero` joins and reads (ADS-C, text positions).
//! Standard: Inmarsat Aeronautical System Definition Manual; ARINC 741; ARINC 618.

use super::coding::su_ok;
use sdrkit_proto_aero::acars::assemble::{Assembler, Joined, Quality};
use sdrkit_proto_aero::acars::Reading;
use sdrkit_proto_aero::acars::{self, reading_of, Block, Check, Message};
use sdrkit_proto_aero::adsc::tag_name;
use serde::{Deserialize, Serialize};

/// The name of an SU type.
pub fn type_name(t: u8, ok: bool) -> &'static str {
    if !ok {
        return "Bad CRC";
    }
    match t {
        0x01 => "Fill-in",
        0x05 => "System table: GES channels",
        0x07 => "System table: GES beam support",
        0x0A => "System table: index",
        0x0C => "System table: satellite",
        0x10 => "Log-on request",
        0x11 => "Log-on confirm",
        0x12 => "Log-off request",
        0x13 => "Log-on reject",
        0x14 => "Log-on interrogation",
        0x15 => "Log-on/off acknowledge",
        0x16 => "Log-on prompt",
        0x17 => "Data channel reassignment",
        0x18 | 0x19 | 0x26 => "Reserved",
        0x21 => "Call announcement",
        0x28 => "EIRP table",
        0x30 => "Call progress",
        0x31 => "C channel assignment (distress)",
        0x32 => "C channel assignment (flight safety)",
        0x33 => "C channel assignment (other safety)",
        0x34 => "C channel assignment (non-safety)",
        0x40 => "P/R channel control",
        0x41 => "T channel control",
        0x51 => "T channel assignment",
        0x60 => "Telephony acknowledge",
        0x61 => "Request for acknowledgement",
        0x62 => "Acknowledgement",
        0x71 => "User data ISU",
        0x74 => "User data 3-octet LSDU",
        0x76 => "User data 4-octet LSDU",
        0xC0..=0xFF => "User data SSU",
        _ => "Unknown",
    }
}

/// The channel grid: channel x 2.5 kHz + 1510 MHz.
pub fn channel_hz(ch: u16) -> f64 {
    1510e6 + ch as f64 * 2.5e3
}

/// The P/R channel control rate code.
pub fn rate_of(code: u8) -> u32 {
    match code {
        0 => 600,
        1 => 1200,
        2 => 2400,
        3 => 4800,
        4 => 6000,
        5 => 5250,
        6 => 10500,
        7 => 8400,
        9 => 21000,
        _ => 0,
    }
}

/// A system fact (satellite id or P/R channel control).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SysFact {
    pub kind: String,
    pub key: String,
    pub text: String,
    pub satellite: Option<u8>,
    pub longitude: Option<f64>,
    pub spot_beam: bool,
    pub ges: Option<u8>,
    pub p_channel_hz: Option<f64>,
    pub rate: Option<u32>,
    pub count: u64,
    pub last: f64,
}

/// What one SU says.
#[derive(Clone, Debug, PartialEq)]
pub enum SuInfo {
    Plain,
    Logon {
        aes: u32,
        ges: u8,
        on: bool,
    },
    Fact(SysFact),
    Isu {
        aes: u32,
        ges: u8,
        q: u8,
        refno: u8,
        ssus: u8,
        last_octets: u8,
        data: [u8; 2],
    },
    Ssu {
        n: u8,
        q: u8,
        refno: u8,
        data: [u8; 8],
    },
}

fn aes_of(s: &[u8]) -> u32 {
    (s[1] as u32) << 16 | (s[2] as u32) << 8 | s[3] as u32
}

/// Reads one SU (its CRC good).
pub fn read(s: &[u8; 12]) -> (SuInfo, String) {
    let t = s[0];
    let name = type_name(t, true);
    match t {
        0x10..=0x17 => {
            let aes = aes_of(s);
            let ges = s[4];
            let info = if t == 0x11 || t == 0x12 {
                SuInfo::Logon {
                    aes,
                    ges,
                    on: t == 0x11,
                }
            } else {
                SuInfo::Plain
            };
            (info, format!("{name} AES {aes:06X} GES {ges:02X}"))
        }
        0x21 | 0x30..=0x34 => (
            SuInfo::Plain,
            format!("{name} AES {:06X} GES {:02X}", aes_of(s), s[4]),
        ),
        0x0C => {
            let sat = ((s[2] << 4) & 0x30) | (s[3] >> 4);
            let mut lon = s[5] as f64 * 1.5;
            if lon > 180.0 {
                lon -= 360.0;
            }
            let p1 = ((s[6] as u16) << 8 | s[7] as u16) & 0x7FFF;
            let spot = s[6] & 0x80 != 0;
            let p = channel_hz(p1);
            let text = format!(
                "satellite {sat} at {:.1}{}, P channel {:.4} MHz{}",
                lon.abs(),
                if lon < 0.0 { "W" } else { "E" },
                p / 1e6,
                if spot { " (spot beam)" } else { "" }
            );
            (
                SuInfo::Fact(SysFact {
                    kind: "satellite".into(),
                    key: format!("sat{sat}"),
                    text: text.clone(),
                    satellite: Some(sat),
                    longitude: Some(lon),
                    spot_beam: spot,
                    p_channel_hz: Some(p),
                    ..Default::default()
                }),
                text,
            )
        }
        0x05 => {
            let ges = s[3];
            let seq = s[2] >> 2;
            let part = s[2] & 3;
            let names = match part {
                0 | 1 => ["Psmc", "Rsmc0", "Rsmc1"],
                2 => ["Rsmc2", "Rsmc3", "Rsmc4"],
                _ => ["Rsmc5", "Rsmc6", "Rsmc7"],
            };
            let chans: Vec<String> = (0..3)
                .map(|k| {
                    let ch = (s[4 + 2 * k] as u16) << 8 | s[5 + 2 * k] as u16;
                    let mut f = channel_hz(ch);
                    if names[k].starts_with('R') {
                        f += 101.5e6;
                    }
                    format!("{} {:.4}", names[k], f / 1e6)
                })
                .collect();
            (
                SuInfo::Plain,
                format!(
                    "{name}: GES {ges:02X} sequence {seq} part {part}: {}",
                    chans.join(", ")
                ),
            )
        }
        0x40 => {
            let ges = s[4];
            let rate = rate_of(s[7] >> 4);
            let pch = (s[8] as u16) << 8 | s[9] as u16;
            let spot = pch & 0x8000 != 0;
            let p = channel_hz(pch & 0x7FFF);
            let text = format!(
                "GES {ges:02X} P channel {:.4} MHz at {rate} bit/s{}",
                p / 1e6,
                if spot { " (spot beam)" } else { "" }
            );
            (
                SuInfo::Fact(SysFact {
                    kind: "p/r channel control".into(),
                    key: format!("ges{ges:02X}-{pch}"),
                    text: text.clone(),
                    ges: Some(ges),
                    p_channel_hz: Some(p),
                    rate: Some(rate),
                    spot_beam: spot,
                    ..Default::default()
                }),
                text,
            )
        }
        0x71 => {
            let info = SuInfo::Isu {
                aes: aes_of(s),
                ges: s[4],
                q: s[5] >> 4,
                refno: s[5] & 15,
                ssus: s[6] & 0x3F,
                last_octets: s[7] >> 4,
                data: [s[8], s[9]],
            };
            (
                info,
                format!("{name} AES {:06X} GES {:02X}", aes_of(s), s[4]),
            )
        }
        0xC0..=0xFF => {
            let mut d = [0u8; 8];
            d.copy_from_slice(&s[2..10]);
            (
                SuInfo::Ssu {
                    n: t & 0x3F,
                    q: s[1] >> 4,
                    refno: s[1] & 15,
                    data: d,
                },
                format!("{name} {}", t & 0x3F),
            )
        }
        _ => (SuInfo::Plain, name.to_string()),
    }
}

/// Builds the SUs of an ISU and its SSUs carrying `data` (for the generator).
pub fn build_user_data(aes: u32, ges: u8, q: u8, refno: u8, data: &[u8]) -> Vec<[u8; 12]> {
    let rest = data.len().saturating_sub(2);
    let ssus = rest.div_ceil(8).max(1);
    let last = if rest == 0 { 0 } else { rest - (ssus - 1) * 8 };
    let mut out = Vec::new();
    let mut isu = [0u8; 12];
    isu[0] = 0x71;
    isu[1] = (aes >> 16) as u8;
    isu[2] = (aes >> 8) as u8;
    isu[3] = aes as u8;
    isu[4] = ges;
    isu[5] = (q << 4) | (refno & 15);
    isu[6] = ssus as u8;
    isu[7] = (last as u8) << 4;
    isu[8] = data.first().copied().unwrap_or(0);
    isu[9] = data.get(1).copied().unwrap_or(0);
    super::coding::su_seal(&mut isu);
    out.push(isu);
    for k in 0..ssus {
        let n = ssus - 1 - k;
        let mut s = [0u8; 12];
        s[0] = 0xC0 | n as u8;
        s[1] = (q << 4) | (refno & 15);
        for i in 0..8 {
            s[2 + i] = data.get(2 + k * 8 + i).copied().unwrap_or(0);
        }
        super::coding::su_seal(&mut s);
        out.push(s);
    }
    out
}

/// An ACARS message (one block or several joined), or user data that is not ACARS.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AeroMessage {
    pub channel: usize,
    pub rate: u32,
    pub aes: String,
    pub ges: String,
    pub downlink: bool,
    pub mode: char,
    pub registration: String,
    pub flight: String,
    pub label: String,
    pub label_name: String,
    pub block_id: char,
    pub message_number: String,
    pub text: String,
    /// Every block check (BCS) was good.
    pub check_ok: bool,
    /// Every block of a multi-block message arrived.
    pub complete: bool,
    /// The ADS-C groups read, by name.
    pub adsc: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_ft: Option<f64>,
    /// The carrier, Hz from the input's centre.
    pub carrier_hz: f64,
    pub icao: Option<u32>,
    pub non_acars: bool,
    /// The one-line reading of the fixed format, empty when nothing was read.
    pub reading: String,
    /// The kind of the reading ("Position report", "CPDLC message" ...).
    pub reading_kind: String,
    /// The category of the reading (other, position, OOOI, ETA ...).
    pub reading_category: String,
    /// The CPDLC envelope, when the text holds one.
    pub cpdlc: Option<CpdlcRow>,
}

/// What a message row keeps of a FANS-1/A CPDLC envelope.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CpdlcRow {
    /// AT1, CR1, CC1 or DR1.
    pub imi: String,
    /// The ground facility.
    pub ground: String,
    /// The aircraft.
    pub aircraft: String,
    /// The envelope check passed.
    pub crc_ok: bool,
    /// The body decoded completely.
    pub parsed: bool,
    /// The decoded text.
    pub text: String,
}

#[derive(Clone, Debug)]
struct WaitingIsu {
    aes: u32,
    ges: u8,
    q: u8,
    refno: u8,
    remaining: u8,
    last_octets: u8,
    data: Vec<u8>,
    time: f64,
}

/// What an ACARS block carries beside its text: the AES and GES of its ISU.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Link {
    pub aes: u32,
    pub ges: u8,
}

/// The reassembler of one receiver (all channels): ISU and SSU into user data, user data into ACARS blocks, and blocks into
/// messages.
#[derive(Default)]
pub struct Reassembler {
    isus: Vec<WaitingIsu>,
    blocks: Assembler<Link>,
    joined: Vec<Joined<Link>>,
    body: Vec<u8>,
}

impl Reassembler {
    /// Feeds one SU heard at signal time `t`; appends complete messages (without channel and carrier filled).
    pub fn push(&mut self, info: &SuInfo, t: f64, out: &mut Vec<AeroMessage>) {
        self.isus.retain(|w| t - w.time <= 60.0);
        self.blocks.expire(t, &mut self.joined);
        match info {
            SuInfo::Isu {
                aes,
                ges,
                q,
                refno,
                ssus,
                last_octets,
                data,
            } => {
                if *ssus == 0 || *last_octets > 8 {
                    return;
                }
                self.isus.retain(|w| {
                    !(w.aes == *aes && w.ges == *ges && w.q == *q && w.refno == *refno)
                });
                if self.isus.len() >= 16 {
                    self.isus.remove(0);
                }
                self.isus.push(WaitingIsu {
                    aes: *aes,
                    ges: *ges,
                    q: *q,
                    refno: *refno,
                    remaining: *ssus,
                    last_octets: *last_octets,
                    data: data.to_vec(),
                    time: t,
                });
            }
            SuInfo::Ssu { n, q, refno, data } => {
                let Some(i) = self
                    .isus
                    .iter()
                    .rposition(|w| w.q == *q && w.refno == *refno && w.remaining == n + 1)
                else {
                    return;
                };
                let w = &mut self.isus[i];
                w.remaining -= 1;
                if w.remaining == 0 {
                    w.data.extend_from_slice(&data[..w.last_octets as usize]);
                    let w = self.isus.remove(i);
                    self.user_data(w.aes, w.ges, &w.data, t, out);
                } else {
                    w.data.extend_from_slice(data);
                }
            }
            _ => {}
        }
        self.give_joined(out);
    }

    /// Returns out every open multi-block message, incomplete (at the end of the input).
    pub fn flush(&mut self, out: &mut Vec<AeroMessage>) {
        self.blocks.flush(&mut self.joined);
        self.give_joined(out);
    }

    /// Forgets every message being put together.
    pub fn clear(&mut self) {
        self.isus.clear();
        self.blocks.clear();
        self.joined.clear();
    }

    fn give_joined(&mut self, out: &mut Vec<AeroMessage>) {
        for j in self.joined.drain(..) {
            out.push(message(&Message::from_joined(&j), j.body.extra));
        }
    }

    fn user_data(&mut self, aes: u32, ges: u8, data: &[u8], t: f64, out: &mut Vec<AeroMessage>) {
        let link = Link { aes, ges };
        let Ok(f) = acars::split_frame(data) else {
            let mut text = String::new();
            sdrkit_core::base64::hex_into(data, &mut text);
            out.push(AeroMessage {
                aes: format!("{aes:06X}"),
                ges: format!("{ges:02X}"),
                text,
                label_name: "Non-ACARS user data".into(),
                non_acars: true,
                check_ok: true,
                complete: true,
                ..Default::default()
            });
            return;
        };
        self.body.clear();
        self.body.extend_from_slice(f.body);
        let quality = match acars::check_and_repair(&mut self.body, f.bcs) {
            Check::Good { bits_fixed } => Quality {
                bits_fixed,
                bcs_ok: true,
            },
            Check::Bad => Quality {
                bits_fixed: 0,
                bcs_ok: false,
            },
        };
        acars::strip_parity(&mut self.body);
        let Ok(b) = Block::parse(&self.body) else {
            return;
        };
        self.blocks
            .push(t, &b, quality, aes, link, &mut self.joined);
    }
}

/// The message of an ACARS message read by `sdrkit-proto-aero`.
fn message(m: &Message, link: Link) -> AeroMessage {
    let mut a = AeroMessage {
        aes: format!("{:06X}", link.aes),
        ges: format!("{:02X}", link.ges),
        downlink: m.downlink,
        mode: m.mode,
        registration: m.registration.clone(),
        flight: m.flight.clone(),
        label: m.label.clone(),
        label_name: m.label_meaning.unwrap_or("").into(),
        block_id: m.block_id,
        message_number: m.msn.clone(),
        text: m.text.clone(),
        check_ok: m.quality.bcs_ok,
        complete: !m.incomplete,
        icao: Some(link.aes),
        ..Default::default()
    };
    if let Reading::Adsc(x) = &m.reading {
        a.adsc = x
            .groups()
            .iter()
            .map(|g| tag_name(g.tag()))
            .collect::<Vec<_>>()
            .join(", ");
    }
    let (reading, kind, category) = reading_of(m);
    a.reading = reading;
    a.reading_kind = kind.into();
    a.reading_category = category.into();
    if let Reading::Cpdlc(c) = &m.reading {
        a.cpdlc = Some(CpdlcRow {
            imi: c.imi.as_str().into(),
            ground: c.ground.clone(),
            aircraft: c.aircraft.clone(),
            crc_ok: c.crc_ok,
            parsed: c.parsed(),
            text: c.text(),
        });
    }
    if let Some((p, alt)) = m.position() {
        a.lat = Some(p.lat_deg);
        a.lon = Some(p.lon_deg);
        a.alt_ft = alt;
    }
    a
}

/// Whether a 12-byte SU checks.
pub fn ok(s: &[u8]) -> bool {
    su_ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_proto_aero::acars::{BlockSpec, End};

    fn block(id: u8, text: &str, end: End) -> Vec<u8> {
        let mut out = Vec::new();
        BlockSpec::new('2', "HB-JHM", id, end)
            .label("H1")
            .text(text)
            .frame(0, false, &mut out)
            .unwrap();
        out
    }

    #[test]
    fn isu_ssu_reassembly_and_multi_block() {
        let blk = block(b'A', "PART ONE OF A LONG UPLINK ", End::Etb);
        let blk2 = block(b'B', "AND THE END", End::Etx);
        let mut r = Reassembler::default();
        let mut out = Vec::new();
        for (k, b) in [blk, blk2].iter().enumerate() {
            for su in build_user_data(0x4B1234, 0x90, 2, k as u8, b) {
                assert!(ok(&su));
                let (i, _) = read(&su);
                r.push(&i, 1.0 + k as f64, &mut out);
            }
        }
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "PART ONE OF A LONG UPLINK AND THE END");
        assert!(out[0].check_ok && out[0].complete);
        assert_eq!(out[0].registration, "HB-JHM");
        assert!(!out[0].downlink);
    }

    #[test]
    fn a_fixed_format_fills_the_reading() {
        let mut blk = Vec::new();
        BlockSpec::new('2', "HB-JHM", b'A', End::Etx)
            .label("QA")
            .text("OMDB1030")
            .frame(0, false, &mut blk)
            .unwrap();
        let mut r = Reassembler::default();
        let mut out = Vec::new();
        for su in build_user_data(0x4B1234, 0x90, 2, 0, &blk) {
            let (i, _) = read(&su);
            r.push(&i, 1.0, &mut out);
        }
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].reading, "from OMDB out 1030");
        assert_eq!(out[0].reading_kind, "OOOI report");
        assert_eq!(out[0].reading_category, "OOOI");
        assert!(out[0].cpdlc.is_none());
    }

    #[test]
    fn acars_end_characters_carry_odd_parity() {
        // ARINC 618: ETX and ETB are sent with odd parity, as 0x83 and 0x97, and the block check covers them as sent.
        assert_eq!((acars::ETX, acars::ETB), (0x83, 0x97));
        for (end, want) in [(End::Etx, 0x83u8), (End::Etb, 0x97)] {
            let b = block(b'A', "TEXT", end);
            let at = b.len() - 4;
            assert_eq!(b[at], want);
            let f = acars::split_frame(&b).unwrap();
            assert_eq!(acars::bcs(f.body), f.bcs);
        }
    }

    #[test]
    fn system_table_text() {
        let mut s = [0x0Cu8, 0, 0, 0x30, 0, 17, 0x80 | 0x0E, 0x2E, 0, 0, 0, 0];
        super::super::coding::su_seal(&mut s);
        let (i, t) = read(&s);
        assert!(matches!(i, SuInfo::Fact(_)));
        assert!(t.starts_with("satellite 3 at 25.5E, P channel"), "{t}");
        assert!(t.ends_with("(spot beam)"));
    }
}
