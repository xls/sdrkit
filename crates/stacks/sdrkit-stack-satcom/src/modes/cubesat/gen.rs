//! The CubeSat test signal: made-up satellites send one frame of each framing on one FM channel in an endless cycle. Noise and
//! the carrier offset come from the standard `gen_*` settings. A named submode sends the frame of its framing alone.

use super::demod::{Framing, Submode};
use super::framings as fr;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::modulate::Afsk;
use sdrkit_algo_sim::modulate::{Cpm, Fm, Linear};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Complex32, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig};
use sdrkit_proto_link::ax25::{Address, Frame};
use sdrkit_proto_link::csp::{Ax100, Header, AX100_SYNC};
use sdrkit_proto_link::hdlc::{Line, Transmitter};
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio};

sdrkit_core::choice_enum! {
    /// The framings the cycle can be limited to.
    pub enum GenFraming {
        /// Every framing in turn.
        All = ("0", "Every framing in turn"),
        /// AX.25 AFSK 1200.
        Afsk = ("1", "AX.25 AFSK 1200"),
        /// AX.25 G3RUH 9600.
        G3ruh = ("2", "AX.25 G3RUH 9600"),
        /// AX100 Mode 5 at 9600 bit/s.
        Mode5Fast = ("3", "AX100 Mode 5, 9600"),
        /// AX100 Mode 5 at 4800 bit/s.
        Mode5Slow = ("4", "AX100 Mode 5, 4800"),
        /// AX100 Mode 6 at 9600 bit/s.
        Mode6 = ("5", "AX100 Mode 6, 9600"),
        /// CCSDS RS at 9600 bit/s.
        Ccsds = ("6", "CCSDS RS, 9600"),
        /// Geoscan at 9600 bit/s.
        Geoscan = ("7", "Geoscan, 9600"),
        /// Lucky-7 at 4800 bit/s.
        Lucky7 = ("8", "Lucky-7, 4800"),
        /// Spino at 2400 bit/s.
        Spino = ("9", "Spino, 2400"),
        /// Erminaz at 9600 bit/s.
        Erminaz = ("10", "Erminaz, 9600"),
        /// USP at 9600 bit/s.
        Usp = ("11", "USP, 9600"),
        /// NGHam at 4800 bit/s.
        Ngham = ("12", "NGHam, 4800"),
        /// U482C on AFSK at 4800 bit/s.
        U482c = ("13", "U482C, AFSK 4800"),
        /// Mobitex-NX at 4800 bit/s.
        Mobitex = ("14", "Mobitex-NX, 4800"),
        /// CCSDS concatenated at 9600 bit/s.
        CcsdsConcat = ("15", "CCSDS concatenated, 9600"),
    }
}

/// The framing of the test signal.
pub const GEN_FRAMING: Setting<field::Choice<GenFraming>> =
    Setting::choice("gen_framing", "Framing", GenFraming::All)
        .section("Test signal")
        .advanced();

/// The working rate of the cycle, Hz.
pub const WR: f64 = 96_000.0;
/// The FM deviation of the AFSK and G3RUH audio, Hz.
pub const DEVIATION_HZ: f64 = 3000.0;
/// The FM deviation of the U482C audio, Hz.
const U482C_DEVIATION_HZ: f64 = 2800.0;
/// The U482C audio carrier, Hz.
const U482C_CARRIER_HZ: f64 = 3600.0;
/// The carrier before and after a frame of a burst framing, s.
const BURST_GAP_S: f64 = 0.075;
/// The CCSDS frame length the cycle sends, bytes.
pub const CCSDS_LEN: usize = 255;

/// What one frame of the cycle carries.
#[derive(Clone, Debug, PartialEq)]
pub struct SentFrame {
    /// The framing.
    pub framing: Framing,
    /// The bit rate.
    pub baud: u32,
    /// AX.25: the frame without its FCS. AX100: the CSP packet. CCSDS: the data field.
    pub data: Vec<u8>,
}

fn ax25_ui(dest: &str, src: &str, info: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let f = Frame::ui(
        Address::parse(dest).expect("a callsign"),
        Address::parse(src).expect("a callsign"),
        &[],
        info,
    )
    .expect("a UI frame");
    f.encode(&mut out).expect("a short frame");
    out
}

