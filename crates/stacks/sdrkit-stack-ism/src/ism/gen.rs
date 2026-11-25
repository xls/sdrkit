//! The `ism` test signal: test devices encode their frames as pulse trains, and each train keys a clean carrier (OOK) or moves
//! a carrier between two tones (FSK) in a scene of several devices.

use super::encode::{self as enc, bits_of, fsk};
use crate::family::CleanScene;
use sdrkit_algo_sim::modulate::{Fm, Ook};
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, SignalInfo};
use sdrkit_proto_ookdev::{Modulation, Train};
use sdrkit_util_bits::line::{Manchester, ManchesterConv};

/// The rate the test trains are counted at: one sample is one microsecond.
pub const TRAIN_RATE_HZ: f64 = 1e6;

/// The test devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestDevice {
    /// Nexus-TH.
    Nexus,
    /// LaCrosse TX141TH-Bv2.
    Tx141th,
    /// Acurite tower.
    AcuriteTower,
    /// Fine Offset WH1080.
    Wh1080,
    /// Ambient Weather F007TH.
    F007th,
    /// Oregon Scientific THGR810 (v3).
    OregonV3,
    /// Oregon Scientific THGR228N (v2.1).
    OregonV21,
    /// Schrader TPMS.
    Schrader,
    /// EV1527 remote.
    Ev1527,
    /// Toyota TPMS.
    Toyota,
    /// Renault TPMS.
    Renault,
    /// Ford TPMS.
    Ford,
    /// Efergy e2.
    Efergy,
    /// CurrentCost TX.
    CurrentCost,
    /// Bresser 6-in-1.
    Bresser6,
    /// Bresser 5-in-1.
    Bresser5,
    /// LaCrosse TX29-IT.
    Tx29,
    /// Fine Offset WH24.
    Wh24,
    /// Wireless M-Bus mode T.
    WmbusT,
    /// Z-Wave R1.
    ZwaveR1,
    /// Z-Wave R2.
    ZwaveR2,
    /// Z-Wave R3.
    ZwaveR3,
    /// Somfy RTS remote.
    SomfyRts,
    /// KeeLoq-style (HCS301) remote.
    Keeloq,
}

impl TestDevice {
    /// Every test device.
    pub const ALL: [TestDevice; 24] = [
        TestDevice::Nexus,
        TestDevice::Tx141th,
        TestDevice::AcuriteTower,
        TestDevice::Wh1080,
        TestDevice::F007th,
        TestDevice::OregonV3,
        TestDevice::OregonV21,
        TestDevice::Schrader,
        TestDevice::Ev1527,
        TestDevice::Toyota,
        TestDevice::Renault,
        TestDevice::Ford,
        TestDevice::Efergy,
        TestDevice::CurrentCost,
        TestDevice::Bresser6,
        TestDevice::Bresser5,
        TestDevice::Tx29,
        TestDevice::Wh24,
        TestDevice::WmbusT,
        TestDevice::ZwaveR1,
        TestDevice::ZwaveR2,
        TestDevice::ZwaveR3,
        TestDevice::SomfyRts,
        TestDevice::Keeloq,
    ];

