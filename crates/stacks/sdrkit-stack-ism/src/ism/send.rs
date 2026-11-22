//! The `send` of the ISM sensor stack: one transmission of a device family whose frames the test signal builds, its train
//! from [`super::gen`] keyed as OOK by [`super::gen::train_samples`] on the tuned frequency. The families: a PT2262 /
//! EV1527 remote and the LaCrosse TX141TH-Bv2 thermometer and hygrometer.

use super::gen::{ev1527_train, train_samples, tx141th_train};
use crate::tx::{refused, Frame, FrameSource, Place};
use crate::typed::{IsmSendArgs, IsmTxEvent, ISM_SEND, ISM_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

sdrkit_core::choice_enum! {
    /// A device family that `send` sends.
    #[derive(Default)]
    pub enum IsmFamily {
        /// A PT2262 / EV1527 remote: a 24-bit code, 6 copies.
        #[default]
        Remote = ("remote", "Remote (PT2262 / EV1527)"),
        /// The LaCrosse TX141TH-Bv2 thermometer and hygrometer: 12 copies of 40 bits.
        Tx141thBv2 = ("tx141thbv2", "LaCrosse TX141TH-Bv2"),
    }
}

/// The device family: the argument `family` of `send`.
pub const FAMILY: Setting<field::Choice<IsmFamily>> =
    Setting::choice("family", "Device family", IsmFamily::Remote)
        .help("The family whose frames are sent; each reads only its own arguments.");

/// The frames of the ISM sensor stack.
#[derive(Default)]
pub struct IsmFrames;

impl FrameSource for IsmFrames {
    const NAME: &'static str = "ism.tx";
    const SEND: &'static Command = &ISM_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &ISM_TX
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: IsmSendArgs = ISM_SEND.args(args)?;
        let (train, target, text) = match a.family.unwrap_or_default() {
            IsmFamily::Remote => {
                let id = u16::try_from(a.id)
                    .ok()
                    .filter(|&i| i != 0)
                    .ok_or_else(|| refused(format!("the id {} is not 1 to 65535", a.id)))?;
                let command = u8::try_from(a.command.unwrap_or(1))
                    .ok()
                    .filter(|&c| c != 0)
                    .ok_or_else(|| refused("the command is not 1 to 255"))?;
                let [hi, lo] = id.to_be_bytes();
                (
                    ev1527_train([hi, lo, command]),
                    format!("Generic-Remote {id}"),
                    format!("command {command}"),
                )
            }
            IsmFamily::Tx141thBv2 => {
                let id = u8::try_from(a.id)
                    .ok()
                    .filter(|&i| i != 0)
                    .ok_or_else(|| refused(format!("the id {} is not 1 to 255", a.id)))?;
                let channel = u8::try_from(a.channel.unwrap_or(1))
                    .ok()
                    .filter(|&c| c <= 3)
                    .ok_or_else(|| refused("the channel is not 0 to 3"))?;
                let t = a.temperature.unwrap_or(20.0);
                if !(-40.0..=140.0).contains(&t) {
                    return Err(refused(format!("the temperature {t} °C is not -40 to 140")));
                }
                let humidity = u8::try_from(a.humidity.unwrap_or(50))
                    .ok()
                    .filter(|h| (1..=100).contains(h))
                    .ok_or_else(|| refused("the humidity is not 1 to 100 %"))?;
                let raw = ((t * 10.0).round() + 500.0) as u16;
                (
                    tx141th_train(id, channel, a.battery_ok.unwrap_or(true), raw, humidity),
                    format!("LaCrosse-TX141THBv2 {id:02x}"),
                    format!("{t:.1} °C, {humidity} %"),
                )
            }
        };
        Ok(Frame {
            samples: train_samples(&train, at.rate, 0.0),
            offset_hz: at.tuned_hz - at.centre_hz,
            ramp_s: 3e-6,
            target,
            text,
        })
    }
}
