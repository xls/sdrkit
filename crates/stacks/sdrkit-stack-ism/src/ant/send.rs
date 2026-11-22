//! The `send` of the ANT+ stack: one broadcast frame with the ANT+ network marker built by [`super::build`] and modulated by
//! [`super::modulate`], the builder and the modulator of the test signal, on the tuned frequency.

use super::{build, modulate, modulator, ANT_PLUS, BIT_RATE};
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{AntSendArgs, IsmTxEvent, ANT_SEND, ANT_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::Values;

/// The frames of the ANT+ stack.
#[derive(Default)]
pub struct AntFrames;

impl FrameSource for AntFrames {
    const NAME: &'static str = "ant.tx";
    const SEND: &'static Command = &ANT_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &ANT_TX
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: AntSendArgs = ANT_SEND.args(args)?;
        let device = u16::try_from(a.device)
            .map_err(|_| refused(format!("the device number {} is not 0 to 65535", a.device)))?;
        let dtype = u8::try_from(a.device_type)
            .ok()
            .filter(|t| (1..=127).contains(t))
            .ok_or_else(|| refused(format!("the device type {} is not 1 to 127", a.device_type)))?;
        let tx = u8::try_from(a.tx_type)
            .map_err(|_| refused(format!("the transmission type {} is not 0 to 255", a.tx_type)))?;
        let page: [u8; 8] = hex_arg("the payload", a.payload)?
            .try_into()
            .map_err(|_| refused("the payload is not 8 octets"))?;
        let pairing = if a.pairing.unwrap_or(false) { 0x80 } else { 0 };
        let bytes = build(ANT_PLUS, device, dtype | pairing, tx, &page);
        let mut cpm = modulator(at.rate).map_err(|e| refused(e.to_string()))?;
        let mut samples = Vec::new();
        modulate(&mut cpm, &bytes, &mut Vec::new(), &mut samples);
        Ok(Frame {
            samples,
            offset_hz: at.tuned_hz - at.centre_hz,
            ramp_s: 4.0 / BIT_RATE,
            target: format!("{device}/{dtype}/{tx}"),
            text: crate::family::hex(&page),
        })
    }
}
