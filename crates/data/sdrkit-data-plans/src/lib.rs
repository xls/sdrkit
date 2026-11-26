//! Band and channel plans, read from plan files with a published schema (`docs/schema/plans.json`).
//!
//! The library holds no plan data and makes no choice of a plan. A host loads plan files ([`Plans::from_files`],
//! [`Plans::from_storage`]) and gets rows back: band groups (each with its published source and the stacks that receive
//! each band), the plan view of each group, and the other rows of the files as they are written. The host picks a plan
//! by its id. A store with no files is valid and holds no plans. A user file in the same format replaces a loaded group
//! with the same id ([`Plans::add_file`]). The tables of standards that hold everywhere are constants: the cellular bands
//! ([`cellular`]) and the DAB blocks ([`dab`]).
//!
//! # Example
//!
//! ```
//! use sdrkit_data_plans::Plans;
//!
//! let tv = r#"{"format": 1, "groups": [{"id": "tv_7mhz", "name": "Television, 7 MHz", "kind": "tv",
//!     "source": {"publisher": "An authority", "title": "Television channel plan", "date": "2024"}, "verified": false,
//!     "unverified_reason": "an example", "bands": [{"name": "VHF", "service": "broadcasting", "start_hz": 223000000,
//!     "end_hz": 230000000, "modes": ["dvbt"], "channels": [{"name": "12", "centre_hz": 226500000, "width_hz": 7000000}]}]}]}"#;
//! let plans = Plans::from_files(&[("tv.json".into(), tv.into())]).unwrap();
//! let plan = plans.get("tv_7mhz").unwrap();
//! let ch12 = plan.find("12").unwrap();
//! assert_eq!((ch12.centre_hz, ch12.width_hz), (226.5e6, 7e6));
//! assert_eq!(plans.name_of(226.5e6, &["tv_7mhz"]).unwrap().name, "12");
//! ```

#![warn(missing_docs)]

pub mod api;
mod cell_table;
pub mod cellular;
mod compile;
pub mod dab;
pub mod data;
pub mod io;
pub mod model;
pub mod naming;
pub mod registry;
pub mod schema;

pub use compile::band_channels;
pub use data::{
    Band, BandChannel, BandGroup, CellNumbers, CellSystem, Cellular, CountryEntry, DataError,
    ServicePart, ServiceState, Duplex, Naming, Numbering, PlanFile, Presets, Raster, RdsVariant, RegionEntry, Service, Source,
    SyncRaster,
};
pub use model::{
    format_frequency, normalise_name, AnalogSystem, Channel, ChannelPlan, Direction, Place,
    PlanKind, Review, Segment,
};
pub use registry::{validate, GroupEntry, ParseFault, PlanError, Plans};
