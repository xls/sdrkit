//! The named CRCs: one `static` engine per catalogue name (for example [`CRC16_IBM_SDLC`] for "CRC-16/IBM-SDLC"), lookup at run
//! time with [`by_name`], [`entry`] and [`by_key`], and the lists [`ENTRIES`], [`ALL`] and [`specs`].
//!
//! Names and check values follow the CRC RevEng catalogue (Greg Cook, "Catalogue of parametrised CRC algorithms") where the CRC is
//! in it. The other entries are named after their standard. The check value is the CRC of the ASCII bytes `"123456789"`.

use super::{Crc, CrcSpec, Trailer};
use crate::Result;

macro_rules! catalog {
    ($( $(#[$doc:meta])* $id:ident : $w:ident = ($name:literal, $width:literal, $poly:literal, $init:literal, $refin:literal,
        $refout:literal, $xorout:literal, $check:literal); )*) => {
        /// The parameters of every catalogue CRC, by the same names as the engines.
        pub mod specs {
            use super::CrcSpec;
            $(
                $(#[$doc])*
                pub const $id: CrcSpec = CrcSpec::new($name, $width, $poly, $init, $refin, $refout, $xorout, $check);
            )*
        }

        $(
            $(#[$doc])*
            pub static $id: Crc<$w> = Crc::<$w>::new(specs::$id);
        )*

        /// Every spec in the catalogue, in the order above.
        pub const ALL: &[CrcSpec] = &[$(specs::$id),*];

        /// Every catalogue entry (key and ready engine), in the same order as [`ALL`].
        pub static ENTRIES: &[Entry] = &[$(Entry { key: key_str!($name), engine: engine_of!($w, $id) }),*];
    };
}

/// The [`Engine`] variant for a register type.
macro_rules! engine_of {
    (u8, $id:ident) => {
        Engine::U8(&$id)
    };
    (u16, $id:ident) => {
        Engine::U16(&$id)
    };
    (u32, $id:ident) => {
        Engine::U32(&$id)
    };
    (u64, $id:ident) => {
        Engine::U64(&$id)
    };
}

/// The key of a catalogue name as a `&'static str`, made at compile time.
macro_rules! key_str {
    ($name:literal) => {{
        const B: [u8; key_len($name)] = key_bytes($name);
        const S: &str = match core::str::from_utf8(&B) {
            Ok(s) => s,
            Err(_) => panic!("CRC key is not ASCII"),
        };
        S
    }};
}

/// Whether a name starts with `CRC-`.
const fn has_crc_dash(b: &[u8]) -> bool {
    b.len() >= 4 && b[0] == b'C' && b[1] == b'R' && b[2] == b'C' && b[3] == b'-'
}

/// The length of the key of `name` (see [`key`]).
const fn key_len(name: &str) -> usize {
    let b = name.as_bytes();
    if has_crc_dash(b) {
        b.len() - 1
    } else {
        b.len()
    }
}

/// The key of `name`, `N` bytes long (see [`key`]).
const fn key_bytes<const N: usize>(name: &str) -> [u8; N] {
    let b = name.as_bytes();
    let mut out = [0u8; N];
    let (mut i, mut o) = (0, 0);
    if has_crc_dash(b) {
        out[0] = b'c';
        out[1] = b'r';
        out[2] = b'c';
        i = 4;
        o = 3;
    }
    while i < b.len() {
        let c = b[i];
        out[o] = if c == b'/' || c == b'-' {
            b'_'
        } else {
            c.to_ascii_lowercase()
        };
        i += 1;
        o += 1;
    }
    out
}

/// A catalogue engine of any register width, for code that picks a CRC by name at run time. It refers to one of the catalogue's
/// `static` engines, so it is `Copy`, costs nothing to make and never allocates.
///
/// ```
/// use sdrkit_util_bits::crc::{catalog, Trailer};
/// let e = catalog::by_key("crc32_mpeg_2").unwrap().engine;
/// assert_eq!(e.checksum(b"123456789"), 0x0376_E6E7);
/// let mut f = b"abc".to_vec();
/// e.append(&mut f, Trailer::Big).unwrap();
/// assert!(e.verify(&f, Trailer::Big));
/// ```
#[derive(Clone, Copy, Debug)]
pub enum Engine {
    /// A CRC of up to 8 bits.
    U8(&'static Crc<u8>),
    /// A CRC of 9 to 16 bits.
    U16(&'static Crc<u16>),
    /// A CRC of 17 to 32 bits.
    U32(&'static Crc<u32>),
    /// A CRC of 33 to 64 bits.
    U64(&'static Crc<u64>),
}

/// Runs `$e` with `$c` bound to the engine of whatever width.
macro_rules! each {
    ($self:expr, $c:ident => $e:expr) => {
        match $self {
            Engine::U8($c) => $e,
            Engine::U16($c) => $e,
            Engine::U32($c) => $e,
            Engine::U64($c) => $e,
        }
    };
}

impl Engine {
    /// The spec.
    pub fn spec(self) -> &'static CrcSpec {
        each!(self, c => c.spec())
    }

    /// The CRC of `data` ([`Crc::checksum`]).
    #[inline]
    // The 64-bit engines need no cast; the others do.
    #[allow(clippy::unnecessary_cast)]
    pub fn checksum(self, data: &[u8]) -> u64 {
        each!(self, c => c.checksum(data) as u64)
    }

    /// The CRC of a bit string, one bit per byte ([`Crc::bits`]).
    #[allow(clippy::unnecessary_cast)]
    pub fn bits(self, bits: &[u8]) -> u64 {
        each!(self, c => c.bits(bits) as u64)
    }

    /// Whether the last `width / 8` bytes of `frame` are the CRC of the bytes before them ([`Crc::verify`]).
    #[inline]
    pub fn verify(self, frame: &[u8], at: Trailer) -> bool {
        each!(self, c => c.verify(frame, at))
    }

    /// Whether the last `width` bits of `bits` are the CRC of the bits before them ([`Crc::verify_bits`]).
    pub fn verify_bits(self, bits: &[u8]) -> bool {
        each!(self, c => c.verify_bits(bits))
    }

    /// Appends the CRC of `frame` to it ([`Crc::append`]).
    pub fn append(self, frame: &mut Vec<u8>, at: Trailer) -> Result<()> {
        each!(self, c => c.append(frame, at))
    }

    /// The trailer order that follows the CRC's own bit order: [`Trailer::Little`] for a reflected CRC (HDLC, 802.11, BLE),
    /// [`Trailer::Big`] otherwise.
    pub fn natural_trailer(self) -> Trailer {
        if self.spec().refout {
            Trailer::Little
        } else {
            Trailer::Big
        }
    }

    /// The 16-bit engine, when this is one.
    pub fn as_u16(self) -> Option<&'static Crc<u16>> {
        match self {
            Engine::U16(c) => Some(c),
            _ => None,
        }
    }
}

impl From<&'static Crc<u8>> for Engine {
    fn from(c: &'static Crc<u8>) -> Self {
        Engine::U8(c)
    }
}

impl From<&'static Crc<u16>> for Engine {
    fn from(c: &'static Crc<u16>) -> Self {
        Engine::U16(c)
    }
}

impl From<&'static Crc<u32>> for Engine {
    fn from(c: &'static Crc<u32>) -> Self {
        Engine::U32(c)
    }
}

impl From<&'static Crc<u64>> for Engine {
    fn from(c: &'static Crc<u64>) -> Self {
        Engine::U64(c)
    }
}

