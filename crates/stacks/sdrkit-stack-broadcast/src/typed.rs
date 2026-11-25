//! The interface of the broadcast family: the shapes of the result sets, the table rows and the events, the tables,
//! series and event kinds, the description of the family ([`family`]) and of the band scan ([`scan_interface`]).

use crate::cdr::results as cdr;
use crate::cdr::CdrSetting;
use crate::cquam::{CquamSetting, StereoMode};
use crate::drm::results as drm;
use crate::drm::DrmSetting;
use crate::hamdrm::{self, HamSetting, HamdrmFields};
use crate::hd::results as hd;
use crate::hd::HdSetting;
use crate::scan::{ProgressFields, STATIONS};
use sdrkit_core::interface::{Family as Described, Fields, Hertz, Seconds, StackShape};
use sdrkit_core::publish::keys;
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_proto_bcdata::publish as bc;
use sdrkit_proto_mpegts::publish::SERVICE_SELECT_KIND;
use std::sync::OnceLock;

// C-QUAM.

/// The results of the C-QUAM stack that the stereo matrix writes. No offset, carrier level, C/N or lock quality
/// without a carrier, no pilot level before the first pilot check, no levels before the first audio, no width in mono
/// and no noise floor before the first noise measure.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CquamFields {
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Carrier to noise in the channel.
    #[field(label = "C/N", unit = "dB")]
    pub cn_db: Option<f64>,
    /// The carrier level.
    #[field(label = "Carrier level", unit = "dBFS")]
    pub carrier_dbfs: Option<f64>,
    /// Stereo now.
    #[field(label = "Stereo")]
    pub stereo: bool,
    /// The 25 Hz pilot is seen (also with mono forced).
    #[field(label = "Pilot seen")]
    pub pilot_seen: bool,
    /// The pilot over its neighbours.
    #[field(label = "Pilot over its neighbours", unit = "dB")]
    pub pilot_db: Option<f64>,
    /// The pilot as a percentage of the carrier.
    #[field(label = "Pilot", unit = "%")]
    pub pilot_pct: Option<f64>,
    /// The mean cos φ of the stereo phase (1 for a mono station).
    #[field(label = "Lock quality")]
    pub lock_quality: Option<f64>,
    /// The left level, rms over 100 ms.
    #[field(label = "Left", unit = "dBFS")]
    pub left_dbfs: Option<f64>,
    /// The right level, rms over 100 ms.
    #[field(label = "Right", unit = "dBFS")]
    pub right_dbfs: Option<f64>,
    /// L - R over L + R.
    #[field(label = "Stereo width", unit = "dB")]
    pub width_db: Option<f64>,
    /// The peak modulation of L + R in the last second.
    #[field(label = "Peak modulation", unit = "%")]
    pub mod_peak_pct: f64,
    /// The stereo mode setting.
    #[field(label = "Stereo mode")]
    pub stereo_mode: StereoMode,
    /// The audio filter width either side of the carrier.
    #[field(label = "Filter width", unit = "Hz")]
    pub filter_width: Hertz,
    /// The noise floor in the audio width.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// Pilot checks that found the pilot.
    #[field(label = "Pilot checks good")]
    pub pilot_checks_ok: i64,
    /// Pilot checks without it.
    #[field(label = "Pilot checks bad")]
    pub pilot_checks_bad: i64,
    /// Signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
}

/// The input rate, which the front end of the C-QUAM stack writes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CquamFrontFields {
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate: Hertz,
}

