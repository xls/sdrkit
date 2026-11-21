//! Text codes for sdrkit: the character codes, alphabets and text layers that radio standards share.
//!
//! | Module | What |
//! |---|---|
//! | [`ita2`] | ITA2 / Baudot (ITU-T S.1) with the ITU and US figures cases, national positions, and the Cospas-Sarsat modified Baudot |
//! | [`ccir476`] | the CCIR 476 seven-unit code of SITOR and NAVTEX, and mode B (FEC-B) transmit and combining receive |
//! | [`varicode`] | PSK31, MFSK16/THOR (IZ8BLY), DominoEX, FSQ and IFKP varicodes |
//! | [`morse`] | International Morse code (ITU-R M.1677-1), a keyer, a decoder that follows the speed, an identification list |
//! | [`ia5`] | IA5 / ASCII with parity, printable filters, the ACARS characters |
//! | [`sixbit`] | AIS 6-bit ASCII and the ICAO aircraft identification set |
//! | [`base40`], [`base91`] | callsign packing (M17, SSDV, UAT), APRS base-91 |
//! | [`ebu`] | the RDS and DAB EBU Latin tables (they differ), and GB 2312 in RDS |
//! | [`charset_field`] | the 4-bit character set field of DAB and DRM text (ETSI TS 101 756 Table 1) |
//! | [`iso6937`], [`dvb`] | ISO/IEC 6937 and DVB SI text (EN 300 468 Annex A) |
//! | [`teletext`] | teletext packets, pages, character sets and rendering |
//! | [`callsign`], [`store`] | callsigns in free text, and the decoded-text store with chat lines |
//! | [`score`] | how much decoded text looks like plain text |

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod base40;
pub mod base91;
pub mod callsign;
pub mod ccir476;
pub mod charset_field;
pub mod compose;
pub mod csv;
pub mod dvb;
pub mod ebu;
mod error;
pub mod ia5;
pub mod iso6937;
pub mod ita2;
pub mod morse;
pub mod score;
pub mod sevenunit;
pub mod sixbit;
pub mod store;
pub mod teletext;
pub mod varicode;

pub use error::{Error, Result};
