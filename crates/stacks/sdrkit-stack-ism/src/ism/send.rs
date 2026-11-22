//! The `send` of the ISM sensor stack: one transmission of a device family, its train built by [`super::encode`] and keyed
//! by [`super::gen::train_samples`] (OOK or FSK by the family) on the tuned frequency. A family with fields takes the values
//! the stack reports and refuses a frame that its protocol does not read back; a family with a raw payload takes the octets
//! as its protocol reads them and adds the checks.

use super::encode as enc;
use super::gen::train_samples;
use crate::tx::{hex_arg, refused, Frame, FrameSource, Place};
use crate::typed::{IsmSendArgs, IsmTxEvent, ISM_SEND, ISM_TX};
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::EventKind;
use sdrkit_core::publish::field;
use sdrkit_core::{Choice as _, Setting, Values};
use sdrkit_proto_ookdev::{Reading, Train};

sdrkit_core::choice_enum! {
    /// A device family that `send` sends.
    #[derive(Default)]
    pub enum IsmFamily {
        /// A PT2262 / EV1527 remote: a 24-bit code, 6 copies.
        #[default]
        Remote = ("remote", "Remote (PT2262 / EV1527)"),
        /// The LaCrosse TX141TH-Bv2 thermometer and hygrometer: 12 copies of 40 bits.
        Tx141thBv2 = ("tx141thbv2", "LaCrosse TX141TH-Bv2"),
        /// Nexus temperature (and humidity) sensor.
        Nexus = ("nexus", "Nexus-TH / Nexus-T"),
        /// Acurite 592TXR tower sensor.
        AcuriteTower = ("acurite_tower", "Acurite 592TXR tower"),
        /// Fine Offset WH1080 weather station.
        Wh1080 = ("wh1080", "Fine Offset WH1080"),
        /// Ambient Weather F007TH.
        F007th = ("f007th", "Ambient Weather F007TH"),
        /// Oregon Scientific THGR810 (v3).
        OregonThgr810 = ("oregon_thgr810", "Oregon Scientific THGR810"),
        /// Oregon Scientific THGR228N (v2.1).
        OregonThgr228n = ("oregon_thgr228n", "Oregon Scientific THGR228N"),
        /// Schrader TPMS (FCC MRXGG4).
        Schrader = ("schrader", "Schrader TPMS"),
        /// Toyota TPMS (PMV-C210).
        Toyota = ("toyota", "Toyota TPMS"),
        /// Renault TPMS.
        Renault = ("renault", "Renault TPMS"),
        /// Ford TPMS (VDO).
        Ford = ("ford", "Ford TPMS"),
        /// Efergy e2 classic current clamp.
        Efergy = ("efergy", "Efergy e2"),
        /// CurrentCost TX power message.
        CurrentCost = ("currentcost", "CurrentCost TX"),
        /// Bresser 6-in-1 weather sensor.
        Bresser6 = ("bresser6", "Bresser 6-in-1"),
        /// Bresser 5-in-1 weather sensor.
        Bresser5 = ("bresser5", "Bresser 5-in-1"),
        /// LaCrosse TX29-IT.
        Tx29 = ("tx29", "LaCrosse TX29-IT"),
        /// LaCrosse TX35DTH-IT.
        Tx35 = ("tx35", "LaCrosse TX35DTH-IT"),
        /// Fine Offset WH24.
        Wh24 = ("wh24", "Fine Offset WH24"),
        /// Prologue temperature and humidity sensor.
        Prologue = ("prologue", "Prologue"),
        /// The Acurite tower family with a raw message.
        AcuriteTxr = ("acurite_txr", "Acurite tower family (raw)"),
        /// Bresser 3CH with a raw message.
        Bresser3ch = ("bresser3ch", "Bresser 3CH (raw)"),
        /// Citroen TPMS (VDO) with a raw message.
        Citroen = ("citroen", "Citroen TPMS (raw)"),
        /// Schrader EG53MA4 TPMS with a raw row.
        SchraderEg53 = ("schrader_eg53", "Schrader EG53MA4 (raw)"),
        /// Bresser 7-in-1 with a raw message.
        Bresser7 = ("bresser7", "Bresser 7-in-1 (raw)"),
    }
}

