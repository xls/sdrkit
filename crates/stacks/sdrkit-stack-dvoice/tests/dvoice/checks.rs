//! Checks of single parts: codes, CRCs, frame layouts, the coarse search, the declared events and the allocations of the
//! receive path.

use crate::allocations;
use crate::common::*;
use sdrkit_algo_fec::conv::tcm::dibit_symbol;
use sdrkit_core::Complex32;
use sdrkit_stack_dvoice::{dmr, nxdn, p25};
use sdrkit_util_bits::crc::catalog::{
    crc32_dmr_packet, CRC16_GENIBUS, CRC16_GSM, CRC16_M17, CRC32_CKSUM, CRC32_DMR,
};

#[test]
fn the_dmr_packet_crc_is_crc32_dmr_not_cksum() {
    // TS 102 361-1 clause B.3.9: CRC-32 with register 0 and no inversion, over the octets taken in pairs.
    assert_eq!(CRC32_DMR.checksum(b"123456789"), 0x89A1_897F);
    assert_eq!(
        crc32_dmr_packet(b"123456789"),
        CRC32_DMR.checksum(b"214365879")
    );
    assert_ne!(
        crc32_dmr_packet(b"123456789"),
        CRC32_CKSUM.checksum(b"214365879")
    );
}

#[test]
fn a_dmr_packet_crc_sent_in_the_wrong_octet_order_is_refused() {
    let mut data = b"TEST MESSAGE".to_vec();
    let crc = crc32_dmr_packet(&data);
    let mut right = data.clone();
    right.extend_from_slice(&crc.to_le_bytes());
    data.extend_from_slice(&crc.to_be_bytes());
    assert!(dmr::packet_crc_ok(&right));
    assert!(!dmr::packet_crc_ok(&data));
}

#[test]
fn the_voice_crcs_have_their_check_values() {
    // The CRC RevEng catalogue check values of "123456789".
    assert_eq!(CRC16_GSM.checksum(b"123456789"), 0xCE3C); // DMR and P25 "CRC-CCITT", System Fusion, TETRAPOL
    assert_eq!(CRC16_GENIBUS.checksum(b"123456789"), 0xD64E); // TETRA
    assert_eq!(CRC16_M17.checksum(b"123456789"), 0x772B); // M17 specification
}

/// The 98 symbol levels of a DMR rate 3/4 block of `info`.
fn trellis_levels(info: &[u8]) -> Vec<f32> {
    let mut codec = dmr::Codecs::default().tcm;
    let mut sent = [0u8; 196];
    codec.encode(info, &mut sent).unwrap();
    sent.chunks(2)
        .map(|d| dibit_symbol((d[0] << 1) | d[1]) as f32)
        .collect()
}

