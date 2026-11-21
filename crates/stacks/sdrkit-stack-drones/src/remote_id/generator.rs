//! The Remote ID test signal: drone 1 on Bluetooth LE channel 38, drone 2 in Wi-Fi beacons on channel 6,
//! one advertiser and one access point without Remote ID. The generator makes the clean signal only: noise and the receiver's
//! impairments come from the standard `gen_*` settings.

use super::messages::{
    beacon, ble_service_data, encode_message, encode_pack, wifi_element, Authentication, BasicId,
    Location, Message, OperatorId, SelfId, System,
};
use super::plan;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_proto_wlan_phy::{ble, dsss, mac, ofdm};

/// Which carriers the generator makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carriers {
    /// Both.
    Both,
    /// Bluetooth LE only.
    Ble,
    /// Wi-Fi only.
    Wifi,
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct RidGenOptions {
    /// Output rate.
    pub rate: f64,
    /// The radio centre (0: both carriers at 0 Hz).
    pub centre_hz: f64,
    /// Carriers.
    pub carriers: Carriers,
    /// The Wi-Fi beacons as OFDM 6 Mbit/s instead of DSSS 1 Mbit/s.
    pub wifi_ofdm: bool,
}

impl Default for RidGenOptions {
    fn default() -> Self {
        RidGenOptions {
            rate: 20e6,
            centre_hz: 2432e6,
            carriers: Carriers::Both,
            wifi_ofdm: false,
        }
    }
}

/// Drone 1's seven messages.
pub fn drone1_messages() -> Vec<Message> {
    vec![
        Message::BasicId(BasicId {
            id_type: 1,
            ua_type: 2,
            id: "1581F5FKD229400B".into(),
        }),
        Message::BasicId(BasicId {
            id_type: 4,
            ua_type: 2,
            id: "SESSION-7F3A".into(),
        }),
        Message::Location(Location {
            status: 2,
            height_type: 0,
            direction: Some(90.0),
            speed: Some(8.5),
            vspeed: Some(1.5),
            lat: Some(54.6872),
            lon: Some(25.2797),
            alt_baro: Some(230.5),
            alt_geo: Some(245.0),
            height: Some(120.0),
            h_acc: 10,
            v_acc: 4,
            baro_acc: 3,
            speed_acc: 3,
            timestamp: Some(12345),
            ts_acc: 2,
        }),
        Message::System(System {
            class_type: 1,
            loc_type: 1,
            op_lat: Some(54.6860),
            op_lon: Some(25.2770),
            area_count: 1,
            area_radius: 0.0,
            area_ceiling: None,
            area_floor: None,
            category: 1,
            class: 2,
            op_alt: Some(126.0),
            timestamp: 245_000_000,
        }),
        Message::OperatorId(OperatorId {
            id_type: 0,
            text: "LTUabcd1234efgh".into(),
        }),
        Message::SelfId(SelfId {
            desc_type: 0,
            text: "Survey flight".into(),
        }),
        Message::Authentication(Authentication {
            auth_type: 1,
            page: 0,
            last_page: Some(0),
            length: Some(17),
            timestamp: Some(245_000_000),
            data: (0xA0u8..=0xB0).collect(),
        }),
    ]
}

/// Drone 2's Message Pack.
pub fn drone2_messages() -> Vec<Message> {
    vec![
        Message::BasicId(BasicId {
            id_type: 2,
            ua_type: 1,
            id: "FIN87astrdge12k8".into(),
        }),
        Message::Location(Location {
            status: 2,
            height_type: 1,
            direction: Some(275.0),
            speed: Some(70.0),
            vspeed: None,
            lat: Some(54.7001),
            lon: Some(25.3002),
            alt_baro: Some(410.0),
            alt_geo: Some(420.0),
            height: Some(300.0),
            h_acc: 11,
            v_acc: 5,
            baro_acc: 0,
            speed_acc: 4,
            timestamp: Some(600),
            ts_acc: 0,
        }),
        Message::System(System {
            class_type: 1,
            loc_type: 0,
            op_lat: Some(54.6950),
            op_lon: Some(25.2900),
            area_count: 0,
            area_radius: 0.0,
            area_ceiling: None,
            area_floor: None,
            category: 2,
            class: 0,
            op_alt: Some(110.0),
            timestamp: 245_000_100,
        }),
        Message::OperatorId(OperatorId {
            id_type: 0,
            text: "FINop-998877".into(),
        }),
    ]
}

