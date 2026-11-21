//! DSC symbols and framing (ITU-R M.493-15 section 3 and Annex 1): the 10-bit symbol with its check bits, the phasing sequence,
//! time diversity, the error-check character, and the bit stream of a whole call.

/// The phasing symbol in DX places.
pub const DX_PHASING: u8 = 125;
/// End-of-sequence characters.
pub const EOS: [u8; 3] = [117, 122, 127];

/// A symbol's ten bits, first sent first: 7 information bits least significant first (1 = Y, 0 = B), then the count of B elements
/// in the seven, most significant bit first.
pub fn encode(sym: u8) -> [u8; 10] {
    let mut b = [0u8; 10];
    let mut zeros = 0;
    for (i, v) in b.iter_mut().take(7).enumerate() {
        *v = (sym >> i) & 1;
        if *v == 0 {
            zeros += 1;
        }
    }
    b[7] = (zeros >> 2) & 1;
    b[8] = (zeros >> 1) & 1;
    b[9] = zeros & 1;
    b
}

/// The symbol of ten bits, or `None` when the check bits do not match.
pub fn decode(bits: &[u8]) -> Option<u8> {
    let mut v = 0u8;
    let mut zeros = 0u8;
    for (i, &b) in bits.iter().take(7).enumerate() {
        v |= (b & 1) << i;
        if b & 1 == 0 {
            zeros += 1;
        }
    }
    let chk = ((bits[7] & 1) << 2) | ((bits[8] & 1) << 1) | (bits[9] & 1);
    (chk == zeros).then_some(v)
}

/// The expected phasing symbol of a slot (0 to 15), when the slot holds one.
pub fn phasing(slot: usize) -> Option<u8> {
    if slot.is_multiple_of(2) {
        (slot <= 10).then_some(DX_PHASING)
    } else if slot <= 15 {
        Some(111 - (slot as u8 - 1) / 2)
    } else {
        None
    }
}

/// The error-check character: even parity of the 7 information bits of the format specifier, every message symbol and the EOS.
pub fn ecc(format: u8, message: &[u8], eos: u8) -> u8 {
    (format ^ eos ^ sdrkit_util_bits::checksum::xor8(message)) & 0x7F
}

/// The symbols of every slot of a call: phasing, then DX and RX places alternating, ending with the RX copy of the ECC.
pub fn slots(format: u8, message: &[u8], eos: u8) -> Vec<u8> {
    let e = ecc(format, message, eos);
    let mut dx = vec![format, format];
    dx.extend_from_slice(message);
    dx.extend_from_slice(&[eos, e, eos, eos]);
    let k = message.len();
    let last = 23 + 2 * k;
    let mut out = Vec::with_capacity(last + 1);
    for s in 0..=last {
        let v = if let Some(p) = phasing(s) {
            if s >= 12 && s % 2 == 0 {
                dx[(s - 12) / 2]
            } else {
                p
            }
        } else if s % 2 == 0 {
            dx[(s - 12) / 2]
        } else {
            dx[(s - 17) / 2]
        };
        out.push(v);
    }
    out
}

/// The bits of a whole call: `dots` bits of dot pattern (alternating B and Y, starting with B), then every slot.
pub fn call_bits(dots: usize, format: u8, message: &[u8], eos: u8) -> Vec<u8> {
    let mut b: Vec<u8> = (0..dots).map(|i| (i % 2) as u8).collect();
    for s in slots(format, message, eos) {
        b.extend_from_slice(&encode(s));
    }
    b
}

/// Two decimal digits per symbol.
pub fn number_symbols(digits: &str) -> Vec<u8> {
    let d: Vec<u8> = digits
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|c| c - b'0')
        .collect();
    d.chunks(2)
        .map(|c| c[0] * 10 + c.get(1).copied().unwrap_or(0))
        .collect()
}

/// An MMSI as 5 symbols (ten digits, the tenth 0).
pub fn mmsi_symbols(mmsi: &str) -> Vec<u8> {
    number_symbols(&format!("{mmsi:0>9}0"))
}

/// Digits of symbols: `~~` for a symbol outside 00 to 99 (or unknown).
pub fn digits(syms: &[Option<u8>]) -> String {
    syms.iter()
        .map(|s| match s {
            Some(v) if *v < 100 => format!("{v:02}"),
            _ => "~~".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table() {
        let s = |v: u8| {
            encode(v)
                .iter()
                .map(|b| if *b == 1 { 'Y' } else { 'B' })
                .collect::<String>()
        };
        assert_eq!(s(121), "YBBYYYYBYB");
        assert_eq!(s(0), "BBBBBBBYYY");
        assert_eq!(s(127), "YYYYYYYBBB");
        for v in 0..128u8 {
            let b = encode(v);
            assert_eq!(decode(&b), Some(v));
            for i in 0..10 {
                let mut c = b;
                c[i] ^= 1;
                assert_eq!(decode(&c), None, "{v} bit {i}");
            }
        }
        assert_eq!(mmsi_symbols("232123456"), vec![23, 21, 23, 45, 60]);
        assert_eq!(mmsi_symbols("002320064"), vec![0, 23, 20, 6, 40]);
    }

    #[test]
    fn framing_of_a_distress_call() {
        let mut msg = mmsi_symbols("232123456");
        msg.extend_from_slice(&[106, 15, 5, 10, 1, 18, 12, 34, 109]);
        assert_eq!(msg.len() + 1, 1 + 5 + 1 + 5 + 2 + 1);
        let s = slots(112, &msg, 127);
        for k in (0..=10).step_by(2) {
            assert_eq!(s[k], 125);
        }
        assert_eq!((s[1], s[11], s[13], s[15]), (111, 106, 105, 104));
        assert_eq!((s[12], s[14], s[16]), (112, 112, 23));
        assert_eq!((s[17], s[19], s[21]), (112, 112, 23));
        let k = msg.len();
        let e = ecc(112, &msg, 127);
        let dx_eos = 16 + 2 * k;
        assert_eq!(
            (s[dx_eos], s[dx_eos + 2], s[dx_eos + 4], s[dx_eos + 6]),
            (127, e, 127, 127)
        );
        assert_eq!(*s.last().unwrap(), e);
        assert_eq!(s.len(), 24 + 2 * k);
    }
}
