//! The `subghzchat` stack: text chat with Flipper Zero devices on CC1101 GFSK packets, joined into lines. Encrypted lines open
//! only with the key the user types for a chat the user takes part in.
//!
//! Standard: TI CC1101 data sheet (packet engine, CRC); NIST SP 800-38D (AES-GCM); FIPS 180-4 (SHA-256).

use crate::family::{self, at_least_value, hex, min_rate_message, CleanScene, REPORT_S};
use crate::typed::{
    SubghzchatFields, SubghzchatLineEvent, SubghzchatLineRow, SubghzchatPacketRow,
    SubghzchatStateEvent, SUBGHZCHAT_CHAT, SUBGHZCHAT_LINE, SUBGHZCHAT_PACKETS, SUBGHZCHAT_STATE,
};
use sdrkit_algo_crypto::{AesKey, Gcm, Key};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
use sdrkit_algo_modem::gmsk::{GfskConfig, GfskDemod};
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, ReadFields};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, StateBlob, StateError,
    Status, Tuning, Value, Values,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Kept, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_util_bits::crc::catalog::CRC16_CMS;
use sdrkit_util_bits::pack::{self, BitOrder};
use sdrkit_util_bits::sync::SyncWord;
use sdrkit_util_objects::commands;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Symbol rate: 26 MHz × (256 + 0x93) × 256 / 2^28 (CC1101 MDMCFG3 and MDMCFG4 of the firmware preset).
pub const BAUD: f64 = 26e6 * (256.0 + 147.0) * 256.0 / 268_435_456.0;
/// Deviation: 26 MHz / 2^17 × (8 + 4) × 8 (CC1101 DEVIATN of the firmware preset).
pub const DEV: f64 = 26e6 / 131072.0 * 12.0 * 8.0;
/// The sync word.
pub const SYNC: u16 = 0x464C;
/// The Gaussian bandwidth-time product of the preset.
pub const BT: f64 = 0.5;
/// The radio is tuned this far above the chat frequency, so its carrier leak stays off the chat.
pub const TUNE_OFFSET_HZ: f64 = 300e3;
/// The least sample rate.
pub const MIN_RATE_HZ: f64 = 200e3;
/// The longest message, octets.
pub const MAX_MESSAGE: usize = 1024;
/// Chat lines kept.
pub const CHAT_CAP: usize = 500;
/// Packets kept in the log.
pub const PACKET_CAP: usize = 200;

sdrkit_core::choice_enum! {
    /// The chat frequencies.
    pub enum ChatFrequency {
        /// 433.92 MHz.
        F433 = ("433.92", "433.92 MHz"),
        /// 315.00 MHz.
        F315 = ("315.00", "315.00 MHz"),
        /// 390.00 MHz.
        F390 = ("390.00", "390.00 MHz"),
        /// 868.35 MHz.
        F868 = ("868.35", "868.35 MHz"),
        /// 915.00 MHz.
        F915 = ("915.00", "915.00 MHz"),
    }
}