/// The quadrature spectrum, 0 to 60 Hz in 1 Hz steps, dB relative to the carrier.
pub const PILOT_SPECTRUM: SeriesDef = SeriesDef {
    key: "pilot_spectrum",
    label: "Pilot spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// What a change of frequency keeps in DRM and HD Radio: the guide and the logos that it names, which describe the
/// services of several stations.
pub const GUIDE: &[&str] = &[
    "guide_services",
    "guide_programmes",
    "guide_groups",
    "logos",
];

/// The interface of the family: what the broadcast stacks declare.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("broadcast", 2, 1)
            .content(
                sdrkit_core::ContentKind::Broadcast,
                &["drm", "hdradio", "cdr", "cquam"],
            )
            .content(sdrkit_core::ContentKind::Pictures, &["hamdrm"])
            .audio(
                sdrkit_core::AudioKind::Programme,
                &["drm", "hdradio", "cdr", "cquam"],
            )
            .pictures(&["hamdrm", "hdradio"])
            .uses(sdrkit_proto_mpegts::publish::interface())
            .uses(bc::interface())
            .uses(scan_interface())
            .shared_source("datafile", &["cdr", "hamdrm"])
            .shared_source("scan", &["cdr", "cquam", "drm", "hdradio"])
            .shared_source("hd", &["hdradio"])
            .shared_source("component", &["cdr", "cquam", "drm", "hamdrm", "hdradio"])
            .stack(
                StackShape::new(crate::drm::ID)
                    .fields::<drm::DrmFields>()
                    .fields::<bc::LabelFields>()
                    .fields::<bc::SlideFields>()
                    .table(&drm::SERVICES)
                    .table(&drm::STREAMS)
                    .table(&drm::OBJECTS)
                    .table(&drm::SLIDES)
                    .table(&drm::PAGES)
                    .table(&drm::PACKET_COUNTERS)
                    .table(&drm::NOTES)
                    .table(&bc::LABEL_TAG_TABLE)
                    .table(&bc::GUIDE_SERVICE_TABLE)
                    .table(&bc::GUIDE_PROGRAMME_TABLE)
                    .table(&bc::GUIDE_GROUP_TABLE)
                    .table(&bc::SLIDE_CATEGORY_TABLE)
                    .table(&bc::LOGO_TABLE)
                    .series(drm::SERIES_CHANNEL)
                    .series(drm::SERIES_IMPULSE)
                    .series(drm::SERIES_FAC)
                    .series(drm::SERIES_SDC)
                    .series(drm::SERIES_MSC)
                    .event(&drm::TEXT_CHANGED)
                    .event(&drm::OBJECT_RECEIVED)
                    .settings(DrmSetting::SETTINGS)
                    .command(&SERVICE_SELECT_KIND)
                    .command(&crate::component::CLEAR)
                    .kept(GUIDE),
            )
            .stack(
                StackShape::new(hamdrm::ID)
                    .fields::<HamdrmFields>()
                    .table(&hamdrm::HISTORY)
                    .table(&hamdrm::MISSING)
                    .settings(HamSetting::SETTINGS)
                    .command(&crate::component::CLEAR)
                    .kept(&["history", "HamdrmFields.files_completed"]),
            )
            .stack(
                StackShape::new(crate::hd::ID)
                    .fields::<hd::HdradioFields>()
                    .fields::<hd::HdSoundFields>()
                    .fields::<bc::LabelFields>()
                    .fields::<bc::SlideFields>()
                    .table(&hd::PROGRAMS)
                    .table(&hd::DATA_SERVICES)
                    .table(&hd::FILES)
                    .table(&bc::LABEL_TAG_TABLE)
                    .table(&bc::GUIDE_SERVICE_TABLE)
                    .table(&bc::GUIDE_PROGRAMME_TABLE)
                    .table(&bc::GUIDE_GROUP_TABLE)
                    .table(&bc::SLIDE_CATEGORY_TABLE)
                    .table(&bc::LOGO_TABLE)
                    .series(hd::SERIES_CONSTELLATION)
                    .event(&hd::SONG)
                    .settings(HdSetting::SETTINGS)
                    .command(&SERVICE_SELECT_KIND)
                    .command(&crate::component::CLEAR)
                    .kept(GUIDE),
            )
            .stack(
                StackShape::new(crate::cdr::ID)
                    .fields::<cdr::CdrFields>()
                    .table(&cdr::SERVICES)
                    .table(&cdr::SERVICE_DATA_TYPES)
                    .table(&cdr::NETWORK_FREQUENCIES)
                    .series(cdr::SERIES_SI)
                    .series(cdr::SERIES_MSC)
                    .series(cdr::SERIES_CHANNEL)
                    .settings(CdrSetting::SETTINGS)
                    .command(&SERVICE_SELECT_KIND)
                    .command(&crate::component::CLEAR)
                    .command(&cdr::RESYNC),
            )
            .stack(
                StackShape::new(crate::cquam::ID)
                    .fields::<CquamFields>()
                    .fields::<CquamFrontFields>()
                    .series(PILOT_SPECTRUM)
                    .settings(CquamSetting::SETTINGS)
                    .command(&crate::component::CLEAR),
            )
            .added(2, 1, ADDED_2_1)
    })
}

/// What version 2.1 added: the sound field of HD Radio and the service that plays in DRM.
const ADDED_2_1: &[&str] = &[
    "hdradio.fields.HdSoundFields",
    "shape.HdSoundFields",
    "shape.HdSoundFields.sound",
    "shape.DrmFields.playing_service",
];

/// The key of the interface of the band scan.
pub const SCAN_KEY: &str = "broadcast_scan";

/// The interface of the band scan of the scanned broadcast stacks ([`crate::scan::BandScan`]): its progress and the
/// stations that it found. A scan is no stack: its results are its own, beside those of the receiver that listens.
pub fn scan_interface() -> &'static Described {
    static SCAN: OnceLock<Described> = OnceLock::new();
    SCAN.get_or_init(|| {
        Described::shared(SCAN_KEY, 1, 1).stack(
            StackShape::new(SCAN_KEY)
                .fields::<ProgressFields>()
                .table(&STATIONS),
        )
    })
}

/// The description of the results of a band scan.
pub fn scan_shape() -> &'static StackShape {
    &scan_interface().stacks[0]
}

/// The description of the stack `id` of the family.
///
/// # Panics
///
/// If the family does not hold the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the family holds the stack {id}"))
}
