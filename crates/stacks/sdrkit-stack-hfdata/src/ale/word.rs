//! The 2G ALE word layer (MIL-STD-188-141 A.5.2): the extended Golay (24,12) code, the interleaving, the triple redundancy and the
//! majority vote, the 8-FSK Gray map, word types and character sets.

use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::gray::{gray, ungray};
use std::sync::OnceLock;

/// Word types (the 3-bit preamble).
pub const DATA: u8 = 0;
pub const THRU: u8 = 1;
pub const TO: u8 = 2;
pub const TWS: u8 = 3;
pub const FROM: u8 = 4;
pub const TIS: u8 = 5;
pub const CMD: u8 = 6;
pub const REP: u8 = 7;

sdrkit_core::choice_enum! {
    /// A word type (the 3-bit preamble), in preamble order.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum WordType {
        /// DATA.
        Data = ("data", "DATA"),
        /// THRU.
        Thru = ("thru", "THRU"),
        /// TO.
        To = ("to", "TO"),
        /// TWS.
        Tws = ("tws", "TWS"),
        /// FROM.
        From = ("from", "FROM"),
        /// TIS.
        Tis = ("tis", "TIS"),
        /// CMD.
        Cmd = ("cmd", "CMD"),
        /// REP.
        Rep = ("rep", "REP"),
    }
}

impl WordType {
    /// The type of preamble `t` (its low 3 bits).
    pub fn of(t: u8) -> WordType {
        use sdrkit_core::Choice;
        WordType::from_index(u16::from(t & 7)).unwrap_or(WordType::Data)
    }
}

/// The extended Golay (24,12) code of MIL-STD-188-141 A.5.2: generator 0xC75, 12 data bits, 11 check bits, then an even parity bit.
fn half_code() -> &'static SmallCode {
    static G: OnceLock<SmallCode> = OnceLock::new();
    G.get_or_init(SmallCode::golay_24_12)
}

/// The 24-bit code word of 12 data bits (the first bit sent is bit 23).
pub fn encode_half(d: u16) -> u32 {
    half_code().encode((d & 0xFFF) as u128) as u32
}

/// Decodes a received 24-bit word. Returns the 12 data bits and the number of bits corrected (0 to 3), or `None` when 4 or more
/// errors are found.
pub fn decode_half(r: u32) -> Option<(u16, u32)> {
    let g = half_code();
    g.decode((r & 0xFF_FFFF) as u128)
        .map(|(cw, n)| (g.info(cw) as u16, n as u32))
}

/// The tribit of tone `t` (0 = 750 Hz .. 7 = 2500 Hz): its Gray code (MIL-STD-188-141 A.5.2).
pub fn tribit(t: u8) -> u8 {
    gray(t as u32 & 7) as u8
}

/// The tone of a tribit.
pub fn tone(b: u8) -> u8 {
    ungray(b as u32 & 7) as u8
}

/// The 49 bits of a word (A1 B1 A2 B2 .. A24 B24, stuff bit 0), most significant first, as a u64 (bit 48 first).
pub fn interleave(word: u32) -> u64 {
    let a = encode_half(((word >> 12) & 0xFFF) as u16);
    let b = encode_half((word & 0xFFF) as u16) ^ 0xFF_FFFF;
    let mut v = 0u64;
    for i in (0..24).rev() {
        v = (v << 1) | ((a >> i) & 1) as u64;
        v = (v << 1) | ((b >> i) & 1) as u64;
    }
    v << 1 // stuff bit 0
}

/// The 49 tones of a word: the 49 bits sent three times, three bits per tone, most significant first.
pub fn word_tones(word: u32) -> [u8; 49] {
    let bits = interleave(word);
    let mut stream = [0u8; 147];
    for c in 0..3 {
        for i in 0..49 {
            stream[c * 49 + i] = ((bits >> (48 - i)) & 1) as u8;
        }
    }
    let mut t = [0u8; 49];
    for (k, v) in t.iter_mut().enumerate() {
        let b = (stream[3 * k] << 2) | (stream[3 * k + 1] << 1) | stream[3 * k + 2];
        *v = tone(b);
    }
    t
}