impl ChatFrequency {
    /// The frequency, Hz.
    pub fn hz(self) -> f64 {
        match self {
            ChatFrequency::F433 => 433.92e6,
            ChatFrequency::F315 => 315.00e6,
            ChatFrequency::F390 => 390.00e6,
            ChatFrequency::F868 => 868.35e6,
            ChatFrequency::F915 => 915.00e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// Where the key of encrypted lines comes from.
    pub enum Encryption {
        /// No key.
        Off = ("off", "Off"),
        /// The SHA-256 of the password.
        Password = ("password", "Password"),
        /// The hex key.
        Hex = ("hex", "Hex key"),
    }
}
/// The amplitude of a constant added to the samples the burst detector sees: -100 dBFS. It adds a noise floor to digital
/// silence, so a burst in silence still ends.
const FLOOR_AMPLITUDE: f32 = 1e-5;

/// A packet on air: `pre` preamble octets, sync, length, payload, CRC-16/CMS over length and payload, high octet first.
pub fn packet(payload: &[u8], pre: usize) -> Vec<u8> {
    let mut p = vec![0xAA; pre];
    p.extend([0x46, 0x4C, payload.len() as u8]);
    p.extend_from_slice(payload);
    let c = CRC16_CMS.checksum(&p[pre + 2..]);
    p.extend(c.to_be_bytes());
    p
}

/// The key of a password: SHA-256 of its octets.
pub fn key_from_password(p: &str) -> [u8; 32] {
    sdrkit_algo_crypto::sha256(p.as_bytes())
}

/// The key of hex text (spaces, ':' and '-' ignored), or `None` unless the text has exactly 64 hex digits.
pub fn key_from_hex(s: &str) -> Option<[u8; 32]> {
    let digits: String = s
        .chars()
        .filter(|c| !matches!(c, ' ' | ':' | '-'))
        .collect();
    if digits.len() != 64 {
        return None;
    }
    family::from_hex(&digits)?.try_into().ok()
}

/// The AES-256-GCM cipher of a 32-octet key.
pub fn cipher(key: &[u8; 32]) -> Gcm {
    Gcm::new(&AesKey::Aes256(Key::new(*key)))
}

/// Whether octets are chat text: UTF-8 with no control character other than tab, CR and LF, and no 0x7F.
pub fn is_text(b: &[u8]) -> bool {
    std::str::from_utf8(b).is_ok()
        && b.iter()
            .all(|&c| c >= 0x20 && c != 0x7F || matches!(c, b'\t' | b'\r' | b'\n'))
}

/// A line for display: trailing CR and LF removed, tabs as spaces, other control characters and bad UTF-8 as '?'.
pub fn show(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    let s: String = s
        .chars()
        .map(|c| {
            if c == '\t' {
                ' '
            } else if c.is_control() && c != '\r' && c != '\n' || c == '\u{FFFD}' {
                '?'
            } else {
                c
            }
        })
        .collect();
    s.trim_end_matches(['\r', '\n']).to_string()
}

/// The sender of encrypted messages: a run id and a counter from 1.
#[derive(Clone, Debug)]
pub struct Sender {
    /// Run id.
    pub run_id: u64,
    /// The next counter.
    pub counter: u32,
}

/// Encrypts a line: run id (little-endian), counter (big-endian), IV, tag, ciphertext. The 12 header octets are the additional
/// authenticated data. Errors only for a line longer than GCM allows.
pub fn encrypt(
    key: &Gcm,
    s: &mut Sender,
    iv: [u8; 12],
    line: &[u8],
) -> Result<Vec<u8>, sdrkit_algo_crypto::Error> {
    let mut m = s.run_id.to_le_bytes().to_vec();
    m.extend(s.counter.to_be_bytes());
    let mut body = line.to_vec();
    let tag = key.encrypt_in_place(&iv, &m, &mut body)?;
    s.counter += 1;
    m.extend(iv);
    m.extend(tag);
    m.extend(body);
    Ok(m)
}

/// The run id, the counter and the plain line of an encrypted message, or `None` if the message is too short or the tag does
/// not match `key`.
pub fn decrypt(key: &Gcm, m: &[u8]) -> Option<(u64, u32, Vec<u8>)> {
    if m.len() < 41 {
        return None;
    }
    let iv: [u8; 12] = m[12..24].try_into().ok()?;
    let tag: [u8; 16] = m[24..40].try_into().ok()?;
    let mut body = m[40..].to_vec();
    key.decrypt_in_place(&iv, &m[..12], &mut body, &tag).ok()?;
    let run = u64::from_le_bytes(m[..8].try_into().ok()?);
    let ctr = u32::from_be_bytes(m[8..12].try_into().ok()?);
    Some((run, ctr, body))
}

/// The GFSK samples of a message, at unit amplitude and `offset_hz` from 0 Hz: packets of at most 60 octets 20 ms apart, `pre`
/// preamble octets each, the carrier ramped up over the first symbol and down over one extra symbol. `None` for a message
/// longer than 1024 octets or a rate below two samples per symbol.
pub fn tx_samples(msg: &[u8], rate: f64, offset_hz: f64, pre: usize) -> Option<Vec<Complex32>> {
    if msg.len() > MAX_MESSAGE || rate < 2.0 * BAUD {
        return None;
    }
    let sps = rate / BAUD;
    let mut m = Cpm::gfsk(BT, 2.0 * DEV / BAUD, sps)
        .and_then(|m| m.with_ramp(Ramp::RaisedCosine(sps)))
        .ok()?;
    let mut out = Vec::new();
    let mut bits = Vec::new();
    let mut levels = Vec::new();
    for (i, chunk) in msg.chunks(60).enumerate() {
        if i > 0 {
            out.resize(out.len() + (0.02 * rate) as usize, Complex32::default());
        }
        bits.clear();
        pack::unpack(&packet(chunk, pre), BitOrder::Msb, &mut bits);
        levels.clear();
        levels.extend(bits.iter().map(|&b| if b != 0 { 1.0f32 } else { -1.0 }));
        levels.push(*levels.last().unwrap_or(&1.0));
        m.push(&levels, &mut out);
        m.flush(&mut out);
    }
    if offset_hz != 0.0 {
        Nco::new(rate, offset_hz).mix_in_place(&mut out);
    }
    Some(out)
}

/// `sgc.front`: mixes the chat frequency to 0 Hz, filters with a Hamming-windowed sinc of 12 D + 1 taps and decimates by D to
/// about 200 ksps.
#[derive(Default)]
pub struct Front {
    nco: Option<Nco>,
    fir: Option<FirDecimator<Complex32>>,
    mixed: Vec<Complex32>,
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("sgc.front", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let d = ((f.rate / 200e3).round() as usize).max(1);
        let out = f.rate / d as f64;
        let cut = 65e3f64.min(0.45 * out) / f.rate;
        let taps = sdrkit_algo_dsp::fir::design_by_taps(12 * d + 1, cut, Window::Hamming)?;
        self.nco = Some(Nco::new(f.rate, -f.signal_offset_hz()));
        self.fir = Some(FirDecimator::new(&taps, d));
        let mut o = f.with_rate(out);
        o.centre_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let (Some(nco), Some(fir)) = (self.nco.as_mut(), self.fir.as_mut()) else {
            return;
        };
        self.mixed.resize(x.len(), Complex32::default());
        nco.mix(x, &mut self.mixed);
        let out = io.iq_out(0);
        fir.process(&self.mixed, out);
    }
    /// Runs the samples the filter still holds out: it feeds the filter as many zeros as it has taps.
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let Some(fir) = self.fir.as_mut() else { return };
        self.mixed.clear();
        self.mixed.resize(fir.num_taps(), Complex32::default());
        fir.process(&self.mixed, io.iq_out(0));
    }
}

/// A packet (`sgc.packet`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Packet {
    /// The payload.
    pub payload: Vec<u8>,
    /// The CRC is right.
    pub crc_ok: bool,
    /// Found with an inverted sync word (an exchanged I and Q).
    pub inverted: bool,
    /// The level, dBFS.
    pub level_dbfs: f32,
    /// The SNR, dB.
    pub snr_db: f32,
    /// The carrier offset from the chat frequency, Hz.
    pub offset_hz: f32,
    /// Signal time, seconds.
    pub time: f64,
}

