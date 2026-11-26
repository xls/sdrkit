//! The `btclassic` test signal: basic-rate packets in GFSK (BT 0.5, index 0.32, 1 Msym/s, a 1 the higher frequency) at unit
//! power, around the frequency where the session listens ([`TEST_CENTRE_HZ`] without a session). A phone and a headset answer
//! inquiries with FHS packets (the phone's with its extended inquiry response 1.25 ms after), a scanner sends GIAC ID
//! packets, two piconets send packets of random header and payload bits, and the headset's piconet sends packets with good
//! headers. A device whose channel lies outside the capture is moved to the three channels nearest the centre. As an option, a noise burst over the whole capture every second.

use crate::testgen::GEN_SECTION;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_proto_bt::baseband::{access_code, sync_word, GIAC};
use sdrkit_proto_bt::fhs::{packet_bits, Fhs};
use sdrkit_proto_bt::packet::{eir_air_bits, header_air_bits};

/// The radio centre that the test signal assumes: the default frequency of the stack, Hz.
pub const TEST_CENTRE_HZ: f64 = 2441e6;
/// The modulation index of the test signal.
pub const INDEX: f64 = 0.32;
/// The bandwidth-time product of the Gaussian filter.
pub const BT: f64 = 0.5;

/// A noise burst over the whole capture, 300 µs every second, 10 dB over a device.
pub const GEN_WIDEBAND: Setting<field::Bool> =
    Setting::new("gen_wideband", "Test signal wideband bursts", "", false)
        .section(GEN_SECTION)
        .advanced();

/// What a device of the test signal sends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sends {
    /// FHS packets that answer an inquiry, whitened with this seed.
    Fhs(Fhs, u8),
    /// GIAC ID packets.
    Inquiry,
    /// Packets of the piconet of this LAP with random header and payload bits.
    Piconet(u32),
    /// The extended inquiry response of the phone ([`phone_eir`]), whitened with this seed.
    Eir(u8),
    /// Packets of the piconet of this LAP whose master has this UAP: headers with a good HEC, whitened by the master clock
    /// (one step a slot), the types POLL, NULL, DH1 and DM1 in turn, random payload bits.
    Connected(u32, u8),
}

/// The EIR data of the phone: the complete name "Test phone" and the 16-bit services Handsfree Audio Gateway (0x111F) and
/// Audio Source (0x110A).
pub fn phone_eir() -> Vec<u8> {
    let mut d = vec![11, 0x09];
    d.extend(b"Test phone");
    d.extend([5, 0x03, 0x1F, 0x11, 0x0A, 0x11]);
    d
}

/// A device of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestDevice {
    /// What it sends.
    pub sends: Sends,
    /// Its channel, 0 to 78.
    pub channel: u32,
    /// Its carrier offset, Hz.
    pub cfo_hz: f64,
    /// Seconds between packets.
    pub period_s: f64,
    /// The first packet, s.
    pub first_s: f64,
}

/// The phone of the test signal: LAP A1B2C3, UAP 5E, NAP 1234, a smartphone with object transfer and telephony.
pub fn phone() -> Fhs {
    let lap = 0xA1_B2C3;
    Fhs {
        parity: sync_word(lap) & ((1 << 34) - 1),
        lap,
        sr: 1,
        uap: 0x5E,
        nap: 0x1234,
        class: 1 << 20 | 1 << 22 | 2 << 8 | 3 << 2 | 1,
        eir: true,
        ..Fhs::default()
    }
}

/// The headset of the test signal: LAP 0F0E0D, UAP 7A, NAP C0DE, a wearable headset with audio.
pub fn headset() -> Fhs {
    let lap = 0x0F_0E0D;
    Fhs {
        parity: sync_word(lap) & ((1 << 34) - 1),
        lap,
        sr: 1,
        uap: 0x7A,
        nap: 0xC0DE,
        class: 1 << 21 | 4 << 8 | 1 << 2 | 1,
        ..Fhs::default()
    }
}

/// The devices of the test signal.
pub fn devices() -> [TestDevice; 7] {
    [
        TestDevice {
            sends: Sends::Fhs(phone(), 0x15),
            channel: 38,
            cfo_hz: 20e3,
            period_s: 0.50,
            first_s: 0.10,
        },
        TestDevice {
            sends: Sends::Eir(0x15),
            channel: 38,
            cfo_hz: 20e3,
            period_s: 0.50,
            first_s: 0.10 + 1.25e-3,
        },
        TestDevice {
            sends: Sends::Connected(headset().lap, headset().uap),
            channel: 40,
            cfo_hz: 5e3,
            period_s: 0.05,
            first_s: 0.0125,
        },
        TestDevice {
            sends: Sends::Fhs(headset(), 0x2A),
            channel: 44,
            cfo_hz: -30e3,
            period_s: 0.55,
            first_s: 0.22,
        },
        TestDevice {
            sends: Sends::Inquiry,
            channel: 41,
            cfo_hz: 10e3,
            period_s: 0.45,
            first_s: 0.30,
        },
        TestDevice {
            sends: Sends::Piconet(0x3C_5A96),
            channel: 35,
            cfo_hz: -40e3,
            period_s: 0.40,
            first_s: 0.05,
        },
        TestDevice {
            sends: Sends::Piconet(0xE1_7B24),
            channel: 47,
            cfo_hz: 55e3,
            period_s: 0.43,
            first_s: 0.17,
        },
    ]
}

