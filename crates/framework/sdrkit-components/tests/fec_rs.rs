//! `fec.rs_decode`, with and without its erasure input: DMR RS(12,9) words built from the parity table of TS 102 361-1 (table B.18),
//! DVB packets with errors (and one beyond the limit), CCSDS frames interleaved to depth 4 in the dual basis, erasures, any cut,
//! and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_components::fec::{Erasures, FrameStatus, RsDecode};
use sdrkit_components::testing::Source;
use sdrkit_core::{ConfigError, Value, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, Timed};

fn bytes_format() -> Format {
    Format::new(Kind::Bytes, 0.0)
}

#[test]
fn dmr_words_from_the_published_parity_table_are_corrected() {
    // TS 102 361-1 table B.18: the parity (p2 p1 p0) of each message unit vector, m8 first. The code is linear, so the parity of
    // a message whose symbols are 0 or 1 is the XOR of the rows of its 1 symbols.
    let rows = [
        [0x1C, 0xBC, 0xFD],
        [0x89, 0x31, 0x08],
        [0xAD, 0x41, 0x36],
        [0x7D, 0x71, 0x16],
        [0xF3, 0xA6, 0x3A],
        [0x08, 0x83, 0x7B],
        [0x3F, 0x6F, 0x02],
        [0x6C, 0x0D, 0xA7],
        [0x0E, 0x38, 0x40],
    ];
    let mut words = Vec::new();
    let mut want = Vec::new();
    for mask in [
        0b1_0000_0001u16,
        0b0_1010_1010,
        0b1_1111_1111,
        0b0_0000_0000,
    ] {
        let data: Vec<u8> = (0..9).map(|i| (mask >> (8 - i)) as u8 & 1).collect();
        let mut parity = [0u8; 3];
        for (i, &d) in data.iter().enumerate() {
            if d == 1 {
                for j in 0..3 {
                    parity[j] ^= rows[i][j];
                }
            }
        }
        let mut w = data.clone();
        w.extend_from_slice(&parity);
        // One symbol in error (the code corrects one).
        w[(mask as usize) % 12] ^= 0x37;
        words.push(w);
        want.push(data);
    }
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&words), 1))
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .set("rs.code", txt("dmr_12_9"))
        .build()
        .unwrap();
    let got = run(&mut g, 6, &["rs.out", "rs.status"]);
    assert_eq!(packets(&got[0].data), want);
    assert!(got[1].status.iter().all(|s| s.ok && s.corrected == 1));
}

fn dvb_packets(rng: &mut Rng, count: usize) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let rs = Rs::new(codes::DVB_204_188).unwrap();
    let mut sent = Vec::new();
    let mut data = Vec::new();
    for i in 0..count {
        let mut w = rng.bytes(204);
        w[0] = 0x47;
        rs.encode_word(&mut w).unwrap();
        data.push(w[..188].to_vec());
        // Packet i has i % 9 errors: 0 to 8, all correctable.
        for p in rng.positions(204, i % 9) {
            w[p] ^= 1 + rng.below(255) as u8;
        }
        sent.push(w);
    }
    (sent, data)
}

#[test]
fn dvb_packets_with_up_to_eight_errors_for_any_cut() {
    let mut rng = Rng(42);
    let (mut sent, data) = dvb_packets(&mut rng, 30);
    // Packet 7 gets 12 errors: beyond the limit.
    for p in rng.positions(204, 12) {
        sent[7][p] ^= 0xA5;
    }
    let mut first_cut = None;
    for per_pass in [1usize, 4, 30] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(bytes_format(), bytes_data(&sent), per_pass),
            )
            .add("rs", RsDecode::default())
            .connect("src", "rs")
            .build()
            .unwrap();
        let got = run(&mut g, 32, &["rs.out", "rs.status"]);
        let out = packets(&got[0].data);
        assert_eq!(out.len(), 30);
        for (i, (o, d)) in out.iter().zip(&data).enumerate() {
            if i == 7 {
                // Passed on as received.
                assert_eq!(o[..], sent[7][..188]);
            } else {
                assert_eq!(o, d, "packet {i}");
            }
        }
        let st = &got[1].status;
        assert_eq!(st.len(), 30);
        assert!(!st[7].ok && st[7].failed_words == 1);
        for (i, s) in st.iter().enumerate().filter(|(i, _)| *i != 7) {
            assert_eq!(
                *s,
                FrameStatus {
                    ok: true,
                    corrected: (i % 9) as u32,
                    iterations: 1,
                    failed_words: 0
                }
            );
        }
        assert_eq!(field(&g, "rs", "packets_failed"), 1.0);
        assert_eq!(field(&g, "rs", "packets_clean"), 4.0);
        assert_eq!(field(&g, "rs", "packets_corrected"), 25.0);
        match &first_cut {
            None => first_cut = Some(out),
            Some(r) => assert_eq!(&out, r),
        }
    }
    // With drop_failed the bad packet goes, and its status stays.
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&sent), 8))
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .set("rs.drop_failed", Value::Bool(true))
        .set("rs.keep_parity", Value::Bool(true))
        .build()
        .unwrap();
    let got = run(&mut g, 5, &["rs.out", "rs.status"]);
    let out = packets(&got[0].data);
    assert_eq!(out.len(), 29);
    assert_eq!(out[0].len(), 204);
    assert_eq!(got[1].status.len(), 30);
}

