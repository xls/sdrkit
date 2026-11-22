//! The `send` of the Wi-SUN stack: an IEEE 802.15.4 frame of version 2015 built as the test signal builds its frames
//! ([`super::ext_hdr`], the UTT-IE, the header termination IE and the payload IEs) and sent by [`super::frame_samples`]
//! (whitened, 4-octet FCS, 2-GFSK) on the channel of the plan (the settings `ranges` and `classes`) nearest the tuned
//! frequency.

use super::frame::{channels, hie};
use super::{ext_hdr, frame_samples, utt_ie, FrameOptions, PlanSetting, PlanTexts};
use crate::mac154::Addr;
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{IsmTxEvent, WisunSendArgs, WISUN_SEND, WISUN_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{Choice as _, Setting, SettingError, SettingId, SettingSet as _, Value, Values};

sdrkit_core::choice_enum! {
    /// The frame type of the UTT-IE of a frame that `send` sends.
    #[derive(Default)]
    pub enum WisunFrameType {
        /// PAN Advert (0).
        Pa = ("pa", "PAN Advert"),
        /// PAN Advert Solicit (1).
        Pas = ("pas", "PAN Advert Solicit"),
        /// PAN Config (2).
        Pc = ("pc", "PAN Config"),
        /// PAN Config Solicit (3).
        Pcs = ("pcs", "PAN Config Solicit"),
        /// Data (4).
        #[default]
        Data = ("data", "Data"),
        /// Ack (5).
        Ack = ("ack", "Ack"),
        /// EAPOL (6).
        Eapol = ("eapol", "EAPOL"),
    }
}

/// The frame type: the argument `frame_type` of `send`.
pub const FRAME_TYPE: Setting<field::Choice<WisunFrameType>> =
    Setting::choice("frame_type", "Frame type", WisunFrameType::Data)
        .help("The frame type of the UTT-IE.");

/// The largest PSDU with its FCS, octets.
pub const MAX_PSDU: usize = 2047;

/// The frames of the Wi-SUN stack: the channel plan of the settings `ranges` and `classes`.
#[derive(Default)]
pub struct WisunFrames {
    plan: PlanTexts,
}

/// A 16-digit EUI-64 of hex text.
fn eui(what: &str, text: &str) -> Result<u64, CommandError> {
    let t = text.trim();
    (t.len() == 16)
        .then(|| u64::from_str_radix(t, 16).ok())
        .flatten()
        .ok_or_else(|| refused(format!("the {what} is not 16 hex digits")))
}

impl FrameSource for WisunFrames {
    const NAME: &'static str = "ws.tx";
    const SEND: &'static Command = &WISUN_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &WISUN_TX
    }
    fn settings() -> &'static [&'static dyn DeclareSetting] {
        PlanSetting::SETTINGS
    }
    fn apply(&mut self, key: &str, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let s = match key {
            k if k == super::RANGES.key() => PlanSetting::Ranges,
            k if k == super::CLASSES.key() => PlanSetting::Classes,
            _ => return Err(SettingError::NoSuchId { id }),
        };
        self.plan.apply(s, id, v)
    }
    fn value(&self, key: &str) -> Option<Value> {
        match key {
            k if k == super::RANGES.key() => Some(Value::Text(self.plan.ranges.clone())),
            k if k == super::CLASSES.key() => Some(Value::Text(self.plan.classes.clone())),
            _ => None,
        }
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: WisunSendArgs = WISUN_SEND.args(args)?;
        let pan = a.pan_id.trim();
        let pan = (pan.len() == 4)
            .then(|| u16::from_str_radix(pan, 16).ok())
            .flatten()
            .ok_or_else(|| refused("the PAN id is not 4 hex digits"))?;
        let src = eui("source", a.source)?;
        let dst = match a.destination.trim() {
            "" => Addr::None,
            t if t.eq_ignore_ascii_case("FFFF") => Addr::Short(0xFFFF),
            t => Addr::Ext(eui("destination", t)?),
        };
        let seq = u8::try_from(a.sequence.unwrap_or(0))
            .map_err(|_| refused("the sequence is not 0 to 255"))?;
        let header_ies = hex_arg("the header IEs", a.header_ies.unwrap_or(""))?;
        let payload = hex_arg("the payload IEs", a.payload)?;
        let broadcast = dst == Addr::None;
        let ty = a.frame_type.unwrap_or_default().index() as u8;
        let mut psdu = ext_hdr(
            1,
            (!broadcast).then_some(seq),
            dst,
            src,
            broadcast.then_some(pan),
            false,
            false,
        );
        let ufsi = u32::try_from(a.ufsi.unwrap_or(0))
            .ok()
            .filter(|&u| u < 1 << 24)
            .ok_or_else(|| refused("the UFSI is not 0 to 16777215"))?;
        psdu.extend(utt_ie(ty, ufsi));
        psdu.extend_from_slice(&header_ies);
        psdu.extend(hie(if payload.is_empty() { 0x7F } else { 0x7E }, &[]));
        psdu.extend_from_slice(&payload);
        if psdu.len() + 4 > MAX_PSDU {
            return Err(refused(format!(
                "the frame is {} octets with its FCS: at most {MAX_PSDU}",
                psdu.len() + 4
            )));
        }
        let plan = self.plan.in_use(at.centre_hz);
        let lim = (0.45 * at.rate - 200e3).max(0.0);
        let (hz, modes) = channels(&plan, at.centre_hz, lim)
            .into_iter()
            .min_by(|x, y| (x.0 - at.tuned_hz).abs().total_cmp(&(y.0 - at.tuned_hz).abs()))
            .ok_or_else(|| refused("no channel of the plan is in the samples"))?;
        let want = a.kbps.unwrap_or(0);
        let mode = if want == 0 {
            modes.iter().min_by_key(|m| m.0).copied()
        } else {
            modes.iter().find(|m| i64::from(m.0) == want).copied()
        }
        .ok_or_else(|| refused(format!("the channel {hz} Hz has no mode of {want} kbit/s")))?;
        let kbps = f64::from(mode.0);
        let samples = frame_samples(&psdu, kbps, f64::from(mode.1), at.rate, FrameOptions::default())
            .map_err(|e| refused(e.to_string()))?;
        let (name, _) = super::frame::utt_name(ty);
        Ok(Frame {
            samples,
            offset_hz: hz - at.centre_hz,
            ramp_s: 1.0 / (kbps * 1e3),
            target: format!("{} > {}", Addr::Ext(src).text(), dst.text()),
            text: format!("{name}, {} kbit/s, {} octets", mode.0, psdu.len() + 4),
        })
    }
}