#[test]
fn dmr_rate_3_4_blocks_decode_from_soft_levels_without_allocating() {
    let info: Vec<u8> = (0..144).map(|i| ((i * 5) % 7 < 3) as u8).collect();
    let mut levels = trellis_levels(&info);
    // Six symbols received just across their decision threshold: wrong as hard decisions, close as levels.
    for &i in &[3usize, 17, 30, 44, 61, 80] {
        let v = levels[i];
        levels[i] = if v > 2.0 || (0.0..2.0).contains(&v) {
            v - 1.1
        } else {
            v + 1.1
        };
    }
    let mut codec = dmr::Codecs::default().tcm;
    let mut out = [0u8; 144];
    codec.decode_levels(&levels, &mut out).unwrap();
    assert_eq!(out[..], info[..]);
    let n = allocations(|| {
        for _ in 0..1000 {
            codec.decode_levels(&levels, &mut out).unwrap();
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn a_long_convolutional_frame_decodes_without_allocating() {
    // The longest NXDN channel (UDCH: 203 input bits) with noise on every soft value and some errors.
    let mut coder = nxdn::ChannelCoder::new(nxdn::UDCH);
    let info: Vec<u8> = (0..184).map(|i| (i % 3 == 1) as u8).collect();
    let sent = coder.encode(&info);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(11);
    let mut soft: Vec<f32> = sent
        .iter()
        .map(|&b| (if b != 0 { 1.0 } else { -1.0 }) + 0.4 * rng.normal_f32())
        .collect();
    for i in [5, 77, 150, 260] {
        soft[i] = -soft[i];
    }
    let (got, ok, _) = coder.decode(&soft);
    assert!(ok);
    assert_eq!(got, info);
    let n = allocations(|| {
        for _ in 0..1000 {
            let _ = coder.decode(&soft);
        }
    });
    // One vector for the information bits returned.
    assert!(n <= 1000, "{n}");
}

#[test]
fn p25_link_control_beyond_the_rs_errors_decodes_with_the_inner_failures_as_erasures() {
    let mut c = p25::Codecs::default();
    let hex: Vec<u8> = (0..12).map(|i| (i * 7 + 5) as u8 & 0x3F).collect();
    let sent = p25::encode_ldu_hexbits(&hex, &c.rs_24_12);
    let mut bits = sent.clone();
    // Ten of the 24 hex bits hit by two errors on check bits 1 and 3 each: the syndrome 0101 is no single error, so the
    // Hamming (10,6) code detects them. RS(24,12) corrects 6 errors, or 12 erasures.
    for w in 0..10 {
        bits[w * 10 + 7] ^= 1;
        bits[w * 10 + 9] ^= 1;
    }
    let (got, ok, _) = p25::decode_ldu_hexbits(&bits, &mut c.rs_24_12);
    assert!(ok);
    assert_eq!(got, hex);
}

#[test]
fn a_p25_rs_word_beyond_repair_is_refused() {
    let mut c = p25::Codecs::default();
    let hex: Vec<u8> = (0..12).map(|i| (i * 3 + 1) as u8 & 0x3F).collect();
    let mut bits = p25::encode_ldu_hexbits(&hex, &c.rs_24_12);
    // Every hex bit wrong with an error its Hamming word corrects to another value.
    for w in 0..24 {
        bits[w * 10 + 9] ^= 1;
        bits[w * 10 + 2] ^= 1;
        bits[w * 10 + 5] ^= 1;
    }
    let (got, ok, _) = p25::decode_ldu_hexbits(&bits, &mut c.rs_24_12);
    assert!(!ok || got == hex, "a wrong word passed: {got:?}");
}

#[test]
fn the_p25_nid_corrects_eleven_errors_without_allocating() {
    let mut c = p25::Codecs::default();
    let w = p25::encode_nid(0x293, p25::duid::LDU1);
    let errors: u64 = (0..11).map(|i| 1u64 << (i * 5 + 3)).sum();
    assert_eq!(
        p25::decode_nid(&mut c.nid, w ^ errors).map(|d| (d.0, d.1)),
        Some((0x293, 5))
    );
    let n = allocations(|| {
        for _ in 0..1000 {
            let _ = p25::decode_nid(&mut c.nid, w ^ errors);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn a_short_lc_with_one_error_per_row_is_corrected() {
    let raw = dmr::encode_short_lc(1, 0x08_0000);
    // Matrix bit i (below 67) is sent at 4 i mod 67: one bit of each of the three Hamming rows.
    let mut bad = raw;
    for i in [2usize, 20, 40] {
        bad[(i * 4) % 67] ^= 1;
    }
    assert_eq!(dmr::decode_short_lc(&bad), Some((1, 0x08_0000, 3)));
}

#[test]
fn encoding_a_short_lc_allocates_nothing() {
    let n = allocations(|| {
        for i in 0..1000u32 {
            let _ = std::hint::black_box(dmr::encode_short_lc(1, i << 8));
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn the_coarse_search_does_not_read_the_environment() {
    // These variables turned the coarse search and the fine loop off on every buffer.
    std::env::set_var("NOCOARSE", "1");
    std::env::set_var("NOFINE", "1");
    // A DMR station 9 kHz off: only the coarse search brings it into the channel.
    let x = impaired("dmr", 96_000.0, 9_000.0, 0.0, 30.0);
    let mut rx = receiver("dmr", &[]);
    let (r, _) = run(rx.as_mut(), 96_000.0, &x, 4800);
    std::env::remove_var("NOCOARSE");
    std::env::remove_var("NOFINE");
    let c = calls(&r);
    assert!(
        c.iter().any(|c| c["src"] == 3101234 && c["dst"] == 91),
        "{}",
        show(&c)
    );
}

#[test]
fn every_event_the_modes_emit_is_declared_and_has_its_frequency() {
    for d in sdrkit_stack_dvoice::descriptors() {
        let decl = sdrkit_stack_dvoice::declaration(d.id);
        let rate = d.tuning.min_rate_hz.max(96_000.0);
        let run = if d.id != "tetra_dmo" {
            sdrkit_dev_test::Harness::new(d)
                .rate(rate)
                .freq(crate::common::start_hz(d))
                .seconds(6.0)
                .run()
                .unwrap()
        } else {
            // A stack whose test signal lies where a radio tuned with its offset sees the channel.
            let rate = d.tuning.rate_hz;
            let h = sdrkit_dev_test::Harness::new(d).rate(rate).seconds(6.0);
            let x = h.generate().unwrap();
            let mut rx = h.receiver().unwrap();
            let f = crate::common::start_hz(d);
            rx.configure(
                &sdrkit_core::InputInfo::new(rate, f + d.tuning.tune_offset_hz, f).recording(),
            )
            .unwrap();
            let mut out = sdrkit_core::Output::default();
            let mut events = Vec::new();
            for c in x.chunks(9600) {
                rx.feed(c, &mut out);
                events.append(&mut out.events);
                out.clear();
            }
            sdrkit_dev_test::Run {
                events,
                ..Default::default()
            }
        };
        let calls: Vec<_> = run.events_of("dvoice.call").collect();
        assert!(!calls.is_empty(), "{} raised no dvoice.call event", d.id);
        for e in &run.events {
            assert!(
                decl.events().iter().any(|k| k.kind == e.kind()),
                "{}: {} is not declared",
                d.id,
                e.kind()
            );
            assert!(e.freq_hz != 0.0, "{}: {} sent at 0 Hz", d.id, e.kind());
        }
    }
}

#[test]
fn the_receiver_allocates_nothing_between_its_reports() {
    let mut rx = receiver("dmr", &[]);
    rx.configure(&sdrkit_core::InputInfo::new(96_000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
    let x: Vec<Complex32> = (0..2 * 96_000).map(|_| rng.complex_normal(0.01)).collect();
    let mut out = sdrkit_core::Output::default();
    // One second of noise: every buffer is sized and the reports of that second are written.
    for c in x[..96_000].chunks(4800) {
        rx.feed(c, &mut out);
        out.clear();
    }
    // The next 50 ms hold no report (one every 0.25 s).
    let n = allocations(|| {
        for c in x[96_000..96_000 + 4800].chunks(1200) {
            rx.feed(c, &mut out);
            out.clear();
        }
    });
    assert_eq!(n, 0);
}

/// A plug-in that keeps the length of every frame it is given.
struct Lengths(std::sync::Arc<std::sync::Mutex<Vec<usize>>>);
impl sdrkit_core::codec::AudioCodec for Lengths {
    fn decode(
        &mut self,
        frame: &[u8],
        erasure: bool,
        pcm: &mut sdrkit_core::codec::Pcm,
    ) -> Result<(), sdrkit_core::codec::CodecError> {
        if !erasure {
            self.0.lock().unwrap().push(frame.len());
        }
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        pcm.samples.extend(std::iter::repeat_n(1000i16, 160));
        Ok(())
    }
}

/// The frame lengths a plug-in registered for `fourcc` in `layout` was given while mode `id` decoded its test signal.
fn frames_given(id: &str, fourcc: &str, layout: &str) -> Vec<usize> {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let mut codecs = sdrkit_core::codec::CodecSet::default();
    codecs.register(
        sdrkit_core::FourCc::new(fourcc),
        Some(layout),
        std::sync::Arc::new(move |_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Lengths(s2.clone())) as Box<dyn sdrkit_core::codec::AudioCodec>)
        }),
    );
    let mut g = generator(id, 96_000.0, 3, &[]);
    let x = generate_all(g.as_mut());
    let mut rx = receiver_with_codecs(id, &[], codecs);
    run(rx.as_mut(), 96_000.0, &x, 4800);
    let v = seen.lock().unwrap().clone();
    v
}

#[test]
fn p25_imbe_frames_leave_as_their_144_channel_bits() {
    // No IMBE de-interleave is invented: the plug-in gets the 144 bits as sent (TIA-102.BABA de-interleave is its own).
    let lens = frames_given("p25", "imbe", "p25_144");
    assert!(!lens.is_empty());
    assert!(lens.iter().all(|&n| n == 18), "{lens:?}");
}

#[test]
fn dstar_ambe_frames_leave_as_their_72_channel_bits() {
    let lens = frames_given("dstar", "ambe", "dstar72");
    assert!(!lens.is_empty());
    assert!(lens.iter().all(|&n| n == 9), "{lens:?}");
}