/// One catalogue CRC: its key and its ready engine.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// The stable lower-case key (see [`key`]): `"crc16_ibm_sdlc"` for "CRC-16/IBM-SDLC". Settings and saved files use it.
    pub key: &'static str,
    /// The engine.
    pub engine: Engine,
}

impl Entry {
    /// The spec.
    pub fn spec(&self) -> &'static CrcSpec {
        self.engine.spec()
    }

    /// The catalogue name, for example `"CRC-16/IBM-SDLC"`.
    pub fn name(&self) -> &'static str {
        self.engine.spec().name
    }
}

/// The catalogue spec named `name` (for example `"CRC-16/IBM-SDLC"`).
pub fn by_name(name: &str) -> Option<&'static CrcSpec> {
    entry(name).map(Entry::spec)
}

/// The catalogue entry named `name` (for example `"CRC-16/IBM-SDLC"`).
pub fn entry(name: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.name() == name)
}

/// The catalogue entry whose key is `key` (for example `"crc16_ibm_sdlc"`).
///
/// ```
/// use sdrkit_util_bits::crc::catalog;
/// let e = catalog::by_key("crc16_ibm_sdlc").unwrap();
/// assert_eq!(e.name(), "CRC-16/IBM-SDLC");
/// assert_eq!(e.engine.as_u16().unwrap().checksum(b"123456789"), 0x906E);
/// ```
pub fn by_key(key: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.key == key)
}

