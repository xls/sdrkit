//! The instrument decoder components: MSU-MR (Meteor), AVHRR packets (MetOp), NOAA HRPT minor frames (with their TIP minor
//! frames) and the HIRS elements of TIP minor frames, each to picture strips.

use super::link::{control_port, item_format, item_port};
use super::{controls, Control};
use crate::instruments::avhrr::{decode_noaa, AvhrrDecoder, AvhrrStats, NoaaFramer};
use crate::instruments::msumr::{MsuMr, MsuStats};
use crate::instruments::tip::{
    hrpt_minor_frame_number, tip_from_hrpt, HirsDecoder, HirsStats, TIP_BYTES,
};
use crate::items::SpacePacket;
use crate::psk::Modulation;
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::command::Command;
use sdrkit_core::ConfigError;
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Item, Port, Timed};

/// The `clear` command of an instrument decoder: a new picture.
pub const CLEAR: Command = Command::new("clear", "New picture").help("Starts a new picture.");

sdrkit_core::commands! {
    /// The commands of every instrument decoder.
    pub enum InstrumentCommand {
        /// Starts a new picture.
        Clear = CLEAR,
    }
}

/// Moves the strips of `from` to the strip output `o`.
fn send_strips(io: &mut Io, o: usize, from: &mut Vec<Strip>) {
    io.items_out::<Strip>(o)
        .extend(from.drain(..).map(|s| Timed {
            time: 0.0,
            value: s,
        }));
}

/// The parts of [`Component`] that every instrument decoder shares: no settings and the `clear` command. The decoder is the
/// field `dec`, with a `new_picture` method.
macro_rules! instrument_owner {
    () => {
        fn apply(
            &mut self,
            id: sdrkit_core::SettingId,
            _: &sdrkit_core::Value,
        ) -> Result<sdrkit_graph::Applied, sdrkit_core::SettingError> {
            Err(sdrkit_core::SettingError::NoSuchId { id })
        }
        fn values(&self) -> sdrkit_core::Values {
            sdrkit_graph::no_values()
        }
        fn commands(&self) -> &sdrkit_core::command::Commands {
            <$crate::components::instruments::InstrumentCommand as sdrkit_core::command::CommandSet>::commands()
        }
        fn invoke(
            &mut self,
            id: sdrkit_core::command::CommandId,
            _: &sdrkit_core::Values,
        ) -> Result<Option<sdrkit_core::Record>, sdrkit_core::command::CommandError> {
            use sdrkit_core::command::CommandSet as _;
            match $crate::components::instruments::InstrumentCommand::from_id(id)
                .ok_or(sdrkit_core::command::CommandError::NoSuchId { id })?
            {
                $crate::components::instruments::InstrumentCommand::Clear => self.dec.new_picture(),
            }
            Ok(None)
        }
    };
}
pub(crate) use instrument_owner;

/// The MSU-MR decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MsuReport {
    /// Counters.
    pub stats: MsuStats,
    /// The learnt row time, ms.
    pub row_time_ms: Option<f64>,
}
impl Item for MsuReport {
    const KIND: &'static str = "satimg.msumr_report";
}

/// Meteor MSU-MR image packets to strips.
#[derive(Default)]
pub struct MsuMrComponent {
    dec: MsuMr,
    strips: Vec<Strip>,
}

