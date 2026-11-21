//! The `ism` test signal: test devices encode their frames as pulse trains, and each train keys a clean carrier (OOK) or moves
//! a carrier between two tones (FSK) in a scene of several devices.

use super::checks::{CRC8_31, CRC8_SCHRADER, CRC8_TOYOTA};
use super::protocols::reflect_nibbles;
use crate::family::CleanScene;
use sdrkit_algo_sim::modulate::{Fm, Ook};
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, SignalInfo};
use sdrkit_proto_ookdev::{encode, Modulation, Pulse, Train};
use sdrkit_util_bits::checksum::{lfsr_digest16, lfsr_digest8, lfsr_digest8_reflect, sum8};
use sdrkit_util_bits::crc::catalog::{CRC8_NRSC_5, CRC8_SMBUS};
use sdrkit_util_bits::line::{Manchester, ManchesterConv};
use sdrkit_util_bits::pack::{unpack, BitOrder};

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
}

impl TestDevice {
    /// Every test device.
    pub const ALL: [TestDevice; 22] = [
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
        }
    }

    /// Whether the protocol checks the frame with a CRC or a checksum (one flipped bit then returns no reading).
    pub fn checked(self) -> bool {
        !matches!(
            self,
            TestDevice::Nexus | TestDevice::Ev1527 | TestDevice::CurrentCost
        )
    }
}

fn bits_of(b: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 * b.len());
    unpack(b, BitOrder::Msb, &mut v);
    v
}

fn train(m: Modulation) -> Train {
    Train::new(m, TRAIN_RATE_HZ)
}

fn push(t: &mut Train, mark_us: f32, space_us: f32) {
    t.pulses
        .push(Pulse::new(t.samples(mark_us), t.samples(space_us)));
}

fn set_last_space(t: &mut Train, space_us: f32) {
    let s = t.samples(space_us);
    if let Some(p) = t.pulses.last_mut() {
        p.space = s;
    }
}

/// Manchester "1 = high then low" (the Thomas convention) with half-bit `half_us`, as OOK.
fn manchester_ook(bits: &[u8], half_us: f32) -> Train {
    let mut t = train(Modulation::Ook);
    let mut chips = Vec::new();
    encode::manchester(bits, ManchesterConv::Thomas, half_us, &mut chips, &mut t);
    t
}

/// The IEEE 802.3 Manchester chips of `bits` (1 is 01), which some devices send as plain bits.
fn ieee_chips(bits: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 * bits.len());
    Manchester::encode(bits, ManchesterConv::Ieee, &mut v);
    v
}

fn fsk(levels: &[u8], bit_us: f32, dev: f64) -> Train {
    let mut t = train(Modulation::Fsk);
    t.deviation_hz = dev;
    encode::nrz(levels, bit_us, &mut t);
    t
}