/// The stable key of the catalogue name `name`: the name in lower case, with `/` and `-` written `_` and a leading `CRC-` written
/// `crc` ("CRC-16/IBM-SDLC" is `crc16_ibm_sdlc`, "CRC-32/MPEG-2" is `crc32_mpeg_2`). `None` when the name is not in the
/// catalogue. A key never changes once published, since settings store it.
pub fn key(name: &str) -> Option<&'static str> {
    entry(name).map(|e| e.key)
}

/// The CRC of `data` computed by the catalogue's own engine for `spec` (matched by name), as a `u64`; `None` when the name is not
/// in the catalogue. For tools that try every CRC.
pub fn checksum_any(spec: &CrcSpec, data: &[u8]) -> Option<u64> {
    entry(spec.name).map(|e| e.engine.checksum(data))
}

catalog! {
    /// CRC-4/INTERLAKEN: x^4 + x + 1. IEEE 802.11-2020 clause 23 (S1G SIG field CRC, 802.11ah); Interlaken Protocol Definition.
    CRC4_INTERLAKEN: u8 = ("CRC-4/INTERLAKEN", 4, 0x3, 0xF, false, false, 0xF, 0xB);
    /// CRC-5/EPC-C1G2: x^5 + x^3 + 1, register 01001. EPCglobal Gen2 version 2.0.1, Table 6.12 and Annex F.1 (the Query CRC-5);
    /// residue 00000.
    CRC5_EPC_C1G2: u8 = ("CRC-5/EPC-C1G2", 5, 0x09, 0x09, false, false, 0x0, 0x00);
    /// CRC-6/NR: gCRC6(D) = D^6 + D^5 + 1. 3GPP TS 38.212 clause 5.1.
    CRC6_NR: u8 = ("CRC-6/NR", 6, 0x21, 0x0, false, false, 0x0, 0x15);
    /// CRC-6/CDR: x^6 + x + 1, register and result inverted. GY/T 268.1-2013 (China Digital Radio, system information).
    CRC6_CDR: u8 = ("CRC-6/CDR", 6, 0x03, 0x3F, false, false, 0x3F, 0x3E);
    /// CRC-6/NXDN: x^6 + x^5 + x^2 + x + 1, register all ones. NXDN Technical Specification Part 1-A (SACCH).
    CRC6_NXDN: u8 = ("CRC-6/NXDN", 6, 0x27, 0x3F, false, false, 0x0, 0x0D);
    /// CRC-7/MMC: x^7 + x^3 + 1. JEDEC MMC; ETSI TS 102 490 (dPMR header CRC-7).
    CRC7_MMC: u8 = ("CRC-7/MMC", 7, 0x09, 0x0, false, false, 0x0, 0x75);
    /// CRC-7/DMR: x^7 + x^5 + x^2 + x + 1, no inversion. ETSI TS 102 361-1 clause B.3.13 (reverse channel).
    CRC7_DMR: u8 = ("CRC-7/DMR", 7, 0x27, 0x0, false, false, 0x0, 0x7C);
    /// CRC-8/SMBUS: x^8 + x^2 + x + 1. SMBus 2.0 PEC; FLAC frame header; ETSI TS 102 361-1 clause B.3.7 (DMR short LC).
    CRC8_SMBUS: u8 = ("CRC-8/SMBUS", 8, 0x07, 0x0, false, false, 0x0, 0xF4);
    /// CRC-8/CDR: x^8 + x^2 + x + 1, register and result inverted. GY/T 268.1-2013 (China Digital Radio).
    CRC8_CDR: u8 = ("CRC-8/CDR", 8, 0x07, 0xFF, false, false, 0xFF, 0x04);
    /// CRC-8/ESB: x^8 + x^2 + x + 1, register all ones. Nordic nRF24L01+ product specification (Enhanced ShockBurst, 1-byte CRC).
    CRC8_ESB: u8 = ("CRC-8/ESB", 8, 0x07, 0xFF, false, false, 0x0, 0xFB);
    /// CRC-8/MAXIM-DOW: x^8 + x^5 + x^4 + 1, reflected. Maxim 1-Wire (application note 27).
    CRC8_MAXIM_DOW: u8 = ("CRC-8/MAXIM-DOW", 8, 0x31, 0x0, true, true, 0x0, 0xA1);
    /// CRC-8/NRSC-5: x^8 + x^5 + x^4 + 1, register all ones. NRSC-5 (HD Radio).
    CRC8_NRSC_5: u8 = ("CRC-8/NRSC-5", 8, 0x31, 0xFF, false, false, 0x0, 0xF7);
    /// CRC-8/SAE-J1850: x^8 + x^4 + x^3 + x^2 + 1, register and result inverted. ETSI ES 201 980 (DRM) 8-bit CRC.
    CRC8_SAE_J1850: u8 = ("CRC-8/SAE-J1850", 8, 0x1D, 0xFF, false, false, 0xFF, 0x4B);
    /// CRC-8/GSM-A: x^8 + x^4 + x^3 + x^2 + 1. The MPEG audio scale-factor CRC as DAB uses it (ETSI EN 300 401).
    CRC8_GSM_A: u8 = ("CRC-8/GSM-A", 8, 0x1D, 0x0, false, false, 0x0, 0x37);
    /// CRC-8/DVB-S2: x^8 + x^7 + x^6 + x^4 + x^2 + 1. ETSI EN 302 307-1 clause 5.1.4.
    CRC8_DVB_S2: u8 = ("CRC-8/DVB-S2", 8, 0xD5, 0x0, false, false, 0x0, 0xBC);
    /// CRC-8/LTE: gCRC8(D) = D^8 + D^7 + D^4 + D^3 + D + 1. 3GPP TS 36.212 clause 5.1.1.
    CRC8_LTE: u8 = ("CRC-8/LTE", 8, 0x9B, 0x0, false, false, 0x0, 0xEA);
    /// CRC-8/CDMA2000: x^8 + x^7 + x^4 + x^3 + x + 1, register all ones. 3GPP2 C.S0002; ETSI TS 103 357 (TS-UNB, mioty: the
    /// header and payload CRCs).
    CRC8_CDMA2000: u8 = ("CRC-8/CDMA2000", 8, 0x9B, 0xFF, false, false, 0x0, 0xDA);
    /// CRC-9/DMR: x^9 + x^6 + x^4 + x^3 + 1, result inverted. ETSI TS 102 361-1 clause B.3.10 (confirmed data blocks); P25
    /// confirmed data.
    CRC9_DMR: u16 = ("CRC-9/DMR", 9, 0x059, 0x0, false, false, 0x1FF, 0x0E3);
    /// CRC-10/GSM: D^10 + D^8 + D^6 + D^5 + D^4 + D^2 + 1, parity inverted. 3GPP TS 45.003 clause 4.7 (SCH).
    CRC10_GSM: u16 = ("CRC-10/GSM", 10, 0x175, 0x0, false, false, 0x3FF, 0x12A);
    /// CRC-11/NR: gCRC11(D) = D^11 + D^10 + D^9 + D^5 + 1. 3GPP TS 38.212 clause 5.1.
    CRC11_NR: u16 = ("CRC-11/NR", 11, 0x621, 0x0, false, false, 0x0, 0x5CA);
    /// CRC-12/DECT: x^12 + x^11 + x^3 + x^2 + x + 1. ETSI EN 300 175-3; Q65. The HD Radio PIDS CRC is a different CRC:
    /// [`crate::crc::cyclic::NRSC5_PIDS_CRC12`].
    CRC12_DECT: u16 = ("CRC-12/DECT", 12, 0x80F, 0x0, false, false, 0x0, 0xF5B);
    /// CRC-12/NXDN: the CRC-12/DECT polynomial with the register all ones. NXDN Technical Specification Part 1-A (FACCH1).
    CRC12_NXDN: u16 = ("CRC-12/NXDN", 12, 0x80F, 0xFFF, false, false, 0x0, 0x37C);
    /// CRC-15/NXDN: x^15 + x^14 + x^11 + x^10 + x^7 + x^6 + x^2 + 1, register all ones. NXDN Technical Specification Part 1-A
    /// (FACCH2, UDCH).
    CRC15_NXDN: u16 = ("CRC-15/NXDN", 15, 0x4CC5, 0x7FFF, false, false, 0x0, 0x53B1);
    /// CRC-16/IBM-3740 ("CRC-CCITT-FALSE"): x^16 + x^12 + x^5 + 1, register all ones. CCSDS CGMS; radiosondes; PACTOR; FLARM;
    /// ANT; ARINC 622; NXDN CAC.
    CRC16_IBM_3740: u16 = ("CRC-16/IBM-3740", 16, 0x1021, 0xFFFF, false, false, 0x0, 0x29B1);
    /// CRC-16/XMODEM: x^16 + x^12 + x^5 + 1. LoRa payload CRC; 3GPP TS 36.212 clause 5.1.1 gCRC16.
    CRC16_XMODEM: u16 = ("CRC-16/XMODEM", 16, 0x1021, 0x0, false, false, 0x0, 0x31C3);
    /// CRC-16/GENIBUS: x^16 + x^12 + x^5 + 1, register all ones, result inverted. ETSI EN 300 401 clause 5.2.1 (DAB FIB CRC);
    /// ETSI ES 201 980 (DRM); ETSI EN 300 392-2 (TETRA); 802.11b PLCP header; ADS-C uplink.
    CRC16_GENIBUS: u16 = ("CRC-16/GENIBUS", 16, 0x1021, 0xFFFF, false, false, 0xFFFF, 0xD64E);
    /// CRC-16/GSM: x^16 + x^12 + x^5 + 1, result inverted. ETSI TS 102 361-1 clause B.3.8 (DMR "CRC-CCITT"); P25; YSF;
    /// TETRAPOL; Sigfox.
    CRC16_GSM: u16 = ("CRC-16/GSM", 16, 0x1021, 0x0, false, false, 0xFFFF, 0xCE3C);
    /// CRC-16/IBM-SDLC ("X-25"): reflected x^16 + x^12 + x^5 + 1, all ones in and out. ISO/IEC 13239 (HDLC FCS); AX.25; AIS;
    /// VDL Mode 2; HFDL; PPP (RFC 1662); D-STAR; ISO/IEC 14443 CRC_B.
    CRC16_IBM_SDLC: u16 = ("CRC-16/IBM-SDLC", 16, 0x1021, 0xFFFF, true, true, 0xFFFF, 0x906E);
    /// CRC-16/KERMIT: reflected x^16 + x^12 + x^5 + 1. ARINC 618 (ACARS block check); IEEE 802.15.4 FCS.
    CRC16_KERMIT: u16 = ("CRC-16/KERMIT", 16, 0x1021, 0x0, true, true, 0x0, 0x2189);
    /// CRC-16/MCRF4XX: reflected x^16 + x^12 + x^5 + 1, register all ones. Microchip MCRF4xx; DJI links (some with another start
    /// value, made with [`CrcSpec::new`]).
    CRC16_MCRF4XX: u16 = ("CRC-16/MCRF4XX", 16, 0x1021, 0xFFFF, true, true, 0x0, 0x6F91);
    /// CRC-16/ISO-IEC-14443-3-A: reflected x^16 + x^12 + x^5 + 1, start 0xC6C6 (0x6363 in the reflected register). ISO/IEC
    /// 14443-3 annex B (CRC_A).
    CRC16_ISO_IEC_14443_3_A: u16 = ("CRC-16/ISO-IEC-14443-3-A", 16, 0x1021, 0xC6C6, true, true, 0x0, 0xBF05);
    /// CRC-16/SPI-FUJITSU ("AUG-CCITT"): x^16 + x^12 + x^5 + 1, start 0x1D0F. ITU-T G.9959 (Z-Wave R3).
    CRC16_SPI_FUJITSU: u16 = ("CRC-16/SPI-FUJITSU", 16, 0x1021, 0x1D0F, false, false, 0x0, 0xE5CC);
    /// CRC-16/SEMTECH-FSK: x^16 + x^12 + x^5 + 1, start 0x1D0F, result inverted. Semtech SX127x packet engine (CCITT mode);
    /// LoRaWAN FSK.
    CRC16_SEMTECH_FSK: u16 = ("CRC-16/SEMTECH-FSK", 16, 0x1021, 0x1D0F, false, false, 0xFFFF, 0x1A33);
    /// CRC-16/M17: x^16 + x^14 + x^12 + x^11 + x^8 + x^5 + x^4 + x^2 + 1, register all ones. M17 specification (CRC).
    CRC16_M17: u16 = ("CRC-16/M17", 16, 0x5935, 0xFFFF, false, false, 0x0, 0x772B);
    /// CRC-16/UMTS ("BUYPASS"): x^16 + x^15 + x^2 + 1. FLAC frame footer.
    CRC16_UMTS: u16 = ("CRC-16/UMTS", 16, 0x8005, 0x0, false, false, 0x0, 0xFEE8);
    /// CRC-16/CMS: x^16 + x^15 + x^2 + 1, register all ones. TI CC1101 hardware CRC; ISO/IEC 11172-3 clause 2.4.3.1 (MPEG audio
    /// header CRC).
    CRC16_CMS: u16 = ("CRC-16/CMS", 16, 0x8005, 0xFFFF, false, false, 0x0, 0xAEE7);
    /// CRC-16/ARC: reflected x^16 + x^15 + x^2 + 1.
    CRC16_ARC: u16 = ("CRC-16/ARC", 16, 0x8005, 0x0, true, true, 0x0, 0xBB3D);
    /// CRC-16/MODBUS: reflected x^16 + x^15 + x^2 + 1, register all ones. Modbus over serial line.
    CRC16_MODBUS: u16 = ("CRC-16/MODBUS", 16, 0x8005, 0xFFFF, true, true, 0x0, 0x4B37);
    /// CRC-16/DNP: reflected 0x3D65, result inverted. IEEE 1815 (DNP3).
    CRC16_DNP: u16 = ("CRC-16/DNP", 16, 0x3D65, 0x0, true, true, 0xFFFF, 0xEA82);
    /// CRC-16/EN-13757: 0x3D65, result inverted. EN 13757-4 (wireless M-Bus).
    CRC16_EN_13757: u16 = ("CRC-16/EN-13757", 16, 0x3D65, 0x0, false, false, 0xFFFF, 0xC2B7);
    /// CRC-16/DECT-R: x^16 + x^10 + x^8 + x^7 + x^3 + 1, last bit inverted. ETSI EN 300 175-3 clause 6.2.5 (the A-field R-CRC).
    CRC16_DECT_R: u16 = ("CRC-16/DECT-R", 16, 0x0589, 0x0, false, false, 0x0001, 0x007E);
    /// CRC-16/DECT-X: the DECT polynomial without inversion. ETSI EN 300 175-3 (the X-CRC).
    CRC16_DECT_X: u16 = ("CRC-16/DECT-X", 16, 0x0589, 0x0, false, false, 0x0, 0x007F);
    /// CRC-16/MAC-SI: x^16 + x^12 + x^5 + 1, register all ones, input reflected and output not. The D2-MAC service information CRC
    /// as decoded today; against ETSI EN 300 250.
    CRC16_MAC_SI: u16 = ("CRC-16/MAC-SI", 16, 0x1021, 0xFFFF, true, false, 0x0, 0x89F6);
    /// DAB+ fire code: G(x) = (x^11 + 1)(x^5 + x^3 + x^2 + x + 1). ETSI TS 102 563 clause 6 (superframe synchronisation).
    CRC16_DAB_FIRE: u16 = ("CRC-16/DAB-FIRE", 16, 0x782F, 0x0, false, false, 0x0, 0xF8FA);
    /// CRC-24/MODE-S: the Mode S parity, G(x) = 0x1FFF409. ICAO Annex 10 Volume IV clause 3.1.2.3.3.
    CRC24_MODE_S: u32 = ("CRC-24/MODE-S", 24, 0xFFF409, 0x0, false, false, 0x0, 0x054268);
    /// CRC-24/LTE-A ("CRC-24Q"): gCRC24A. 3GPP TS 36.212 clause 5.1.1; RTCM 10403 (RTCM 3); SBAS; Galileo.
    CRC24_LTE_A: u32 = ("CRC-24/LTE-A", 24, 0x864CFB, 0x0, false, false, 0x0, 0xCDE703);
    /// CRC-24/OPENPGP: 0x864CFB, register 0xB704CE. RFC 4880 clause 6.1 (the ASCII armor checksum).
    CRC24_OPENPGP: u32 = ("CRC-24/OPENPGP", 24, 0x864CFB, 0xB704CE, false, false, 0x0, 0x21CF02);
    /// CRC-24/LTE-B: gCRC24B. 3GPP TS 36.212 clause 5.1.1.
    CRC24_LTE_B: u32 = ("CRC-24/LTE-B", 24, 0x800063, 0x0, false, false, 0x0, 0x23EF52);
    /// CRC-24/NR-C: gCRC24C. 3GPP TS 38.212 clause 5.1.
    CRC24_NR_C: u32 = ("CRC-24/NR-C", 24, 0xB2B117, 0x0, false, false, 0x0, 0xF48279);
    /// CRC-24/BLE: x^24 + x^10 + x^9 + x^6 + x^4 + x^3 + x + 1, reflected, start 0x555555 (advertising). Bluetooth Core
    /// Specification Vol 6 Part B clause 3.1.1.
    CRC24_BLE: u32 = ("CRC-24/BLE", 24, 0x00065B, 0x555555, true, true, 0x0, 0xC25A56);
    /// CRC-30/CDMA: x^30 + x^29 + x^21 + x^20 + x^15 + x^13 + x^12 + x^11 + x^8 + x^7 + x^6 + x^2 + x + 1, register all ones,
    /// result inverted. 3GPP2 C.S0004 clause 2.1.1.5.1.2 (the cdma2000 Sync Channel capsule).
    CRC30_CDMA: u32 = ("CRC-30/CDMA", 30, 0x2030B9C7, 0x3FFFFFFF, false, false, 0x3FFFFFFF, 0x04C34ABF);
    /// CRC-32/ISO-HDLC: the Ethernet CRC. ISO/IEC 13239; IEEE 802.3 and 802.11 FCS; PNG (ISO/IEC 15948); gzip (RFC 1952).
    CRC32_ISO_HDLC: u32 = ("CRC-32/ISO-HDLC", 32, 0x04C11DB7, 0xFFFFFFFF, true, true, 0xFFFFFFFF, 0xCBF43926);
    /// CRC-32/MPEG-2. ISO/IEC 13818-1 annex A (PSI sections); ETSI TS 102 606 (GSE); DMB.
    CRC32_MPEG2: u32 = ("CRC-32/MPEG-2", 32, 0x04C11DB7, 0xFFFFFFFF, false, false, 0x0, 0x0376E6E7);
    /// CRC-32/BZIP2. GY/T 268.1-2013 (China Digital Radio).
    CRC32_BZIP2: u32 = ("CRC-32/BZIP2", 32, 0x04C11DB7, 0xFFFFFFFF, false, false, 0xFFFFFFFF, 0xFC891918);
    /// CRC-32/CKSUM: register 0, result inverted. TIA-102.BAAA (P25 packet data CRC-32).
    CRC32_CKSUM: u32 = ("CRC-32/CKSUM", 32, 0x04C11DB7, 0x0, false, false, 0xFFFFFFFF, 0x765E7680);
    /// CRC-32/DMR: register 0, no inversion. ETSI TS 102 361-1 clause B.3.9 (packet data message CRC). The standard takes the
    /// octets in pairs, second octet first, and sends the CRC least significant octet first; [`catalog::crc32_dmr_packet`](crate::crc::catalog::crc32_dmr_packet) does both.
    CRC32_DMR: u32 = ("CRC-32/DMR", 32, 0x04C11DB7, 0x0, false, false, 0x0, 0x89A1897F);
    /// CRC-32/ISCSI ("CRC-32C"). RFC 3720 annex B.4; CubeSat Space Protocol.
    CRC32_ISCSI: u32 = ("CRC-32/ISCSI", 32, 0x1EDC6F41, 0xFFFFFFFF, true, true, 0xFFFFFFFF, 0xE3069283);
    /// CRC-40/GSM: the GSM fire code (D^23 + 1)(D^17 + D^3 + 1), parity inverted. 3GPP TS 45.003 clause 4.1.2.
    CRC40_GSM: u64 = ("CRC-40/GSM", 40, 0x0004820009, 0x0, false, false, 0xFFFFFFFFFF, 0xD4164FC646);
}

