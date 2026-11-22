//! The binary LDPC codes of the family, each built once on first use. FT8, FT4, FT2 and JS8 use the tables that
//! `sdrkit-algo-fec` ships, FST4 and FST4W the tables of [`super::fst4code`], and MSK144 the table of
//! [`super::msk144code`]. Q65 uses the code over GF(64) of [`super::q65code`].
//! Standard: QEX July/August 2020 "The FT4 and FT8 Communication Protocols" section 3.

use super::{fst4code, msk144code};
use sdrkit_algo_fec::ldpc::{tables::wsjt, InfoAt, LdpcCode};
use std::sync::OnceLock;

/// The (174,91) code of FT8, FT4 and FT2: 91 information bits (77 message bits and the CRC-14), then 83 parity bits.
pub fn ft8() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("ft8 (174,91)", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start)
            .expect("the FT8 table of sdrkit-algo-fec is consistent")
    })
}

/// The (174,87) code of JS8 in transmission order: 87 parity bits, then 87 information bits (75 frame bits and the CRC-12).
pub fn js8() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("js8 (174,87)", 174, 87, &wsjt::JS8_CHECKS, InfoAt::End)
            .expect("the JS8 table of sdrkit-algo-fec is consistent")
    })
}

/// The (240,101) code of FST4: 101 information bits (the 77 scrambled message bits and the CRC-24), then 139 parity bits.
pub fn fst4() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("fst4 (240,101)", 240, 101, &fst4code::FST4_CHECKS, InfoAt::Start)
            .expect("the FST4 table is consistent")
    })
}

/// The (240,74) code of FST4W: 74 information bits (the 50 message bits and the CRC-24), then 166 parity bits.
pub fn fst4w() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("fst4w (240,74)", 240, 74, &fst4code::FST4W_CHECKS, InfoAt::Start)
            .expect("the FST4W table is consistent")
    })
}

/// The (128,90) code of MSK144: 90 information bits (the 77 message bits and the CRC-13), then 38 parity bits.
pub fn msk144() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("msk144 (128,90)", 128, 90, &msk144code::MSK144_CHECKS, InfoAt::Start)
            .expect("the MSK144 table is consistent")
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use sdrkit_algo_fec::ldpc::LdpcCode;

    /// The `n` bits of a hex text, most significant bit of each digit first.
    pub fn hex_bits(hex: &str, n: usize) -> Vec<u8> {
        let mut v: Vec<u8> = hex
            .chars()
            .flat_map(|c| {
                let d = c.to_digit(16).expect("a hex digit") as u8;
                (0..4).rev().map(move |k| d >> k & 1)
            })
            .collect();
        assert!(v[n..].iter().all(|&b| b == 0), "pad bits of {hex}");
        v.truncate(n);
        v
    }

    /// The (weight, count) pairs of the rows of `checks`, by weight.
    pub fn row_weights(checks: &[&[u8]]) -> Vec<(usize, usize)> {
        let mut w = std::collections::BTreeMap::new();
        for c in checks {
            *w.entry(c.len()).or_insert(0) += 1;
        }
        w.into_iter().collect()
    }

    /// The check lists, the variable lists and the generator rows describe `code`: the variable lists are the check lists
    /// read by columns, each check list is in increasing order, and every single-bit information word encodes to the
    /// generator row of its bit and satisfies every check.
    pub fn check_table(code: &LdpcCode, checks: &[&[u8]], vars: &[[u8; 3]], parity: &[&str]) {
        let (n, k) = (code.n(), code.k());
        assert_eq!((checks.len(), vars.len(), parity.len()), (n - k, n, k));
        let mut cols = vec![Vec::new(); n];
        for (i, c) in checks.iter().enumerate() {
            assert!(c.windows(2).all(|w| w[0] < w[1]), "check {i}");
            for &b in *c {
                cols[b as usize].push(i as u8);
            }
        }
        for (j, v) in vars.iter().enumerate() {
            assert_eq!(cols[j], v.to_vec(), "bit {j}");
        }
        assert_eq!(checks.iter().map(|c| c.len()).sum::<usize>(), 3 * n);
        let mut cw = vec![0u8; n];
        for (j, row) in parity.iter().enumerate() {
            let mut info = vec![0u8; k];
            info[j] = 1;
            code.encode(&info, &mut cw).unwrap();
            assert_eq!(cw[..k], info[..], "row {j}");
            assert_eq!(cw[k..], hex_bits(row, n - k)[..], "row {j}");
            assert!(code.is_codeword(&cw), "row {j}");
        }
    }

    /// The printed code word `hex` satisfies every check of `code`, has `weight` ones, and is the encoding of its
    /// information bits.
    pub fn check_word(code: &LdpcCode, hex: &str, weight: usize) {
        let w = hex_bits(hex, code.n());
        assert_eq!(w.iter().filter(|&&b| b == 1).count(), weight, "{hex}");
        assert!(code.is_codeword(&w), "{hex}");
        let mut cw = vec![0u8; code.n()];
        code.encode(&w[..code.k()], &mut cw).unwrap();
        assert_eq!(cw, w, "{hex}");
    }
}