    /// The model name it is reported as.
    pub fn model(self) -> &'static str {
        match self {
            TestDevice::Nexus => "Nexus-TH",
            TestDevice::Tx141th => "LaCrosse-TX141THBv2",
            TestDevice::AcuriteTower => "Acurite-Tower",
            TestDevice::Wh1080 => "Fineoffset-WHx080",
            TestDevice::F007th => "Ambientweather-F007TH",
            TestDevice::OregonV3 => "Oregon-THGR810",
            TestDevice::OregonV21 => "Oregon-THGR228N",
            TestDevice::Schrader => "Schrader",
            TestDevice::Ev1527 => "Generic-Remote",
            TestDevice::Toyota => "Toyota",
            TestDevice::Renault => "Renault",
            TestDevice::Ford => "Ford",
            TestDevice::Efergy => "Efergy-e2CT",
            TestDevice::CurrentCost => "CurrentCost-TX",
            TestDevice::Bresser6 => "Bresser-6in1",
            TestDevice::Bresser5 => "Bresser-5in1",
            TestDevice::Tx29 => "LaCrosse-TX29IT",
            TestDevice::Wh24 => "Fineoffset-WH24",
            TestDevice::WmbusT => "Wireless-MBus",
            TestDevice::ZwaveR1 => "Z-Wave-R1",
            TestDevice::ZwaveR2 => "Z-Wave-R2",
            TestDevice::ZwaveR3 => "Z-Wave-R3",
            TestDevice::SomfyRts => "Somfy-RTS",
            TestDevice::Keeloq => "KeeLoq-HCS",
        }
    }

    /// The id it is reported with.
    pub fn id(self) -> &'static str {
        match self {
            TestDevice::Nexus => "90",
            TestDevice::Tx141th => "3c",
            TestDevice::AcuriteTower => "4660",
            TestDevice::Wh1080 => "123",
            TestDevice::F007th => "156",
            TestDevice::OregonV3 => "165",
            TestDevice::OregonV21 => "62",
            TestDevice::Schrader => "1A2B3C4",
            TestDevice::Ev1527 => "23100",
            TestDevice::Toyota => "a1b2c3d4",
            TestDevice::Renault => "c0ffee",
            TestDevice::Ford => "12ab34cd",
            TestDevice::Efergy => "1234",
            TestDevice::CurrentCost => "1957",
            TestDevice::Bresser6 => "12345678",
            TestDevice::Bresser5 => "108",
            TestDevice::Tx29 => "43",
            TestDevice::Wh24 => "191",
            TestDevice::WmbusT => "12345678",
            TestDevice::ZwaveR1 => "C0FFEE01:2",
            TestDevice::ZwaveR2 => "DEADBEEF:7",
            TestDevice::ZwaveR3 => "12345678:9",
            TestDevice::SomfyRts => "5A3C81",
            TestDevice::Keeloq => "1A2B3C4",
        }
    }

    /// Whether the protocol checks the frame with a CRC or a checksum (one flipped bit then returns no reading).
    pub fn checked(self) -> bool {
        !matches!(
            self,
            TestDevice::Nexus | TestDevice::Ev1527 | TestDevice::CurrentCost | TestDevice::Keeloq
        )
    }
}

/// The bytes of a Z-Wave test MPDU for transmission `k`.
pub fn zwave_mpdu(d: TestDevice, k: u64) -> Vec<u8> {
    let seq = (k % 16) as u8;
    match d {
        TestDevice::ZwaveR1 => {
            let v = if k % 2 == 1 { 0xFF } else { 0x00 };
            crate::zwave::frame::build(0xC0FFEE01, 2, 0x41, seq, &[1], &[0x25, 0x03, v], 1)
        }
        TestDevice::ZwaveR2 => {
            let tb = (215 + (k % 2) as i16).to_be_bytes();
            crate::zwave::frame::build(
                0xDEADBEEF,
                7,
                0x41,
                seq,
                &[1],
                &[0x31, 0x05, 0x01, 0x22, tb[0], tb[1]],
                2,
            )
        }
        _ => {
            let v = (12345 + (k % 2) as u32).to_be_bytes();
            crate::zwave::frame::build(
                0x12345678,
                9,
                0x41,
                seq,
                &[1],
                &[0x32, 0x02, 0x01, 0x34, v[0], v[1], v[2], v[3]],
                3,
            )
        }
    }
}

/// The Wireless M-Bus test frame (format A) for transmission `k`, without CRCs.
pub fn wmbus_frame(k: u64) -> Vec<u8> {
    let m = crate::wmbus::link::manufacturer_code("KAM");
    vec![
        0x0F,
        0x44,
        m as u8,
        (m >> 8) as u8,
        0x78,
        0x56,
        0x34,
        0x12,
        0x1B,
        0x16,
        0x7A,
        k as u8,
        0x00,
        0x00,
        0x00,
        0x2F,
    ]
}

/// An OOK train of (on, µs) levels: runs of one level are joined; the last space is `tail_us`.
fn levels_train(levels: &[(bool, f32)], tail_us: f32) -> Train {
    let mut pairs: Vec<(f32, f32)> = Vec::new();
    for &(on, us) in levels {
        match (on, pairs.last_mut()) {
            (true, Some(p)) if p.1 == 0.0 => p.0 += us,
            (true, _) => pairs.push((us, 0.0)),
            (false, Some(p)) => p.1 += us,
            (false, None) => {}
        }
    }
    if let Some(p) = pairs.last_mut() {
        p.1 += tail_us;
    }
    Train::from_us(Modulation::Ook, TRAIN_RATE_HZ, &pairs)
}

