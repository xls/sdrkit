//! The V2X test signal: every 100 ms a QoS data frame with WSMP that holds a basic safety message and one with GeoNetworking
//! that holds a cooperative awareness message, each a non-HT PPDU built at the 20 MHz sample rate and played at 10 Msps (the
//! 10 MHz channel), then resampled to the rate asked for. Unit power while a frame is sent.

use super::its::{
    write_bsm, write_cam, write_data_header, Bsm, BsmTail, Cam, CAM_MIN, CAM_PORT,
    ETHERTYPE_GEONET, ETHERTYPE_WSMP,
};
use super::{MIN_RATE, WORK_RATE};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use sdrkit_proto_wlan_phy::mac::push_fcs;
use sdrkit_proto_wlan_phy::ofdm::{build_ppdu, Rate};

sdrkit_core::choice_enum! {
    /// The data rate of the test frames in the 10 MHz channel.
    pub enum GenRate {
        /// 3 Mbit/s: BPSK 1/2.
        Mbps3 = ("3", "3 Mbit/s (BPSK 1/2)"),
        /// 6 Mbit/s: QPSK 1/2.
        Mbps6 = ("6", "6 Mbit/s (QPSK 1/2)"),
    }
}

/// The data rate of the test frames.
pub const GEN_RATE: Setting<field::Choice<GenRate>> =
    Setting::choice("gen_rate", "Test signal data rate", GenRate::Mbps6)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `v2x` stack: the test signal settings.
    pub enum V2xSetting {
        /// The data rate of the test frames.
        GenRate = GEN_RATE,
    }
}

/// The period of the pair of frames, s.
pub const PERIOD_S: f64 = 0.1;
/// The scrambler seed of the test frames.
const SEED: u8 = 0x5D;

/// The basic safety message of the test signal: the published example (message 72, id 00002894).
pub fn test_bsm() -> Bsm {
    Bsm {
        msg_count: 72,
        id: 0x0000_2894,
        sec_mark: 48_275,
        position: Some((40.236_412_7, -83.366_735_1)),
        elevation_m: Some(275.0),
        transmission: 7,
        speed_m_s: Some(6.1),
        heading_deg: Some(275.0),
    }
}

/// The cooperative awareness message of the test signal: station CAFE01, a car at 54.68, 25.28.
pub fn test_cam() -> Cam {
    Cam {
        protocol: 2,
        station_id: 0x00CA_FE01,
        generation_delta_ms: 1000,
        station_type: 5,
        position: Some((54.68, 25.28)),
    }
}

/// The MPDU with FCS of a WSMP frame that holds the test BSM.
pub fn bsm_psdu() -> Vec<u8> {
    let mut f = Vec::new();
    write_data_header([0x02, 0, 0, 0, 0, 0x01], 1, ETHERTYPE_WSMP, &mut f);
    // A WSMP header: version 3, PSID 0x20, no extensions, then the length of the data.
    let mut msg = Vec::new();
    write_bsm(&test_bsm(), &BsmTail::default(), &mut msg);
    f.extend_from_slice(&[0x03, 0x20, 0x00, msg.len() as u8]);
    f.extend_from_slice(&msg);
    push_fcs(&mut f);
    f
}

/// The MPDU with FCS of a GeoNetworking frame that holds the test CAM.
pub fn cam_psdu() -> Vec<u8> {
    let mut f = Vec::new();
    write_data_header([0x02, 0, 0, 0, 0, 0x02], 2, ETHERTYPE_GEONET, &mut f);
    // Basic header, common header (single-hop broadcast, next header BTP-B) and the single-hop extended header as zeros.
    f.extend_from_slice(&[
        0x11, 0x00, 0x1A, 0x01, 0x20, 0x50, 0x02, 0x80, 0x00, 0x24, 0x01, 0x00,
    ]);
    f.extend_from_slice(&[0u8; 28]);
    f.extend_from_slice(&CAM_PORT);
    f.extend_from_slice(&[0x00, 0x00]);
    write_cam(&test_cam(), CAM_MIN + 2, &mut f);
    push_fcs(&mut f);
    f
}

/// One period of the test signal at 10 Msps: the two PPDUs at 10 ms and 50 ms.
pub fn period(rate: GenRate) -> Vec<Complex32> {
    let r = match rate {
        GenRate::Mbps3 => Rate::R6,
        GenRate::Mbps6 => Rate::R12,
    };
    let n = (PERIOD_S * WORK_RATE) as usize;
    let mut x = vec![Complex32::default(); n];
    for (at_s, psdu) in [(0.01, bsm_psdu()), (0.05, cam_psdu())] {
        let mut ppdu = Vec::new();
        if build_ppdu(&psdu, r, SEED, &mut ppdu).is_err() {
            continue;
        }
        let at = (at_s * WORK_RATE) as usize;
        let end = (at + ppdu.len()).min(n);
        x[at..end].copy_from_slice(&ppdu[..end - at]);
    }
    x
}

/// The looping test signal.
pub struct V2xGenerator {
    rate: f64,
    block: Vec<Complex32>,
    pos: usize,
}

impl V2xGenerator {
    /// The test signal of the settings of `cfg` at its rate.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below 10 Msps.
    pub fn from_config(cfg: &GenConfig) -> Result<V2xGenerator, ConfigError> {
        if cfg.rate_hz < MIN_RATE * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: cfg.rate_hz,
            });
        }
        let x = period(cfg.settings.get(&GEN_RATE));
        let block = if (cfg.rate_hz - WORK_RATE).abs() < 1e-6 {
            x
        } else {
            let mut r = Resampler::<Complex32>::new(WORK_RATE, cfg.rate_hz).map_err(|_| {
                ConfigError::RateTooLow {
                    need: MIN_RATE,
                    got: cfg.rate_hz,
                }
            })?;
            let mut y =
                Vec::with_capacity((x.len() as f64 * cfg.rate_hz / WORK_RATE) as usize + 64);
            r.process(&x, &mut y);
            y
        };
        Ok(V2xGenerator {
            rate: cfg.rate_hz,
            block,
            pos: 0,
        })
    }
}

impl Generator for V2xGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        if self.block.is_empty() {
            out.fill(Complex32::default());
            return out.len();
        }
        for z in out.iter_mut() {
            *z = self.block[self.pos];
            self.pos += 1;
            if self.pos == self.block.len() {
                self.pos = 0;
            }
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(WORK_RATE))
    }
}
