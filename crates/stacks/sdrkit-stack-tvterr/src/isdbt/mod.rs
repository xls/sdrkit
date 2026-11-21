//! ISDB-T: the channel geometry and rates, the receiver graph and the stack. The submodules hold the TMCC, the frame layout, the
//! layer coding, the synchronisation, the equaliser, the components and the test signal.
//!
//! Standard: ARIB STD-B31, ABNT NBR 15601.

pub mod chain;
pub mod eq;
pub mod fec;
pub mod gen;
pub mod graph;
pub mod layout;
pub mod results;
pub mod sync;
pub mod tmcc;

use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, Receiver, Setting, SettingSet, SignalType,
    StackConfig, StackDescriptor, StageError, Tuning,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_mpegts::layer::{self, TsLayer, DECODE_VIDEO};
use std::sync::Arc;
use tmcc::LayerParams;

/// The stack id.
pub const ID: &str = "isdbt";

sdrkit_core::choice_enum! {
    /// The channel width.
    pub enum Width {
        /// 6 MHz.
        W6 = ("6", "6 MHz"),
        /// 7 MHz.
        W7 = ("7", "7 MHz"),
        /// 8 MHz (Maldives).
        W8 = ("8", "8 MHz (Maldives)"),
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
    }
}

impl Width {
    /// The width in MHz that the receiver starts with (8 for automatic).
    pub fn start_mhz(self) -> f64 {
        match self {
            Width::W6 => 6.0,
            Width::W7 => 7.0,
            Width::W8 | Width::Auto => 8.0,
        }
    }

    /// The lowest input rate for the width.
    pub fn min_rate(self) -> f64 {
        self.start_mhz() * 1e6
    }
}

/// The channel width.
pub const WIDTH: Setting<field::Choice<Width>> =
    Setting::choice("width", "Channel width", Width::W6).restart();

sdrkit_core::settings! {
    /// The settings of `isdbt.sync` and `isdbt.report`.
    pub enum IsdbtSetting {
        /// The channel width.
        Width = WIDTH,
    }
}

sdrkit_core::settings! {
    /// The settings of the ISDB-T stack.
    pub enum IsdbtStackSetting {
        /// The channel width.
        Width = WIDTH,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
    }
}

sdrkit_core::choice_enum! {
    /// The segments a narrow-band (ISDB-TSB) receiver takes.
    pub enum SegmentCount {
        /// One segment first, three when the TMCC announces them and the rate carries them.
        Auto = ("auto", "Automatic"),
        /// One segment.
        One = ("1", "1"),
        /// Three segments.
        Three = ("3", "3"),
    }
}

impl SegmentCount {
    /// The receiver's segment choice.
    pub fn segments(self) -> sync::Segments {
        match self {
            SegmentCount::Auto => sync::Segments::Auto,
            SegmentCount::One => sync::Segments::One,
            SegmentCount::Three => sync::Segments::Three,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The value of the continual pilot above the top segment of a narrow band.
    pub enum EdgeChoice {
        /// The pilot sequence's value at that carrier.
        Sequence = ("sequence", "Pilot sequence"),
        /// +4/3.
        Plus = ("plus", "+4/3"),
        /// −4/3.
        Minus = ("minus", "-4/3"),
    }
}

impl EdgeChoice {
    /// The layout's rule.
    pub fn edge(self) -> layout::EdgePilot {
        match self {
            EdgeChoice::Sequence => layout::EdgePilot::Sequence,
            EdgeChoice::Plus => layout::EdgePilot::Plus,
            EdgeChoice::Minus => layout::EdgePilot::Minus,
        }
    }
}

/// The segment count of a narrow band.
pub const SEGMENTS: Setting<field::Choice<SegmentCount>> =
    Setting::choice("segments", "Segments", SegmentCount::Auto).restart();

/// The upper-edge pilot of a narrow band.
pub const EDGE_PILOT: Setting<field::Choice<EdgeChoice>> =
    Setting::choice("edge_pilot", "Upper-edge pilot", EdgeChoice::Sequence)
        .restart()
        .advanced();

sdrkit_core::settings! {
    /// The settings of `isdbt.sync` in a narrow-band (ISDB-TSB) receiver.
    pub enum NarrowSetting {
        /// The channel raster.
        Width = WIDTH,
        /// The segment count.
        Segments = SEGMENTS,
        /// The upper-edge pilot.
        EdgePilot = EDGE_PILOT,
    }
}

/// The IFFT rate of a 6 MHz channel, 512/63 MHz.
pub const RATE_6: f64 = 512e6 / 63.0;

/// The IFFT rate of a channel `width_mhz` wide.
pub fn ifft_rate(width_mhz: f64) -> f64 {
    RATE_6 * width_mhz / 6.0
}

/// Data carriers per segment.
pub fn data_carriers(mode: u8) -> usize {
    96 << (mode - 1)
}

/// Frame length in seconds (204 symbols; the same for a band of any segment count).
pub fn frame_seconds(mode: u8, guard: f64, width_mhz: f64) -> f64 {
    204.0 * layout::Band::FULL.fft_size(mode) as f64 * (1.0 + guard) / ifft_rate(width_mhz)
}

/// Transport packets per frame and segment of a layer: data cells × bits × code rate / (204 × 8). Standard: ARIB STD-B31 table
/// 3-3.
pub fn packets_per_segment(mode: u8, p: &LayerParams) -> usize {
    let (num, den) = p.rate();
    (data_carriers(mode) * 204 * p.bits() as usize * num as usize / den as usize) / 1632
}

/// Net bit rate of a layer, bit/s.
pub fn layer_rate(mode: u8, guard: f64, width_mhz: f64, p: &LayerParams) -> f64 {
    (packets_per_segment(mode, p) * p.segments as usize) as f64 * 188.0 * 8.0
        / frame_seconds(mode, guard, width_mhz)
}

/// The guard fractions.
pub const GUARDS: [f64; 4] = [0.25, 0.125, 0.0625, 0.03125];

sdrkit_core::choice_enum! {
    /// The guard interval.
    pub enum Guard {
        /// 1/4.
        G4 = ("1/4", "1/4"),
        /// 1/8.
        G8 = ("1/8", "1/8"),
        /// 1/16.
        G16 = ("1/16", "1/16"),
        /// 1/32.
        G32 = ("1/32", "1/32"),
    }
}

impl Guard {
    /// The guard of a fraction of [`GUARDS`] (1/32 for any other value).
    pub fn of(fraction: f64) -> Guard {
        match GUARDS.iter().position(|&g| g == fraction) {
            Some(0) => Guard::G4,
            Some(1) => Guard::G8,
            Some(2) => Guard::G16,
            _ => Guard::G32,
        }
    }

    /// The fraction as people write it ("1/8").
    pub fn text(self) -> &'static str {
        match self {
            Guard::G4 => "1/4",
            Guard::G8 => "1/8",
            Guard::G16 => "1/16",
            Guard::G32 => "1/32",
        }
    }
}

/// The ISDB-T receiver graph for a width, with the transport stream layer after it.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(width: Width, catalog: &Arc<Catalog>) -> Result<sdrkit_graph::Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("dc", sdrkit_graph::components::DcRemoval::default())
        .add("sync", graph::SyncComp::new(width))
        .add("equalise", graph::EqComp::default())
        .add("deinterleave", graph::DeintComp::default())
        .add("fec", graph::FecComp::default())
        .add("merge", graph::MergeComp::default())
        .add("report", graph::Report::new(width))
        .input("dc.in")
        .connect("dc.out", "sync.in")
        .connect("sync.carriers", "equalise.in")
        .connect("equalise.out", "deinterleave.in")
        .connect("deinterleave.out", "fec.in")
        .connect("fec.out", "merge.in")
        .connect("sync.state", "report.sync")
        .connect("equalise.state", "report.eq")
        .connect("fec.state", "report.fec")
        .output("sync.tmcc")
        .status_from("report");
    layer::attach(b, "merge.ts", &TsLayer::basic())
        .catalog(catalog)
        .build()
}

