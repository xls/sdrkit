//! The `send` of the Wireless M-Bus stack: a link-layer frame (EN 13757-4) built by [`super::link_header`] and
//! [`super::set_length`] and modulated by [`super::frame_samples`], the builder and the modulator of the test signal, on the
//! channel of its mode in the plan of the stack ([`super::rx::plan`]) nearest the tuned frequency.

use super::link::manufacturer_code;
use super::{frame_samples, link_header, phy_of, rx, set_length};
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{IsmTxEvent, WmbusSendArgs, WMBUS_SEND, WMBUS_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

sdrkit_core::choice_enum! {
    /// The physical mode and the frame format of a frame that `send` sends.
    #[derive(Default)]
    pub enum WmbusMode {
        /// S1, Manchester at 32 768 chip/s, format A.
        S1 = ("S1", "S1"),
        /// T1, 3 of 6 at 100 kchip/s, format A.
        #[default]
        T1 = ("T1", "T1"),
        /// C1 with frame format A.
        C1a = ("C1A", "C1 format A"),
        /// C1 with frame format B.
        C1b = ("C1B", "C1 format B"),
        /// R2, Manchester at 4800 chip/s, format A.
        R2 = ("R2", "R2"),
        /// N1 at 169 MHz, NRZ at 4800 bit/s, format A.
        N1 = ("N1", "N1"),
    }
}

impl WmbusMode {
    /// The name of the mode in the frames table.
    pub const fn name(self) -> &'static str {
        match self {
            WmbusMode::S1 => "S1",
            WmbusMode::T1 => "T1",
            WmbusMode::C1a => "C1 (A)",
            WmbusMode::C1b => "C1 (B)",
            WmbusMode::R2 => "R2",
            WmbusMode::N1 => "N1",
        }
    }
}

/// The mode: the argument `mode` of `send`.
pub const MODE: Setting<field::Choice<WmbusMode>> = Setting::choice("mode", "Mode", WmbusMode::T1)
    .help("S1, T1, C1 in frame format A or B and R2 on 868 MHz; N1 on 169 MHz.");

/// The frames of the Wireless M-Bus stack.
#[derive(Default)]
pub struct WmbusFrames;

impl FrameSource for WmbusFrames {
    const NAME: &'static str = "wmbus.tx";
    const SEND: &'static Command = &WMBUS_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &WMBUS_TX
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: WmbusSendArgs = WMBUS_SEND.args(args)?;
        let mode = a.mode.unwrap_or_default();
        let maker = a.manufacturer.trim();
        if maker.len() != 3 || !maker.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(refused("the manufacturer is not three letters A to Z"));
        }
        let id = a.address.trim();
        let id = (id.len() == 8)
            .then(|| u32::from_str_radix(id, 16).ok())
            .flatten()
            .ok_or_else(|| refused("the address is not 8 hex digits"))?;
        let octet = |what: &str, v: i64| {
            u8::try_from(v).map_err(|_| refused(format!("the {what} {v} is not 0 to 255")))
        };
        let version = octet("version", a.version)?;
        let dev_type = octet("device type", a.device_type)?;
        let payload = hex_arg("the payload", a.payload)?;
        let name = mode.name();
        let (phy, fmt) = phy_of(name);
        let mut d = link_header(manufacturer_code(maker), id, version, dev_type);
        d.extend_from_slice(&payload);
        let most = if mode == WmbusMode::C1b { 252 } else { 256 };
        if payload.is_empty() || d.len() > most {
            return Err(refused(format!(
                "the payload is {} octets: 1 to {} in this mode",
                payload.len(),
                most - 10
            )));
        }
        set_length(&mut d, fmt);
        let hz = rx::plan(at.centre_hz, at.rate)
            .into_iter()
            .filter(|c| c.phy == phy)
            .map(|c| c.hz)
            .min_by(|x, y| (x - at.tuned_hz).abs().total_cmp(&(y - at.tuned_hz).abs()))
            .ok_or_else(|| refused(format!("no channel of {name} is in the samples")))?;
        Ok(Frame {
            samples: frame_samples(name, &d, at.rate),
            offset_hz: hz - at.centre_hz,
            ramp_s: 4.0 / phy.chip_rate(),
            target: format!("{maker} {id:08X}"),
            text: format!("{name}, {} octets", d.len()),
        })
    }
}
