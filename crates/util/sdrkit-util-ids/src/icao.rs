//! ICAO 24-bit aircraft addresses: the State each address is allocated to, and the registrations that are a published function
//! of the address.
//!
//! **Allocations.** ICAO Annex 10, Volume III, Part I, Chapter 9, Appendix to Chapter 9, Table 9-1 ("Allocation of 24-bit
//! aircraft addresses to States") allocates each State a run of addresses that starts with a fixed bit pattern; the run is
//! 2<sup>n</sup> addresses long and starts at a multiple of its length. The State names here are short English names.
//! New Zealand's run is confirmed by CAA of New Zealand AC 91-2 Revision 2 (26 October 2022): "national code 1100 1000 0",
//! C80000 to C87FFF.
//!
//! **Registrations from the address.** Two States assign addresses by a fixed rule, and their public registers show it:
//!
//! - United States: the FAA assigns N-numbers in order, N1 = A00001 up to N99999 = ADF7C7 (the FAA Aircraft Registry,
//!   `MASTER.txt` of the releasable aircraft database, column "MODE S CODE HEX").
//! - Canada: C-FAAA = C00001 onwards, then C-GAAA = C044A9 onwards, the last three letters counted in base 26 (Transport Canada,
//!   Canadian Civil Aircraft Register database `carscurr.txt`, field "aircraft address").
//!
//! Other States assign addresses from their register in their own order (New Zealand by the register's record number), so no
//! registration is derived for them.
//!
//! ```
//! use sdrkit_util_ids::icao;
//!
//! assert_eq!(icao::country(0x4CA123), Some("Ireland"));
//! assert_eq!(icao::registration(0xA835AF).unwrap().as_str(), "N628TS");
//! assert_eq!(icao::address("C-GABC"), Some(0xC044C5));
//! ```

use std::fmt;

/// One State's run of addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allocation {
    /// The first address.
    pub first: u32,
    /// The run is 2<sup>`size_log2`</sup> addresses long.
    pub size_log2: u8,
    /// The State (or ICAO itself for its own runs).
    pub country: &'static str,
}

impl Allocation {
    /// The last address of the run.
    pub const fn last(&self) -> u32 {
        self.first + (1 << self.size_log2) - 1
    }

    /// Whether `addr` is in the run.
    pub const fn contains(&self, addr: u32) -> bool {
        addr >= self.first && addr <= self.last()
    }
}

const fn a(first: u32, size_log2: u8, country: &'static str) -> Allocation {
    Allocation {
        first,
        size_log2,
        country,
    }
}