/// The frames of the cycle, in order.
pub fn frames() -> Vec<SentFrame> {
    let csp = |port: u8| Header {
        priority: 2,
        source: 1,
        destination: 10,
        dest_port: port,
        source_port: 7,
        flags: 0,
    };
    let mut p5 = csp(10).to_bytes().to_vec();
    p5.extend_from_slice(b"TEST-SAT AX100 MODE 5 TELEMETRY 0123456789");
    let mut p6 = csp(11).to_bytes().to_vec();
    p6.extend_from_slice(b"TEST-SAT AX100 MODE 6 BEACON");
    let mut cc: Vec<u8> = b"TEST-SAT CCSDS FRAME ".to_vec();
    cc.resize(CCSDS_LEN - 32, 0x55);
    let sent = |framing, baud, data| SentFrame {
        framing,
        baud,
        data,
    };
    vec![
        sent(
            Framing::Afsk,
            1200,
            ax25_ui("CQ", "RS0ISS", b"=4807.83N/01134.50E-ISS ARISS PACKET TEST"),
        ),
        sent(
            Framing::G3ruh,
            9600,
            ax25_ui("CQ", "TSTSAT-1", b"TEST-SAT 9600 BAUD TELEMETRY FRAME"),
        ),
        sent(Framing::Mode5, 9600, p5.clone()),
        sent(Framing::Mode5, 4800, p5),
        sent(Framing::Mode6, 9600, p6),
        sent(Framing::Ccsds, 9600, cc),
    ]
    .into_iter()
    .chain(burst_frames())
    .collect()
}

/// The frames of the burst framings, in the order of [`GenFraming`] 7 to 15: what the receiver reports for each.
pub fn burst_frames() -> Vec<SentFrame> {
    let mut rng = Rng::new(0x5EED);
    let mut bytes = |n: usize| -> Vec<u8> { (0..n).map(|_| rng.next_u32() as u8).collect() };
    let geoscan = {
        let mut d = bytes(64);
        let n = d.len();
        d.extend_from_slice(
            &sdrkit_util_bits::crc::catalog::CRC16_CMS
                .checksum(&d[..n])
                .to_be_bytes(),
        );
        d
    };
    let mut lucky7 = bytes(35);
    lucky7[1] = 0;
    let spino = fr::spino_encode(&[0xAA; 16], &bytes(40));
    let mut erminaz = bytes(223);
    erminaz[..2].copy_from_slice(&[0x44, 0x85]);
    erminaz[2..5].copy_from_slice(&[0x00, 0x01, 0x02]);
    let usp = ax25_ui("CQ", "RS20S", b"TEST-SAT USP BEACON 0123456789");
    let mut csp = Header {
        priority: 2,
        source: 3,
        destination: 10,
        dest_port: 31,
        source_port: 0,
        flags: 0,
    }
    .to_bytes()
    .to_vec();
    csp.extend_from_slice(b"TEST-SAT U482C TELEMETRY");
    let blocks: Vec<[u8; 18]> = (0..4u8).map(|b| [b * 0x11 + 1; 18]).collect();
    let mobitex = {
        let mut f = vec![0x23, 0x01];
        f.extend_from_slice(b"DP0TST");
        f.extend_from_slice(
            &sdrkit_util_bits::crc::catalog::CRC16_XMODEM
                .checksum(b"DP0TST")
                .to_be_bytes(),
        );
        for b in &blocks {
            f.extend_from_slice(b);
        }
        f.extend_from_slice(&[0xAA, 0, 0, 0, 0, 0xBB]);
        f
    };
    let mut concat: Vec<u8> = b"TEST-SAT CCSDS CONCATENATED FRAME ".to_vec();
    concat.resize(223, 0x33);
    let sent = |framing, baud, data| SentFrame {
        framing,
        baud,
        data,
    };
    vec![
        sent(Framing::Geoscan, 9600, geoscan),
        sent(Framing::Lucky7, 4800, lucky7),
        sent(Framing::Spino, 2400, spino),
        sent(Framing::Erminaz, 9600, erminaz),
        sent(Framing::Usp, 9600, usp),
        sent(Framing::Ngham, 4800, b"TEST-SAT NGHAM BEACON".to_vec()),
        sent(Framing::U482c, 4800, csp),
        sent(Framing::MobitexNx, 4800, mobitex),
        sent(Framing::CcsdsConcat, 9600, concat),
    ]
}