/// The DMR packet data message CRC (ETSI TS 102 361-1 clause B.3.9) of `data` (user data and pad octets): the octets are taken in
/// 16-bit words, least significant octet first, and the CRC is [`CRC32_DMR`]. The result is sent least significant octet first.
/// An odd final octet is taken as it is.
pub fn crc32_dmr_packet(data: &[u8]) -> u32 {
    let mut d = CRC32_DMR.digest();
    for pair in data.chunks(2) {
        match *pair {
            [a, b] => d.update(&[b, a]),
            [a] => d.update(&[a]),
            _ => {}
        }
    }
    d.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalogue_crc_has_its_check_value() {
        for s in ALL {
            assert_eq!(s.compute(b"123456789"), s.check, "{} (bit-serial)", s.name);
            assert_eq!(
                checksum_any(s, b"123456789"),
                Some(s.check),
                "{} (table)",
                s.name
            );
        }
        assert_eq!(ENTRIES.len(), ALL.len());
        for (e, s) in ENTRIES.iter().zip(ALL) {
            assert_eq!(e.spec(), s);
            assert_eq!(
                e.engine.checksum(b"123456789"),
                s.check,
                "{} (entry)",
                s.name
            );
            assert_eq!(
                e.engine.as_u16().is_some(),
                (9..=16).contains(&s.width),
                "{}",
                s.name
            );
        }
        let mut names: Vec<_> = ALL.iter().map(|s| s.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ALL.len(), "duplicate names");
        assert!(by_name("CRC-16/IBM-SDLC").is_some());
        assert!(by_name("CRC-16/CCITT").is_none());
    }

    #[test]
    fn keys_are_stable_snake_case_and_unique() {
        // Published keys: these never change, since settings store them.
        assert_eq!(key("CRC-16/IBM-SDLC"), Some("crc16_ibm_sdlc"));
        assert_eq!(key("CRC-32/ISO-HDLC"), Some("crc32_iso_hdlc"));
        assert_eq!(key("CRC-32/MPEG-2"), Some("crc32_mpeg_2"));
        assert_eq!(key("CRC-16/DAB-FIRE"), Some("crc16_dab_fire"));
        assert_eq!(
            key("CRC-16/ISO-IEC-14443-3-A"),
            Some("crc16_iso_iec_14443_3_a")
        );
        assert_eq!(key("CRC-16/CCITT"), None);
        let mut seen = std::collections::BTreeSet::new();
        for e in ENTRIES {
            assert!(
                e.key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{}",
                e.key
            );
            assert!(seen.insert(e.key), "{} twice", e.key);
            assert_eq!(by_key(e.key).map(Entry::name), Some(e.name()));
        }
        assert!(by_key("CRC16_IBM_SDLC").is_none());
    }

    #[test]
    fn mode_s_frames_have_zero_parity() {
        // DF17 extended squitters widely published as examples (all 112 bits, parity included, divide to zero).
        for hex in [
            "8D4840D6202CC371C32CE0576098",
            "8D40621D58C382D690C8AC2863A7",
        ] {
            let b: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(CRC24_MODE_S.checksum(&b), 0, "{hex}");
            assert!(CRC24_MODE_S.verify(&b, crate::crc::Trailer::Big));
        }
    }

    #[test]
    fn dect_r_crc_is_the_x_crc_with_the_last_bit_inverted() {
        for msg in [&b"123456789"[..], b"\x00\x11\x22\x33\x44\x55"] {
            assert_eq!(CRC16_DECT_R.checksum(msg), CRC16_DECT_X.checksum(msg) ^ 1);
        }
    }

    #[test]
    fn dmr_packet_crc_swaps_octet_pairs() {
        // "123456789" swapped in pairs is "214365879"; the CRC is CRC-32/DMR over that.
        assert_eq!(
            crc32_dmr_packet(b"123456789"),
            CRC32_DMR.checksum(b"214365879")
        );
        assert_eq!(
            crc32_dmr_packet(b"21436587"),
            CRC32_DMR.checksum(b"12345678")
        );
    }

    #[test]
    fn hdlc_fcs_of_a_frame_leaves_the_good_residue() {
        // RFC 1662 appendix C: the FCS of a frame plus its own FCS (low octet first) leaves 0xF0B8.
        let mut f = b"\xFF\x03\xC0\x21hello".to_vec();
        CRC16_IBM_SDLC
            .append(&mut f, crate::crc::Trailer::Little)
            .unwrap();
        let mut d = CRC16_IBM_SDLC.digest();
        d.update(&f);
        assert_eq!(d.register(), 0xF0B8);
    }
}
