//! The `send` of the Zigbee stack: an IEEE 802.15.4 MAC frame built by [`crate::mac154::build_header`] (frame version 2003, or
//! 2006 with the auxiliary security header and CCM* of [`super::security`] when secured) and [`super::with_fcs`], and sent as
//! the PPDU of [`super::ppdu_chip_pairs`] in the O-QPSK of [`super::oqpsk`], the builder and the modulator of the test signal,
//! on the tuned frequency.

use super::{oqpsk, ppdu_chip_pairs, security, with_fcs};
use crate::mac154::{build_header, Addr};
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{IsmTxEvent, ZigbeeSendArgs, ZIGBEE_SEND, ZIGBEE_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

sdrkit_core::choice_enum! {
    /// The type of an IEEE 802.15.4 MAC frame that `send` sends.
    #[derive(Default)]
    pub enum ZigbeeFrameType {
        /// A beacon (type 0).
        Beacon = ("beacon", "Beacon"),
        /// A data frame (type 1).
        #[default]
        Data = ("data", "Data"),
        /// A MAC command (type 3).
        Command = ("command", "MAC command"),
    }
}

impl ZigbeeFrameType {
    /// The frame type field.
    pub const fn number(self) -> u8 {
        match self {
            ZigbeeFrameType::Beacon => 0,
            ZigbeeFrameType::Data => 1,
            ZigbeeFrameType::Command => 3,
        }
    }
}

/// The frame type: the argument `frame_type` of `send`.
pub const FRAME_TYPE: Setting<field::Choice<ZigbeeFrameType>> =
    Setting::choice("frame_type", "Frame type", ZigbeeFrameType::Data).help(
        "A beacon carries the source only; data and MAC command frames carry the addresses given.",
    );

/// The largest PSDU, octets.
pub const MAX_PSDU: usize = 127;

/// An address of hex text: empty for none, 4 digits short, 16 digits extended.
fn address(what: &str, text: &str) -> Result<Addr, CommandError> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bad = || refused(format!("the {what} is not empty, 4 or 16 hex digits"));
    match t.len() {
        0 => Ok(Addr::None),
        4 => u16::from_str_radix(&t, 16).map(Addr::Short).map_err(|_| bad()),
        16 => u64::from_str_radix(&t, 16).map(Addr::Ext).map_err(|_| bad()),
        _ => Err(bad()),
    }
}

/// The frames of the Zigbee stack.
#[derive(Default)]
pub struct ZigbeeFrames;

impl FrameSource for ZigbeeFrames {
    const NAME: &'static str = "zb.tx";
    const SEND: &'static Command = &ZIGBEE_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &ZIGBEE_TX
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: ZigbeeSendArgs = ZIGBEE_SEND.args(args)?;
        let pan = a.pan_id.trim();
        let pan = (pan.len() == 4)
            .then(|| u16::from_str_radix(pan, 16).ok())
            .flatten()
            .ok_or_else(|| refused("the PAN id is not 4 hex digits"))?;
        let ty = a.frame_type.unwrap_or_default();
        let dst = address("destination", a.destination)?;
        let src = address("source", a.source)?;
        if ty == ZigbeeFrameType::Beacon && (dst != Addr::None || src == Addr::None) {
            return Err(refused("a beacon has a source address and no destination"));
        }
        if dst == Addr::None && src == Addr::None {
            return Err(refused("the frame needs a destination or a source address"));
        }
        let seq = u8::try_from(a.sequence.unwrap_or(0))
            .map_err(|_| refused("the sequence is not 0 to 255"))?;
        let payload = hex_arg("the payload", a.payload)?;
        let both = dst != Addr::None && src != Addr::None;
        let level = u8::try_from(a.security_level.unwrap_or(0))
            .ok()
            .filter(|&l| l <= 7)
            .ok_or_else(|| refused("the security level is not 0 to 7"))?;
        let mut psdu = build_header(
            ty.number(),
            a.ack.unwrap_or(false),
            both,
            u8::from(level > 0),
            seq,
            (dst != Addr::None).then_some(pan),
            dst,
            (src != Addr::None && !both).then_some(pan),
            src,
            level > 0,
        );
        if level == 0 {
            psdu.extend_from_slice(&payload);
        } else {
            let Addr::Ext(eui) = src else {
                return Err(refused("a secured frame needs an extended source address for its nonce"));
            };
            let key = security::key_of(a.key.unwrap_or(""))
                .ok_or_else(|| refused("the key is not 32 hex digits"))?;
            let counter = u32::try_from(a.frame_counter.unwrap_or(0))
                .map_err(|_| refused("the frame counter is not 0 to 4294967295"))?;
            let mode = u8::try_from(a.key_id_mode.unwrap_or(1))
                .ok()
                .filter(|&m| m <= 3)
                .ok_or_else(|| refused("the key identifier mode is not 0 to 3"))?;
            let index = u8::try_from(a.key_index.unwrap_or(1))
                .ok()
                .filter(|&i| i >= 1)
                .ok_or_else(|| refused("the key index is not 1 to 255"))?;
            let source = hex_arg("the key source", a.key_source.unwrap_or(""))?;
            let want = [0, 0, 4, 8][usize::from(mode)];
            if source.len() != want {
                return Err(refused(format!(
                    "the key source is {} octets; key identifier mode {mode} takes {want}",
                    source.len()
                )));
            }
            psdu.extend(security::aux_header(level, mode, counter, &source, index));
            let secured = security::protect(&key, eui, counter, level, ty.number(), &psdu, &payload)
                .ok_or_else(|| refused("the payload is too long to secure"))?;
            psdu.extend(secured);
        }
        if psdu.len() + 2 > MAX_PSDU {
            return Err(refused(format!(
                "the frame is {} octets with its FCS: at most {MAX_PSDU}",
                psdu.len() + 2
            )));
        }
        let psdu = with_fcs(psdu);
        let samples = oqpsk(&ppdu_chip_pairs(&psdu, false, false), at.rate)
            .map_err(|e| refused(e.to_string()))?;
        let target = format!("{} > {}", src.text(), dst.text());
        let text = super::describe(&psdu[..psdu.len() - 2]).map_or_else(String::new, |d| d.text);
        Ok(Frame {
            samples,
            offset_hz: at.tuned_hz - at.centre_hz,
            ramp_s: 4e-6,
            target,
            text,
        })
    }
}
