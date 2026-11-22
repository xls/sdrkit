//! Horus telemetry: the worked examples of the horusdemodlib wiki (the test packet, its line and its checksum, the RS41ng
//! custom bytes), Golay correction, layer 2 sizes and round trips with errors, sentences and the payload ID list, and one
//! channel on the test signal: binary v1 and v2 and RTTY at three rates near their floors, impairments, the SNR reading and
//! noise alone.

use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_stack_science::horus::layer2::{
    codewords, frame_len, interleaver_step, Layer2, V1_PAYLOAD, V2_PAYLOAD,
};
use sdrkit_stack_science::horus::packet::{
    custom_fields, parse_formats, parse_sentence, rs41ng, with_checksum, Binary, CustomFormat,
    IdList, Post,
};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// The test packet of the wiki (v2).
const TEST_PACKET: [u8; 32] = [
    0x00, 0x01, 0x5F, 0x00, 0x0C, 0x22, 0x38, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x52, 0x06, 0x9E, 0x3F, 0xC8, 0x7B, 0xD2, 0x04, 0x29, 0xBE,
];

fn wiki_format() -> CustomFormat {
    CustomFormat {
        callsign: "4FSKTEST-V2".into(),
        comment: String::new(),
        structure: "<BfBBH".into(),
        fields: ["a", "b", "c", "d", "e"]
            .iter()
            .zip([
                Post::None,
                Post::None,
                Post::Battery5vByte,
                Post::DivideBy10,
                Post::DivideBy100,
            ])
            .map(|(n, p)| (n.to_string(), p))
            .collect(),
        sentence: Vec::new(),
        other_payloads: Vec::new(),
    }
}

#[test]
fn the_test_packet_its_line_and_a_flipped_bit() {
    assert_eq!(CRC16_IBM_3740.checksum(&TEST_PACKET[..30]), 0xBE29);
    let p = Binary::parse(&TEST_PACKET).expect("the CRC is good");
    assert_eq!((p.version, p.id, p.seq, p.time), (2, 256, 95, (12, 34, 56)));
    assert!(!p.has_fix());
    let ids = IdList::parse("256, 4FSKTEST-V2");
    let call = ids.callsign(p.id);
    let custom = custom_fields(Some(&wiki_format()), p.custom.as_ref().unwrap());
    let line = p.sentence(&call, &custom);
    assert_eq!(
        line,
        "$$4FSKTEST-V2,95,12:34:56,0.00000,0.00000,0,0,0,0,0.00,1,1.234568,3.92,12.3,12.34*BBDB"
    );
    assert_eq!(p.to_bytes(), TEST_PACKET);
    for i in 0..TEST_PACKET.len() * 8 {
        let mut b = TEST_PACKET;
        b[i / 8] ^= 0x80 >> (i % 8);
        assert!(Binary::parse(&b).is_none(), "bit {i}");
    }
    let s = parse_sentence(&line, &[]).expect("the checksum is good");
    assert_eq!(
        (s.callsign.as_str(), s.seq, s.time),
        ("4FSKTEST-V2", 95, (12, 34, 56))
    );
    assert_eq!(s.fields.len(), 9);
    assert_eq!(s.fields[0], ("field 7".to_string(), "0".to_string()));
    let mut bad = line.clone();
    bad.replace_range(3..4, "X");
    assert!(parse_sentence(&bad, &[]).is_none());
    assert!(parse_sentence(
        &line
            .to_lowercase()
            .replace("$$4fsktest-v2", "$$4FSKTEST-V2"),
        &[]
    )
    .is_some());
}

#[test]
fn the_rs41ng_custom_bytes_and_the_fallbacks() {
    let b = [0x12, 0x01, 0x4A, 0xFE, 0x00, 0x48, 0x04, 0x00, 0x00];
    let f = custom_fields(Some(&rs41ng()), &b);
    let v: Vec<(&str, &str)> = f.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    assert_eq!(
        v,
        [
            ("ascent_rate", "2.74"),
            ("ext_temperature", "-43.8"),
            ("ext_humidity", "0"),
            ("ext_pressure", "109.6")
        ]
    );
    // A callsign without a format takes the RS41ng format.
    assert_eq!(custom_fields(None, &b), f);
    // A layout of more than 9 bytes, or with an unknown character, gives the raw bytes.
    for st in ["<ffH", "<hhQ"] {
        let fmt = CustomFormat {
            structure: st.into(),
            ..rs41ng()
        };
        assert_eq!(
            custom_fields(Some(&fmt), &b),
            [("custom".to_string(), "12014AFE0048040000".to_string())]
        );
    }
    let fmt = CustomFormat {
        structure: ">2Hx4x".into(),
        fields: Vec::new(),
        ..rs41ng()
    };
    let v = custom_fields(Some(&fmt), &b);
    assert_eq!(
        v,
        [
            ("field 1".to_string(), "4609".to_string()),
            ("field 2".to_string(), "19198".to_string())
        ]
    );
    let list = parse_formats(
        "4FSKTEST-V2|<hhBHxx|ascent_rate:divide_by_100,ext_temperature:divide_by_10,ext_humidity:none,ext_pressure:divide_by_10||257,300|Default custom fields for RS41ng",
    )
    .expect("the list reads");
    assert_eq!(list[0].fields, rs41ng().fields);
    assert_eq!(list[0].other_payloads, [257, 300]);
    assert_eq!(parse_formats("X|<hhZ"), Err(1));
}