/// Drone 1's address.
pub const DRONE1: [u8; 6] = [0xC6, 0x12, 0x34, 0x56, 0x78, 0x9A];
/// Drone 2's address.
pub const DRONE2: [u8; 6] = [0x60, 0x60, 0x1F, 0xAA, 0xBB, 0xCC];

/// The BLE PDU of packet `k` of the BLE stream (every eighth is the advertiser without Remote ID), with the Remote ID counter.
pub fn ble_pdu(k: u64, counter: u8) -> Vec<u8> {
    let mut pdu = Vec::with_capacity(39);
    if k % 8 == 7 {
        let mut data = vec![2, 1, 6, 7, 9];
        data.extend_from_slice(b"Thermo");
        let a = ble::Addr::public(air_order([0xD0, 0x05, 0x04, 0x03, 0x02, 0x01]));
        ble::build_adv(&mut pdu, ble::PduType::AdvInd, &a, &data).expect("a short PDU");
        return pdu;
    }
    let m = &drone1_messages()[(k % 8) as usize];
    let sd = ble_service_data(counter, &encode_message(m, 2));
    let a = ble::Addr::random(air_order(DRONE1));
    ble::build_adv(&mut pdu, ble::PduType::AdvNonconnInd, &a, &sd).expect("a short PDU");
    pdu
}

fn air_order(mut a: [u8; 6]) -> [u8; 6] {
    a.reverse();
    a
}

/// The Wi-Fi frame `m` of the Wi-Fi stream (every fourth is the plain access point), with the Remote ID counter and the FCS.
pub fn wifi_frame(m: u64, counter: u8) -> Vec<u8> {
    let mut f = if m % 4 == 3 {
        beacon([0x00, 0x04, 0x0E, 0x11, 0x22, 0x33], "Home", m as u16, &[])
    } else {
        let el = wifi_element(counter, &encode_pack(&drone2_messages(), 2));
        beacon(DRONE2, "RID-DJI-2", m as u16, &el)
    };
    mac::push_fcs(&mut f);
    f
}

