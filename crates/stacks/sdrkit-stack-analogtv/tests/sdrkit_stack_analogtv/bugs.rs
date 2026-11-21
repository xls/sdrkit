//! Checks of single contracts: code tables against the standards, allocation on the hot paths, and the channel command.

use super::allocations;
use sdrkit_core::{Complex32, ConfigError, Generator, InputInfo, Output, Receiver};
use sdrkit_graph::{Component, ComponentInfo, Format, GraphBuilder, Io, Kind, Port, Timed};
use sdrkit_stack_analogtv::atv::nicam_rx::NicamDecoder;
use sdrkit_stack_analogtv::atv::Tune;
use sdrkit_stack_analogtv::mac::codes::{dispersal, group_crc};
use sdrkit_stack_analogtv::mac::frame::{
    decode_special, special_data, Rdf, Sdf, RDF_BITS, SDF_BITS, UDT_BITS,
};
use sdrkit_stack_analogtv::mac::packet::{encode_group, Command, Param};
use sdrkit_stack_analogtv::nicam::scrambling_bits;
use sdrkit_stack_analogtv::sound_gen::SecondSound;
use sdrkit_stack_analogtv::systems::SystemId;
use sdrkit_stack_analogtv::{AtvGenParams, AtvGenerator};
use sdrkit_util_bits::crc::catalog::CRC16_MAC_SI;

/// The MAC service information CRC is `CRC-16/MAC-SI` of the shared catalogue: x^16 + x^12 + x^5 + 1, register all ones,
/// input reflected, output not, check value 0x89F6 over "123456789". A data group ends with it, low byte first.
#[test]
fn mac_si_crc_is_crc16_mac_si() {
    assert_eq!(group_crc(b"123456789"), 0x89F6);
    assert_eq!(CRC16_MAC_SI.checksum(b"123456789"), 0x89F6);
    let g = encode_group(
        3,
        &[Command {
            ci: 0x90,
            params: vec![Param {
                pi: 0x40,
                data: b"\0TEST".to_vec(),
            }],
        }],
    );
    let n = g.len();
    let crc = CRC16_MAC_SI.checksum(&g[..n - 2]);
    assert_eq!((g[n - 2], g[n - 1]), ((crc & 0xFF) as u8, (crc >> 8) as u8));
}

/// The BCH (71,57) and (94,80) codes of line 625 correct two errors: two errors in the SDF and two in each RDF decode.
#[test]
fn mac_bch_corrects_two_errors() {
    let sdf = Sdf {
        chid: 0x1A2B,
        sdfscr: 0,
        mvscg: Sdf::mvscg_of(true, false, true, false),
        cafcnt: 77,
    };
    let rdf = Rdf {
        tdmcid: 1,
        fcnt: 42,
    };
    let clean = special_data(&sdf, &rdf);
    for (a, b) in [(0usize, 1usize), (3, 40), (10, 70), (55, 56)] {
        let mut bits = clean.clone();
        bits[UDT_BITS + a] ^= 1;
        bits[UDT_BITS + b] ^= 1;
        for k in 0..5 {
            let base = UDT_BITS + SDF_BITS + k * RDF_BITS;
            bits[base + a] ^= 1;
            bits[base + (b + 20).min(RDF_BITS - 1)] ^= 1;
        }
        let (s, r, bad) = decode_special(&bits);
        assert_eq!(s, Some(sdf), "SDF errors at {a} and {b}");
        assert_eq!(r, Some(rdf), "RDF errors at {a} and {b}");
        assert_eq!(bad, 0);
    }
}

/// The NICAM scrambler is the sequence `a(n) = a(n-5) XOR a(n-9)` from all ones, which the shared crate names x^9 + x^5 + 1
/// (`PRBS9_DISPERSAL`, EN 300 163 clause 3.5), restarted every frame.
#[test]
fn nicam_scrambler_is_the_x9_x5_sequence() {
    let s = scrambling_bits();
    let first: String = s[..20].iter().map(|b| char::from(b'0' + b)).collect();
    assert_eq!(first, "00000111101111100010");
    let mut a = vec![1u8; 9];
    for _ in 0..720 {
        let n = a.len();
        a.push(a[n - 5] ^ a[n - 9]);
    }
    assert_eq!(&a[9..], &s[..]);
    let mut r = sdrkit_util_bits::seq::PRBS9_DISPERSAL;
    let mut b = [0u8; 720];
    r.fill_bits(&mut b);
    assert_eq!(&b, s);
}

/// Returns the NICAM decoder its control item at every tick: the vision carrier of system I at `carrier_hz`, second sound on.
struct TuneSource {
    carrier_hz: f64,
}

