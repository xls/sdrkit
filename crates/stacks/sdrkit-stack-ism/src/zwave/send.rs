//! The `send` of the Z-Wave stack: a singlecast MPDU (ITU-T G.9959, section 6.5 of the stack) built by
//! [`super::frame::build`] and modulated by [`super::frame_samples`], the builder and the modulator of the test signal, on
//! the first channel of its rate in the setting `channels`.

use super::{channels_or_centre, frame, frame_samples, parse_channels, CHANNELS};
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{IsmTxEvent, ZwaveSendArgs, ZWAVE_SEND, ZWAVE_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{Setting, SettingError, SettingId, Value, Values};

sdrkit_core::choice_enum! {
    /// The rate of a Z-Wave frame.
    #[derive(Default)]
    pub enum ZwaveRate {
        /// 9.6 kbit/s, Manchester.
        R1 = ("r1", "R1 (9.6 kbit/s)"),
        /// 40 kbit/s.
        #[default]
        R2 = ("r2", "R2 (40 kbit/s)"),
        /// 100 kbit/s, GFSK.
        R3 = ("r3", "R3 (100 kbit/s)"),
    }
}

impl ZwaveRate {
    /// The rate number (1 to 3).
    pub const fn number(self) -> u8 {
        match self {
            ZwaveRate::R1 => 1,
            ZwaveRate::R2 => 2,
            ZwaveRate::R3 => 3,
        }
    }
}

/// The rate of the frame: the argument `rate` of `send`.
pub const RATE: Setting<field::Choice<ZwaveRate>> = Setting::choice("rate", "Rate", ZwaveRate::R2)
    .help("R1 is 9.6 kbit/s Manchester, R2 40 kbit/s, R3 100 kbit/s GFSK.");

/// The largest MPDU at R1 and R2, octets.
pub const MAX_MPDU_R12: usize = 64;
/// The largest MPDU at R3, octets.
pub const MAX_MPDU_R3: usize = 170;

/// The frames of the Z-Wave stack: the channels of the setting `channels`.
#[derive(Default)]
pub struct ZwaveFrames {
    text: String,
    channels: Vec<(f64, u8)>,
}

impl FrameSource for ZwaveFrames {
    const NAME: &'static str = "zw.tx";
    const SEND: &'static Command = &ZWAVE_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &ZWAVE_TX
    }
    fn settings() -> &'static [&'static dyn DeclareSetting] {
        &[&CHANNELS]
    }
    fn apply(&mut self, key: &str, id: SettingId, v: &Value) -> Result<(), SettingError> {
        if key != CHANNELS.key() {
            return Err(SettingError::NoSuchId { id });
        }
        let t = CHANNELS.read(id, v)?;
        self.channels = parse_channels(t).ok_or(SettingError::Malformed { id, at: None })?;
        t.clone_into(&mut self.text);
        Ok(())
    }
    fn value(&self, key: &str) -> Option<Value> {
        (key == CHANNELS.key()).then(|| Value::Text(self.text.clone()))
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: ZwaveSendArgs = ZWAVE_SEND.args(args)?;
        let home = a.home_id.trim();
        let home = (home.len() == 8)
            .then(|| u32::from_str_radix(home, 16).ok())
            .flatten()
            .ok_or_else(|| refused("the home id is not 8 hex digits"))?;
        let src = u8::try_from(a.source)
            .ok()
            .filter(|s| (1..=232).contains(s))
            .ok_or_else(|| refused(format!("the source node {} is not 1 to 232", a.source)))?;
        let dst = u8::try_from(a.destination)
            .ok()
            .filter(|d| (1..=232).contains(d) || *d == 255)
            .ok_or_else(|| {
                refused(format!(
                    "the destination node {} is not 1 to 232 or 255",
                    a.destination
                ))
            })?;
        let payload = hex_arg("the command", a.payload)?;
        let r = a.rate.unwrap_or_default().number();
        let (max, ck) = if r == 3 {
            (MAX_MPDU_R3, 2)
        } else {
            (MAX_MPDU_R12, 1)
        };
        let room = max - 9 - ck;
        if payload.is_empty() || payload.len() > room {
            return Err(refused(format!(
                "the command is {} octets: 1 to {room} at R{r}",
                payload.len()
            )));
        }
        let seq = a.sequence.unwrap_or(0);
        if !(0..=15).contains(&seq) {
            return Err(refused(format!("the sequence {seq} is not 0 to 15")));
        }
        let fc1 = 0x01 | if a.ack.unwrap_or(false) { 0x40 } else { 0 };
        let mpdu = frame::build(home, src, fc1, seq as u8, &[dst], &payload, r);
        let list = channels_or_centre(&self.channels, at.centre_hz);
        let hz = list
            .iter()
            .find(|c| c.1 == r)
            .map(|c| c.0)
            .ok_or_else(|| refused(format!("the setting channels has no channel of R{r}")))?;
        let offset_hz = hz - at.centre_hz;
        if offset_hz.abs() >= 0.45 * at.rate {
            return Err(refused(format!(
                "the channel {hz} Hz of R{r} is outside the samples"
            )));
        }
        let chip_rate = [19_200.0, 40_000.0, 100_000.0][usize::from(r - 1)];
        let text = frame::parse(&mpdu, r).map_or_else(String::new, |m| frame::describe(&m));
        Ok(Frame {
            samples: frame_samples(r, &mpdu, at.rate),
            offset_hz,
            ramp_s: 4.0 / chip_rate,
            target: format!("{home:08X}:{dst}"),
            text,
        })
    }
}