impl Item for Packet {
    const KIND: &'static str = "sgc.packet";
}

/// The time of a burst, for the state lock state (`sgc.burst_time`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurstTime(pub f64);

impl Item for BurstTime {
    const KIND: &'static str = "sgc.burst_time";
}

/// The demodulator of one burst: the frequency offset by the midpoint of the 10th and 90th percentiles of the discriminator over
/// the burst body, then the GFSK discriminator receiver of sdrkit-algo-modem, then the sync search on the hard bits. Its buffers are
/// made once and reused.
pub struct BurstDemod {
    rate: f64,
    disc: Discriminator,
    gfsk: GfskDemod,
    freq: Vec<f32>,
    body: Vec<f64>,
    scratch: Vec<f64>,
    shifted: Vec<Complex32>,
    llr: Vec<f32>,
    bits: Vec<u8>,
    hits: Vec<(usize, sdrkit_util_bits::sync::Hit)>,
}

impl BurstDemod {
    /// A demodulator for bursts at `rate_hz` (at least two samples per symbol).
    ///
    /// # Errors
    ///
    /// The errors of the discriminator and of the GFSK demodulator for a rate they refuse.
    pub fn new(rate_hz: f64) -> Result<Self, ConfigError> {
        let sps = rate_hz / BAUD;
        let mut cfg = GfskConfig::new(sps, DEV / rate_hz, Some(BT));
        cfg.bn_t = 0.02;
        cfg.level_alpha = 0.05;
        Ok(BurstDemod {
            rate: rate_hz,
            disc: Discriminator::try_new(rate_hz, DEV)?,
            gfsk: GfskDemod::new(cfg)?.with_inverted(true),
            freq: Vec::new(),
            body: Vec::new(),
            scratch: Vec::new(),
            shifted: Vec::new(),
            llr: Vec::new(),
            bits: Vec::new(),
            hits: Vec::new(),
        })
    }

    /// Finds the packets of a burst: `x` its samples, `lead` the index where the signal starts, `t0` the signal time of `x[0]`,
    /// `noise` the floor power per sample. Packets go to `out`.
    pub fn packets(
        &mut self,
        x: &[Complex32],
        lead: usize,
        t0: f64,
        noise: f32,
        out: &mut Vec<Packet>,
    ) {
        let n = x.len();
        let sps = self.rate / BAUD;
        if (n as f64) < 40.0 * sps || lead >= n {
            return;
        }
        // The carrier offset and the level over the burst body.
        self.freq.resize(n, 0.0);
        self.disc.set_last(x[0]);
        self.disc.process_into(x, &mut self.freq);
        let a = (lead + (0.4e-3 * self.rate) as usize).min(n - 1);
        let e = n.saturating_sub(60).max(a + 1).min(n);
        self.body.clear();
        self.body
            .extend(self.freq[a..e].iter().map(|&v| v as f64 * DEV));
        let lo = quantile_f64(&self.body, 0.1, &mut self.scratch).unwrap_or(0.0);
        let hi = quantile_f64(&self.body, 0.9, &mut self.scratch).unwrap_or(0.0);
        let off = (lo + hi) / 2.0;
        let lvl = x[a..e].iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / (e - a) as f64;
        let snr = 10.0 * (lvl / noise.max(1e-20) as f64 - 1.0).max(1e-3).log10();
        // Bits from the signal start, with the offset taken off.
        self.shifted.clear();
        self.shifted.extend_from_slice(&x[lead..]);
        Nco::new(self.rate, -off).mix_in_place(&mut self.shifted);
        self.gfsk.reset();
        self.llr.clear();
        if self.gfsk.process(&self.shifted, &mut self.llr).is_err() {
            return;
        }
        self.bits.clear();
        self.bits.extend(self.llr.iter().map(|&l| (l < 0.0) as u8));
        // The sync word, normal or inverted, after a preamble octet with at most one wrong bit.
        let Ok(sync) = SyncWord::new(SYNC as u128, 16, 0) else {
            return;
        };
        let mut sync = sync.with_inverted(true);
        self.hits.clear();
        sync.scan(&self.bits, &mut self.hits);
        let bits = &self.bits;
        let mut next = 0usize;
        for &(i, hit) in &self.hits {
            if i < next || i < 24 {
                continue;
            }
            let pre = pack::to_u64(&bits[i - 24..i - 16]) as u8;
            if (pre ^ 0xAA).count_ones().min((pre ^ 0x55).count_ones()) > 1 {
                continue;
            }
            let inv = hit.inverted();
            let rd = |k: usize| -> Option<u8> {
                let v = pack::to_u64(bits.get(k..k + 8)?) as u8;
                Some(if inv { !v } else { v })
            };
            let l = match rd(i) {
                Some(l) if (1..=61).contains(&l) => l as usize,
                _ => continue,
            };
            let mut bytes = Vec::with_capacity(l + 3);
            bytes.push(l as u8);
            for j in 0..l + 2 {
                match rd(i + 8 + 8 * j) {
                    Some(v) => bytes.push(v),
                    None => break,
                }
            }
            if bytes.len() != l + 3 {
                continue;
            }
            let crc = (bytes[l + 1] as u16) << 8 | bytes[l + 2] as u16;
            let good = CRC16_CMS.checksum(&bytes[..l + 1]) == crc;
            out.push(Packet {
                payload: bytes[1..l + 1].to_vec(),
                crc_ok: good,
                inverted: inv,
                level_dbfs: (10.0 * lvl.max(1e-20).log10()) as f32,
                snr_db: snr as f32,
                offset_hz: off as f32,
                time: t0 + lead as f64 / self.rate + (i - 24) as f64 / BAUD,
            });
            if good {
                next = i + 8 * (l + 3);
            }
        }
    }
}