/// The device family: the argument `family` of `send`.
pub const FAMILY: Setting<field::Choice<IsmFamily>> =
    Setting::choice("family", "Device family", IsmFamily::Remote)
        .help("The family whose frames are sent; each reads only its own arguments.");

/// The frames of the ISM sensor stack.
#[derive(Default)]
pub struct IsmFrames;

/// `v` as an integer from `lo` to `hi`, or a refusal that names `what`.
fn int(what: &str, v: i64, lo: i64, hi: i64) -> Result<i64, CommandError> {
    if (lo..=hi).contains(&v) {
        Ok(v)
    } else {
        Err(refused(format!("the {what} {v} is not {lo} to {hi} for this family")))
    }
}

/// `v` in units of `step` plus `offset` units, rounded, as an integer from `lo` to `hi`, or a refusal that names `what`.
fn scaled(what: &str, v: f64, step: f64, offset: f64, lo: i64, hi: i64) -> Result<i64, CommandError> {
    let r = (v / step + offset).round();
    if r.is_finite() && r >= lo as f64 && r <= hi as f64 {
        Ok(r as i64)
    } else {
        Err(refused(format!("the {what} {v} is outside what this family sends")))
    }
}

/// The number of a pressure in PSI of `kpa`.
fn psi(kpa: f64) -> f64 {
    kpa / 6.894_757
}

/// The index of the 16 directions from north nearest `deg`.
fn dir16(deg: f64) -> u8 {
    ((deg / 22.5).round() as i64).rem_euclid(16) as u8
}

/// The octets of the payload, exactly `n` of them.
fn payload<const N: usize>(text: Option<&str>) -> Result<[u8; N], CommandError> {
    let b = hex_arg("the payload", text.unwrap_or(""))?;
    b.as_slice()
        .try_into()
        .map_err(|_| refused(format!("the payload is {} octets, not {N}", b.len())))
}

/// The model names of the readings of a family with fields.
fn models(f: IsmFamily) -> &'static [&'static str] {
    match f {
        IsmFamily::Remote => &["Generic-Remote"],
        IsmFamily::Tx141thBv2 => &["LaCrosse-TX141THBv2"],
        IsmFamily::Nexus => &["Nexus-TH", "Nexus-T"],
        IsmFamily::AcuriteTower => &["Acurite-Tower"],
        IsmFamily::Wh1080 => &["Fineoffset-WHx080"],
        IsmFamily::F007th => &["Ambientweather-F007TH"],
        IsmFamily::OregonThgr810 => &["Oregon-THGR810"],
        IsmFamily::OregonThgr228n => &["Oregon-THGR228N"],
        IsmFamily::Schrader => &["Schrader"],
        IsmFamily::Toyota => &["Toyota"],
        IsmFamily::Renault => &["Renault"],
        IsmFamily::Ford => &["Ford"],
        IsmFamily::Efergy => &["Efergy-e2CT"],
        IsmFamily::CurrentCost => &["CurrentCost-TX"],
        IsmFamily::Bresser6 => &["Bresser-6in1"],
        IsmFamily::Bresser5 => &["Bresser-5in1"],
        IsmFamily::Tx29 => &["LaCrosse-TX29IT"],
        IsmFamily::Tx35 => &["LaCrosse-TX35DTHIT"],
        IsmFamily::Wh24 => &["Fineoffset-WH24"],
        IsmFamily::Prologue => &["Prologue-TH"],
        IsmFamily::AcuriteTxr
        | IsmFamily::Bresser3ch
        | IsmFamily::Citroen
        | IsmFamily::SchraderEg53
        | IsmFamily::Bresser7 => &[],
    }
}

/// Whether the family takes a raw payload.
fn raw(f: IsmFamily) -> bool {
    models(f).is_empty()
}