impl Component for TuneSource {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.tune",
            vec![Port::iq("in")],
            vec![Port::typed::<Tune>("tune").rate_from(sdrkit_graph::RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Tune>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if io.tick() {
            let t = Tune {
                carrier_hz: self.carrier_hz,
                spacing_hz: 6e6,
                second_sound: true,
                ..Default::default()
            };
            io.items_out::<Tune>(0).push(Timed {
                time: 0.0,
                value: t,
            });
        }
    }
}

/// Once the NICAM decoder is locked, a piece of signal allocates nothing except the report it sends every 10 ms.
#[test]
fn nicam_decoding_allocates_no_buffer_per_pass() {
    let rate = 10e6;
    let mut g = AtvGenerator::new(AtvGenParams {
        rate_hz: rate,
        system: SystemId::I,
        second: SecondSound::NicamStereo,
        ..Default::default()
    });
    let carrier_hz = g.vision_hz();
    let mut graph = GraphBuilder::new()
        .add("tune", TuneSource { carrier_hz })
        .add("nicam", NicamDecoder::default())
        .input("tune.in")
        .input("nicam.in")
        .feedback("tune.tune", "nicam.control")
        .output("nicam.out")
        .clock(0.01)
        .build()
        .unwrap();
    graph
        .configure(&InputInfo::new(rate, 600e6, 600e6).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); 2_000];
    let mut out = Output::default();
    let mut feed = |graph: &mut sdrkit_graph::Graph, pieces: usize, out: &mut Output| {
        let mut audio = 0;
        for _ in 0..pieces {
            g.generate(&mut x);
            out.clear();
            graph.feed(&x, out);
            audio += out.audio().len();
        }
        audio
    };
    // 0.5 s to lock and warm up. The last 0.1 s must play sound.
    feed(&mut graph, 2_000, &mut out);
    assert!(feed(&mut graph, 500, &mut out) > 0, "the decoder plays");
    // 0.2 s more: 1000 pieces, 20 reports and 5 pictures of the generator, each picture with its clock text.
    let pieces = 1_000;
    let n = allocations(|| {
        feed(&mut graph, pieces, &mut out);
    });
    assert!(n <= 40, "{n} allocations in {pieces} pieces");
}

/// The MAC energy dispersal sequence is the shared `MAC_PRBS15`: x^15 + x + 1 from all ones, `a(n) = a(n-1) XOR a(n-15)`, of
/// maximal length 32767.
#[test]
fn mac_prbs15_matches_the_standard_sequence() {
    let mut r = dispersal();
    let got: Vec<u8> = (0..1000).map(|_| r.bit()).collect();
    let mut a = vec![1u8; 15];
    for _ in 0..1000 {
        let n = a.len();
        a.push(a[n - 1] ^ a[n - 15]);
    }
    assert_eq!(&a[15..], &got[..]);
    assert_eq!(dispersal().period(40_000), Some(32_767));
}

/// The `channel.tune` command requests the host to tune to a channel of the setting `channels`, and refuses a channel that
/// the setting lacks.
#[test]
fn the_channel_command_asks_the_host_to_tune() {
    let channels = super::common::atv_channels("an_bg");
    let mut rx = super::common::atv(10e6, &[("channels", channels.as_str())]);
    super::common::invoke(&mut rx, "channel.tune", "E21").unwrap();
    assert!(matches!(
        super::common::invoke(&mut rx, "channel.tune", "99"),
        Err(sdrkit_core::CommandError::NoSuchRow { .. })
    ));
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 200_000], &mut out);
    assert!(
        out.requests.iter().any(|r| matches!(
            r,
            sdrkit_core::HostRequest::Retune { tuned_hz, .. } if *tuned_hz == 474e6
        )),
        "{:?}",
        out.requests
    );
}

/// The same `channel.tune` runs with the channels of two plans of the data folder: B/G channel 21 at 474 MHz and M channel 14
/// at 473 MHz; the table `channels` lists the channels of the plan set.
#[test]
fn the_channel_command_follows_the_plan_set() {
    for (id, label, hz) in [("an_bg", "21", 474e6), ("an_m", "14", 473e6)] {
        let channels = super::common::atv_channels(id);
        let mut rx = super::common::atv(10e6, &[("channels", channels.as_str())]);
        let rows = sdrkit_stack_analogtv::typed::ATV_CHANNELS
            .rows::<sdrkit_stack_analogtv::typed::AtvChannelRow>(
                sdrkit_core::owner::Owner::results(&rx),
            )
            .count();
        assert_eq!(rows, channels.split(';').count(), "{id}");
        super::common::invoke(&mut rx, "channel.tune", label).unwrap();
        let mut out = Output::default();
        rx.feed(&vec![Complex32::default(); 200_000], &mut out);
        assert!(
            out.requests.iter().any(|r| matches!(
                r,
                sdrkit_core::HostRequest::Retune { tuned_hz, .. } if *tuned_hz == hz
            )),
            "{id}: {:?}",
            out.requests
        );
    }
}

/// The `atv` stack graph on its own test signal (PAL B/G with NICAM stereo), once locked: 0.5 s of signal allocates at most 600
/// times (the pictures, the report items and the results of 50 ticks).
#[test]
fn the_atv_receive_path_allocations() {
    let rate = 10e6;
    let mut g = AtvGenerator::new(AtvGenParams {
        rate_hz: rate,
        second: SecondSound::NicamStereo,
        ..Default::default()
    });
    let mut rx = super::common::atv(rate, &[]);
    let mut x = vec![Complex32::default(); 50_000];
    let mut out = Output::default();
    let mut feed = |rx: &mut sdrkit_graph::StackGraph, pieces: usize, out: &mut Output| {
        for _ in 0..pieces {
            g.generate(&mut x);
            out.clear();
            rx.feed(&x, out);
        }
    };
    feed(&mut rx, 300, &mut out);
    let n = allocations(|| feed(&mut rx, 100, &mut out));
    assert!(n <= 600, "{n} allocations in 0.5 s");
}