#[test]
fn ccsds_dual_basis_frames_interleaved_to_depth_four() {
    let rs = Rs::new(codes::CCSDS_255_223).unwrap();
    let mut rng = Rng(9);
    let mut frames = Vec::new();
    let mut want = Vec::new();
    for f in 0..3 {
        let words: Vec<Vec<u8>> = (0..4)
            .map(|_| {
                let mut w = rng.bytes(255);
                rs.encode_word(&mut w).unwrap();
                w
            })
            .collect();
        let mut frame = vec![0u8; 255 * 4];
        for (c, w) in words.iter().enumerate() {
            for (i, &b) in w.iter().enumerate() {
                frame[i * 4 + c] = b;
            }
        }
        want.push(frame[..223 * 4].to_vec());
        // 16 errors in each code word: a burst of 64 bytes.
        for b in &mut frame[100 + f..164 + f] {
            *b ^= 0xFF;
        }
        frames.push(frame);
    }
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&frames), 2))
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .set("rs.code", txt("ccsds_255_223"))
        .set("rs.depth", Value::F64(4.0))
        .build()
        .unwrap();
    let got = run(&mut g, 3, &["rs.out", "rs.status"]);
    assert_eq!(packets(&got[0].data), want);
    assert!(got[1].status.iter().all(|s| s.ok && s.corrected == 64));
    // The same frames read in the conventional basis are not code words.
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&frames), 3))
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .set("rs.code", txt("ccsds_255_223"))
        .set("rs.depth", Value::F64(4.0))
        .set("rs.basis", txt("conventional"))
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["rs.status"]);
    assert!(got[0].status.iter().all(|s| !s.ok));
}

/// Emits one erasure list per pass, from a list of lists.
struct ErasureSource(Vec<Vec<u16>>, usize);

impl Component for ErasureSource {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("test.erasures", vec![Port::typed::<Erasures>("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.1 = 0;
        Ok(vec![Format::new(Kind::typed::<Erasures>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = self.0.get(self.1) {
            io.items_out::<Erasures>(0).push(Timed {
                time: 0.0,
                value: Erasures(l.clone()),
            });
            self.1 += 1;
        }
    }
}

#[test]
fn erasures_double_what_can_be_corrected() {
    // 16 erased symbols per DVB packet: twice the error limit, corrected only because their places are known.
    let mut rng = Rng(77);
    let rs = Rs::new(codes::DVB_204_188).unwrap();
    let mut sent = Vec::new();
    let mut lists = Vec::new();
    let mut want = Vec::new();
    for _ in 0..5 {
        let mut w = rng.bytes(204);
        rs.encode_word(&mut w).unwrap();
        want.push(w[..188].to_vec());
        let pos = rng.positions(204, 16);
        for &p in &pos {
            w[p] ^= 0x3C;
        }
        sent.push(w);
        lists.push(pos.iter().map(|&p| p as u16).collect::<Vec<u16>>());
    }
    // Packets arrive two per pass, erasure lists one per pass: the decoder pairs them in order.
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&sent), 2))
        .add("er", ErasureSource(lists, 0))
        .add("rs", RsDecode::default())
        .connect("src", "rs.in")
        .connect("er", "rs.erasures")
        .build()
        .unwrap();
    let got = run(&mut g, 8, &["rs.out", "rs.status"]);
    assert_eq!(packets(&got[0].data), want);
    assert!(got[1].status.iter().all(|s| s.ok && s.corrected == 16));
    // Without the lists the same packets fail.
    let mut g = Tb::new()
        .add("src", Source::new(bytes_format(), bytes_data(&sent), 2))
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .build()
        .unwrap();
    let got = run(&mut g, 4, &["rs.status"]);
    assert!(got[0].status.iter().all(|s| !s.ok));
}

#[test]
fn wrong_lengths_and_strict_settings() {
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(bytes_format(), bytes_data(&[vec![0u8; 100]]), 1),
        )
        .add("rs", RsDecode::default())
        .connect("src", "rs")
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["rs.out", "rs.status"]);
    assert!(packets(&got[0].data).is_empty());
    assert_eq!(got[1].status.len(), 1);
    assert!(!got[1].status[0].ok);

    let mut rs = RsDecode::default();
    assert!(set(&mut rs, "code", &Value::Text("dvb_204_187".into())).is_err());
    assert!(set(&mut rs, "depth", &Value::F64(0.0)).is_err());
    assert!(set(&mut rs, "depth", &Value::F64(33.0)).is_err());
    assert!(set(&mut rs, "basis", &Value::Text("normal".into())).is_err());
    assert!(set(&mut rs, "length", &Value::F64(300.0)).is_err());
    assert!(set(&mut rs, "keep_parity", &Value::Text("perhaps".into())).is_err());
    // The dual basis exists only for the CCSDS field; a length must leave room for the parity.
    for (k, v) in [
        ("rs.basis", Value::Text("dual".into())),
        ("rs.length", Value::F64(16.0)),
    ] {
        let mut g = Tb::new()
            .add("src", Source::new(bytes_format(), bytes_data(&[]), 1))
            .add("rs", RsDecode::default())
            .connect("src", "rs")
            .set(k, v)
            .build()
            .unwrap();
        assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err(), "{k}");
    }
}