#[test]
fn golay_corrects_up_to_three_errors_and_counts_them() {
    let g = SmallCode::golay_23_12();
    let mut rng = Rng::new(4);
    for _ in 0..4000 {
        let info = u128::from(rng.below(4096) as u16);
        let cw = g.encode(info);
        let w = rng.below(4) as usize;
        let mut e = 0u128;
        while (e.count_ones() as usize) < w {
            e |= 1 << rng.below(23);
        }
        let (got, n) = g.decode(cw ^ e).expect("3 errors or fewer");
        assert_eq!((g.info(got), n as usize), (info, w));
    }
}

fn test_payload(version: u8) -> Binary {
    Binary {
        version,
        id: if version == 1 { 0 } else { 256 },
        seq: 1234,
        time: (1, 2, 3),
        lat: -34.5,
        lon: 138.25,
        alt_m: 12345,
        speed_kmh: 50,
        sats: 9,
        temp_c: -40,
        battery: 153,
        custom: (version == 2).then_some([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    }
}

#[test]
fn layer_2_sizes_steps_and_round_trips_with_errors() {
    assert_eq!((codewords(V1_PAYLOAD), frame_len(V1_PAYLOAD)), (15, 45));
    assert_eq!((codewords(V2_PAYLOAD), frame_len(V2_PAYLOAD)), (22, 65));
    assert_eq!(
        (interleaver_step(43), interleaver_step(63)),
        (Some(337), Some(389))
    );
    let mut rng = Rng::new(7);
    for version in [1, 2] {
        let p = test_payload(version);
        let bytes = p.to_bytes();
        let l2 = Layer2::new(bytes.len()).unwrap();
        let frame = l2.encode(&bytes);
        assert_eq!(frame.len(), frame_len(bytes.len()));
        assert_eq!(frame[..2], [0x24, 0x24]);
        assert_eq!(l2.decode(&frame[2..]), Some((bytes.clone(), 0)));
        for _ in 0..50 {
            let mut f = frame.clone();
            for _ in 0..6 {
                let i = 16 + rng.below(8 * (f.len() as u64 - 2)) as usize;
                f[i / 8] ^= 0x80 >> (i % 8);
            }
            let (back, _) = l2
                .decode(&f[2..])
                .expect("six errors spread by the interleaver");
            let q = Binary::parse(&back).expect("the CRC is good");
            assert_eq!((q.seq, q.alt_m, q.temp_c), (1234, 12345, -40));
        }
    }
}

/// Soft decoding: four errors in one Golay code word are past the hard decoder, and decode when the decoder knows that
/// those bits are the weakest.
#[test]
fn the_reliabilities_reach_the_soft_decoder() {
    let bytes = test_payload(2).to_bytes();
    let n = bytes.len();
    let l2 = Layer2::new(n).unwrap();
    let mut frame = l2.encode(&bytes);
    let nb = frame_len(n) - 2;
    let (step, bits) = (interleaver_step(nb).unwrap(), 8 * nb);
    // Bit q of the frame body before the interleaver (most significant first) is sent at body bit (step x lsb) mod N,
    // counted least significant first.
    let sent_at = |q: usize| (step * ((q / 8) * 8 + 7 - q % 8)) % bits;
    // Code word 3: data bits 36 to 47 of the payload, parity bits from 8 n + 33.
    let word: Vec<usize> = [36, 39, 42]
        .iter()
        .copied()
        .chain([8 * n + 35])
        .map(sent_at)
        .collect();
    let body = &mut frame[2..];
    let mut rel = vec![10.0f32; bits];
    for &i in &word {
        body[i / 8] ^= 1 << (i % 8);
        rel[i] = 0.1;
    }
    assert_ne!(l2.decode(body).map(|(p, _)| p), Some(bytes.clone()));
    let (soft, fixed) = l2
        .decode_soft(body, &rel)
        .expect("the weakest bits are flipped back");
    assert_eq!((soft, fixed), (bytes, 4));
}

/// The test signal keeps its power: the last second of 200 s has the power of the first.
#[test]
fn the_test_signal_keeps_its_power() {
    use sdrkit_stack_science::horus::gen::{render, GenOptions};
    let x = render(&GenOptions {
        seconds: 200.0,
        ..GenOptions::default()
    });
    let power = |s: &[sdrkit_core::Complex32]| {
        let on: Vec<f64> = s
            .iter()
            .map(|z| f64::from(z.norm_sqr()))
            .filter(|&p| p > 0.0)
            .collect();
        on.iter().sum::<f64>() / on.len() as f64
    };
    let (a, b) = (power(&x[..48_000]), power(&x[x.len() - 48_000..]));
    assert!(
        (a - 0.25).abs() < 1e-4 && (b - 0.25).abs() < 1e-4,
        "{a} {b}"
    );
}

#[test]
fn a_built_sentence_reads_back_and_the_id_list_parses() {
    let s = with_checksum("RTTYTEST,101,12:00:05,54.89850,23.90360,275,38,9,13");
    let names = vec!["speed".to_string(), "sats".to_string()];
    let r = parse_sentence(&format!("xx{s}\n"), &names).expect("a sentence");
    assert_eq!(
        (r.callsign.as_str(), r.seq, r.alt_m),
        ("RTTYTEST", 101, 275.0)
    );
    assert_eq!((r.lat, r.lon), (54.8985, 23.9036));
    assert_eq!(r.fields[0], ("speed".to_string(), "38".to_string()));
    assert_eq!(r.fields[2], ("field 9".to_string(), "13".to_string()));
    let no_colons = with_checksum("RTTYTEST,1,120005,1.0,2.0,3");
    assert_eq!(parse_sentence(&no_colons, &[]).unwrap().time, (12, 0, 5));
    assert!(parse_sentence(&with_checksum("A,1,12:00:00,1,2"), &[]).is_none());
    let l = IdList::parse("# x\n0, HORUSBINARY\n1\tCALL-1 # comment\nbad line here\n70000, X\n");
    assert_eq!(l.unreadable, 2);
    assert_eq!(l.ids.len(), 2);
    assert_eq!(
        (
            l.callsign(0).as_str(),
            l.callsign(1).as_str(),
            l.callsign(5).as_str()
        ),
        ("HORUSBINARY", "CALL-1", "ID 5")
    );
}

mod loopback {
    use sdrkit_dev_test::Awgn;
    use sdrkit_stack_science::horus::demod::{Channel, Decoded, Found};
    use sdrkit_stack_science::horus::gen::{render, Content, GenOptions};
    use sdrkit_stack_science::horus::packet::parse_sentence;

    /// The packets of one channel on the dial of `o`, with noise of `snr_db` in 3 kHz.
    pub fn receive(o: &GenOptions, snr_db: Option<f64>, seed: u64) -> Vec<Found> {
        let mut x = render(o);
        if let Some(s) = snr_db {
            Awgn::new(o.amplitude * o.amplitude, s, 3_000.0, o.rate_hz, seed).process(&mut x);
        }
        let mut ch = Channel::new(
            o.rate_hz,
            sdrkit_stack_science::horus::gen::dial_hz(o.rate_hz),
        )
        .unwrap();
        let mut out = Vec::new();
        for c in x.chunks((o.rate_hz / 50.0) as usize) {
            ch.process(c, &mut out);
        }
        out
    }

    /// The packets that `o` sends in full.
    pub fn sent(o: &GenOptions) -> usize {
        sdrkit_stack_science::horus::gen::complete(o)
    }

    pub fn binaries(f: &[Found]) -> usize {
        f.iter()
            .filter(|p| matches!(p.decoded, Decoded::Binary { .. }))
            .count()
    }

    #[test]
    fn binary_v1_and_v2_down_to_minus_3_db() {
        for content in [Content::V2, Content::V1] {
            for snr in [-3.0, 0.0, 10.0] {
                let o = GenOptions {
                    content,
                    carrier_hz: 1_200.0,
                    clock_ppm: 1_000.0,
                    seconds: 60.0,
                    ..GenOptions::default()
                };
                let got = binaries(&receive(&o, Some(snr), 3));
                let n = sent(&o);
                println!("{content:?} at {snr} dB: {got} of {n}");
                assert!(got + 1 >= n, "{content:?} at {snr} dB: {got} of {n}");
            }
        }
    }

    fn sentences(f: &[Found]) -> Vec<String> {
        f.iter()
            .filter_map(|p| match &p.decoded {
                Decoded::Rtty { sentence, .. } => Some(sentence.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rtty_at_three_rates() {
        for (content, floor) in [
            (Content::Rtty50, 3.0),
            (Content::Rtty100, 3.0),
            (Content::Rtty300, 10.0),
        ] {
            for snr in [20.0, floor] {
                let o = GenOptions {
                    content,
                    carrier_hz: 1_200.0,
                    clock_ppm: 1_000.0,
                    seconds: 60.0,
                    ..GenOptions::default()
                };
                let f = receive(&o, Some(snr), 5);
                let mut got = sentences(&f);
                got.dedup();
                let n = sent(&o);
                println!("{content:?} at {snr} dB: {} of {n}", got.len());
                assert!(
                    got.len() + 1 >= n,
                    "{content:?} at {snr} dB: {} of {n}",
                    got.len()
                );
                assert!(got
                    .iter()
                    .all(|s| parse_sentence(s, &[]).is_some_and(|p| p.callsign == "RTTYTEST")));
            }
        }
    }

    /// The sent packets of `o` that `f` holds with exactly the bytes sent (each counted once), and the decoded packets whose
    /// bytes no sent packet has.
    pub fn matched(o: &GenOptions, f: &[Found]) -> (usize, usize) {
        use sdrkit_stack_science::horus::gen::{flight, rtty_sentence};
        let version = match o.content {
            Content::V1 => 1,
            Content::V2 => 2,
            _ => 0,
        };
        let mut got = std::collections::BTreeSet::new();
        let mut wrong = 0;
        for p in f {
            let ok = match &p.decoded {
                Decoded::Binary { bytes, .. } if version != 0 => {
                    let seq = u16::from_le_bytes(if version == 1 {
                        [bytes[1], bytes[2]]
                    } else {
                        [bytes[2], bytes[3]]
                    });
                    let k = usize::from(seq.wrapping_sub(100));
                    let t = start_of(o, k);
                    (k < 1000 && flight(version, k, t).to_bytes() == *bytes).then_some(k)
                }
                Decoded::Rtty { sentence, .. } if version == 0 => parse_sentence(sentence, &[])
                    .and_then(|s| {
                        let k = (s.seq as usize).checked_sub(100)?;
                        // The checksum reads in either case.
                        (k < 1000
                            && rtty_sentence(&flight(0, k, start_of(o, k)))
                                .eq_ignore_ascii_case(sentence))
                        .then_some(k)
                    }),
                _ => None,
            };
            match ok {
                Some(k) => {
                    got.insert(k);
                }
                None => wrong += 1,
            }
        }
        (got.len(), wrong)
    }

    /// The start time of packet `k` of the single balloon of `o`, s after 12:00:00.
    fn start_of(o: &GenOptions, k: usize) -> f64 {
        sdrkit_stack_science::horus::gen::packet_starts(o)
            .get(k)
            .copied()
            .unwrap_or(-1.0)
    }

    /// The packets of `content` received at each of `snrs` over `seeds`, `seconds` a seed, at 48 ksps with the carrier
    /// +1.2 kHz and the clock +1000 ppm: (SNR, received, sent, wrong), summed over the seeds.
    pub fn sweep(
        content: Content,
        snrs: &[f64],
        seconds: f64,
        seeds: &[u64],
    ) -> Vec<(f64, usize, usize, usize)> {
        let o = GenOptions {
            content,
            carrier_hz: 1_200.0,
            clock_ppm: 1_000.0,
            seconds,
            ..GenOptions::default()
        };
        let n = sent(&o);
        let jobs: Vec<(f64, u64)> = snrs
            .iter()
            .flat_map(|&snr| seeds.iter().map(move |&seed| (snr, seed)))
            .collect();
        let done: Vec<(f64, usize, usize)> = std::thread::scope(|s| {
            let handles: Vec<_> = jobs
                .chunks(jobs.len().div_ceil(3))
                .map(|part| {
                    let o = o.clone();
                    s.spawn(move || {
                        part.iter()
                            .map(|&(snr, seed)| {
                                let (got, wrong) = matched(&o, &receive(&o, Some(snr), seed));
                                (snr, got, wrong)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        snrs.iter()
            .map(|&snr| {
                let at = done.iter().filter(|d| d.0 == snr);
                let (got, wrong) = at.fold((0, 0), |(g, w), d| (g + d.1, w + d.2));
                (snr, got, n * seeds.len(), wrong)
            })
            .collect()
    }

    /// The SNRs of the sweep of the specification, dB in 3 kHz.
    const SNRS: [f64; 13] = [
        -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0, 2.0, 3.0, 4.0, 6.0, 10.0,
    ];

    /// At each SNR of the sweep, over three seeds of 200 s, the share of the sent packets received is at least the share
    /// that section 3.12 of the specification measured, and no packet has content that was not sent.
    fn the_sweep_holds(content: Content, spec_pct: [u32; 13]) {
        let r = sweep(content, &SNRS, 200.0, &[11, 12, 13]);
        let line: Vec<String> = r
            .iter()
            .map(|(s, g, n, w)| {
                format!(
                    "{s}: {g}/{n} = {:.0} %{}",
                    100.0 * *g as f64 / *n as f64,
                    if *w > 0 {
                        format!(" ({w} wrong)")
                    } else {
                        String::new()
                    }
                )
            })
            .collect();
        println!("{content:?}: {}", line.join(", "));
        for ((snr, got, n, wrong), pct) in r.iter().zip(spec_pct) {
            assert_eq!(*wrong, 0, "{content:?} at {snr} dB: {wrong} wrong");
            assert!(
                100 * got >= pct as usize * n,
                "{content:?} at {snr} dB: {got} of {n}, {pct} % needed"
            );
        }
    }

    #[test]
    fn the_sweep_of_binary_v2() {
        the_sweep_holds(
            Content::V2,
            [57, 92, 98, 98, 98, 98, 98, 98, 98, 98, 98, 98, 98],
        );
    }

    #[test]
    fn the_sweep_of_binary_v1() {
        the_sweep_holds(
            Content::V1,
            [58, 91, 97, 98, 98, 98, 98, 98, 98, 98, 98, 98, 98],
        );
    }

    #[test]
    fn the_sweep_of_rtty_50_baud() {
        the_sweep_holds(
            Content::Rtty50,
            [0, 6, 17, 56, 78, 94, 94, 94, 94, 94, 94, 94, 94],
        );
    }

    #[test]
    fn the_sweep_of_rtty_100_baud() {
        the_sweep_holds(
            Content::Rtty100,
            [0, 0, 0, 0, 9, 24, 68, 91, 97, 97, 97, 97, 97],
        );
    }

    #[test]
    fn the_sweep_of_rtty_300_baud() {
        the_sweep_holds(
            Content::Rtty300,
            [0, 0, 0, 0, 0, 0, 0, 0, 13, 41, 72, 99, 99],
        );
    }

    /// The bit error rate of non-coherent orthogonal 4FSK at `snr_db` in 3 kHz at 100 baud: Es/N0 is the SNR + 14.8 dB, and
    /// two thirds of the wrong symbols' bits are wrong.
    fn theory_ber(snr_db: f64) -> f64 {
        let g = 10f64.powf((snr_db + 10.0 * 30f64.log10()) / 10.0);
        let ps = 1.5 * (-g / 2.0).exp() - (-2.0 * g / 3.0).exp() + 0.25 * (-0.75 * g).exp();
        2.0 / 3.0 * ps
    }

    /// The 4FSK chain on the true tones (the shared channelizer, the tone bank, the frame timing, the soft values and the
    /// decoder): the bits that Golay corrects per frame give a bit error rate within 0.2 dB of theory, and every frame
    /// decodes but at most one.
    #[test]
    fn the_chain_on_true_tones_meets_theory() {
        use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
        use sdrkit_stack_science::horus::demod::FourFsk;
        let o = GenOptions {
            content: Content::V2,
            carrier_hz: 1_200.0,
            clock_ppm: 1_000.0,
            seconds: 200.0,
            ..GenOptions::default()
        };
        let clean = render(&o);
        for snr in [-5.0, -6.0] {
            let (mut fixed, mut frames, mut got, mut wrong) = (0usize, 0usize, 0usize, 0usize);
            for seed in [11, 12, 13] {
                let mut x = clean.clone();
                Awgn::new(o.amplitude * o.amplitude, snr, 3_000.0, o.rate_hz, seed).process(&mut x);
                let mut cfg = ChannelConfig::new(48e3, 0.0, 0.8 * 9600.0, 9600.0);
                cfg.exact_rate = true;
                cfg.atten_db = 70.0;
                let mut chan = Channelizer::new(cfg).unwrap();
                let mut y = Vec::new();
                chan.process(&x, &mut y);
                let mut fsk = FourFsk::new(chan.output_rate());
                fsk.retune([795.0, 1065.0, 1335.0, 1605.0]);
                let mut out = Vec::new();
                for c in y.chunks(256) {
                    fsk.process(c, 0.0, &mut out);
                }
                for p in &out {
                    if let Decoded::Binary { fixed: f, .. } = p.decoded {
                        fixed += f;
                        frames += 1;
                    }
                }
                let (g, w) = matched(&o, &out);
                got += g;
                wrong += w;
            }
            let ber = fixed as f64 / frames.max(1) as f64 / 506.0;
            let n = 3 * sent(&o);
            println!(
                "true tones at {snr} dB: {got} of {n}, bit errors {ber:.4} against {:.4}",
                theory_ber(snr)
            );
            assert!(got + 3 >= n && wrong == 0, "{got} of {n}, {wrong} wrong");
            assert!(
                ber <= theory_ber(snr - 0.2),
                "{ber} against {}",
                theory_ber(snr)
            );
        }
    }

    #[test]
    fn impairments_the_snr_reading_and_noise_alone() {
        let base = GenOptions {
            seconds: 40.0,
            ..GenOptions::default()
        };
        let cases = [
            (
                "v2 mirrored, -800 Hz",
                GenOptions {
                    mirrored: true,
                    carrier_hz: -800.0,
                    ..base.clone()
                },
                10.0,
            ),
            (
                "v2 at +3 kHz",
                GenOptions {
                    carrier_hz: 3_000.0,
                    ..base.clone()
                },
                10.0,
            ),
            (
                "v1 at -3000 ppm",
                GenOptions {
                    content: Content::V1,
                    clock_ppm: -3_000.0,
                    ..base.clone()
                },
                10.0,
            ),
            (
                "RTTY 300 mirrored, +2000 ppm, +500 Hz",
                GenOptions {
                    content: Content::Rtty300,
                    mirrored: true,
                    clock_ppm: 2_000.0,
                    carrier_hz: 500.0,
                    ..base.clone()
                },
                15.0,
            ),
        ];
        for (name, o, snr) in cases {
            let f = receive(&o, Some(snr), 9);
            let mut got = binaries(&f);
            if got == 0 {
                let mut s = sentences(&f);
                s.dedup();
                got = s.len();
            }
            let n = sent(&o);
            println!("{name}: {got} of {n}");
            assert!(n >= 6 && got + 1 >= n, "{name}: {got} of {n}");
        }
        let f = receive(
            &GenOptions {
                seconds: 40.0,
                ..GenOptions::default()
            },
            Some(20.0),
            4,
        );
        let snr: Vec<f64> = f.iter().map(|p| p.snr_db).collect();
        assert!(snr.iter().all(|s| (s - 20.0).abs() < 6.0), "{snr:?}");
        let quiet = GenOptions {
            amplitude: 0.0,
            seconds: 30.0,
            ..GenOptions::default()
        };
        let mut x = render(&quiet);
        Awgn::with_sigma(0.1, 8).process(&mut x);
        let mut ch = Channel::new(48e3, 0.0).unwrap();
        let mut out = Vec::new();
        for c in x.chunks(960) {
            ch.process(c, &mut out);
        }
        assert!(out.is_empty(), "{out:?}");
    }
}

#[test]
fn the_data_files_read() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/science");
    let ids = IdList::parse(&std::fs::read_to_string(dir.join("horus_payload_ids.txt")).unwrap());
    assert_eq!(
        (
            ids.unreadable,
            ids.callsign(0).as_str(),
            ids.callsign(256).as_str()
        ),
        (0, "4FSKTEST", "4FSKTEST-V2")
    );
    let f = parse_formats(&std::fs::read_to_string(dir.join("horus_custom_fields.txt")).unwrap())
        .unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].structure.as_str(), &f[0].fields),
        ("<hhBHxx", &rs41ng().fields)
    );
    let csv = std::fs::read_to_string(dir.join("horus_frequencies.csv")).unwrap();
    let hz: Vec<f64> = csv
        .lines()
        .skip_while(|l| l.starts_with('#'))
        .skip(1)
        .map(|l| l.split(',').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(hz, [434.2e6, 434.714e6, 437.6e6, 431.05e6]);
    // The stack holds no frequency: every frequency of the file lies in its range, and its default custom formats are those
    // of the file.
    let t = &sdrkit_stack_science::horus::DESCRIPTOR.tuning;
    assert_eq!(t.default_hz, 0.0);
    assert!(hz.iter().all(|f| (t.min_hz..=t.max_hz).contains(f)));
    assert_eq!(
        parse_formats(sdrkit_stack_science::horus::CUSTOM_FORMATS.default),
        Ok(f)
    );
}

/// The whole stack: three balloons in one capture, the shared decimator of a wide input, the saved state, the tables and a
/// change of frequency.
mod stack {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_core::{InputInfo, Output, Owner, Receiver, StackConfig, Values};
    use sdrkit_core::{SettingSet, Value};
    use sdrkit_dev_test::Awgn;
    use sdrkit_graph::Component;
    use sdrkit_graph::StackGraph;
    use sdrkit_stack_science::horus::gen::{Content, GenOptions, Signal};
    use sdrkit_stack_science::horus::payloads::PayloadTable;
    use sdrkit_stack_science::horus::{self, HorusSetting};
    use sdrkit_stack_science::typed::{
        HorusChannelRow, HorusCustomFieldRow, HorusFields, HorusLogFields, HorusPayloadRow,
        HORUS_CHANNELS, HORUS_CUSTOM_FIELDS, HORUS_PACKET, HORUS_PAYLOADS,
    };

    /// The radio is tuned 30 kHz above the dial.
    const DIAL_HZ: f64 = 434.2e6;

    /// A receiver at the default settings, configured for `rate` with the dial `dial` Hz from the centre.
    fn receiver(rate: f64, dial: f64) -> StackGraph {
        let values = Values::defaults(&(horus::DESCRIPTOR.settings)());
        let mut rx = horus::receiver(&StackConfig::new(values)).unwrap();
        rx.configure(&InputInfo::new(rate, DIAL_HZ - dial, DIAL_HZ).recording())
            .unwrap();
        rx
    }

    /// Feeds `seconds` of `o` with noise of `snr_db` in 3 kHz a balloon; returns the events.
    fn feed(
        rx: &mut StackGraph,
        o: &GenOptions,
        snr_db: f64,
        seconds: f64,
    ) -> Vec<sdrkit_core::Event> {
        let mut sig = Signal::new(o);
        let mut awgn = Awgn::new(o.amplitude * o.amplitude, snr_db, 3_000.0, o.rate_hz, 21);
        let piece = (o.rate_hz / 20.0) as usize;
        let mut x = vec![sdrkit_core::Complex32::default(); piece];
        let mut out = Output::default();
        let mut events = Vec::new();
        for _ in 0..(seconds * 20.0) as usize {
            sig.fill(&mut x);
            awgn.process(&mut x);
            out.clear();
            rx.feed(&x, &mut out);
            events.append(&mut out.events);
        }
        out.clear();
        rx.finish(&mut out);
        events.append(&mut out.events);
        events
    }

    fn payloads(rx: &StackGraph) -> Vec<(String, HorusPayloadRow<'_>)> {
        // Each map point shows its payload row.
        let r = Owner::results(rx);
        for p in r.points().values() {
            let link = p.row.as_ref().expect("a row");
            assert_eq!(Some(link.table), r.decl().table_id(HORUS_PAYLOADS.key));
            assert!(
                r.table(link.table).unwrap().row(&link.key).is_some(),
                "{}",
                p.id
            );
        }
        HORUS_PAYLOADS
            .rows::<HorusPayloadRow>(Owner::results(rx))
            .map(|(k, r)| (k.to_string(), r))
            .collect()
    }

    #[test]
    fn three_balloons_in_one_capture() {
        let o = GenOptions {
            rate_hz: 192e3,
            content: Content::Three,
            dial_hz: -30e3,
            carrier_hz: 1_500.0,
            amplitude: 1.0,
            ..GenOptions::default()
        };
        let mut rx = receiver(o.rate_hz, o.dial_hz);
        let events = feed(&mut rx, &o, 8.0, 60.0);
        let r = Owner::results(&rx);
        assert!(!r.points().is_empty());
        let p = payloads(&rx);
        let names: Vec<&str> = p.iter().map(|(k, _)| k.as_str()).collect();
        println!("payloads {names:?}");
        for (k, row) in &p {
            println!(
                "  {k}: {} packets, offset {:.0}, spacing {:.1}, climb {:?}, track {}",
                row.packets, row.offset.0, row.spacing.0, row.climb_m_s, row.track_points
            );
        }
        let get = |name: &str| {
            p.iter()
                .find(|(k, _)| k == name)
                .map(|(_, r)| r.clone())
                .unwrap_or_else(|| panic!("no {name} in {names:?}"))
        };
        let v2 = get("4FSKTEST-V2");
        let v1 = get("ID 0");
        let rtty = get("RTTYTEST");
        assert_eq!(p.len(), 3, "no packet of another callsign: {names:?}");
        assert!(v2.packets >= 12, "{}", v2.packets);
        assert!(v1.packets >= 8, "{}", v1.packets);
        assert!(rtty.packets >= 4, "{}", rtty.packets);
        let f = HorusFields::read(r.fields()).unwrap();
        assert!(f.channels >= 3, "{}", f.channels);
        assert!((v1.offset.0 - 13_500.0).abs() <= 300.0, "{}", v1.offset.0);
        assert!(
            (rtty.offset.0 + 16_500.0).abs() <= 300.0,
            "{}",
            rtty.offset.0
        );
        assert_eq!((rtty.baud, rtty.rtty_bits), (100.0, Some(7)));
        assert!((v2.offset.0 - 1_500.0).abs() <= 100.0, "{}", v2.offset.0);
        assert!((v2.spacing.0 - 270.0).abs() <= 15.0, "{}", v2.spacing.0);
        assert!(v2.track_points >= 10, "{}", v2.track_points);
        let climb = v2.climb_m_s.unwrap();
        assert!((climb - 5.0).abs() <= 0.6, "{climb}");
        // The row key ends with the place of the field.
        let mut custom: Vec<(String, String)> = HORUS_CUSTOM_FIELDS
            .rows::<HorusCustomFieldRow>(r)
            .filter(|(_, c)| c.payload == "4FSKTEST-V2")
            .map(|(k, c)| (k.to_string(), c.value.to_string()))
            .collect();
        custom.sort();
        let custom: Vec<String> = custom.into_iter().map(|(_, v)| v).collect();
        assert_eq!(
            custom[..2],
            ["5.0".to_string(), "-12.3".to_string()],
            "{custom:?}"
        );
        assert_eq!((v2.sats, v2.speed_kmh), (Some(9), Some(38.0)));
        assert!((v2.battery_v.unwrap() - 3.0).abs() <= 0.02);
        let n_events = events.iter().filter(|e| e.is(HORUS_PACKET.def())).count() as i64;
        let log = HorusLogFields::read(r.fields()).unwrap();
        assert_eq!(n_events, log.packets);
        assert_eq!(log.packets, v2.packets + v1.packets + rtty.packets);
        // The saved state: a round trip keeps the counts, a broken blob is refused, a new ID list renames ID 0.
        let blob = rx
            .graph()
            .component("payloads")
            .and_then(|c| c.save_state())
            .unwrap();
        let mut back = receiver(o.rate_hz, o.dial_hz);
        back.graph_mut()
            .component_as_mut::<PayloadTable>("payloads")
            .unwrap()
            .restore_state(&blob)
            .unwrap();
        back.forget();
        let restored = payloads(&back);
        assert_eq!(restored.len(), 3);
        let counts = |p: &[(String, HorusPayloadRow)]| {
            p.iter()
                .map(|(k, r)| (k.clone(), r.packets))
                .collect::<Vec<_>>()
        };
        assert_eq!(counts(&restored), counts(&p));
        assert_eq!(
            HorusLogFields::read(Owner::results(&back).fields())
                .unwrap()
                .packets,
            log.packets
        );
        // The same through the receiver, as the engine saves and restores it.
        let saved = Receiver::save_state(&rx).unwrap();
        let mut again = receiver(o.rate_hz, o.dial_hz);
        again.restore_state(&saved).unwrap();
        // The results show what was restored after the next pass.
        let mut out = Output::default();
        again.feed(&vec![sdrkit_core::Complex32::default(); 19_200], &mut out);
        assert_eq!(counts(&payloads(&again)), counts(&p));
        let text = String::from_utf8(blob.bytes().to_vec()).unwrap();
        let broken = sdrkit_core::StateBlob::new(
            blob.owner().to_string(),
            blob.version(),
            text.replacen('\t', "x", 1).into_bytes(),
        );
        let table = back
            .graph_mut()
            .component_as_mut::<PayloadTable>("payloads")
            .unwrap();
        assert!(table.restore_state(&broken).is_err());
        let id = HorusSetting::PayloadIds.id();
        sdrkit_core::owner::apply(
            &mut back,
            id,
            &Value::Text("0, HORUSV1\n256, 4FSKTEST-V2".into()),
        )
        .unwrap();
        back.forget();
        let renamed = payloads(&back);
        assert!(
            renamed
                .iter()
                .any(|(k, r)| k == "HORUSV1" && r.packets == v1.packets),
            "{:?}",
            counts(&renamed)
        );
        assert!(!renamed.iter().any(|(k, _)| k == "ID 0"));
    }

    #[test]
    fn a_change_of_frequency_keeps_the_payloads_and_clears_the_channels() {
        let o = GenOptions {
            rate_hz: 192e3,
            content: Content::Three,
            dial_hz: -30e3,
            amplitude: 1.0,
            ..GenOptions::default()
        };
        let mut rx = receiver(o.rate_hz, o.dial_hz);
        let _ = feed(&mut rx, &o, 15.0, 20.0);
        let before = payloads(&rx).len();
        assert!(before >= 2, "{before}");
        assert!(
            HorusFields::read(Owner::results(&rx).fields())
                .unwrap()
                .channels
                >= 2
        );
        rx.forget();
        let r = Owner::results(&rx);
        assert_eq!(payloads(&rx).len(), before);
        let f = HorusFields::read(r.fields()).unwrap();
        assert_eq!((f.channels, f.syncs, f.snr_db), (1, 0, None));
        let rows: Vec<HorusChannelRow> = HORUS_CHANNELS
            .rows::<HorusChannelRow>(r)
            .map(|(_, c)| c)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].packets, 0);
    }

    #[test]
    fn a_wide_input_through_the_shared_decimator() {
        let o = GenOptions {
            rate_hz: 5e6,
            content: Content::V2,
            dial_hz: -30e3,
            carrier_hz: -700.0,
            amplitude: 1.0,
            ..GenOptions::default()
        };
        let mut rx = receiver(o.rate_hz, o.dial_hz);
        let _ = feed(&mut rx, &o, 10.0, 12.0);
        let p = payloads(&rx);
        assert_eq!(
            p.len(),
            1,
            "{:?}",
            p.iter().map(|(k, _)| k).collect::<Vec<_>>()
        );
        assert!(p[0].1.packets >= 2, "{}", p[0].1.packets);
        assert!(
            (p[0].1.offset.0 + 700.0).abs() <= 100.0,
            "{}",
            p[0].1.offset.0
        );
    }

    #[test]
    fn a_custom_format_list_that_does_not_read_is_refused() {
        let mut rx = receiver(48e3, 0.0);
        let id = HorusSetting::CustomFormats.id();
        assert!(sdrkit_core::owner::apply(&mut rx, id, &Value::Text("X|<hhZ".into())).is_err());
        let ids = HorusSetting::PayloadIds.id();
        sdrkit_core::owner::apply(
            &mut rx,
            ids,
            &Value::Text("0, A\nbad line here\n70000, X".into()),
        )
        .unwrap();
        let o = GenOptions {
            content: Content::V1,
            amplitude: 1.0,
            ..GenOptions::default()
        };
        let _ = feed(&mut rx, &o, 20.0, 10.0);
        let log = HorusLogFields::read(Owner::results(&rx).fields()).unwrap();
        assert_eq!(log.unreadable_ids, 2);
        assert!(payloads(&rx).iter().any(|(k, _)| k == "A"));
    }
}