/// Every allocation of ICAO Annex 10 Vol. III Table 9-1, in address order.
pub static ALLOCATIONS: [Allocation; 190] = [
    a(0x004000, 10, "Zimbabwe"),
    a(0x006000, 12, "Mozambique"),
    a(0x008000, 15, "South Africa"),
    a(0x010000, 15, "Egypt"),
    a(0x018000, 15, "Libya"),
    a(0x020000, 15, "Morocco"),
    a(0x028000, 15, "Tunisia"),
    a(0x030000, 10, "Botswana"),
    a(0x032000, 12, "Burundi"),
    a(0x034000, 12, "Cameroon"),
    a(0x035000, 10, "Comoros"),
    a(0x036000, 12, "Congo"),
    a(0x038000, 12, "Cote d'Ivoire"),
    a(0x03E000, 12, "Gabon"),
    a(0x040000, 12, "Ethiopia"),
    a(0x042000, 12, "Equatorial Guinea"),
    a(0x044000, 12, "Ghana"),
    a(0x046000, 12, "Guinea"),
    a(0x048000, 10, "Guinea-Bissau"),
    a(0x04A000, 10, "Lesotho"),
    a(0x04C000, 12, "Kenya"),
    a(0x050000, 12, "Liberia"),
    a(0x054000, 12, "Madagascar"),
    a(0x058000, 12, "Malawi"),
    a(0x05A000, 10, "Maldives"),
    a(0x05C000, 12, "Mali"),
    a(0x05E000, 10, "Mauritania"),
    a(0x060000, 10, "Mauritius"),
    a(0x062000, 12, "Niger"),
    a(0x064000, 12, "Nigeria"),
    a(0x068000, 12, "Uganda"),
    a(0x06A000, 10, "Qatar"),
    a(0x06C000, 12, "Central African Republic"),
    a(0x06E000, 12, "Rwanda"),
    a(0x070000, 12, "Senegal"),
    a(0x074000, 10, "Seychelles"),
    a(0x076000, 10, "Sierra Leone"),
    a(0x078000, 12, "Somalia"),
    a(0x07A000, 10, "Eswatini"),
    a(0x07C000, 12, "Sudan"),
    a(0x080000, 12, "Tanzania"),
    a(0x084000, 12, "Chad"),
    a(0x088000, 12, "Togo"),
    a(0x08A000, 12, "Zambia"),
    a(0x08C000, 12, "DR Congo"),
    a(0x090000, 12, "Angola"),
    a(0x094000, 10, "Benin"),
    a(0x096000, 10, "Cabo Verde"),
    a(0x098000, 10, "Djibouti"),
    a(0x09A000, 12, "Gambia"),
    a(0x09C000, 12, "Burkina Faso"),
    a(0x09E000, 10, "Sao Tome and Principe"),
    a(0x0A0000, 15, "Algeria"),
    a(0x0A8000, 12, "Bahamas"),
    a(0x0AA000, 10, "Barbados"),
    a(0x0AB000, 10, "Belize"),
    a(0x0AC000, 12, "Colombia"),
    a(0x0AE000, 12, "Costa Rica"),
    a(0x0B0000, 12, "Cuba"),
    a(0x0B2000, 12, "El Salvador"),
    a(0x0B4000, 12, "Guatemala"),
    a(0x0B6000, 12, "Guyana"),
    a(0x0B8000, 12, "Haiti"),
    a(0x0BA000, 12, "Honduras"),
    a(0x0BC000, 10, "Saint Vincent and the Grenadines"),
    a(0x0BE000, 12, "Jamaica"),
    a(0x0C0000, 12, "Nicaragua"),
    a(0x0C2000, 12, "Panama"),
    a(0x0C4000, 12, "Dominican Republic"),
    a(0x0C6000, 12, "Trinidad and Tobago"),
    a(0x0C8000, 12, "Suriname"),
    a(0x0CA000, 10, "Antigua and Barbuda"),
    a(0x0CC000, 10, "Grenada"),
    a(0x0D0000, 15, "Mexico"),
    a(0x0D8000, 15, "Venezuela"),
    a(0x100000, 20, "Russia"),
    a(0x201000, 10, "Namibia"),
    a(0x202000, 10, "Eritrea"),
    a(0x300000, 18, "Italy"),
    a(0x340000, 18, "Spain"),
    a(0x380000, 18, "France"),
    a(0x3C0000, 18, "Germany"),
    a(0x400000, 18, "United Kingdom"),
    a(0x440000, 15, "Austria"),
    a(0x448000, 15, "Belgium"),
    a(0x450000, 15, "Bulgaria"),
    a(0x458000, 15, "Denmark"),
    a(0x460000, 15, "Finland"),
    a(0x468000, 15, "Greece"),
    a(0x470000, 15, "Hungary"),
    a(0x478000, 15, "Norway"),
    a(0x480000, 15, "Netherlands"),
    a(0x488000, 15, "Poland"),
    a(0x490000, 15, "Portugal"),
    a(0x498000, 15, "Czechia"),
    a(0x4A0000, 15, "Romania"),
    a(0x4A8000, 15, "Sweden"),
    a(0x4B0000, 15, "Switzerland"),
    a(0x4B8000, 15, "Turkey"),
    a(0x4C0000, 15, "Serbia"),
    a(0x4C8000, 10, "Cyprus"),
    a(0x4CA000, 12, "Ireland"),
    a(0x4CC000, 12, "Iceland"),
    a(0x4D0000, 10, "Luxembourg"),
    a(0x4D2000, 10, "Malta"),
    a(0x4D4000, 10, "Monaco"),
    a(0x500000, 10, "San Marino"),
    a(0x501000, 10, "Albania"),
    a(0x501C00, 10, "Croatia"),
    a(0x502C00, 10, "Latvia"),
    a(0x503C00, 10, "Lithuania"),
    a(0x504C00, 10, "Moldova"),
    a(0x505C00, 10, "Slovakia"),
    a(0x506C00, 10, "Slovenia"),
    a(0x507C00, 10, "Uzbekistan"),
    a(0x508000, 15, "Ukraine"),
    a(0x510000, 10, "Belarus"),
    a(0x511000, 10, "Estonia"),
    a(0x512000, 10, "North Macedonia"),
    a(0x513000, 10, "Bosnia and Herzegovina"),
    a(0x514000, 10, "Georgia"),
    a(0x515000, 10, "Tajikistan"),
    a(0x516000, 10, "Montenegro"),
    a(0x600000, 10, "Armenia"),
    a(0x600800, 10, "Azerbaijan"),
    a(0x601000, 10, "Kyrgyzstan"),
    a(0x601800, 10, "Turkmenistan"),
    a(0x680000, 10, "Bhutan"),
    a(0x681000, 10, "Micronesia"),
    a(0x682000, 10, "Mongolia"),
    a(0x683000, 10, "Kazakhstan"),
    a(0x684000, 10, "Palau"),
    a(0x700000, 12, "Afghanistan"),
    a(0x702000, 12, "Bangladesh"),
    a(0x704000, 12, "Myanmar"),
    a(0x706000, 12, "Kuwait"),
    a(0x708000, 12, "Laos"),
    a(0x70A000, 12, "Nepal"),
    a(0x70C000, 10, "Oman"),
    a(0x70E000, 12, "Cambodia"),
    a(0x710000, 15, "Saudi Arabia"),
    a(0x718000, 15, "South Korea"),
    a(0x720000, 15, "North Korea"),
    a(0x728000, 15, "Iraq"),
    a(0x730000, 15, "Iran"),
    a(0x738000, 15, "Israel"),
    a(0x740000, 15, "Jordan"),
    a(0x748000, 15, "Lebanon"),
    a(0x750000, 15, "Malaysia"),
    a(0x758000, 15, "Philippines"),
    a(0x760000, 15, "Pakistan"),
    a(0x768000, 15, "Singapore"),
    a(0x770000, 15, "Sri Lanka"),
    a(0x778000, 15, "Syria"),
    a(0x780000, 18, "China"),
    a(0x7C0000, 18, "Australia"),
    a(0x800000, 18, "India"),
    a(0x840000, 18, "Japan"),
    a(0x880000, 15, "Thailand"),
    a(0x888000, 15, "Viet Nam"),
    a(0x890000, 12, "Yemen"),
    a(0x894000, 12, "Bahrain"),
    a(0x895000, 10, "Brunei"),
    a(0x896000, 12, "United Arab Emirates"),
    a(0x897000, 10, "Solomon Islands"),
    a(0x898000, 12, "Papua New Guinea"),
    a(0x899000, 10, "Taiwan"),
    a(0x8A0000, 15, "Indonesia"),
    a(0x900000, 10, "Marshall Islands"),
    a(0x901000, 10, "Cook Islands"),
    a(0x902000, 10, "Samoa"),
    a(0xA00000, 20, "United States"),
    a(0xC00000, 18, "Canada"),
    a(0xC80000, 15, "New Zealand"),
    a(0xC88000, 12, "Fiji"),
    a(0xC8A000, 10, "Nauru"),
    a(0xC8C000, 10, "Saint Lucia"),
    a(0xC8D000, 10, "Tonga"),
    a(0xC8E000, 10, "Kiribati"),
    a(0xC90000, 10, "Vanuatu"),
    a(0xE00000, 18, "Argentina"),
    a(0xE40000, 18, "Brazil"),
    a(0xE80000, 12, "Chile"),
    a(0xE84000, 12, "Ecuador"),
    a(0xE88000, 12, "Paraguay"),
    a(0xE8C000, 12, "Peru"),
    a(0xE90000, 12, "Uruguay"),
    a(0xE94000, 12, "Bolivia"),
    a(0xF00000, 15, "ICAO (temporary addresses)"),
    a(0xF09000, 10, "ICAO (special use)"),
];