/// What a decoded word holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The 24 bits (preamble and three characters).
    pub word: u32,
    /// Both Golay halves decoded.
    pub good: bool,
    /// Positions whose three copies were not unanimous (0 to 49).
    pub votes: u32,
    /// Golay corrections (4 for a half that failed).
    pub golay: u32,
    /// The stuff bit was 0.
    pub stuff0: bool,
}

/// Decodes 147 received bits (as 49 tribits, the first tone first).
pub fn decode_tribits(tb: &[u8; 49]) -> Decoded {
    let mut bits = [0u8; 147];
    for (k, &b) in tb.iter().enumerate() {
        bits[3 * k] = (b >> 2) & 1;
        bits[3 * k + 1] = (b >> 1) & 1;
        bits[3 * k + 2] = b & 1;
    }
    let mut votes = 0;
    let mut w49 = 0u64;
    for i in 0..49 {
        let s = bits[i] + bits[49 + i] + bits[98 + i];
        if s != 0 && s != 3 {
            votes += 1;
        }
        w49 = (w49 << 1) | (s >= 2) as u64;
    }
    let stuff0 = w49 & 1 == 0;
    let v = w49 >> 1; // 48 bits, A1 first
    let (mut a, mut b) = (0u32, 0u32);
    for i in 0..24 {
        a = (a << 1) | ((v >> (47 - 2 * i)) & 1) as u32;
        b = (b << 1) | ((v >> (46 - 2 * i)) & 1) as u32;
    }
    b ^= 0xFF_FFFF;
    let da = decode_half(a);
    let dbb = decode_half(b);
    let golay = da.map_or(4, |x| x.1) + dbb.map_or(4, |x| x.1);
    let good = da.is_some() && dbb.is_some();
    let word = match (da, dbb) {
        (Some((x, _)), Some((y, _))) => ((x as u32) << 12) | y as u32,
        _ => ((a >> 12) << 12) | (b >> 12),
    };
    Decoded {
        word,
        good,
        votes,
        golay,
        stuff0,
    }
}

/// The word type.
pub fn wtype(word: u32) -> u8 {
    ((word >> 21) & 7) as u8
}

/// The three 7-bit characters.
pub fn chars(word: u32) -> [u8; 3] {
    [
        ((word >> 14) & 0x7F) as u8,
        ((word >> 7) & 0x7F) as u8,
        (word & 0x7F) as u8,
    ]
}

/// A word from its type and three characters.
pub fn make(t: u8, c: [u8; 3]) -> u32 {
    ((t as u32 & 7) << 21)
        | ((c[0] as u32 & 0x7F) << 14)
        | ((c[1] as u32 & 0x7F) << 7)
        | (c[2] as u32 & 0x7F)
}

/// A character of the 38-character basic set (A-Z, 0-9, @, ?).
pub fn basic(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'@' || c == b'?'
}

/// An ASCII-64 character (0x20 to 0x5F).
pub fn ascii64(c: u8) -> bool {
    (0x20..=0x5F).contains(&c)
}