/// `sgc.packets`: the burst detector of sdrkit-algo-modem over the channel, then the demodulator of each burst.
#[derive(Default)]
pub struct Packets {
    rate: f64,
    window: usize,
    det: Option<BurstDetector>,
    demod: Option<BurstDemod>,
    seen: Vec<Complex32>,
    burst: Vec<Complex32>,
    found: Vec<Packet>,
    base: u64,
}

impl Packets {
    fn drain(&mut self, packets: &mut Vec<Timed<Packet>>, times: &mut Vec<Timed<BurstTime>>) {
        let (Some(det), Some(demod)) = (self.det.as_mut(), self.demod.as_mut()) else {
            return;
        };
        while let Some(b) = det.take() {
            let t0 = (self.base + b.start) as f64 / self.rate;
            let lead = (b.open - b.start) as usize;
            times.push(Timed {
                time: t0,
                value: BurstTime(t0),
            });
            self.burst.clear();
            self.burst.extend_from_slice(b.samples);
            self.found.clear();
            demod.packets(&self.burst, lead, t0, b.floor, &mut self.found);
            packets.extend(self.found.drain(..).map(|p| Timed {
                time: p.time,
                value: p,
            }));
        }
    }
}

impl Component for Packets {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sgc.packets",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Packet>("packets").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BurstTime>("burst_times").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        // Windows of about 100 µs (one symbol): open at 9 dB over the floor, close below 6 dB. 100 samples before, 80 after, at most 0.25 s.
        // The floor is the least window power, which is below the mean noise, so the close level must be above that mean.
        self.window = ((100e-6 * f.rate).round() as usize).max(1);
        let windows_per_s = f.rate / self.window as f64;
        let floor = MinFollower::with_times(windows_per_s, 0.01, 3.0)?;
        let mut cfg = BurstConfig::new(self.window, (0.25 * f.rate) as usize);
        cfg.open_db = 9.0;
        cfg.close_db = 6.0;
        cfg.pre = 100;
        cfg.post = 80;
        self.det = Some(BurstDetector::new(cfg, FloorTracker::MinFollower(floor))?);
        self.demod = Some(BurstDemod::new(f.rate)?);
        self.base = 0;
        Ok(vec![
            f.with_kind(Kind::typed::<Packet>()).with_rate(0.0),
            f.with_kind(Kind::typed::<BurstTime>()).with_rate(0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if self.det.is_none() {
            return;
        }
        let mut packets = Vec::new();
        let mut times = Vec::new();
        for c in x.chunks(self.window.max(1)) {
            self.seen.clear();
            self.seen
                .extend(c.iter().map(|&s| s + Complex32::new(FLOOR_AMPLITUDE, 0.0)));
            if let Some(d) = self.det.as_mut() {
                d.process(&self.seen);
            }
            self.drain(&mut packets, &mut times);
        }
        Self::send(io, packets, times);
    }
    /// Closes the burst that is still open (the input ended) and decodes it.
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let Some(d) = self.det.as_mut() else { return };
        d.end();
        let (mut packets, mut times) = (Vec::new(), Vec::new());
        self.drain(&mut packets, &mut times);
        Self::send(io, packets, times);
    }
}

impl Packets {
    fn send(io: &mut Io, packets: Vec<Timed<Packet>>, times: Vec<Timed<BurstTime>>) {
        if !packets.is_empty() {
            io.items_out::<Packet>(0).extend(packets);
        }
        if !times.is_empty() {
            io.items_out::<BurstTime>(1).extend(times);
        }
    }
}

sdrkit_core::choice_enum! {
    /// How a chat line was read.
    #[derive(Serialize, Deserialize)]
    pub enum LineKind {
        /// Plain text.
        Plain = ("plain", "Plain"),
        /// Decrypted with the user's key.
        Decrypted = ("decrypted", "Decrypted"),
        /// Encrypted, and this key does not open it.
        Unreadable = ("encrypted", "Encrypted"),
        /// An encrypted repeat that the replay guard dropped.
        Repeat = ("repeat", "Repeat"),
        /// Not text and too short to be encrypted: shown in hex.
        Octets = ("octets", "Octets"),
    }
}

/// One chat line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatLine {
    /// Signal time, seconds.
    pub time: f64,
    /// Heard (`false`) or sent by the user (`true`).
    pub sent: bool,
    /// How it was read.
    pub kind: LineKind,
    /// The text.
    pub text: String,
    /// The message octets in hex.
    pub hex: String,
    /// Packets joined.
    pub packets: usize,
    /// Level of the first packet, dBFS.
    pub level_dbfs: f32,
    /// SNR of the first packet, dB.
    pub snr_db: f32,
    /// Carrier offset of the first packet, Hz.
    pub offset_hz: f32,
    /// Run id (hex) and counter of an encrypted line.
    pub run: Option<(String, u32)>,
}

