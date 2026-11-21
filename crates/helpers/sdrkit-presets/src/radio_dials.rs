//! Starting dials of the cellular, digital voice, LoRa and voice families, in the format of [`crate::default_dials`].
//!
//! The descriptors of these stacks hold no starting frequency. [`dials`] reads the rows of the families in
//! [`FAMILIES`]; [`crate::default_dials::dial_hz`] gives the dial of one stack or submode.

use crate::data::Data;
use crate::default_dials::{read_dials, DefaultDial, DEFAULT_DIALS};
use crate::error::DataError;

/// The families of this module whose folder holds a file [`DEFAULT_DIALS`].
pub const FAMILIES: &[&str] = &["cell", "dvoice", "lora", "voice"];

/// The starting dials of every family in [`FAMILIES`], in the order of the list and of each file.
///
/// # Errors
///
/// As [`read_dials`]; a family without the file is [`DataError::Missing`].
pub fn dials(data: &Data) -> Result<Vec<DefaultDial>, DataError> {
    let mut out = Vec::new();
    for f in FAMILIES {
        out.extend(read_dials(&data.folder().join(f).join(DEFAULT_DIALS))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_dials::dial_hz;

    #[test]
    fn every_row_of_every_file_loads() {
        let d = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
        let all = dials(&d).unwrap();
        assert_eq!(all.len(), 5 + 18 + 5 + 11);
        for r in &all {
            assert!(r.hz > 0.0 && !r.stack.is_empty(), "{r:?}");
        }
        assert_eq!(dial_hz(&all, "lbj", None), Some(821.2375e6));
        assert_eq!(dial_hz(&all, "nr", None), Some(3541.44e6));
        assert_eq!(dial_hz(&all, "freedv", Some("700D")), Some(14.236e6));
        assert_eq!(dial_hz(&all, "dcr", None), Some(351.2875e6));
        assert_eq!(dial_hz(&all, "aprs", None), Some(144.8e6));
        assert_eq!(dial_hz(&all, "fanet", None), Some(868.2e6));
        assert_eq!(dial_hz(&all, "voice_channels", None), Some(27.21e6));
        assert_eq!(dial_hz(&all, "voice_channels", Some("frs")), Some(462.643_75e6));
        assert_eq!(dial_hz(&all, "wmic", None), Some(590e6));
        assert_eq!(dial_hz(&all, "ebicab", None), None);
    }
}