/// The train of the arguments `a` for family `f`.
fn train_of(f: IsmFamily, a: &IsmSendArgs) -> Result<Train, CommandError> {
    let batt = a.battery_ok.unwrap_or(true);
    let t = a.temperature.unwrap_or(20.0);
    let h = a.humidity.unwrap_or(50);
    let ch = a.channel.unwrap_or(1);
    let id = a.id;
    let tenths = |lo: f64, hi: f64| -> Result<i16, CommandError> {
        Ok(scaled("temperature", t, 0.1, 0.0, (lo * 10.0) as i64, (hi * 10.0) as i64)? as i16)
    };
    Ok(match f {
        IsmFamily::Remote => {
            let id = int("id", id, 1, 0xFFFF)? as u16;
            let command = int("command", a.command.unwrap_or(1), 1, 255)? as u8;
            let [hi, lo] = id.to_be_bytes();
            enc::ev1527([hi, lo, command])
        }
        IsmFamily::Tx141thBv2 => enc::tx141th(
            int("id", id, 1, 255)? as u8,
            int("channel", ch, 0, 3)? as u8,
            batt,
            scaled("temperature", t, 0.1, 500.0, 100, 1900)? as u16,
            int("humidity", h, 1, 100)? as u8,
        ),
        IsmFamily::Nexus => enc::nexus(
            int("id", id, 0, 255)? as u8,
            int("channel", ch, 1, 3)? as u8 - 1,
            batt,
            tenths(-204.8, 204.7)?,
            int("humidity", h, 0, 100)? as u8,
        ),
        IsmFamily::AcuriteTower => enc::acurite_tower(
            int("id", id, 0, 0x3FFF)? as u16,
            [3, 2, 0][int("channel", ch, 1, 3)? as usize - 1],
            batt,
            if h == 0 { 127 } else { int("humidity", h, 1, 100)? as u8 },
            scaled("temperature", t, 0.1, 1000.0, 600, 1700)? as u16,
        ),
        IsmFamily::Wh1080 => enc::wh1080(&enc::Wh1080 {
            id: int("id", id, 0, 255)? as u8,
            battery_ok: batt,
            t: scaled("temperature", t, 0.1, 400.0, 0, 1023)? as u16,
            humidity: int("humidity", h, 0, 100)? as u8,
            speed: scaled("wind speed", a.wind_speed.unwrap_or(0.0), 0.34, 0.0, 0, 255)? as u8,
            gust: scaled("wind gust", a.wind_gust.unwrap_or(0.0), 0.34, 0.0, 0, 255)? as u8,
            rain: scaled("rain", a.rain.unwrap_or(0.0), 0.3, 0.0, 0, 4095)? as u16,
            dir: dir16(a.wind_dir.unwrap_or(0.0)),
        }),
        IsmFamily::F007th => enc::f007th(
            int("id", id, 0, 255)? as u8,
            int("channel", ch, 1, 8)? as u8 - 1,
            batt,
            scaled("temperature", t * 1.8 + 32.0, 0.1, 400.0, 0, 3839)? as u16,
            int("humidity", h, 0, 100)? as u8,
        ),
        IsmFamily::OregonThgr810 | IsmFamily::OregonThgr228n => {
            let o = enc::Oregon {
                channel: int("channel", ch, 0, 15)? as u8,
                id: int("id", id, 0, 255)? as u8,
                battery_ok: batt,
                t: tenths(-79.9, 79.9)?,
                humidity: int("humidity", h, 0, 99)? as u8,
            };
            if f == IsmFamily::OregonThgr810 {
                enc::oregon_v3(&o)
            } else {
                enc::oregon_v21(&o)
            }
        }
        IsmFamily::Schrader => enc::schrader(
            int("flags", a.flags.unwrap_or(0), 0, 255)? as u8,
            int("id", id, 0, 0x0FFF_FFFF)? as u32,
            scaled("pressure", a.pressure.unwrap_or(0.0), 2.5, 0.0, 0, 255)? as u8,
            scaled("temperature", t, 1.0, 50.0, 0, 255)? as u8,
        ),
        IsmFamily::Toyota => enc::toyota(
            int("id", id, 0, 0xFFFF_FFFF)? as u32,
            scaled("pressure", psi(a.pressure.unwrap_or(0.0)), 0.25, 28.0, 0, 255)? as u8,
            scaled("temperature", t, 1.0, 40.0, 0, 255)? as u8,
            int("flags", a.flags.unwrap_or(0), 0, 255)? as u8,
        ),
        IsmFamily::Renault => enc::renault(
            int("flags", a.flags.unwrap_or(0), 0, 63)? as u8,
            scaled("pressure", a.pressure.unwrap_or(0.0), 0.75, 0.0, 0, 1023)? as u16,
            scaled("temperature", t, 1.0, 30.0, 0, 255)? as u8,
            int("id", id, 0, 0xFF_FFFF)? as u32,
        ),
        IsmFamily::Ford => enc::ford(
            int("id", id, 0, 0xFFFF_FFFF)? as u32,
            scaled("pressure", psi(a.pressure.unwrap_or(0.0)), 0.25, 0.0, 0, 511)? as u16,
            scaled("temperature", t, 1.0, 56.0, 0, 127)? as u8,
        ),
        IsmFamily::Efergy => {
            let s = int("interval", a.interval.unwrap_or(6), 6, 24)?;
            if s % 6 != 0 {
                return Err(refused(format!("the interval {s} s is not 6, 12, 18 or 24")));
            }
            let amps = a.current.unwrap_or(0.0);
            if !(0.0..=511.0).contains(&amps) {
                return Err(refused(format!("the current {amps} A is not 0 to 511")));
            }
            // The finest exponent that holds the current in 16 bits.
            let (raw, fact) = (7..=23)
                .rev()
                .map(|f: i32| ((amps * f64::from(1 << f)).round(), f))
                .find(|&(r, _)| r <= 65535.0)
                .unwrap_or((0.0, 7));
            enc::efergy(
                int("id", id, 0, 0xFFFF)? as u16,
                batt,
                (s / 6 - 1) as u8,
                raw as u16,
                (15 - fact) as i8,
            )
        }
        IsmFamily::CurrentCost => enc::currentcost(
            int("id", id, 0, 0xFFF)? as u16,
            int("power", a.power.unwrap_or(0), 0, 0x7FFF)? as u16,
        ),
        IsmFamily::Bresser6 => enc::bresser6(&enc::Bresser6 {
            id: int("id", id, 0, 0xFFFF_FFFF)? as u32,
            channel: int("channel", ch, 0, 7)? as u8,
            battery_ok: batt,
            t: tenths(-50.0, 99.9)?,
            humidity: int("humidity", h, 0, 99)? as u8,
            gust: scaled("wind gust", a.wind_gust.unwrap_or(0.0), 0.1, 0.0, 0, 999)? as u16,
            speed: scaled("wind speed", a.wind_speed.unwrap_or(0.0), 0.1, 0.0, 0, 999)? as u16,
            dir: scaled("wind direction", a.wind_dir.unwrap_or(0.0), 1.0, 0.0, 0, 359)? as u16,
            uv: scaled("UV index", a.uv.unwrap_or(0.0), 0.1, 0.0, 0, 999)? as u16,
        }),
        IsmFamily::Bresser5 => enc::bresser5(&enc::Bresser5 {
            id: int("id", id, 0, 255)? as u8,
            battery_ok: batt,
            t: tenths(-99.9, 99.9)?,
            humidity: int("humidity", h, 0, 99)? as u8,
            gust: scaled("wind gust", a.wind_gust.unwrap_or(0.0), 0.1, 0.0, 0, 0xFFF)? as u16,
            speed: scaled("wind speed", a.wind_speed.unwrap_or(0.0), 0.1, 0.0, 0, 999)? as u16,
            dir: dir16(a.wind_dir.unwrap_or(0.0)),
            rain: scaled("rain", a.rain.unwrap_or(0.0), 0.1, 0.0, 0, 9999)? as u16,
        }),
        IsmFamily::Tx29 | IsmFamily::Tx35 => enc::lacrosse_it(
            if f == IsmFamily::Tx29 { 55.0 } else { 105.0 },
            int("id", id, 0, 63)? as u8,
            batt,
            scaled("temperature", t, 0.1, 400.0, 0, 999)? as u16,
            if h == 0 { 0x6A } else { int("humidity", h, 1, 100)? as u8 },
        ),
        IsmFamily::Wh24 => enc::wh24(&enc::Wh24 {
            id: int("id", id, 0, 255)? as u8,
            battery_ok: batt,
            t: scaled("temperature", t, 0.1, 400.0, 0, 0x7FE)? as u16,
            humidity: if h == 0 { 0xFF } else { int("humidity", h, 1, 100)? as u8 },
            dir: scaled("wind direction", a.wind_dir.unwrap_or(0.0), 1.0, 0.0, 0, 359)? as u16,
            speed: scaled("wind speed", a.wind_speed.unwrap_or(0.0), 0.14, 0.0, 0, 0x1FE)? as u16,
            gust: scaled("wind gust", a.wind_gust.unwrap_or(0.0), 1.12, 0.0, 0, 0xFE)? as u8,
            rain: scaled("rain", a.rain.unwrap_or(0.0), 0.3, 0.0, 0, 0xFFFF)? as u16,
            uv: wh24_uv(int("UV index", a.uv.unwrap_or(0.0).round() as i64, 0, 13)? as usize),
            light: scaled("light", a.light.unwrap_or(0.0), 0.1, 0.0, 0, 0xFF_FFFE)? as u32,
        }),
        IsmFamily::Prologue => enc::prologue(
            int("id", id, 0, 255)? as u8,
            int("channel", ch, 1, 4)? as u8 - 1,
            batt,
            tenths(-204.8, 204.7)?,
            if h == 0 { 0xCC } else { int("humidity", h, 1, 100)? as u8 },
        ),
        IsmFamily::AcuriteTxr => {
            let b = hex_arg("the payload", a.payload.unwrap_or(""))?;
            if !(5..=9).contains(&b.len()) {
                return Err(refused(format!("the payload is {} octets, not 5 to 9", b.len())));
            }
            if b[0] >> 6 == 1 {
                return Err(refused("the channel bits 01 of octet 0 are not a channel"));
            }
            enc::acurite_txr(&b)
        }
        IsmFamily::Bresser3ch => enc::bresser3ch(payload::<4>(a.payload)?),
        IsmFamily::Citroen => {
            let b = payload::<9>(a.payload)?;
            if b[6] == 0 || b[7] == 0 {
                return Err(refused("octets 6 and 7 (pressure, temperature) are not 0 in a Citroen message"));
            }
            enc::citroen(b)
        }
        IsmFamily::SchraderEg53 => {
            let b = payload::<14>(a.payload)?;
            if b[0] & 0x80 != 0 {
                return Err(refused("the first bit of the row is not 0"));
            }
            enc::schrader_eg53(b)
        }
        IsmFamily::Bresser7 => {
            let b = payload::<23>(a.payload)?;
            if b[19] ^ 0xAA == 0 {
                return Err(refused("octet 21 is 0xAA, which is sent as 0"));
            }
            enc::bresser7(b)
        }
    })
}