/// A Somfy RTS frame written from the public description (see the decoder): key 0xA7, button `button`, a rolling code, the
/// address 5A3C81; two hardware syncs of 2416 µs, the software sync of 4550 and 604 µs, 56 Manchester bits of 604 µs
/// half-bits (a 1 is low then high), then 30.4 ms of silence.
pub fn somfy_train(button: u8, rolling: u16) -> Train {
    let mut d = [0xA7, button << 4, (rolling >> 8) as u8, rolling as u8, 0x81, 0x3C, 0x5A];
    let x = d.iter().fold(0u8, |a, &b| a ^ (b >> 4) ^ (b & 15));
    d[1] |= x;
    let mut f = d;
    for k in 1..7 {
        f[k] = d[k] ^ f[k - 1];
    }
    let mut lv = Vec::new();
    for _ in 0..2 {
        lv.extend([(true, 2416.0), (false, 2416.0)]);
    }
    lv.extend([(true, 4550.0), (false, 604.0)]);
    for b in bits_of(&f) {
        lv.extend([(b == 0, 604.0), (b == 1, 604.0)]);
    }
    levels_train(&lv, 30_415.0)
}

/// Two HCS301 code words written from the data sheet: 12 preamble pulses of TE = 400 µs, a 10 TE header, 66 PWM bits least
/// significant first (a stand-in hopping code, serial 1A2B3C4, button S1, VLOW 0, RPT 0 then 1), a 39 TE guard.
pub fn keeloq_train() -> Train {
    let te = 400.0;
    let mut lv = Vec::new();
    for rpt in 0..2u64 {
        for i in 0..12 {
            lv.extend([(true, te), (false, if i == 11 { 10.0 * te } else { te })]);
        }
        let word: u128 = 0x3C5A_9E17 | (0x1A2_B3C4u128 << 32) | (1u128 << 62) | (u128::from(rpt) << 65);
        for i in 0..66 {
            let one = (word >> i) & 1 == 1;
            let (on, off) = if one { (te, 2.0 * te) } else { (2.0 * te, te) };
            lv.extend([(true, on), (false, off)]);
        }
        lv.push((false, 39.0 * te));
    }
    levels_train(&lv, 0.0)
}