/// Whether `a` is an address a station can send as its own or call: 1 to 15 characters of A-Z and 0-9.
pub fn sendable_address(a: &str) -> bool {
    (1..=15).contains(&a.len())
        && a.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Whether a word's characters are plausible for its type.
pub fn plausible(word: u32) -> bool {
    let c = chars(word);
    match wtype(word) {
        CMD => c[0] >= 0x20,
        DATA | REP => c.iter().all(|&x| ascii64(x)),
        _ => c.iter().all(|&x| basic(x)),
    }
}

/// The words of an address field: the first of type `t`, then DATA, REP, DATA ..., padded with '@' to a multiple of 3.
pub fn address_words(t: u8, addr: &str) -> Vec<u32> {
    let mut b: Vec<u8> = addr.bytes().map(|c| c.to_ascii_uppercase()).collect();
    while b.is_empty() || !b.len().is_multiple_of(3) {
        b.push(b'@');
    }
    text_words(t, &b)
}

/// The words of a text field of type `t` (already padded to a multiple of 3).
pub fn text_words(t: u8, b: &[u8]) -> Vec<u32> {
    b.chunks(3)
        .enumerate()
        .map(|(i, c)| {
            let ty = if i == 0 {
                t
            } else if i % 2 == 1 {
                DATA
            } else {
                REP
            };
            make(ty, [c[0], c[1], c[2]])
        })
        .collect()
}

/// The words of an AMD message: CMD then DATA, REP ..., padded with spaces to a multiple of 3.
pub fn amd_words(text: &str) -> Vec<u32> {
    let mut b: Vec<u8> = text.bytes().map(|c| c.to_ascii_uppercase()).collect();
    while b.is_empty() || !b.len().is_multiple_of(3) {
        b.push(b' ');
    }
    text_words(CMD, &b)
}

/// The LQA command word: 'a', then BER (5 bits), SINAD (5 bits), multipath (4 bits) in the low 7 bits of the next two characters.
pub fn lqa_word(ber: u8, sinad: u8, mp: u8) -> u32 {
    let v = ((ber as u32 & 31) << 9) | ((sinad as u32 & 31) << 4) | (mp as u32 & 15);
    make(CMD, [b'a', (v >> 7) as u8, (v & 0x7F) as u8])
}

/// A link quality report, the figures of the LQA command: BER and SINAD 0 to 30 ([`Lqa::NONE`] for no figure),
/// multipath 0 to 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lqa {
    /// The mean vote errors a word, 0 to 30 (30: 30 or more).
    pub ber: u8,
    /// SINAD, dB, 0 to 30 (30: 30 dB or more).
    pub sinad: u8,
    /// Multipath, 0 to 15.
    pub mp: u8,
}

impl Lqa {
    /// The value of a BER or SINAD with no figure.
    pub const NONE: u8 = 31;
    /// The report of a station that asks for the figures and has none to give.
    pub const REQUEST: Lqa = Lqa {
        ber: Lqa::NONE,
        sinad: Lqa::NONE,
        mp: 0,
    };

    /// The figures of a received transmission of `words` words with `votes` vote errors in all and a mean word SNR of
    /// `snr_db` in 3 kHz: BER is the mean vote errors a word, SINAD is 10 log10(1 + SNR) dB, each rounded and held to 0
    /// to 30. Multipath is not measured and is 0. `words` 0 gives [`Lqa::REQUEST`].
    pub fn measure(words: u32, votes: u32, snr_db: f64) -> Lqa {
        if words == 0 {
            return Lqa::REQUEST;
        }
        let ber = (f64::from(votes) / f64::from(words))
            .round()
            .clamp(0.0, 30.0) as u8;
        let snr = 10f64.powf(snr_db / 10.0);
        let sinad = (10.0 * (1.0 + snr).log10()).round().clamp(0.0, 30.0) as u8;
        Lqa { ber, sinad, mp: 0 }
    }

    /// The figures of the second and third characters of an LQA command.
    pub fn of_chars(c2: u8, c3: u8) -> Lqa {
        let v = ((c2 as u32 & 0x7F) << 7) | (c3 as u32 & 0x7F);
        Lqa {
            ber: ((v >> 9) & 31) as u8,
            sinad: ((v >> 4) & 31) as u8,
            mp: (v & 15) as u8,
        }
    }

    /// The LQA command word of these figures.
    pub fn word(self) -> u32 {
        lqa_word(self.ber, self.sinad, self.mp)
    }

    /// The report for people: "LQA BER b SINAD s MP m".
    pub fn text(self) -> String {
        format!("LQA BER {} SINAD {} MP {}", self.ber, self.sinad, self.mp)
    }
}

/// The longest AMD text, characters.
pub const AMD_MAX: usize = 90;

/// The AMD text that `text` sends: folded to upper case, 1 to [`AMD_MAX`] characters of ASCII-64 (0x20 to 0x5F).
/// `None` for an empty text, a text that is too long or one with another character.
pub fn sendable_amd(text: &str) -> Option<String> {
    let t = text.to_ascii_uppercase();
    ((1..=AMD_MAX).contains(&t.len()) && t.bytes().all(ascii64)).then_some(t)
}

/// Removes '@' padding at the end of an address, except for the allcall "@?@" and the anycall "@@?".
pub fn trim_address(a: &str) -> String {
    if a == "@?@" || a == "@@?" {
        return a.to_string();
    }
    a.trim_end_matches('@').to_string()
}