impl Shift for ChatLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// A line of the packet log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketLine {
    /// Signal time, seconds.
    pub time: f64,
    /// Payload length, octets.
    pub len: usize,
    /// The CRC is right.
    pub crc_ok: bool,
    /// Found inverted.
    pub inverted: bool,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// SNR, dB.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub offset_hz: f32,
    /// The payload in hex.
    pub hex: String,
}

impl Shift for PacketLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for ChatLine {
    type Row = SubghzchatLineRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let heard = !self.sent;
        let run = self.run.as_ref().map(|(r, c)| format!("{r} {c}"));
        out.put(&SubghzchatLineRow {
            time_s: self.time,
            own: self.sent,
            kind: self.kind,
            text: &self.text,
            packets: self.packets as i64,
            level_dbfs: heard.then_some(f64::from(self.level_dbfs)),
            snr_db: heard.then_some(f64::from(self.snr_db)),
            offset_hz: heard.then_some(Hertz(f64::from(self.offset_hz))),
            run: run.as_deref(),
        });
    }
}

impl Shaped for PacketLine {
    type Row = SubghzchatPacketRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SubghzchatPacketRow {
            time_s: self.time,
            length: self.len as i64,
            crc_ok: self.crc_ok,
            inverted: self.inverted,
            level_dbfs: f64::from(self.level_dbfs),
            snr_db: f64::from(self.snr_db),
            offset_hz: Hertz(f64::from(self.offset_hz)),
            hex: &self.hex,
        });
    }
}

/// What `sgc.chat` keeps across sessions: the chat, the packet log, the counters and the replay guard.
#[derive(Serialize, Deserialize)]
pub struct Store {
    chat: LogRing<ChatLine>,
    packets: LogRing<PacketLine>,
    good: u64,
    bad: u64,
    bursts: u64,
    messages: u64,
    decrypted: u64,
    unreadable: u64,
    repeats: u64,
    sent: u64,
    /// The last counter accepted from each run id.
    guard: BTreeMap<u64, u32>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            chat: LogRing::new(CHAT_CAP),
            packets: LogRing::new(PACKET_CAP),
            good: 0,
            bad: 0,
            bursts: 0,
            messages: 0,
            decrypted: 0,
            unreadable: 0,
            repeats: 0,
            sent: 0,
            guard: BTreeMap::new(),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.chat.shift_times(by_s);
        self.packets.shift_times(by_s);
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.subghzchat", 1);
    /// Forgets the chat, the packets and the counters. The replay guard stays: a Flipper's counter only rises, and forgetting
    /// it lets a recording made before the clear play again.
    fn clear(&mut self) {
        let guard = std::mem::take(&mut self.guard);
        let (chat, packets) = (self.chat.cap(), self.packets.cap());
        *self = Store {
            chat: LogRing::new(chat),
            packets: LogRing::new(packets),
            guard,
            ..Store::default()
        };
    }
    fn adopt(&mut self, saved: Self) {
        self.chat.adopt(saved.chat);
        self.packets.adopt(saved.packets);
        self.good = saved.good;
        self.bad = saved.bad;
        self.bursts = saved.bursts;
        self.messages = saved.messages;
        self.decrypted = saved.decrypted;
        self.unreadable = saved.unreadable;
        self.repeats = saved.repeats;
        self.sent = saved.sent;
        self.guard = saved.guard;
    }
}

/// `sgc.chat`: joins packets into lines, decrypts with the user's key, keeps the replay guard, the chat and the packet log.
pub struct Chat {
    published: Published,
    store: Store,
    key: Option<Gcm>,
    mode: Encryption,
    password: String,
    hexkey: String,
    name: String,
    frequency: Option<ChatFrequency>,
    joining: Vec<u8>,
    first: Option<Packet>,
    npk: usize,
    last_pkt: f64,
    last_burst: f64,
    now: f64,
    centre_hz: f64,
    configured: bool,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    snr: f32,
    heard: bool,
    level: f32,
    was_locked: bool,
}

impl Default for Chat {
    fn default() -> Self {
        Chat {
            published: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(SubghzchatFields::SHAPE.fields)
                    .with_table(&SUBGHZCHAT_CHAT)
                    .with_table(&SUBGHZCHAT_PACKETS),
            )),
            store: Store::default(),
            key: None,
            mode: Encryption::Off,
            password: String::new(),
            hexkey: String::new(),
            name: "sdrkit".into(),
            frequency: None,
            joining: Vec::new(),
            first: None,
            npk: 0,
            last_pkt: f64::NEG_INFINITY,
            last_burst: f64::NEG_INFINITY,
            now: 0.0,
            centre_hz: 0.0,
            configured: false,
            events: Vec::new(),
            requests: Vec::new(),
            snr: 0.0,
            heard: false,
            level: f32::NEG_INFINITY,
            was_locked: false,
        }
    }
}

impl Chat {
    fn update_key(&mut self) {
        let k = match self.mode {
            Encryption::Password if !self.password.is_empty() => {
                Some(key_from_password(&self.password))
            }
            Encryption::Hex => key_from_hex(&self.hexkey),
            Encryption::Password | Encryption::Off => None,
        };
        self.key = k.as_ref().map(cipher);
    }

