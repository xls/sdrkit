//! ANT+ manufacturer identifiers: the maker named in ANT+ common data page 80 and in FIT files.
//!
//! **Data.** `data/ant_manufacturer.tsv` holds the whole `manufacturer` type of the Garmin FIT SDK profile (see the file header
//! for the version and date). Names are the FIT identifiers as published, for example `wahoo_fitness`.
//!
//! **Update:**
//!
//! ```text
//! curl -O https://raw.githubusercontent.com/garmin/fit-python-sdk/main/garmin_fit_sdk/profile.py
//! cargo run -p sdrkit-util-ids --example update_tables -- ant-manufacturer profile.py > crates/util/sdrkit-util-ids/data/ant_manufacturer.tsv
//! ```
//!
//! ```
//! use sdrkit_util_ids::ant;
//! assert_eq!(ant::manufacturer(1), Some("garmin"));
//! assert_eq!(ant::manufacturer(32), Some("wahoo_fitness"));
//! assert_eq!(ant::manufacturer(0), None);
//! ```

use crate::pool::{self, find};

/// The FIT name of an ANT+ manufacturer identifier, or `None` for an identifier not in the list.
pub fn manufacturer(id: u16) -> Option<&'static str> {
    find(&pool::ANT_MAN_KEYS, &id).map(|i| pool::ANT_MAN_POOL.get(pool::ANT_MAN_NAME[i]))
}