/// Channel `k` lies inside a capture of `rate` Hz centred on `centre_hz`, 0.6 MHz from its edges.
pub fn covered(k: u32, rate: f64, centre_hz: f64) -> bool {
    (sdrkit_proto_bt::baseband::channel_hz(k) - centre_hz).abs() <= rate / 2.0 - 0.6e6
}

/// The air bits of one packet of `sends` at signal time `t`.
pub fn packet(sends: &Sends, t: f64, rng: &mut Rng) -> Vec<u8> {
    let mut bits = Vec::with_capacity(366);
    match *sends {
        Sends::Fhs(mut f, seed) => {
            f.clock = ((t / 1.25e-3) as u64 & ((1 << 26) - 1)) as u32;
            access_code(GIAC, true, &mut bits);
            packet_bits(&f, seed, &mut bits);
        }
        Sends::Inquiry => access_code(GIAC, false, &mut bits),
        Sends::Piconet(lap) => {
            access_code(lap, true, &mut bits);
            bits.extend((0..294).map(|_| rng.bit()));
        }
        Sends::Eir(seed) => {
            access_code(GIAC, true, &mut bits);
            eir_air_bits(&phone_eir(), seed, &mut bits);
        }
        Sends::Connected(lap, uap) => {
            // CLK1..CLK6 of the master: one step a slot of 625 µs.
            let slot = (t / 625e-6).round() as u64;
            let kind = [1u8, 0, 4, 3][(slot / 80 % 4) as usize];
            let payload: Vec<u8> = (0..if kind < 2 { 0 } else { 240 }).map(|_| rng.bit()).collect();
            access_code(lap, true, &mut bits);
            header_air_bits(1, kind, uap, (slot & 63) as u8, &payload, &mut bits);
        }
    }
    bits
}

/// GFSK samples of `bits` at `rate`.
///
/// # Errors
///
/// [`ConfigError`] if the modulator refuses the rate.
pub fn gfsk(bits: &[u8], rate: f64) -> Result<Vec<Complex32>, ConfigError> {
    let mut m = Cpm::gfsk(BT, INDEX, rate / 1e6)
        .map_err(ConfigError::from)?
        .with_held_edges();
    let levels: Vec<f32> = bits
        .iter()
        .map(|&b| if b == 1 { 1.0 } else { -1.0 })
        .collect();
    let mut out = Vec::with_capacity((bits.len() as f64 * rate / 1e6) as usize + 64);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// The channel nearest `centre_hz` (0 to 78).
fn nearest_channel(centre_hz: f64) -> u32 {
    ((centre_hz - sdrkit_proto_bt::baseband::channel_hz(0)) / 1e6)
        .round()
        .clamp(0.0, 78.0) as u32
}

/// The test signal at `rate` with seed `seed` for a radio centred on `centre_hz`, with the wideband bursts when `wideband` is
/// set.
///
/// # Errors
///
/// [`ConfigError`] if the scene or the modulator refuses the rate.
pub fn scene(rate: f64, seed: u64, wideband: bool, centre_hz: f64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let near = nearest_channel(centre_hz);
    for (i, d) in devices().into_iter().enumerate() {
        let channel = if covered(d.channel, rate, centre_hz) {
            d.channel
        } else {
            (near + i as u32 % 3).saturating_sub(1).min(78)
        };
        let offset = sdrkit_proto_bt::baseband::channel_hz(channel) - centre_hz + d.cfo_hz;
        let mut rng = Rng::stream(seed, 0xB7 + i as u64);
        let mut t = d.first_s;
        scene.add_source(Box::new(move || {
            let bits = packet(&d.sends, t, &mut rng);
            let samples = gfsk(&bits, rate).ok()?;
            let b = Burst {
                start_s: t,
                offset_hz: offset,
                level_db: 0.0,
                samples,
            };
            t += d.period_s;
            Some(b)
        }));
    }
    if wideband {
        let mut rng = Rng::stream(seed, 0x3B);
        let n = (300e-6 * rate) as usize;
        let mut t = 0.7;
        scene.add_source(Box::new(move || {
            let k = std::f32::consts::FRAC_1_SQRT_2;
            let samples = (0..n)
                .map(|_| Complex32::new(k * rng.normal_f32(), k * rng.normal_f32()))
                .collect();
            let b = Burst {
                start_s: t,
                offset_hz: 0.0,
                level_db: 10.0,
                samples,
            };
            t += 1.0;
            Some(b)
        }));
    }
    Ok(scene)
}

/// The `btclassic` generator.
pub struct BtGenerator {
    scene: Scene,
    rate: f64,
}

impl BtGenerator {
    /// The test signal of the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// As [`scene`].
    pub fn from_config(cfg: &GenConfig) -> Result<BtGenerator, ConfigError> {
        let wideband = crate::testgen::gen_setting(cfg, &GEN_WIDEBAND).unwrap_or(false);
        Ok(BtGenerator {
            scene: scene(
                cfg.rate_hz,
                cfg.seed,
                wideband,
                cfg.tuned_hz().unwrap_or(TEST_CENTRE_HZ),
            )?,
            rate: cfg.rate_hz,
        })
    }
}

impl Generator for BtGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(1e6)
                .with_symbol_rate(1e6)
                .with_bit_rate(1e6)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
