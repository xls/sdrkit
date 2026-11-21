//! Presets and start helpers for hosts.
//!
//! [`Data::open`] reads a data folder: plan files, lists of known stations, cities, DAB transmitters and countries.
//! [`Data::preset`] works out the settings, the frequency and the channels to start a stack in a [`Region`].
//! [`start`] applies a preset through the public calls of the engine. The lookups of [`lookup`] list the plans that
//! a country uses, build the channels of a scan and the labels of a span, search plans and frequency entries, name a
//! frequency and keep the user's plan file. This crate holds no regional data: every frequency, width and plan comes
//! from the files of the folder.
//!
//! The folder:
//!
//! | Path | Contents |
//! |---|---|
//! | `plans/*.json`, `plans/*.csv` | Plan files in the formats of `sdrkit-data-plans`. |
//! | `stations/<kind>/<ISO2>.csv` | The known stations of a country, see [`StationKind`] and [`Station`]. |
//! | `cities/<ISO2>.csv` | The cities of a country, see [`City`]. |
//! | `dab-tii/<ISO2>.csv` | The DAB transmitters of a country, see [`Transmitter`]. |
//! | `countries.csv` | The countries, see [`Country`]. |
//! | `links/*.csv` | Satellite downlinks by stack and submode, see [`Downlink`]. |
//! | any `*.csv` with a `# stacks:` line | Data files that stacks read through their settings, see [`Data::stack_files`]. |

#![warn(missing_docs)]

pub mod analogtv;
pub mod avdata;
pub mod broadcast;
pub mod cell;
mod choice;
pub mod dab;
mod data;
pub mod default_dials;
mod downlinks;
pub mod dvbt;
mod error;
mod family;
pub mod fm;
pub mod freqdb;
pub mod general;
pub mod ism;
pub mod lookup;
pub mod lora;
pub mod marine;
mod plans;
mod preset;
pub mod radio_dials;
mod region;
mod start;
pub mod tv;
pub mod wlan;
pub mod wsjt;

pub use choice::{data_folder, RegionChoice};
pub use data::{City, Country, Data, Station, StationKind, Transmitter};
pub use downlinks::Downlink;
pub use freqdb::FrequencyRow;
pub use error::{DataError, PresetError, RegionError, StartError};
pub use lookup::{Hit, HitKind};
pub use preset::{Options, PlanChannel, Preset};
pub use region::Region;
pub use start::{preset_settings, preset_values, start, start_preset, PresetSetting};