/// The line bits of a burst frame after its preamble: the sync word and the frame.
fn burst_bits(f: &SentFrame) -> Vec<u8> {
    let mut bits = Vec::new();
    match f.framing {
        Framing::Geoscan => {
            bits_msb(&fr::GEOSCAN_SYNC.to_be_bytes(), &mut bits);
            bits_msb(&fr::geoscan_encode(&f.data[..f.data.len() - 2]), &mut bits);
        }
        Framing::Lucky7 => {
            let mut p = [0u8; 35];
            p.copy_from_slice(&f.data);
            bits_msb(&fr::lucky7_encode(&p), &mut bits);
        }
        Framing::Spino => {
            bits_msb(&fr::SPINO_SYNC.to_be_bytes(), &mut bits);
            bits_msb(&f.data, &mut bits);
        }
        Framing::Erminaz => {
            let mut coded = Vec::new();
            ChannelCoder::new(ChannelConfig::rs_223(1))
                .expect("a CCSDS code")
                .encode(&f.data, &mut coded)
                .expect("a frame of the code's length");
            bits_msb(&coded, &mut bits);
        }
        Framing::Usp => {
            bits_msb(&fr::USP_SYNC.to_be_bytes(), &mut bits);
            bits.extend(fr::usp_encode(&f.data));
        }
        Framing::Ngham => {
            bits_msb(&fr::NGHAM_SYNC.to_be_bytes(), &mut bits);
            bits_msb(
                &fr::ngham_encode(&f.data, 0, true).expect("a short payload"),
                &mut bits,
            );
        }
        Framing::U482c => {
            bits_msb(&fr::U482C_SYNC.to_be_bytes(), &mut bits);
            let body = fr::u482c_encode(&f.data, fr::U482C_RS | fr::U482C_RANDOMISER)
                .expect("a short packet");
            bits_msb(&body, &mut bits);
        }
        Framing::MobitexNx => {
            bits_msb(&fr::MOBITEX_NX_SYNC.to_be_bytes(), &mut bits);
            let blocks: Vec<[u8; 18]> = f.data[10..10 + 4 * 18]
                .chunks(18)
                .map(|c| c.try_into().expect("18 bytes"))
                .collect();
            bits.extend(fr::mobitex_encode(
                fr::MobitexLayout::Nx,
                f.data[0],
                f.data[1],
                "DP0TST",
                &blocks,
            ));
        }
        _ => {}
    }
    bits
}

/// Bytes as bits, first bit the most significant of the first byte.
fn bits_msb(b: &[u8], out: &mut Vec<u8>) {
    out.extend(
        b.iter()
            .flat_map(|v| (0..8).rev().map(move |i| (v >> i) & 1)),
    );
}

/// The FM baseband (at [`WR`], unit amplitude) of one frame, with 40 ms of carrier before and after.
pub fn render(f: &SentFrame) -> Vec<Complex32> {
    let gap = (0.04 * WR) as usize;
    let mut out = vec![Complex32::new(1.0, 0.0); gap];
    let fm = |audio: &[f32], out: &mut Vec<Complex32>| {
        let mut m = Fm::new(WR, DEVIATION_HZ).expect("a deviation below half the rate");
        let start = out.len();
        out.resize(start + audio.len(), Complex32::default());
        m.process(audio, &mut out[start..])
            .expect("the same lengths");
    };
    let mut audio = Vec::new();
    match f.framing {
        Framing::Afsk => {
            afsk1200_audio(&[&f.data], 24, WR, &mut audio).expect("an AX.25 frame");
            fm(&audio, &mut out);
        }
        Framing::G3ruh => {
            g3ruh9600_audio(&[&f.data], 16, WR, &mut audio).expect("an AX.25 frame");
            fm(&audio, &mut out);
        }
        f2 if f2.is_burst() => {
            let gap = (BURST_GAP_S * WR) as usize;
            out.resize(gap, Complex32::new(1.0, 0.0));
            let mut bits = Vec::new();
            bits_msb(&[0x55; 48], &mut bits);
            if f2 == Framing::CcsdsConcat {
                bits.extend(
                    fr::concat_encode(&fr::ConcatConfig::default(), &f.data)
                        .expect("a frame of the default size"),
                );
            } else {
                bits.extend(burst_bits(f));
                bits_msb(&[0x55; 4], &mut bits);
            }
            if f2 == Framing::U482c {
                // AFSK on the audio carrier, a tone above it for a 1, then FM.
                let mut a = Afsk::new(
                    WR,
                    f64::from(f.baud),
                    U482C_CARRIER_HZ + 1200.0,
                    U482C_CARRIER_HZ - 1200.0,
                )
                .expect("tones below half the rate");
                a.push(&bits, &mut audio);
                a.flush(&mut audio);
                let mut m =
                    Fm::new(WR, U482C_DEVIATION_HZ).expect("a deviation below half the rate");
                let start = out.len();
                out.resize(start + audio.len(), Complex32::default());
                m.process(&audio, &mut out[start..])
                    .expect("the same lengths");
            } else {
                // Gaussian filtered FSK, BT 0.5, deviation 1.25 x bit rate / 4.
                let levels: Vec<f32> = bits
                    .iter()
                    .map(|&b| if b == 1 { 1.0 } else { -1.0 })
                    .collect();
                let mut m = Cpm::gfsk(0.5, 0.625, WR / f.baud as f64).expect("a GFSK modulator");
                m.push(&levels, &mut out);
                m.flush(&mut out);
            }
            out.extend(std::iter::repeat_n(Complex32::new(1.0, 0.0), gap));
            return out;
        }
        Framing::Mode6 => {
            // The RS coded packet in an HDLC frame with no FCS, G3RUH scrambled, as raised-cosine levels.
            let mut payload = Vec::new();
            Ax100::new()
                .expect("the AX100 codes")
                .mode6_encode(&f.data, &mut payload)
                .expect("a short packet");
            let mut bits = Vec::new();
            let mut tx = Transmitter::new(Line::G3ruh);
            tx.encode_without_fcs(&[&payload], 16, &mut bits);
            tx.flags(4, &mut bits);
            let symbols: Vec<Complex32> = bits
                .iter()
                .map(|&b| Complex32::new(if b == 1 { 1.0 } else { -1.0 }, 0.0))
                .collect();
            let mut m = Linear::new(Shape::Rc { beta: 0.5 }, 8.0, WR / 9600.0).expect("a pulse");
            let mut iq = Vec::new();
            m.push(&symbols, &mut iq);
            m.flush(&mut iq);
            audio.extend(iq.iter().map(|s| s.re));
            fm(&audio, &mut out);
        }
        _ => {
            // AX100 Mode 5 and CCSDS: a preamble, the sync word and the coded bytes, GMSK.
            let mut bits: Vec<u8> = (0..64).map(|i| (i % 2 == 0) as u8).collect();
            if f.framing == Framing::Mode5 {
                bits_msb(&AX100_SYNC.to_be_bytes(), &mut bits);
                let mut coded = Vec::new();
                Ax100::new()
                    .expect("the AX100 codes")
                    .mode5_encode(&f.data, &mut coded)
                    .expect("a short packet");
                bits_msb(&coded, &mut bits);
            } else {
                let cfg = ChannelConfig::rs_223(1).with_virtual_fill(255 - CCSDS_LEN);
                let mut coded = Vec::new();
                ChannelCoder::new(cfg)
                    .expect("a CCSDS code")
                    .encode(&f.data, &mut coded)
                    .expect("a frame of the code's length");
                bits_msb(&coded, &mut bits);
            }
            bits.extend((0..16).map(|i| (i % 2 == 0) as u8));
            let levels: Vec<f32> = bits
                .iter()
                .map(|&b| if b == 1 { 1.0 } else { -1.0 })
                .collect();
            let mut m = Cpm::gmsk(0.5, WR / f.baud as f64).expect("a GMSK modulator");
            m.push(&levels, &mut out);
            m.flush(&mut out);
        }
    }
    out.extend(std::iter::repeat_n(Complex32::new(1.0, 0.0), gap));
    out
}

