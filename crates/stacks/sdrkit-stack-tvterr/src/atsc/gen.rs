//! The ATSC 1.0 test signal: the test programme (or numbered transport packets), the field encoder, 8 levels plus the 1.25 pilot, the VSB baseband centred
//! on the channel (symbols turned by (−j)^k and shaped by the root-raised-cosine pulse at one sample per symbol), resampled to
//! the output rate. It makes the clean signal at unit mean power, and the framework adds the impairments.
//!
//! Standard: ATSC A/53 Part 2 clause 6.

use super::fec::{FieldEncoder, DATA_SEGS};
use super::rx::{vsb_pulse, FSYM};
use crate::band::resampler;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::source::{
    Data, Fill, NullSource, Numbered, NumberedSource, PacketSource, TestMux, TestService, TestSound, TestVideo,
};

/// The bandwidth the SNR of the test signal is measured in.
pub const NOISE_BANDWIDTH_HZ: f64 = 5.38e6;

/// The net bit rate: 312 packets of 188 bytes per field.
pub const BIT_RATE: f64 = 312.0 * 188.0 * 8.0 * FSYM / (313.0 * 832.0);

/// The service id of the test programme.
pub const SERVICE_ID: u16 = 1;
/// The name of the test programme in the virtual channel table.
pub const SERVICE_NAME: &str = "SDRKIT";
/// The major channel number of the test programme: it is channel 39.1.
pub const MAJOR_CHANNEL: u16 = 39;
/// The PID of the numbered packets of the test programme.
pub const DATA_PID: u16 = 0x0101;
/// The PID of the sound of the test programme.
pub const SOUND_PID: u16 = 0x0102;
/// The PID of the video of the test programme.
pub const VIDEO_PID: u16 = 0x0103;

/// The transport packets of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtscPackets {
    /// Numbered packets of this pattern, every packet.
    Numbered(Numbered),
    /// The test programme of [`programme`].
    Programme,
}

/// The generator's options.
#[derive(Clone, Debug, PartialEq)]
pub struct AtscGenParams {
    /// Output rate.
    pub rate_hz: Hz,
    /// The packets. The default is the numbered packets of [`Numbered::TERRESTRIAL`], which the measurements count.
    pub packets: AtscPackets,
}

impl Default for AtscGenParams {
    fn default() -> Self {
        AtscGenParams {
            rate_hz: 8e6,
            packets: AtscPackets::Numbered(Numbered::TERRESTRIAL),
        }
    }
}

/// The test programme: a PAT, a PMT and a terrestrial virtual channel table that name service [`SERVICE_ID`] channel
/// [`MAJOR_CHANNEL`].1 [`SERVICE_NAME`], its colour bars on [`VIDEO_PID`] and its tone on [`SOUND_PID`] (which carries the
/// PCR), and one packet in 7 of the rest a numbered packet ([`Fill::I31N7`]) on [`DATA_PID`], at the net bit rate.
///
/// # Errors
///
/// None in practice: the options are fixed and valid.
pub fn programme() -> Result<TestMux, sdrkit_proto_mpegts::Error> {
    TestMux::builder(1)
        .service(TestService::tv(SERVICE_ID, SERVICE_NAME, 0))
        .virtual_channels(MAJOR_CHANNEL)
        .data(DATA_PID, Data::Numbered(Fill::I31N7))
        .data_every(7)
        .sound(TestSound {
            pid: SOUND_PID,
            mux_bit_rate: BIT_RATE,
        })
        .video(TestVideo {
            pid: VIDEO_PID,
            mux_bit_rate: BIT_RATE,
        })
        .build()
}

/// The generator.
pub struct AtscGenerator {
    p: AtscGenParams,
    enc: FieldEncoder,
    source: Box<dyn PacketSource>,
    k: usize,
    pulse: Fir<Complex32>,
    rs: FractionalResampler<Complex32>,
    pending: Vec<Complex32>,
    at: usize,
    lv: Vec<f32>,
    tmp: Vec<Complex32>,
    pk: Vec<[u8; TS]>,
}

impl AtscGenerator {
    /// A generator with these options.
    ///
    /// # Errors
    ///
    /// [`DspError`] if no resampler serves the output rate.
    pub fn new(p: AtscGenParams) -> Result<AtscGenerator, DspError> {
        Ok(AtscGenerator {
            // The pulse leaves ±3.0 MHz: flat to there, cut where the first alias or image of that band begins.
            rs: resampler(FSYM, p.rate_hz, 3.0e6, p.rate_hz.min(FSYM) - 3.0e6)?,
            source: match p.packets {
                AtscPackets::Numbered(n) => Box::new(NumberedSource::new(n)),
                AtscPackets::Programme => match programme() {
                    Ok(m) => Box::new(m),
                    Err(_) => Box::new(NullSource),
                },
            },
            p,
            enc: FieldEncoder::new(),
            k: 0,
            pulse: Fir::new(&vsb_pulse()),
            pending: Vec::new(),
            at: 0,
            lv: Vec::new(),
            tmp: Vec::new(),
            pk: Vec::with_capacity(DATA_SEGS),
        })
    }

    /// From the engine's configuration: the test programme (the stack has no test-signal options of its own).
    ///
    /// # Errors
    ///
    /// As [`AtscGenerator::new`].
    pub fn from_config(cfg: &GenConfig) -> Result<AtscGenerator, DspError> {
        AtscGenerator::new(AtscGenParams {
            rate_hz: cfg.rate_hz,
            packets: AtscPackets::Programme,
        })
    }

    fn next_field(&mut self) {
        self.pk.clear();
        let mut p = [0u8; TS];
        for _ in 0..DATA_SEGS {
            self.source.next_packet(&mut p);
            self.pk.push(p);
        }
        self.lv.clear();
        self.enc.encode_field(&self.pk, &mut self.lv);
        self.tmp.clear();
        // Unit power at the output: levels power 21 + pilot 1.5625, through a pulse of unit energy.
        let g = (1.0f32 / (21.0 + 1.5625)).sqrt();
        let rot = [
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, -1.0),
            Complex32::new(-1.0, 0.0),
            Complex32::new(0.0, 1.0),
        ];
        for &l in &self.lv {
            let b = (l + 1.25) * g;
            let s = rot[self.k & 3] * b;
            self.k = self.k.wrapping_add(1);
            self.tmp.push(s);
        }
        self.pulse.process_in_place(&mut self.tmp);
        self.pending.clear();
        self.rs.process(&self.tmp, &mut self.pending);
        self.at = 0;
    }

    /// Seconds of one field.
    pub fn field_seconds() -> f64 {
        313.0 * 832.0 / FSYM
    }
}

impl Generator for AtscGenerator {
    fn rate_hz(&self) -> Hz {
        self.p.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut w = 0;
        while w < out.len() {
            if self.at >= self.pending.len() {
                self.next_field();
                continue;
            }
            let n = (out.len() - w).min(self.pending.len() - self.at);
            out[w..w + n].copy_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            w += n;
        }
        w
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(NOISE_BANDWIDTH_HZ)
                .with_symbol_rate(FSYM)
                .with_bit_rate(BIT_RATE),
        )
    }
}