fn oregon_checksum(m: &mut [u8]) {
    let c = sdrkit_util_bits::checksum::nibble_sum(m, 15);
    m[7] = (m[7] & 0xF0) | (c & 0xF);
    m[8] = (c >> 4) << 4 | (m[8] & 0xF);
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

/// The train of test device `d` for transmission `k`, as a receiver passes it to the protocols (lengths at 1 MHz).
pub fn device_train(d: TestDevice, k: u64) -> Train {
    match d {
        TestDevice::Nexus => {
            let t = 213 + (k % 4) as u16;
            let b = [0x5Au8, 0x90 | ((t >> 8) as u8 & 0xF), t as u8, 0xF2, 0xD0];
            let bits = &bits_of(&b)[..36];
            let mut tr = train(Modulation::Ook);
            for _ in 0..12 {
                encode::ppm(bits, 500.0, 1000.0, 2000.0, &mut tr);
                push(&mut tr, 500.0, 4000.0);
            }
            tr
        }
        TestDevice::Tx141th => {
            let t = 187 + (k % 3) as u16 + 500;
            let mut b = [0x3Cu8, 0x10 | ((t >> 8) as u8 & 0xF), t as u8, 62, 0];
            b[4] = lfsr_digest8_reflect(&b[0..4], 0x31, 0xF4);
            let bits = bits_of(&b);
            let mut tr = train(Modulation::Ook);
            for _ in 0..12 {
                for _ in 0..4 {
                    push(&mut tr, 833.0, 833.0);
                }
                encode::pwm(&bits, (417.0, 208.0), (208.0, 417.0), &mut tr);
            }
            tr
        }
        TestDevice::AcuriteTower => {
            let t = 1225 + (k % 3) as u16;
            // Bytes 2 to 5 each carry an even parity bit in their top bit.
            let par = |x: u8| if x.count_ones() % 2 == 1 { x | 0x80 } else { x };
            let mut b = [
                0xD2u8,
                0x34,
                par(0x44),
                par(55),
                par((t >> 7) as u8 & 0x7F),
                par(t as u8 & 0x7F),
                0,
            ];
            b[6] = sum8(&b[0..6]);
            let bits = bits_of(&b);
            let mut tr = train(Modulation::Ook);
            for r in 0..3 {
                for _ in 0..4 {
                    push(&mut tr, 620.0, 596.0);
                }
                encode::pwm(&bits, (408.0, 204.0), (220.0, 392.0), &mut tr);
                if r < 2 {
                    set_last_space(&mut tr, 2192.0);
                }
            }
            tr
        }
        TestDevice::Wh1080 => {
            let t = 124 + (k % 3) as u16 + 400;
            let mut b = [
                0xFFu8,
                0xA7,
                0xB0 | ((t >> 8) as u8 & 3),
                t as u8,
                71,
                3,
                7,
                0x01,
                0x23,
                0x04,
                0,
            ];
            b[10] = CRC8_NRSC_5.checksum(&b[0..10]);
            let mut tr = train(Modulation::Ook);
            encode::pwm(&bits_of(&b), (544.0, 1036.0), (1524.0, 1036.0), &mut tr);
            tr
        }
        TestDevice::F007th => {
            let t = 685 + (k % 3) as u16 + 400;
            let mut b = [0x45u8, 0x9C, 0x20 | ((t >> 8) as u8 & 0xF), t as u8, 40, 0];
            b[5] = lfsr_digest8(&b[0..5], 0x98, 0x3E) ^ 0x64;
            let mut bits = vec![0u8; 15];
            bits.push(1);
            bits.extend(bits_of(&b));
            manchester_ook(&bits, 500.0)
        }
        TestDevice::OregonV3 => {
            let tenths = 6 + (k % 2) as u8;
            let mut m = [
                0xF8u8,
                0x24,
                0x15,
                0xA0,
                (tenths << 4) | 9,
                0x10,
                0x25,
                0x00,
                0x00,
            ];
            oregon_checksum(&mut m);
            let mut bits = vec![0u8; 24];
            bits.extend_from_slice(&[0, 1, 0, 1]);
            for x in m {
                bits.extend(bits_of(&[reflect_nibbles(x)]));
            }
            manchester_ook(&bits, 488.0)
        }
        TestDevice::OregonV21 => {
            let tenths = 2 + (k % 2) as u8;
            let mut m = [
                0x1Du8,
                0x20,
                0x1E,
                0x30,
                (tenths << 4) | 5,
                0x00,
                0x08,
                0x00,
                0x00,
            ];
            oregon_checksum(&mut m);
            let mut data = Vec::new();
            for x in m {
                data.extend(bits_of(&[reflect_nibbles(x)]));
            }
            data.truncate(68);
            let mut bits = bits_of(&[0x55, 0x55, 0x55, 0x55, 0x99]);
            bits.extend(ieee_chips(&data));
            manchester_ook(&bits, 488.0)
        }
        TestDevice::Schrader => {
            let mut b = [0x00u8, 0xB1, 0xA2, 0xB3, 0xC4, 92 + (k % 2) as u8, 75, 0];
            b[7] = CRC8_SCHRADER.checksum(&b[0..7]);
            let mut bits = vec![0u8, 1, 1, 1];
            bits.extend(bits_of(&b));
            manchester_ook(&bits, 120.0)
        }
        TestDevice::Ev1527 => {
            let bits = bits_of(&[0x5A, 0x3C, 0x81]);
            let mut tr = train(Modulation::Ook);
            for _ in 0..6 {
                encode::pwm(&bits, (1404.0, 464.0), (464.0, 1404.0), &mut tr);
                push(&mut tr, 464.0, 14000.0);
            }
            tr
        }
        TestDevice::Toyota => {
            let p1 = 156 + (k % 2) as u8;
            let tt = 62u8;
            let mut b = [
                0xA1u8,
                0xB2,
                0xC3,
                0xD4,
                p1 >> 1,
                ((p1 & 1) << 7) | (tt >> 1),
                (tt & 1) << 7,
                !p1,
                0,
            ];
            b[8] = CRC8_TOYOTA.checksum(&b[0..8]);
            let mut data = bits_of(&b);
            data.extend_from_slice(&[0, 0, 0, 0]);
            // The preamble, then the data in FM0 (biphase space): a level change at each cell start, and one in the middle for a 0.
            let mut bits: Vec<u8> = "01010101001111".bytes().map(|c| c - b'0').collect();
            let mut lvl = 1u8;
            for d in data {
                let a = 1 - lvl;
                let c = if d != 0 { a } else { 1 - a };
                bits.push(a);
                bits.push(c);
                lvl = c;
            }
            fsk(&bits, 52.0, 30e3)
        }
        TestDevice::Renault | TestDevice::Ford => {
            let b: Vec<u8> = if d == TestDevice::Renault {
                let pr = 333 + (k % 2) as u16;
                let mut b = vec![
                    (0x34 << 2) | (pr >> 8) as u8,
                    pr as u8,
                    48,
                    0xEE,
                    0xFF,
                    0xC0,
                    0xFF,
                    0xFF,
                    0,
                ];
                b[8] = CRC8_SMBUS.checksum(&b[0..8]);
                b
            } else {
                let mut b = vec![0x12, 0xAB, 0x34, 0xCD, 140 + (k % 2) as u8, 76, 0x05, 0];
                b[7] = sum8(&b[0..7]);
                b
            };
            let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xA9]);
            bits.extend(ieee_chips(&bits_of(&b)));
            bits.extend(bits_of(&[0xAA]));
            let inv: Vec<u8> = bits.iter().map(|x| 1 - x).collect();
            fsk(&inv, 52.0, 30e3)
        }
        TestDevice::Efergy => {
            let mut b = [
                0x00u8,
                0xD2,
                0x04,
                0x50,
                0x58 + (k % 2) as u8,
                0x00,
                0x03,
                0,
            ];
            b[7] = sum8(&b[0..7]);
            let mut tr = train(Modulation::Fsk);
            tr.deviation_hz = 20e3;
            push(&mut tr, 500.0, 136.0);
            encode::pwm(&bits_of(&b), (64.0, 136.0), (136.0, 64.0), &mut tr);
            set_last_space(&mut tr, 200.0);
            tr
        }
        TestDevice::CurrentCost => {
            let pw = 1234 + (k % 2) as u16;
            let b = [0x07u8, 0xA5, 0x80 | (pw >> 8) as u8, pw as u8, 0, 0, 0, 0];
            let mut bits = bits_of(&[0xCC]);
            bits.extend(bits_of(&[0xCC, 0xCC, 0xCC, 0xCE, 0x91, 0x5D])[..45].iter());
            bits.extend(ieee_chips(&bits_of(&b)));
            let inv: Vec<u8> = bits.iter().map(|x| 1 - x).collect();
            fsk(&inv, 250.0, 20e3)
        }
        TestDevice::Bresser6 => {
            let traw = 156 + (k % 3) as u32;
            let mut m = [0u8; 18];
            m[2..6].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
            m[6] = 0x10;
            m[7] = !0x03;
            m[8] = !0x41;
            m[9] = !0x02;
            m[10] = 0x22;
            m[11] = 0x50;
            let tens = traw / 10;
            m[12] = (((tens / 10) % 10) << 4 | (tens % 10)) as u8;
            m[13] = (((traw % 10) as u8) << 4) | 0x02;
            m[14] = 0x67;
            m[15] = !0x03;
            m[16] = 0xD0;
            m[17] = 0xFFu8.wrapping_sub(sum8(&m[2..17]));
            let dg = lfsr_digest16(&m[2..17], 0x8810, 0x5412);
            m[0] = (dg >> 8) as u8;
            m[1] = dg as u8;
            let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
            bits.extend(bits_of(&m));
            fsk(&bits, 124.0, 40e3)
        }
        TestDevice::Bresser5 => {
            let t = 87 + (k % 2) as u8;
            let mut m = [0u8; 26];
            m[14..26].copy_from_slice(&[
                0x6C,
                0x00,
                52,
                0x40,
                0x31,
                0x00,
                ((t / 10) << 4) | (t % 10),
                0,
                0x81,
                0x23,
                0x01,
                0x00,
            ]);
            m[13] = m[14..26].iter().map(|b| b.count_ones()).sum::<u32>() as u8;
            for i in 0..13 {
                m[i] = !m[i + 13];
            }
            let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
            bits.extend(bits_of(&m));
            fsk(&bits, 124.0, 40e3)
        }
        TestDevice::Tx29 => {
            let tenths = 7 + (k % 2) as u8;
            let mut b = [0x9Au8, 0xC6, 0x10 | tenths, 0x6A, 0];
            b[4] = CRC8_31.checksum(&b[0..4]);
            let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
            bits.extend(bits_of(&b));
            fsk(&bits, 55.0, 30e3)
        }
        TestDevice::Wh24 => {
            let t = 518 + (k % 2) as u16;
            let mut b = [
                0x24u8,
                0xBF,
                0x0A,
                0x80 | ((t >> 8) as u8 & 7),
                t as u8,
                78,
                8,
                2,
                0x00,
                0x4A,
                0,
                0,
                0,
                0,
                100,
                0,
                0,
            ];
            b[15] = CRC8_31.checksum(&b[0..15]);
            b[16] = sum8(&b[0..16]);
            let mut bits = bits_of(&[0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0x2D, 0xD4]);
            bits.extend(bits_of(&b));
            fsk(&bits, 58.0, 30e3)
        }
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