/// The train of test device `d` for transmission `k`, as a receiver passes it to the protocols (lengths at 1 MHz).
pub fn device_train(d: TestDevice, k: u64) -> Train {
    match d {
        TestDevice::Nexus => enc::nexus(0x5A, 1, true, 213 + (k % 4) as i16, 45),
        TestDevice::Tx141th => enc::tx141th(0x3C, 1, true, 187 + (k % 3) as u16 + 500, 62),
        TestDevice::AcuriteTower => enc::acurite_tower(0x1234, 3, true, 55, 1225 + (k % 3) as u16),
        TestDevice::Wh1080 => enc::wh1080(&enc::Wh1080 {
            id: 0x7B,
            battery_ok: true,
            t: 124 + (k % 3) as u16 + 400,
            humidity: 71,
            speed: 3,
            gust: 7,
            rain: 0x123,
            dir: 4,
        }),
        TestDevice::F007th => enc::f007th(0x9C, 2, true, 685 + (k % 3) as u16 + 400, 40),
        TestDevice::OregonV3 => enc::oregon_v3(&enc::Oregon {
            channel: 1,
            id: 0xA5,
            battery_ok: true,
            t: 196 + (k % 2) as i16,
            humidity: 52,
        }),
        TestDevice::OregonV21 => enc::oregon_v21(&enc::Oregon {
            channel: 1,
            id: 0x3E,
            battery_ok: true,
            t: 52 + (k % 2) as i16,
            humidity: 80,
        }),
        TestDevice::Schrader => enc::schrader(0x0B, 0x1A2B3C4, 92 + (k % 2) as u8, 75),
        TestDevice::Ev1527 => enc::ev1527([0x5A, 0x3C, 0x81]),
        TestDevice::Toyota => enc::toyota(0xA1B2C3D4, 156 + (k % 2) as u8, 62, 0),
        TestDevice::Renault => enc::renault(0x34, 333 + (k % 2) as u16, 48, 0xC0FFEE),
        TestDevice::Ford => enc::ford(0x12AB34CD, 140 + (k % 2) as u16, 76),
        TestDevice::Efergy => enc::efergy(1234, true, 1, 0x5800 + (k % 2) as u16 * 0x100, 3),
        TestDevice::CurrentCost => enc::currentcost(0x7A5, 1234 + (k % 2) as u16),
        TestDevice::Bresser6 => enc::bresser6(&enc::Bresser6 {
            id: 0x12345678,
            channel: 0,
            battery_ok: true,
            t: 156 + (k % 3) as i16,
            humidity: 67,
            gust: 34,
            speed: 21,
            dir: 225,
            uv: 32,
        }),
        TestDevice::Bresser5 => enc::bresser5(&enc::Bresser5 {
            id: 0x6C,
            battery_ok: true,
            t: 87 + (k % 2) as i16,
            humidity: 81,
            gust: 52,
            speed: 31,
            dir: 4,
            rain: 123,
        }),
        TestDevice::Tx29 => enc::lacrosse_it(55.0, 43, true, 617 + (k % 2) as u16, 0x6A),
        TestDevice::Wh24 => enc::wh24(&enc::Wh24 {
            id: 0xBF,
            battery_ok: true,
            t: 518 + (k % 2) as u16,
            humidity: 78,
            dir: 0x10A,
            speed: 8,
            gust: 2,
            rain: 0x4A,
            uv: 0,
            light: 100,
        }),
        TestDevice::WmbusT => {
            let air = crate::wmbus::link::add_crcs(&wmbus_frame(k), crate::wmbus::link::Format::A);
            let mut bits = bits_of(&[0x55, 0x55, 0x55, 0x55, 0x54, 0x3D]);
            for byte in air {
                for nib in [byte >> 4, byte & 0xF] {
                    let c = sdrkit_util_bits::line::three_of_six::encode(nib);
                    bits.extend((0..6).rev().map(|i| (c >> i) & 1));
                }
            }
            bits.extend_from_slice(&[0, 1, 0, 1]);
            fsk(&bits, 10.0, 50e3)
        }
        TestDevice::SomfyRts => somfy_train(0x2, 0x1234 + k as u16),
        TestDevice::Keeloq => keeloq_train(),
        TestDevice::ZwaveR1 | TestDevice::ZwaveR2 | TestDevice::ZwaveR3 => {
            let m = zwave_mpdu(d, k);
            let (pre, chip, dev) = match d {
                TestDevice::ZwaveR1 => (12, 1e6 / 19200.0, 20e3),
                TestDevice::ZwaveR2 => (12, 25.0, 20e3),
                _ => (10, 10.0, 29e3),
            };
            let mut bytes = vec![0x55u8; pre];
            bytes.push(0xF0);
            bytes.extend_from_slice(&m);
            bytes.push(0x00);
            let mut bits = bits_of(&bytes);
            if d == TestDevice::ZwaveR1 {
                let mut chips = Vec::new();
                Manchester::encode(&bits, ManchesterConv::Thomas, &mut chips);
                bits = chips;
            }
            fsk(&bits, chip as f32, dev)
        }
    }
}

/// The clean samples of a train at `rate`, at 0 Hz and unit power while sending. OOK keys the carrier with 3 µs raised-cosine
/// edges. FSK sends 100 µs of the lower tone first and moves between the tones with 6 µs edges. `truncate` (0 to 1) cuts the
/// end of the burst away.
pub fn train_samples(t: &Train, rate: f64, truncate: f64) -> Vec<Complex32> {
    let fsk = t.modulation == Modulation::Fsk;
    let Ok(mut key) = Ook::new(rate, if fsk { 6e-6 } else { 3e-6 }) else {
        return Vec::new();
    };
    let mut env = Vec::new();
    let us = |x: f32| t.us(x) as f64 * 1e-6;
    if fsk {
        let _ = key.push(false, 100e-6, &mut env);
    }
    let n = t.pulses.len();
    for (i, p) in t.pulses.iter().enumerate() {
        let _ = key.push(true, us(p.mark), &mut env);
        if fsk || i + 1 < n {
            let _ = key.push(false, us(p.space), &mut env);
        }
    }
    let _ = key.push(false, 10e-6, &mut env);
    key.flush(&mut env);
    let keep = ((env.len() as f64) * (1.0 - truncate.clamp(0.0, 1.0))).round() as usize;
    env.truncate(keep);
    if !fsk {
        return env.iter().map(|&a| Complex32::new(a, 0.0)).collect();
    }
    let levels: Vec<f32> = env.iter().map(|&a| 2.0 * a - 1.0).collect();
    let mut out = vec![Complex32::default(); levels.len()];
    if let Ok(mut fm) = Fm::new(rate, t.deviation_hz) {
        let _ = fm.process(&levels, &mut out);
    }
    out
}