/// The lowest WH24 UV value that reads as `index`: one above the threshold below it.
fn wh24_uv(index: usize) -> u16 {
    const TH: [u16; 13] = [
        432, 851, 1210, 1570, 2017, 2450, 2761, 3100, 3512, 3918, 4277, 4650, 5029,
    ];
    if index == 0 {
        0
    } else {
        TH[index - 1] + 1
    }
}

impl FrameSource for IsmFrames {
    const NAME: &'static str = "ism.tx";
    const SEND: &'static Command = &ISM_SEND.command;
    fn event() -> &'static EventKind<IsmTxEvent<'static>> {
        &ISM_TX
    }
    fn frame(&self, args: &Values, at: &Place) -> Result<Frame, CommandError> {
        let a: IsmSendArgs = ISM_SEND.args(args)?;
        let f = a.family.unwrap_or_default();
        let train = train_of(f, &a)?;
        let mut read: Vec<Reading> = Vec::new();
        let _ = super::protocol_list().decode(&train, &mut read);
        let own = read.iter().find(|r| models(f).contains(&r.model.as_ref()));
        if own.is_none() && !raw(f) {
            return Err(refused(format!(
                "these values do not make a frame that the stack reads as {}",
                f.label()
            )));
        }
        let (target, text) = match own.or(read.first()) {
            Some(r) => (format!("{} {}", r.model, r.id), r.line()),
            None => (f.label().to_string(), String::new()),
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