/// The GFSK samples of a BLE PDU on `channel` at `rate`, unit power.
pub fn ble_burst(pdu: &[u8], channel: u8, rate: f64) -> Vec<Complex32> {
    let mut bits = Vec::new();
    ble::air_bits(pdu, channel, &mut bits).expect("a PDU of legal length");
    let levels: Vec<f32> = bits
        .iter()
        .map(|&b| if b == 1 { 1.0 } else { -1.0 })
        .collect();
    let mut m = Cpm::gfsk(ble::BT, 0.5, rate / ble::SYMBOL_RATE)
        .expect("a GFSK modulator")
        .with_held_edges();
    let mut out = Vec::with_capacity((levels.len() as f64 * rate / 1e6) as usize + 16);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// Resamples a burst from `from` to `to`. The end is padded with zeros and the filter tail stays in the output, unlike
/// [`sdrkit_algo_dsp::resample::resample_span`], which holds the edge samples and keeps the span's length.
fn resample(x: Vec<Complex32>, from: f64, to: f64) -> Vec<Complex32> {
    if (from - to).abs() < 1e-6 {
        return x;
    }
    let mut r = Resampler::new(from, to).expect("a resampler between the two rates");
    let tail = (r.delay() * from / to).ceil() as usize + 64;
    let mut input = x;
    input.resize(input.len() + tail, Complex32::default());
    let mut out = Vec::with_capacity((input.len() as f64 * to / from) as usize + 16);
    r.process(&input, &mut out);
    let skip = (r.delay() * to / from).round() as usize;
    out.drain(..skip.min(out.len()));
    out
}

/// The DSSS 1 Mbit/s samples of an MPDU (FCS included) at `rate`.
pub fn dsss_burst(mpdu: &[u8], rate: f64) -> Vec<Complex32> {
    let header = dsss::PlcpHeader::for_psdu(dsss::Rate::Dbpsk1, mpdu.len()).expect("a beacon fits");
    let mut bits = Vec::new();
    dsss::ppdu_bits(dsss::Preamble::Long, &header, mpdu, &mut bits)
        .expect("the PSDU matches the header");
    dsss::scramble(&mut bits, dsss::Preamble::Long);
    let mut sym = Vec::new();
    dsss::symbols(&bits, dsss::Preamble::Long, dsss::Rate::Dbpsk1, &mut sym)
        .expect("a Barker rate");
    let mut chips = Vec::new();
    dsss::spread(&sym, &mut chips);
    resample(chips, dsss::CHIP_RATE, rate)
}

/// The OFDM 6 Mbit/s samples of an MPDU (FCS included) at `rate`.
pub fn ofdm_burst(mpdu: &[u8], rate: f64) -> Vec<Complex32> {
    let mut x = Vec::new();
    ofdm::build_ppdu(mpdu, ofdm::Rate::R6, 0x5D, &mut x).expect("a beacon fits");
    resample(x, ofdm::SAMPLE_RATE, rate)
}

fn unit_power(mut x: Vec<Complex32>) -> Vec<Complex32> {
    let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len().max(1) as f64;
    let g = (1.0 / p.max(1e-30)).sqrt() as f32;
    x.iter_mut().for_each(|v| *v *= g);
    x
}

/// The Remote ID generator: BLE packets every 2 ms and Wi-Fi beacons every 3 ms, each at unit power while it sends.
pub struct RemoteIdGenerator {
    o: RidGenOptions,
    scene: Scene,
}

impl RemoteIdGenerator {
    /// A generator with these options.
    pub fn new(o: RidGenOptions) -> Self {
        let pl = plan(o.centre_hz);
        let ble_off = pl.ble.map_or(0.0, |b| b.1);
        let wifi_off = pl.wifi.1;
        let rate = o.rate;
        let mut scene = Scene::new(rate).expect("a positive rate");
        if o.carriers != Carriers::Wifi {
            let (mut k, mut counter) = (0u64, 0u8);
            scene.add_source(Box::new(move || {
                let pdu = ble_pdu(k, counter);
                if k % 8 != 7 {
                    counter = counter.wrapping_add(1);
                }
                let b = Burst {
                    start_s: 0.001 + 0.002 * k as f64,
                    offset_hz: ble_off,
                    level_db: 0.0,
                    samples: unit_power(ble_burst(&pdu, 38, rate)),
                };
                k += 1;
                Some(b)
            }));
        }
        if o.carriers != Carriers::Ble {
            let (mut m, mut counter, ofdm) = (0u64, 0u8, o.wifi_ofdm);
            scene.add_source(Box::new(move || {
                let f = wifi_frame(m, counter);
                if m % 4 != 3 {
                    counter = counter.wrapping_add(1);
                }
                let x = if ofdm {
                    ofdm_burst(&f, rate)
                } else {
                    dsss_burst(&f, rate)
                };
                let b = Burst {
                    start_s: 0.0005 + 0.003 * m as f64,
                    offset_hz: wifi_off,
                    level_db: 0.0,
                    samples: unit_power(x),
                };
                m += 1;
                Some(b)
            }));
        }
        RemoteIdGenerator { o, scene }
    }

    /// From the stack's settings. The schema of `cfg.settings` must hold the settings of [`super::RidSetting`].
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        RemoteIdGenerator::new(RidGenOptions {
            rate: cfg.rate_hz,
            centre_hz: s.get(&super::GEN_CENTRE_MHZ) * 1e6,
            carriers: match s.get(&super::GEN_CARRIERS) {
                super::GenCarriers::Ble => Carriers::Ble,
                super::GenCarriers::Wifi => Carriers::Wifi,
                super::GenCarriers::Both => Carriers::Both,
            },
            wifi_ofdm: s.get(&super::GEN_WIFI_PHY) == super::GenWifiPhy::Ofdm,
        })
    }
}

impl Generator for RemoteIdGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let mut i = SignalInfo::new(if self.o.carriers == Carriers::Ble {
            1e6
        } else {
            20e6
        });
        i.symbol_rate_hz = Some(1e6);
        i.bit_rate_hz = Some(1e6);
        Some(i)
    }
}