/// One device of a scene.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    /// The device.
    pub dev: TestDevice,
    /// Hz from the centre of the samples.
    pub offset_hz: f64,
    /// The first transmission, seconds.
    pub phase_s: f64,
    /// The period, seconds.
    pub period_s: f64,
    /// Level relative to the other devices, dB.
    pub level_db: f64,
    /// The part of each burst cut away at its end (0 to 1).
    pub truncate: f64,
}

/// The devices of scene 0 (433.92 MHz), 1 (868.3 MHz) or 2 (Z-Wave).
pub fn scene_slots(scene: u32) -> Vec<Slot> {
    let s = |dev, off: f64, ph: f64, per: f64| Slot {
        dev,
        offset_hz: off,
        phase_s: ph,
        period_s: per,
        level_db: 0.0,
        truncate: 0.0,
    };
    use TestDevice::*;
    match scene {
        1 => vec![
            s(Bresser6, 0.0, 0.3, 5.0),
            s(Bresser5, 20e3, 1.2, 5.0),
            s(Tx29, -15e3, 2.1, 5.0),
            s(Wh24, 30e3, 3.0, 5.0),
            s(WmbusT, 650e3, 3.9, 5.0),
        ],
        2 => vec![
            s(ZwaveR1, -20e3, 0.3, 5.0),
            s(ZwaveR2, 20e3, 1.5, 5.0),
            s(ZwaveR3, 0.0, 2.7, 5.0),
        ],
        _ => vec![
            s(Nexus, -30e3, 0.20, 10.0),
            s(Tx141th, 40e3, 1.30, 10.0),
            s(AcuriteTower, -50e3, 1.90, 10.0),
            s(Wh1080, 15e3, 2.20, 10.0),
            s(F007th, 48e3, 2.50, 10.0),
            s(OregonV3, -10e3, 2.80, 10.0),
            s(OregonV21, 25e3, 3.10, 10.0),
            s(Schrader, -45e3, 3.40, 10.0),
            s(Ev1527, 0.0, 3.60, 10.0),
            s(Toyota, 20e3, 4.20, 10.0),
            s(Renault, -35e3, 4.40, 10.0),
            s(Ford, 45e3, 4.60, 10.0),
            s(Efergy, -370e3, 4.80, 10.0),
            s(CurrentCost, 5e3, 5.00, 10.0),
            s(SomfyRts, -40e3, 5.30, 10.0),
            s(Keeloq, 35e3, 5.70, 10.0),
        ],
    }
}

/// The generator settings of the stack.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenParams {
    /// Scene 0, 1 or 2.
    pub scene: u32,
    /// A continuous carrier 20 dB above the devices, 300 kHz above the centre.
    pub carrier: bool,
}

/// The `ism` test signal at `rate`: the slots' devices send their trains at their offsets and levels, and a continuous carrier
/// when `p.carrier` is set. The devices' bursts have unit power.
pub fn generator(rate: f64, p: &GenParams, slots: Vec<Slot>) -> Option<CleanScene> {
    let mut scene = Scene::new(rate).ok()?;
    let mut next: Vec<(f64, u64)> = slots.iter().map(|s| (s.phase_s, 0)).collect();
    let src = move || {
        let (j, _) = next
            .iter()
            .enumerate()
            .min_by(|a, b| a.1 .0.total_cmp(&b.1 .0))?;
        let (t, k) = next[j];
        let sl = slots[j];
        next[j] = (t + sl.period_s, k + 1);
        Some(Burst {
            start_s: t,
            offset_hz: sl.offset_hz.clamp(-rate / 2.0, rate / 2.0),
            level_db: sl.level_db,
            samples: train_samples(&device_train(sl.dev, k), rate, sl.truncate),
        })
    };
    scene.add_source(Box::new(src));
    if p.carrier && 300e3 < rate / 2.0 {
        let tone = vec![Complex32::new(1.0, 0.0); rate as usize];
        let _ = scene.add_repeating(0.0, 1.0, 300e3, 20.0, tone);
    }
    // The widest device signal (Wireless M-Bus T, 100 kchip/s at 50 kHz deviation) is about 200 kHz wide.
    Some(CleanScene::new(scene, rate, SignalInfo::new(200e3)))
}
