//! Identity tables for sdrkit: who an address, code or number belongs to, and the tone tables of analogue signalling.
//!
//! Every table entry comes from a published public source named in the module that holds it. Lookups return `&'static str`.
//!
//! | Module | Holds | Source |
//! |---|---|---|
//! | [`icao`] | ICAO 24-bit address to State; US and Canadian registrations from the address | ICAO Annex 10 Vol. III Table 9-1; FAA and Transport Canada registers |
//! | [`mid`] | maritime identification digits to country | ITU MID table (RR Appendix 43) |
//! | [`mmsi`] | MMSI forms: ship, coast, group, SAR aircraft, AtoN, SART, MOB, EPIRB, handheld, parent-ship craft, AMRD | ITU-R M.585-9 |
//! | [`mcc`] | MCC to country, MCC/MNC to network | ITU-T E.212 lists (ITU OB 1117 and 1162) |
//! | [`oui`] | IEEE OUI to organisation (subset) | IEEE Registration Authority MA-L listing |
//! | [`ant`] | ANT+ and FIT manufacturer identifiers | Garmin FIT SDK profile, type `manufacturer` |
//! | [`ic_maker`] | IC manufacturer codes of NFC and RFID UIDs (subset) | ISO/IEC 7816-6 Amd 1 table 9 |
//! | [`bluetooth`] | company identifiers, 16-bit UUIDs | Bluetooth SIG Assigned Numbers |
//! | [`lorawan`] | NetID and DevAddr layout, NetID to operator | LoRaWAN TS002; LoRa Alliance NetID Allocations R119 |
//! | [`pty`] | programme types of RDS, RBDS, DAB and DRM | IEC 62106 / NRSC-4; ETSI TS 101 756; ETSI ES 201 980 |
//! | [`content_type`] | RadioText Plus and DL Plus content types | ETSI TS 102 980 Annex A |
//! | [`announcement`] | DAB announcement types | ETSI TS 101 756 Tables 14 and 15 |
//! | [`language`] | ISO 639-2; DAB/RDS and DRM language codes | Library of Congress ISO 639-2 list; TS 101 756; ES 201 980 |
//! | [`tones`] | CTCSS, DCS, DTMF, selcall tone sets, SELCAL | TIA-603 / EIA RS-220-A; ITU-T Q.23; manufacturer charts; ASRI |
//! | [`names`] | id-to-name lists imported from CSV or TSV, with a report of refused rows | — |
//! | [`lists`] | watch and ignore lists | — |
//! | [`table`] | separator detection and RFC 4180 records | RFC 4180 |
//!

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod announcement;
pub mod ant;
pub mod bluetooth;
pub mod content_type;
pub mod ic_maker;
pub mod icao;
pub mod language;
pub mod lists;
pub mod lorawan;
pub mod mcc;
pub mod mid;
pub mod mmsi;
pub mod names;
pub mod oui;
mod pool;
pub mod pty;
pub mod table;
pub mod tones;