/// The allocation holding `addr`, or `None` for an unallocated address or a value above 24 bits.
pub fn allocation(addr: u32) -> Option<&'static Allocation> {
    if addr > 0xFF_FFFF {
        return None;
    }
    let i = ALLOCATIONS.partition_point(|x| x.first <= addr);
    let al = ALLOCATIONS.get(i.checked_sub(1)?)?;
    al.contains(addr).then_some(al)
}

/// The State an address is allocated to.
pub fn country(addr: u32) -> Option<&'static str> {
    allocation(addr).map(|a| a.country)
}

/// A registration mark of at most eight ASCII characters, held without allocation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Registration {
    bytes: [u8; 8],
    len: u8,
}

impl Registration {
    fn new() -> Registration {
        Registration {
            bytes: [0; 8],
            len: 0,
        }
    }

    fn push(&mut self, b: u8) {
        if (self.len as usize) < self.bytes.len() {
            self.bytes[self.len as usize] = b;
            self.len += 1;
        }
    }

    fn push_number(&mut self, n: u32) {
        let mut digits = [0u8; 10];
        let mut k = 0;
        let mut v = n;
        loop {
            digits[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        for i in (0..k).rev() {
            self.push(digits[i]);
        }
    }

    /// The mark as text.
    pub fn as_str(&self) -> &str {
        // Only ASCII is ever pushed.
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

impl fmt::Display for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Registration({:?})", self.as_str())
    }
}

/// The letters of US N-numbers: A to Z without I and O (14 CFR 47.15).
const N_LETTERS: &[u8; 24] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
/// The first US address (N1).
const N_FIRST: u32 = 0xA00001;
/// The last US address with an N-number (N99999).
const N_LAST: u32 = 0xADF7C7;
/// Addresses taken by a letter suffix at one level: none, one letter (24) or two letters (24 x 24), 601 in all.
const N_SUFFIXES: u32 = 601;
/// Addresses below a fourth digit: none, one letter (24) or one more digit (10).
const N_SPAN4: u32 = 35;
const N_SPAN3: u32 = 10 * N_SPAN4 + N_SUFFIXES;
const N_SPAN2: u32 = 10 * N_SPAN3 + N_SUFFIXES;
const N_SPAN1: u32 = 10 * N_SPAN2 + N_SUFFIXES;

fn push_suffix(r: &mut Registration, off: u32) {
    // off is 1 to 600: one letter, or two letters.
    let first = (off - 1) / 25;
    let second = (off - 1) % 25;
    r.push(N_LETTERS[first as usize]);
    if second > 0 {
        r.push(N_LETTERS[second as usize - 1]);
    }
}

/// The US N-number of an address (A00001 = N1 to ADF7C7 = N99999).
pub fn n_number(addr: u32) -> Option<Registration> {
    if !(N_FIRST..=N_LAST).contains(&addr) {
        return None;
    }
    let mut r = Registration::new();
    r.push(b'N');
    let mut off = addr - N_FIRST;
    r.push_number(off / N_SPAN1 + 1);
    off %= N_SPAN1;
    for span in [N_SPAN2, N_SPAN3] {
        if off < N_SUFFIXES {
            if off > 0 {
                push_suffix(&mut r, off);
            }
            return Some(r);
        }
        off -= N_SUFFIXES;
        r.push_number(off / span);
        off %= span;
    }
    if off < N_SUFFIXES {
        if off > 0 {
            push_suffix(&mut r, off);
        }
        return Some(r);
    }
    off -= N_SUFFIXES;
    r.push_number(off / N_SPAN4);
    off %= N_SPAN4;
    match off {
        0 => {}
        1..=24 => r.push(N_LETTERS[off as usize - 1]),
        _ => r.push_number(off - 25),
    }
    Some(r)
}

/// The Canadian mark of an address (C00001 = C-FAAA to C044A8 = C-FZZZ, C044A9 = C-GAAA to C08950 = C-GZZZ).
pub fn canada_mark(addr: u32) -> Option<Registration> {
    const FIRST: u32 = 0xC00001;
    const PER: u32 = 26 * 26 * 26;
    if !(FIRST..FIRST + 2 * PER).contains(&addr) {
        return None;
    }
    let off = addr - FIRST;
    let mut r = Registration::new();
    for b in [b'C', b'-', if off < PER { b'F' } else { b'G' }] {
        r.push(b);
    }
    let n = off % PER;
    for v in [n / 676, n / 26 % 26, n % 26] {
        r.push(b'A' + v as u8);
    }
    Some(r)
}

/// The registration that is a published function of the address (United States and Canada), or `None`.
pub fn registration(addr: u32) -> Option<Registration> {
    n_number(addr).or_else(|| canada_mark(addr))
}

/// The address of a US N-number or a Canadian C-F/C-G mark (either case, with or without the hyphen), or `None` for any
/// other mark or a mark that cannot exist (an N-number with I or O, more than five characters after the N, a letter before a
/// digit, two letters after four digits).
pub fn address(registration: &str) -> Option<u32> {
    let s = registration.trim().as_bytes();
    if s.len() < 2 || s.len() > 7 {
        return None;
    }
    let up = |b: u8| b.to_ascii_uppercase();
    match up(s[0]) {
        b'N' => n_address(&s[1..]),
        b'C' => {
            let rest = if s.get(1) == Some(&b'-') {
                &s[2..]
            } else {
                &s[1..]
            };
            if rest.len() != 4 || !rest.iter().all(u8::is_ascii_alphabetic) {
                return None;
            }
            let base = match up(rest[0]) {
                b'F' => 0xC00001,
                b'G' => 0xC00001 + 26 * 26 * 26,
                _ => return None,
            };
            let v = |b: u8| (up(b) - b'A') as u32;
            Some(base + v(rest[1]) * 676 + v(rest[2]) * 26 + v(rest[3]))
        }
        _ => None,
    }
}

fn n_letter(b: u8) -> Option<u32> {
    N_LETTERS
        .iter()
        .position(|&l| l == b.to_ascii_uppercase())
        .map(|i| i as u32)
}

fn n_address(t: &[u8]) -> Option<u32> {
    if t.is_empty() || t.len() > 5 || !(b'1'..=b'9').contains(&t[0]) {
        return None;
    }
    let mut addr = N_FIRST + (t[0] - b'1') as u32 * N_SPAN1;
    let mut i = 1;
    for span in [N_SPAN2, N_SPAN3, N_SPAN4] {
        match t.get(i) {
            None => return Some(addr),
            Some(b) if b.is_ascii_digit() => {
                addr += N_SUFFIXES + (b - b'0') as u32 * span;
                i += 1;
            }
            Some(_) => {
                let l1 = n_letter(t[i])?;
                let off = match t.get(i + 1) {
                    None => 1 + l1 * 25,
                    Some(&b2) if i + 2 == t.len() => 1 + l1 * 25 + n_letter(b2)? + 1,
                    Some(_) => return None,
                };
                return Some(addr + off);
            }
        }
    }
    // After four digits: nothing, one letter or a fifth digit.
    match t.get(i) {
        None => Some(addr),
        Some(_) if i + 1 != t.len() => None,
        Some(b) if b.is_ascii_digit() => Some(addr + 25 + (b - b'0') as u32),
        Some(&b) => Some(addr + 1 + n_letter(b)?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocations_are_aligned_disjoint_and_sorted() {
        for al in ALLOCATIONS.iter() {
            assert_eq!(
                al.first % (1 << al.size_log2),
                0,
                "{} not aligned",
                al.country
            );
        }
        for w in ALLOCATIONS.windows(2) {
            assert!(
                w[0].last() < w[1].first,
                "{} overlaps {}",
                w[0].country,
                w[1].country
            );
        }
    }

    #[test]
    fn countries() {
        assert_eq!(country(0x4CA123), Some("Ireland"));
        assert_eq!(country(0xA12345), Some("United States"));
        assert_eq!(
            country(0xC80E65),
            Some("New Zealand"),
            "the example of CAA NZ AC 91-2"
        );
        assert_eq!(country(0xC87FFF), Some("New Zealand"));
        assert_eq!(country(0xC88000), Some("Fiji"));
        assert_eq!(country(0x0ACFFF), Some("Colombia"));
        assert_eq!(country(0x0AD000), None, "Colombia has 4096 addresses");
        assert_eq!(country(0x000000), None);
        assert_eq!(country(0xFFFFFF), None);
        assert_eq!(country(0x1_000_000), None);
        assert_eq!(allocation(0x3C0000).unwrap().last(), 0x3FFFFF);
    }

    #[test]
    fn registrations_from_published_registers() {
        // FAA Aircraft Registry, MODE S CODE HEX.
        for (addr, reg) in [
            (0xA00002, "N1A"),
            (0xA004B3, "N100"),
            (0xA061D9, "N12345"),
            (0xA835AF, "N628TS"),
            (0xA12239, "N172SP"),
            (0xAA0CA5, "N747BA"),
            (0xAC3F08, "N8888L"),
            (0xA6AE13, "N53"),
            (0xAC9C85, "N911XX"),
            (0xA58BD4, "N45678"),
            (0xADD2A1, "N99ZZ"),
            (0xA6377D, "N5Z"),
            (0xA00A76, "N101WY"),
            (0xA065A8, "N1245U"),
            (0xA1E472, "N2208A"),
            (0xA20B38, "N23021"),
            (0xA43222, "N37HA"),
            (0xA6D0EB, "N5383E"),
            (0xA9A937, "N721W"),
            (0xAA0B65, "N7461X"),
            (0xAA97F2, "N78199"),
            (0xAC6BC1, "N9PB"),
            (0xAD915C, "N9731Y"),
        ] {
            assert_eq!(registration(addr).unwrap().as_str(), reg, "{addr:06X}");
            assert_eq!(address(reg), Some(addr), "{reg}");
        }
        // Transport Canada, Canadian Civil Aircraft Register, aircraft address.
        for (addr, reg) in [
            (0xC00001, "C-FAAA"),
            (0xC010F6, "C-FGKZ"),
            (0xC044A8, "C-FZZZ"),
            (0xC044C5, "C-GABC"),
            (0xC06F0D, "C-GQBK"),
        ] {
            assert_eq!(registration(addr).unwrap().as_str(), reg, "{addr:06X}");
            assert_eq!(address(reg), Some(addr), "{reg}");
        }
        assert_eq!(n_number(0xA00001).unwrap().as_str(), "N1");
        assert_eq!(n_number(0xADF7C7).unwrap().as_str(), "N99999");
        assert_eq!(n_number(0xADF7C8), None);
        assert_eq!(registration(0x4CA123), None);
        assert_eq!(
            registration(0x7C0000),
            None,
            "Australia assigns from its register"
        );
    }

    #[test]
    fn every_us_and_canadian_address_round_trips() {
        for addr in N_FIRST..=N_LAST {
            let r = n_number(addr).unwrap();
            assert!(r.len as usize <= 6, "{r}");
            assert_eq!(address(r.as_str()), Some(addr), "{r}");
        }
        for addr in 0xC00001..0xC00001 + 2 * 17576 {
            assert_eq!(address(canada_mark(addr).unwrap().as_str()), Some(addr));
        }
    }

    #[test]
    fn marks_that_cannot_exist() {
        for bad in [
            "", "N", "N0", "N01", "NI", "N1I", "N1O", "N123456", "N1ABC", "N1A2", "N1234AB",
            "C-FAA", "C-HAAA", "X-ABCD", "C-FA1A", "N12 3",
        ] {
            assert_eq!(address(bad), None, "{bad:?}");
        }
        assert_eq!(address("n628ts"), Some(0xA835AF));
        assert_eq!(address("cgabc"), Some(0xC044C5));
    }
}