/// The ISDB-T receiver: the graph in a [`StackGraph`] that refuses a rate below the minimum of its width.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let width = cfg.settings.get(&WIDTH);
    StackGraph::of(crate::typed::shape(ID), graph(width, &Catalog::of(cfg)?)?)
        .min_rate(
            width.min_rate(),
            "the sample rate is too low for the ISDB-T channel width",
        )
        .route("width", &["sync.width", "report.width"])?
        .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g = gen::IsdbtGenerator::from_config(cfg)
        .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

/// The ISDB-T stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ISDB-T",
    group: "TV",
    blurb: "ISDB-T (Japan, Brazil and most of South America; 8 MHz channels in the Maldives), 6, 7 or 8 MHz channel, 13 segments",
    tuning: Tuning { min_hz: 90e6, max_hz: 806e6, ..Tuning::anywhere(473e6 + 1e6 / 7.0, 8e6, 6e6, 6e6) },
    submodes: &[],
    settings: IsdbtStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::IsdbT],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ISDBT),
    benchmarks: Some(crate::benchmarks),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_and_rates() {
        let b = layout::Band::FULL;
        assert_eq!(
            (b.carriers(1), b.carriers(2), b.carriers(3)),
            (1405, 2809, 5617)
        );
        assert!((frame_seconds(1, 0.25, 6.0) - 64.26e-3).abs() < 1e-5);
        assert!((frame_seconds(1, 0.125, 6.0) - 57.834e-3).abs() < 1e-5);
        assert!((frame_seconds(2, 0.0625, 6.0) - 109.242e-3).abs() < 1e-5);
        assert!((frame_seconds(3, 0.03125, 6.0) - 212.058e-3).abs() < 1e-5);
        assert!((ifft_rate(7.0) - 9.4815e6).abs() < 100.0);
        assert!((ifft_rate(8.0) - 10.8360e6).abs() < 100.0);
        assert!((frame_seconds(3, 0.125, 8.0) / frame_seconds(3, 0.125, 6.0) - 0.75).abs() < 1e-9);
        let l = |m, r| LayerParams {
            modulation: m,
            code_rate: r,
            interleave: 0,
            segments: 1,
        };
        for (mode, m, r, n) in [
            (1, 0, 0, 12),
            (3, 0, 0, 48),
            (1, 1, 4, 21),
            (2, 1, 3, 40),
            (1, 2, 1, 32),
            (3, 2, 4, 168),
            (1, 3, 0, 36),
            (2, 3, 2, 108),
            (1, 3, 4, 63),
            (3, 3, 4, 252),
        ] {
            assert_eq!(
                packets_per_segment(mode, &l(m, r)),
                n,
                "mode {mode} mod {m} rate {r}"
            );
        }
        let full = LayerParams {
            modulation: 3,
            code_rate: 2,
            interleave: 0,
            segments: 13,
        };
        assert_eq!(packets_per_segment(1, &full) * 13, 702);
        assert!((layer_rate(1, 0.125, 6.0, &full) / 1e6 - 18.255).abs() <= 0.002);
        let q16 = LayerParams {
            modulation: 2,
            code_rate: 0,
            interleave: 0,
            segments: 13,
        };
        assert!((layer_rate(1, 0.25, 6.0, &q16) / 1e6 - 7.302).abs() <= 0.002);
    }
}