/// The test signal generator.
pub struct CubesatGenerator {
    rate: f64,
    cycle: Vec<Complex32>,
    pos: usize,
    rs: Resampler<Complex32>,
    work: Vec<Complex32>,
    resampled: Vec<Complex32>,
    held: std::collections::VecDeque<Complex32>,
    /// Moves the station to the tune offset below the centre, where a radio tuned with the stack's offset sees it.
    nco: sdrkit_algo_dsp::nco::Nco,
    /// The frames in the cycle, in order.
    pub sent: Vec<SentFrame>,
}

impl CubesatGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let mut which = usize::from(cfg.settings.get(&GEN_FRAMING).index());
        // A named submode sends its own framing.
        if let Some(i) = Submode::NAMED
            .iter()
            .position(|(_, id)| Some(*id) == cfg.submode.as_deref())
        {
            which = 7 + i;
        }
        Self::new(cfg.rate_hz, which)
    }

    /// `which` 0: every framing in turn; 1 to 15: that one (in the order of [`frames`]).
    pub fn new(rate: f64, which: usize) -> Self {
        let all = frames();
        let sent: Vec<SentFrame> = match which {
            1..=15 => vec![all[which - 1].clone()],
            _ => all,
        };
        let cycle: Vec<Complex32> = sent.iter().flat_map(render).collect();
        CubesatGenerator {
            rate,
            cycle,
            pos: 0,
            rs: Resampler::new(WR, rate).expect("a resampler"),
            work: Vec::with_capacity(1024),
            resampled: Vec::new(),
            held: std::collections::VecDeque::new(),
            nco: sdrkit_algo_dsp::nco::Nco::new(rate, -super::DESCRIPTOR.tuning.tune_offset_hz),
            sent,
        }
    }

    /// The length of one cycle, seconds.
    pub fn cycle_s(&self) -> f64 {
        self.cycle.len() as f64 / WR
    }
}

impl Generator for CubesatGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.held.is_empty() {
                self.work.clear();
                for _ in 0..1024 {
                    self.work.push(self.cycle[self.pos]);
                    self.pos = (self.pos + 1) % self.cycle.len();
                }
                self.resampled.clear();
                self.rs.process(&self.work, &mut self.resampled);
                self.held.extend(self.resampled.iter().copied());
            }
            *o = self.held.pop_front().unwrap_or_default() * self.nco.next_sample();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The carrier to noise is stated in 25 kHz, the width of an FM channel.
        Some(SignalInfo::new(25e3))
    }
}