impl Component for MsuMrComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.msumr",
            vec![Port::typed::<SpacePacket>("in")],
            vec![item_port::<Strip>("out"), item_port::<MsuReport>("report")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Strip>(), item_format::<MsuReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<SpacePacket>(0) {
            self.dec.push(&p.value, &mut self.strips);
        }
        send_strips(io, 0, &mut self.strips);
        if io.tick() {
            let r = MsuReport {
                stats: self.dec.stats.clone(),
                row_time_ms: self.dec.placer().row_time_ms(),
            };
            io.items_out::<MsuReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

/// The AVHRR decoders' report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AvhrrReport {
    /// Counters.
    pub stats: AvhrrStats,
    /// NOAA: the framer holds the lock.
    pub noaa_locked: bool,
    /// NOAA: polarity + 2 x chip phase.
    pub noaa_rotation: u8,
}
impl Item for AvhrrReport {
    const KIND: &'static str = "satimg.avhrr_report";
}

/// MetOp AVHRR packets (APID 103 and 104) to strips.
#[derive(Default)]
pub struct AvhrrComponent {
    dec: AvhrrDecoder,
    strips: Vec<Strip>,
}

impl Component for AvhrrComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.avhrr",
            vec![Port::typed::<SpacePacket>("in")],
            vec![
                item_port::<Strip>("out"),
                item_port::<AvhrrReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Strip>(), item_format::<AvhrrReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<SpacePacket>(0) {
            self.dec.push_packet(&p.value, &mut self.strips);
        }
        send_strips(io, 0, &mut self.strips);
        if io.tick() {
            let r = AvhrrReport {
                stats: self.dec.stats.clone(),
                ..Default::default()
            };
            io.items_out::<AvhrrReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

/// One TIP minor frame of 104 bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct TipFrame(pub [u8; TIP_BYTES]);
impl Item for TipFrame {
    const KIND: &'static str = "satimg.tip_frame";
}

/// The HIRS decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HirsReport {
    /// Counters.
    pub stats: HirsStats,
}
impl Item for HirsReport {
    const KIND: &'static str = "satimg.hirs_report";
}

/// TIP minor frames to HIRS strips: 20 channels, 56 Earth views a line ([`HirsDecoder`]).
#[derive(Default)]
pub struct HirsComponent {
    dec: HirsDecoder,
    strips: Vec<Strip>,
}

impl Component for HirsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.hirs",
            vec![Port::typed::<TipFrame>("in")],
            vec![item_port::<Strip>("out"), item_port::<HirsReport>("report")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Strip>(), item_format::<HirsReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for f in io.items::<TipFrame>(0) {
            self.dec.push_frame(&f.value.0, &mut self.strips);
        }
        send_strips(io, 0, &mut self.strips);
        if io.tick() {
            let r = HirsReport {
                stats: self.dec.stats,
            };
            io.items_out::<HirsReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

/// NOAA HRPT: soft split-phase chips to minor frames to strips. The minor frames of number 1 also give their five TIP minor
/// frames.
#[derive(Default)]
pub struct NoaaComponent {
    on: bool,
    framer: NoaaFramer,
    dec: AvhrrDecoder,
    frames: Vec<Vec<u16>>,
    strips: Vec<Strip>,
}

impl NoaaComponent {
    /// A decoder that starts on (the NOAA BPSK link) or off.
    pub fn new(on: bool) -> Self {
        NoaaComponent {
            on,
            ..NoaaComponent::default()
        }
    }
}

impl Component for NoaaComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.noaa_hrpt",
            vec![Port::llr("in"), control_port()],
            vec![
                item_port::<Strip>("out"),
                item_port::<AvhrrReport>("report"),
                item_port::<TipFrame>("tip"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.framer.reset();
        Ok(vec![
            item_format::<Strip>(),
            item_format::<AvhrrReport>(),
            item_format::<TipFrame>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            if let Control::Link(l) = c {
                let on = l.modulation == Modulation::Bpsk;
                if on != self.on {
                    self.on = on;
                    self.framer.reset();
                }
            }
        }
        self.frames.clear();
        if self.on {
            self.framer.process(io.llrs(0), &mut self.frames);
        }
        for f in &self.frames {
            self.dec.stats.noaa_frames += 1;
            if let Some(l) = decode_noaa(f) {
                self.dec.line(&l, &mut self.strips);
            }
            if f.len() > 6 && hrpt_minor_frame_number(f[6]) == 1 {
                if let Some(tip) = tip_from_hrpt(f) {
                    io.items_out::<TipFrame>(2)
                        .extend(tip.into_iter().map(|t| Timed {
                            time: 0.0,
                            value: TipFrame(t),
                        }));
                }
            }
        }
        send_strips(io, 0, &mut self.strips);
        if io.tick() {
            let r = AvhrrReport {
                stats: self.dec.stats.clone(),
                noaa_locked: self.framer.locked(),
                noaa_rotation: self.framer.rotation,
            };
            io.items_out::<AvhrrReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}