/// A character for display ('.' for one that does not print).
pub fn printable(c: u8) -> char {
    if (0x20..0x7F).contains(&c) {
        c as char
    } else {
        '.'
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn u1_golay_corrects_three_and_detects_four() {
        let mut rng = Rng::new(7);
        for d in 0..4096u16 {
            let c = encode_half(d);
            assert_eq!(c.count_ones() % 2, 0);
            for w in 0..=3u32 {
                let mut e = 0u32;
                while e.count_ones() < w {
                    e |= 1 << rng.below(24);
                }
                assert_eq!(decode_half(c ^ e), Some((d, w)), "d {d} e {e:06x}");
            }
            let mut e = 0u32;
            while e.count_ones() < 4 {
                e |= 1 << rng.below(24);
            }
            assert_eq!(decode_half(c ^ e), None, "4 errors decoded silently");
        }
    }

    #[test]
    fn known_golay_words() {
        // Data 001 encodes to the generator x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1 itself, then its parity bit.
        assert_eq!(encode_half(0x001), 0x0018EB);
        assert_eq!(encode_half(0x000), 0);
        assert_eq!(decode_half(0x0018EB ^ 0x80_0001), Some((0x001, 2)));
    }

    #[test]
    fn u2_gray_map() {
        for t in 0..8 {
            assert_eq!(tone(tribit(t)), t);
        }
        assert_eq!(tribit(0), 0b000);
        assert_eq!(tribit(7), 0b100);
        assert_eq!(tribit(2), 0b011);
    }

    fn tribits_of(word: u32) -> [u8; 49] {
        let t = word_tones(word);
        let mut tb = [0u8; 49];
        for i in 0..49 {
            tb[i] = tribit(t[i]);
        }
        tb
    }

    fn flip(tb: &mut [u8; 49], bit: usize) {
        tb[bit / 3] ^= 4 >> (bit % 3);
    }

    #[test]
    fn u3_votes_and_golay() {
        let w = make(TO, *b"ABC");
        let mut tb = tribits_of(w);
        // One copy wrong at every 5th position: 10 positions (0, 5, .. 45).
        for (k, i) in (0..49).step_by(5).enumerate() {
            flip(&mut tb, (k % 3) * 49 + i);
        }
        let d = decode_tribits(&tb);
        assert_eq!((d.word, d.votes, d.golay, d.good), (w, 10, 0, true));
        // Two copies wrong at one position: the vote takes the wrong bit and Golay repairs it.
        let mut tb = tribits_of(w);
        flip(&mut tb, 7);
        flip(&mut tb, 49 + 7);
        let d = decode_tribits(&tb);
        assert_eq!((d.word, d.votes, d.golay, d.good), (w, 1, 1, true));
    }

    #[test]
    fn lqa_figures_and_amd_texts() {
        assert_eq!(
            Lqa::measure(4, 0, 40.0),
            Lqa {
                ber: 0,
                sinad: 30,
                mp: 0
            }
        );
        // 12 vote errors over 4 words; SNR 10 dB gives SINAD 10 log10(11) = 10.4 dB.
        assert_eq!(
            Lqa::measure(4, 12, 10.0),
            Lqa {
                ber: 3,
                sinad: 10,
                mp: 0
            }
        );
        assert_eq!(Lqa::measure(0, 0, 0.0), Lqa::REQUEST);
        let q = Lqa {
            ber: 3,
            sinad: 21,
            mp: 2,
        };
        let c = chars(q.word());
        assert_eq!(
            (wtype(q.word()), c[0], Lqa::of_chars(c[1], c[2])),
            (CMD, b'a', q)
        );
        assert_eq!(sendable_amd("qsy 7102").as_deref(), Some("QSY 7102"));
        assert_eq!(sendable_amd(""), None);
        assert_eq!(sendable_amd("A~B"), None);
        assert_eq!(sendable_amd(&"A".repeat(91)), None);
    }

    #[test]
    fn u4_address_words_and_trim() {
        let w = address_words(TO, "ABCDEFGHIJ");
        let types: Vec<u8> = w.iter().map(|&x| wtype(x)).collect();
        assert_eq!(types, vec![TO, DATA, REP, DATA]);
        assert_eq!(trim_address("ABC@@"), "ABC");
        assert_eq!(trim_address("@?@"), "@?@");
        assert_eq!(trim_address("@@?"), "@@?");
    }
}