    /// Ends the line being joined: makes its chat row and event from the packets joined so far.
    fn end_line(&mut self) {
        if self.joining.is_empty() {
            return;
        }
        let m = std::mem::take(&mut self.joining);
        let Some(p) = self.first.take() else {
            return;
        };
        let s = &mut self.store;
        s.messages += 1;
        let mut line = ChatLine {
            time: p.time,
            sent: false,
            kind: LineKind::Plain,
            text: String::new(),
            hex: hex(&m),
            packets: self.npk,
            level_dbfs: p.level_dbfs,
            snr_db: p.snr_db,
            offset_hz: p.offset_hz,
            run: None,
        };
        let opened = self.key.as_ref().and_then(|k| decrypt(k, &m));
        if let Some((run, ctr, plain)) = opened {
            line.run = Some((format!("{run:016X}"), ctr));
            if s.guard.get(&run).is_some_and(|&last| ctr <= last) {
                line.kind = LineKind::Repeat;
                line.text = "a repeated encrypted message (dropped)".into();
                s.repeats += 1;
            } else {
                s.guard.insert(run, ctr);
                line.kind = LineKind::Decrypted;
                line.text = show(&plain);
                s.decrypted += 1;
            }
        } else if is_text(&m) {
            line.text = show(&m);
        } else if m.len() >= 41 {
            line.kind = LineKind::Unreadable;
            line.text = "encrypted, not readable with this key".into();
            s.unreadable += 1;
        } else {
            line.kind = LineKind::Octets;
            line.text = hex(&m);
        }
        let ev = match line.kind {
            LineKind::Plain | LineKind::Decrypted => format!("Flipper chat: {}", line.text),
            LineKind::Unreadable => {
                "Flipper chat: an encrypted message that this key does not open".into()
            }
            _ => "Flipper chat: a repeated or unreadable message".into(),
        };
        self.events.push(SUBGHZCHAT_LINE.event(
            line.time,
            self.centre_hz + f64::from(line.offset_hz),
            &SubghzchatLineEvent {
                text: &ev,
                family: Family::Message,
                kind: line.kind,
                snr_db: f64::from(line.snr_db),
            },
        ));
        self.store.chat.push(line);
    }

    fn packet(&mut self, p: &Packet) {
        self.store.packets.push(PacketLine {
            time: p.time,
            len: p.payload.len(),
            crc_ok: p.crc_ok,
            inverted: p.inverted,
            level_dbfs: p.level_dbfs,
            snr_db: p.snr_db,
            offset_hz: p.offset_hz,
            hex: hex(&p.payload),
        });
        self.now = self.now.max(p.time);
        if !p.crc_ok {
            self.store.bad += 1;
            return;
        }
        self.store.good += 1;
        self.heard = true;
        self.snr = 0.8 * self.snr + 0.2 * p.snr_db;
        self.level = p.level_dbfs;
        if !self.joining.is_empty() && p.time - self.last_pkt > 0.5 {
            self.end_line();
        }
        if self.joining.is_empty() {
            self.first = Some(p.clone());
            self.npk = 0;
        }
        self.last_pkt = p.time;
        self.joining.extend_from_slice(&p.payload);
        self.npk += 1;
        if self.joining.len() >= MAX_MESSAGE {
            self.end_line();
        }
    }

    fn publish(&mut self) {
        let s = &self.store;
        let state = if self.now - self.last_pkt < 5.0 && s.good > 0 {
            2
        } else if self.now - self.last_burst < 10.0 && s.bursts > 0 {
            1
        } else {
            0
        };
        if state == 2 && !self.was_locked {
            self.events.push(SUBGHZCHAT_STATE.event(
                self.now,
                self.centre_hz,
                &SubghzchatStateEvent {
                    text: "Flipper chat packets are coming in",
                    family: Family::Message,
                },
            ));
        }
        self.was_locked = state == 2;
        let summary = if s.messages == 0 && s.sent == 0 {
            format!(
                "listening for Flipper chat{}",
                if self.key.is_some() { " (key set)" } else { "" }
            )
        } else {
            format!(
                "{} messages heard, {} sent, {} packets",
                s.messages, s.sent, s.good
            )
        };
        let p = &mut self.published;
        p.results_mut().set_status(&Status {
            lock_state: LockState::from_level(state),
            snr_db: self.heard.then_some(self.snr),
            level_dbfs: self.level.is_finite().then_some(self.level),
            offset_hz: None,
            summary,
        });
        p.put(&SubghzchatFields {
            messages: s.messages as i64,
            frames_ok: s.good as i64,
            frames_bad: s.bad as i64,
            bursts: s.bursts as i64,
            decrypted: s.decrypted as i64,
            unreadable: s.unreadable as i64,
            repeats: s.repeats as i64,
            sent: s.sent as i64,
            key_set: self.key.is_some(),
            snr_db: self.heard.then_some(f64::from(self.snr)),
            level_dbfs: self.level.is_finite().then_some(f64::from(self.level)),
        });
        shaped::log_rows(&s.chat, &SUBGHZCHAT_CHAT, self.now, p);
        shaped::log_rows(&s.packets, &SUBGHZCHAT_PACKETS, self.now, p);
    }

    /// Adds a line the user sent at signal time `time` (`<name>: <line>` as it went out).
    pub fn sent(&mut self, line: &str, time: f64) {
        self.store.sent += 1;
        let text = format!("{}: {line}", self.name);
        self.store.chat.push(ChatLine {
            time,
            sent: true,
            kind: LineKind::Plain,
            hex: hex(text.as_bytes()),
            text,
            packets: 0,
            level_dbfs: 0.0,
            snr_db: 0.0,
            offset_hz: 0.0,
            run: None,
        });
    }
}

