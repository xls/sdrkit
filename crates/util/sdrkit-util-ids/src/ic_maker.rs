//! Integrated circuit manufacturer codes: the byte that names the chip maker in an ISO/IEC 15693 or ISO/IEC 14443 UID.
//!
//! The list holds the codes the sdrkit receivers meet. It is a subset of the register in ISO/IEC 7816-6.
//! Standard: ISO/IEC 7816-6:1996/Amd 1:2000 table 9 (ISO/IEC 15693-3 clause 4.1 names it for the UID).
//!
//! ```
//! use sdrkit_util_ids::ic_maker;
//! assert_eq!(ic_maker::name(0x04), Some("NXP Semiconductors"));
//! assert_eq!(ic_maker::name(0xFF), None);
//! ```

/// The registered makers in the list, sorted by code.
pub const MAKERS: &[(u8, &str)] = &[
    (0x01, "Motorola"),
    (0x02, "STMicroelectronics"),
    (0x04, "NXP Semiconductors"),
    (0x05, "Infineon Technologies"),
    (0x07, "Texas Instruments"),
    (0x16, "EM Microelectronic-Marin"),
    (0x2B, "Maxim Integrated"),
];

/// The maker of an IC manufacturer code, or `None` for a code not in the list.
pub fn name(code: u8) -> Option<&'static str> {
    MAKERS
        .binary_search_by_key(&code, |m| m.0)
        .ok()
        .map(|i| MAKERS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sorted_and_unique() {
        assert!(MAKERS.windows(2).all(|w| w[0].0 < w[1].0));
        for &(c, n) in MAKERS {
            assert_eq!(name(c), Some(n));
        }
    }
}