impl Component for Chat {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "sgc.chat",
            vec![
                Port::typed::<Packet>("packets"),
                Port::typed::<BurstTime>("bursts"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(ChatSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A stream restart finishes a half-joined line.
        self.end_line();
        self.centre_hz = inputs[0].centre_hz;
        self.configured = true;
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for t in io.items::<BurstTime>(1) {
            self.store.bursts += 1;
            self.last_burst = t.value.0;
            self.now = self.now.max(t.value.0);
        }
        for p in io.items::<Packet>(0) {
            self.packet(&p.value);
            new = true;
        }
        if io.tick() {
            self.now += REPORT_S;
            if !self.joining.is_empty() && self.now - self.last_pkt > 0.5 {
                self.end_line();
            }
        }
        if new || io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if !self.requests.is_empty() {
            let now = self.now;
            io.items_out::<HostRequest>(1)
                .extend(self.requests.drain(..).map(|r| Timed {
                    time: now,
                    value: r,
                }));
        }
    }
    /// Ends the line still being joined (the input ended) and publishes.
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        if !self.joining.is_empty() {
            self.end_line();
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChatSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChatSetting::Encryption => {
                self.mode = ENCRYPTION.read(id, v)?;
                self.update_key();
            }
            ChatSetting::Password => {
                PASSWORD.read(id, v)?.clone_into(&mut self.password);
                self.update_key();
            }
            ChatSetting::Key => {
                let t = KEY.read(id, v)?;
                if !t.trim().is_empty() && key_from_hex(t).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                t.clone_into(&mut self.hexkey);
                self.update_key();
            }
            ChatSetting::Name => NAME.read(id, v)?.clone_into(&mut self.name),
            ChatSetting::Frequency => {
                let f = FREQUENCY.read(id, v)?;
                // Only a change requests a retune from the host. The value set at construction does not.
                if self.frequency.is_some_and(|old| old != f) {
                    self.requests
                        .push(HostRequest::retune(f.hz(), "Flipper chat frequency"));
                }
                self.frequency = Some(f);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ChatSetting::Name, Value::Text(self.name.clone())),
            (ChatSetting::Encryption, Value::Choice(self.mode.index())),
            (ChatSetting::Password, Value::Text(self.password.clone())),
            (ChatSetting::Key, Value::Text(self.hexkey.clone())),
            (
                ChatSetting::Frequency,
                Value::Choice(self.frequency.unwrap_or(ChatFrequency::F433).index()),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        ChatCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ChatCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ChatCommand::Clear => {
                self.store.clear();
                self.joining.clear();
                self.first = None;
                self.published.clear_rows(&SUBGHZCHAT_CHAT);
                self.published.clear_rows(&SUBGHZCHAT_PACKETS);
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(SUBGHZCHAT_CHAT.key) {
            self.store.chat.clear();
            self.published.clear_rows(&SUBGHZCHAT_CHAT);
        }
        if !kept.table(SUBGHZCHAT_PACKETS.key) {
            self.store.packets.clear();
            self.published.clear_rows(&SUBGHZCHAT_PACKETS);
        }
        let s = &mut self.store;
        for (key, n) in [
            ("messages", &mut s.messages),
            ("frames_ok", &mut s.good),
            ("frames_bad", &mut s.bad),
            ("bursts", &mut s.bursts),
            ("decrypted", &mut s.decrypted),
            ("unreadable", &mut s.unreadable),
            ("repeats", &mut s.repeats),
            ("sent", &mut s.sent),
        ] {
            if !kept.field(key) {
                *n = 0;
            }
        }
        self.joining.clear();
        self.first = None;
        self.heard = false;
        self.snr = 0.0;
        self.level = f32::NEG_INFINITY;
        self.last_pkt = f64::NEG_INFINITY;
        self.last_burst = f64::NEG_INFINITY;
        self.was_locked = false;
        if self.configured {
            self.publish();
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now)?;
        self.joining.clear();
        self.first = None;
        if self.configured {
            self.publish();
        }
        Ok(())
    }
}

const NAME: Setting<field::Text> = Setting::new("name", "Your chat name", "", "sdrkit");
const ENCRYPTION: Setting<field::Choice<Encryption>> =
    Setting::choice("encryption", "Encryption", Encryption::Off);
const PASSWORD: Setting<field::Text> =
    Setting::new("password", "Password (kept in memory only)", "", "");
const KEY: Setting<field::Text> = Setting::new("key", "Hex key (kept in memory only)", "", "")
    .help("64 hex digits. Spaces, ':' and '-' are ignored.");
const FREQUENCY: Setting<field::Choice<ChatFrequency>> =
    Setting::choice("frequency", "Chat frequency", ChatFrequency::F433);

sdrkit_core::settings! {
    /// The settings of [`Chat`] and of the `subghzchat` stack. The password and the key stay in memory only.
    pub enum ChatSetting {
        /// The chat name.
        Name = NAME,
        /// Where the key comes from.
        Encryption = ENCRYPTION,
        /// The password.
        Password = PASSWORD,
        /// The hex key.
        Key = KEY,
        /// The chat frequency.
        Frequency = FREQUENCY,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Chat`].
    pub enum ChatCommand {
        /// Forgets the chat, the packets and the counters.
        Clear = commands::CLEAR,
    }
}

/// The `subghzchat` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", Front::default())
        .add("packets", Packets::default())
        .add("chat", Chat::default())
        .input("front")
        .connect("front", "packets")
        .connect("packets.packets", "chat.packets")
        .connect("packets.burst_times", "chat.bursts")
        .output("chat.events")
        .output("chat.requests")
        .clock(REPORT_S)
        .status_from("chat")
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape("subghzchat"), graph)
        .min_rate(MIN_RATE_HZ, &min_rate_message("Sub-GHz chat", MIN_RATE_HZ));
    for (k, target) in [
        (NAME.key(), "chat.name"),
        (ENCRYPTION.key(), "chat.encryption"),
        (PASSWORD.key(), "chat.password"),
        (KEY.key(), "chat.key"),
        (FREQUENCY.key(), "chat.frequency"),
    ] {
        g = g.route(k, &[target])?;
    }
    g.command("clear", sdrkit_graph::CommandTarget::of("chat", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The test conversation: (line, encrypted, pause after in seconds).
pub fn conversation() -> Vec<(&'static str, bool, f64)> {
    vec![
        ("Flipper1 joined chat.", false, 0.6),
        ("Flipper1: hello from the test signal", false, 0.9),
        ("Flipper2: hi! plain text works", false, 0.9),
        ("Flipper1: this line is encrypted with AES-256-GCM", true, 0.9),
        ("Flipper2: and this one is long enough to need more than one packet on the air, so the receiver joins them", true, 1.2),
        ("Flipper2 left chat.", false, 1.5),
    ]
}

/// The password of the encrypted test lines.
pub const TEST_PASSWORD: &str = "testkey";

/// The test signal: two Flippers (run ids 0x1111111111111111 and 0x2222222222222222) in the conversation, the encrypted lines
/// under the key of [`TEST_PASSWORD`], from 0.3 s, at unit power and `offset_hz` from 0 Hz (folded into the band at a low rate).
/// With `repeat`, every message goes out twice.
///
/// # Errors
///
/// [`ConfigError::RateTooLow`] below two samples a symbol, and the errors of the scene.
pub fn generator(
    rate: f64,
    seed: u64,
    offset_hz: f64,
    repeat: bool,
) -> Result<CleanScene, ConfigError> {
    let key = cipher(&key_from_password(TEST_PASSWORD));
    let mut senders = [
        Sender {
            run_id: 0x1111_1111_1111_1111,
            counter: 1,
        },
        Sender {
            run_id: 0x2222_2222_2222_2222,
            counter: 1,
        },
    ];
    let mut rng = Rng::stream(seed, 0xF1);
    let mut t = 0.3f64;
    let mut bursts = Vec::new();
    let reps = if repeat { 2 } else { 1 };
    for (line, enc, pause) in conversation() {
        let who = line.starts_with("Flipper2") as usize;
        let text = format!("{line}\r\n");
        let msg = if enc {
            let mut iv = [0u8; 12];
            rng.fill_bytes(&mut iv);
            encrypt(&key, &mut senders[who], iv, text.as_bytes())
                .map_err(|e| ConfigError::Stage(StageError::new(e)))?
        } else {
            text.into_bytes()
        };
        let s = tx_samples(&msg, rate, 0.0, 4).ok_or(ConfigError::RateTooLow {
            need: 2.0 * BAUD,
            got: rate,
        })?;
        for _ in 0..reps {
            bursts.push(Burst {
                start_s: t,
                offset_hz: fold(offset_hz, rate),
                level_db: 0.0,
                samples: s.clone(),
            });
            t += s.len() as f64 / rate + pause;
        }
    }
    let mut scene = Scene::new(rate)?;
    let mut it = bursts.into_iter();
    scene.add_source(Box::new(move || it.next()));
    let info = SignalInfo::new(2.0 * DEV + BAUD)
        .with_symbol_rate(BAUD)
        .with_bit_rate(BAUD);
    Ok(CleanScene::new(scene, rate, info).with_station_at(offset_hz))
}

/// `f` folded into the band of a complex stream at `rate`.
fn fold(f: f64, rate: f64) -> f64 {
    (f + rate / 2.0).rem_euclid(rate) - rate / 2.0
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(
        cfg.rate_hz,
        cfg.seed,
        -TUNE_OFFSET_HZ,
        false,
    )?))
}

/// Sub-GHz chat with Flipper Zero devices.
pub static SUBGHZCHAT: StackDescriptor = StackDescriptor {
    id: "subghzchat",
    name: "Sub-GHz chat (Flipper Zero)",
    group: "Chat, 433.92 MHz",
    blurb: "Text chat with Flipper Zero devices: plain lines, and AES-256-GCM lines with the password or key of a chat you take part in.",
    tuning: Tuning { kind: Some(sdrkit_core::TuningKind::Channels), min_hz: 300e6, max_hz: 928e6, tune_offset_hz: TUNE_OFFSET_HZ, ..Tuning::anywhere(433.92e6, 2e6, 200e3, 0.2e6) },
    submodes: &[],
    settings: ChatSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two Flippers chatting", o, 0.0);
    c.seconds = o.seconds.max(9.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = SubghzchatFields::read(r.fields());
        let n = |v: fn(&SubghzchatFields) -> i64| f.as_ref().map_or(0, v);
        vec![
            at_least_value("messages", n(|f| f.messages), 6),
            at_least_value("frames_ok", n(|f| f.frames_ok), 7),
            at_least_value("unreadable", n(|f| f.unreadable), 2),
        ]
    });
    vec![c]
}

/// The verify case of `subghzchat`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};
