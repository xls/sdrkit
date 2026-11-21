//! Test streams: the numbered packets against the bytes the recordings and vectors hold, and the test multiplex through the
//! demultiplexer.

use sdrkit_proto_mpegts::packet::TS_SIZE;
use sdrkit_proto_mpegts::source::{
    null_packet, Data, FileSource, Fill, Numbered, NumberedCheck, NumberedSource, PacketSource,
    TestMux, TestNetwork, TestService, TestSound,
};
use sdrkit_proto_mpegts::{Demultiplexer, DemuxOutput, Error, MuxFault};
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;

fn ends(p: &[u8; TS_SIZE]) -> ([u8; 12], [u8; 4]) {
    (p[..12].try_into().unwrap(), p[184..].try_into().unwrap())
}

#[test]
fn numbered_packets_have_the_bytes_of_the_recordings() {
    // The DVB-S/S2/C recordings: PID 0x0020, packet 2912.
    let p = Numbered::SATELLITE_CABLE.packet(2912);
    assert_eq!(
        ends(&p),
        (
            [0x47, 0x00, 0x20, 0x10, 0x00, 0x00, 0x0B, 0x60, 0x08, 0x15, 0x22, 0x2F],
            [0xF8, 0x05, 0x12, 0x1F]
        )
    );
    // The ISDB-T, ATSC and DTMB vectors: PID 0x0100, packet 7.
    let p = Numbered::TERRESTRIAL.packet(7);
    assert_eq!(
        ends(&p),
        (
            [0x47, 0x01, 0x00, 0x17, 0x00, 0x00, 0x00, 0x07, 0x99, 0xA6, 0xB3, 0xC0],
            [0x89, 0x96, 0xA3, 0xB0]
        )
    );
    // The DVB-T vectors: PID 0x0101, packet 5.
    let p = Numbered::new(0x0101, Fill::I31N7).packet(5);
    assert_eq!(
        ends(&p),
        (
            [0x47, 0x01, 0x01, 0x15, 0x00, 0x00, 0x00, 0x05, 0x1B, 0x3A, 0x59, 0x78],
            [0x6B, 0x8A, 0xA9, 0xC8]
        )
    );
}

#[test]
fn numbered_check_counts_good_wrong_flagged_and_gaps() {
    let pat = Numbered::TERRESTRIAL;
    let mut c = NumberedCheck::default();
    c.push(&pat, &pat.packet(0), 0.0);
    c.push(&pat, &pat.packet(1), 0.1);
    let mut bad = pat.packet(2);
    bad[100] ^= 1;
    c.push(&pat, &bad, 0.2);
    c.push(&pat, &pat.packet(3), 0.3); // the wrong packet explains the jump
    let mut flagged = pat.packet(4);
    flagged[1] |= 0x80;
    c.push(&pat, &flagged, 0.4);
    c.push(&pat, &pat.packet(9), 0.5); // the flagged packet explains the jump
    c.push(&pat, &pat.packet(20), 0.6); // a gap
    c.push(&pat, &null_packet(), 0.7);
    assert_eq!(
        (c.good, c.wrong, c.flagged, c.gaps, c.other),
        (5, 1, 1, 1, 1)
    );
    assert_eq!(
        (c.first, c.last, c.first_time),
        (Some(0), Some(20), Some(0.0))
    );
    assert_eq!(pat.number_of(&pat.packet(77)), Some(77));
    assert_eq!(pat.number_of(&Numbered::SATELLITE_CABLE.packet(77)), None);
    let mut s = NumberedSource::new(pat);
    let mut p = [0; TS_SIZE];
    s.next_packet(&mut p);
    s.next_packet(&mut p);
    assert_eq!(pat.number_of(&p), Some(1));
}

fn sections_of(mux: &mut TestMux, packets: usize) -> (Demultiplexer, Vec<[u8; TS_SIZE]>) {
    let mut d = Demultiplexer::new();
    d.auto_select = true;
    let mut out = DemuxOutput::default();
    let mut all = Vec::new();
    let mut p = [0u8; TS_SIZE];
    for i in 0..packets {
        mux.next_packet(&mut p);
        d.push(&p, Some(i as f64 * 1e-3), false, &mut out);
        out.clear();
        all.push(p);
    }
    (d, all)
}

#[test]
fn the_test_multiplex_returns_services_names_and_channel_numbers() {
    let mut mux = TestMux::builder(0x0401)
        .service(TestService::tv(0x0101, "sdrkit één", 7))
        .service(TestService::radio(0x0102, "sdrkit radio", 701))
        .network(TestNetwork {
            network_id: 0x3001,
            name: "sdrkit net".into(),
        })
        .build()
        .unwrap();
    let (d, packets) = sections_of(&mut mux, 600);
    let si = &d.si;
    let tv = si.service(0x0101).expect("the TV service");
    assert_eq!(tv.name, "sdrkit één"); // UTF-8 with selector 0x15
    assert_eq!(tv.lcn, 7);
    assert_eq!(si.service(0x0102).unwrap().lcn, 701);
    assert_eq!(si.tsid, 0x0401);
    // Every section CRC is right: a section with its CRC returns 0 (ISO/IEC 13818-1 Annex A).
    let pat = &packets[0];
    let len = (((pat[6] & 0x0F) as usize) << 8) | pat[7] as usize;
    assert_eq!(CRC32_MPEG2.checksum(&pat[5..5 + 3 + len]), 0);
    // The continuity counter of each table PID goes up by one per repetition.
    let ccs: Vec<u8> = packets
        .iter()
        .filter(|p| p[1] & 0x1F == 0 && p[2] == 0)
        .map(|p| p[3] & 15)
        .collect();
    assert_eq!(&ccs[..4], &[0, 1, 2, 3]);
    // The data stream is the numbered pattern.
    let num = mux.numbered().unwrap();
    let mut c = NumberedCheck::default();
    for p in &packets {
        c.push(&num, p, 0.0);
    }
    assert!(c.good > 500 && c.wrong == 0 && c.gaps == 0);
}

#[test]
fn the_stack_presets() {
    let mut dvbt = TestMux::dvbt(1, "sdrkit").unwrap();
    let mut p = [0u8; TS_SIZE];
    let (mut nulls, mut data) = (0, 0);
    for _ in 0..600 {
        dvbt.next_packet(&mut p);
        match ((p[1] as u16 & 0x1F) << 8) | p[2] as u16 {
            0x1FFF => nulls += 1,
            0x0101 => data += 1,
            _ => {}
        }
    }
    assert!(nulls > 400 && (70..90).contains(&data), "{nulls} {data}");
    let mut sat = TestMux::satellite_cable(100).unwrap();
    let (d, _) = sections_of(&mut sat, 300);
    assert_eq!(d.si.service(1).unwrap().name, "sdrkit test");
}

#[test]
fn the_sound_of_the_first_service_is_a_tone_at_the_pace_of_the_multiplex() {
    // A multiplex of 5 Mbit/s: one second is 3324 packets.
    let rate = 5e6;
    let n = (rate / (188.0 * 8.0)) as usize;
    let sound = TestSound {
        pid: 0x0102,
        mux_bit_rate: rate,
    };
    let mut mux = TestMux::dvbt_builder(1, "sdrkit").sound(sound).build().unwrap();
    let (d, packets) = sections_of(&mut mux, n);
    assert_eq!((d.selection.audio, d.selection.pcr), (Some(0x0102), Some(0x0102)));
    // The sound takes the places of null packets only: the numbered packets are where the multiplex without sound has them.
    let (_, plain) = sections_of(&mut TestMux::dvbt(1, "sdrkit").unwrap(), n);
    let data = |all: &[[u8; TS_SIZE]]| -> Vec<usize> {
        let pid = |p: &[u8; TS_SIZE]| (p[1] as u16 & 0x1F) << 8 | p[2] as u16;
        all.iter().enumerate().filter(|(_, p)| pid(p) != 0x1FFF && pid(p) != 0x0102).map(|(i, _)| i).collect()
    };
    assert_eq!(data(&packets), data(&plain));
    // The demultiplexer gives a frame each 24 ms with a PTS 2160 ticks after the one before, and each decodes to the tone.
    let mut demux = sdrkit_proto_mpegts::Demultiplexer::new();
    demux.auto_select = true;
    let mut out = DemuxOutput::default();
    let mut frames = Vec::new();
    for (i, p) in packets.iter().enumerate() {
        demux.push(p, Some(i as f64 * 188.0 * 8.0 / rate), false, &mut out);
        frames.extend(out.media.drain(..).filter(|m| m.pid == 0x0102));
        out.clear();
    }
    assert!((41..=42).contains(&frames.len()), "{}", frames.len());
    let pts: Vec<i64> = frames.iter().filter_map(|m| m.packet.pts).collect();
    assert_eq!(pts.len(), frames.len());
    assert!(pts.windows(2).all(|w| w[1] - w[0] == 2160), "{pts:?}");
    let mut dec = sdrkit_codec_mpa::Decoder::new();
    let mut pcm = Vec::new();
    for m in &frames {
        let h = dec.decode(m.packet.bytes(), &mut pcm).unwrap();
        assert_eq!((h.sample_rate, h.bitrate_kbps), (48_000, 128));
    }
    // The tone after the first frame: an amplitude of about a quarter of full scale.
    let peak = pcm[2 * 1152 * 2..].iter().fold(0f32, |m, s| m.max(s.abs()));
    assert!((0.2..0.3).contains(&peak), "{peak}");
    // A sound without null packets to fill, or on a PID of the multiplex, is refused.
    assert_eq!(
        TestMux::dvbt_builder(1, "a").sound(TestSound { pid: 0x0101, ..sound }).build().unwrap_err(),
        Error::TestMux(MuxFault::BadSoundPid(0x0101))
    );
    assert_eq!(
        TestMux::dvbt_builder(1, "a").sound(TestSound { mux_bit_rate: 1e5, ..sound }).build().unwrap_err(),
        Error::TestMux(MuxFault::BadSoundRate)
    );
}

#[test]
fn bad_configurations_are_refused() {
    assert_eq!(
        TestMux::builder(1).build().unwrap_err(),
        Error::TestMux(MuxFault::NoServices)
    );
    let tv = || TestService::tv(1, "a", 0);
    assert_eq!(
        TestMux::builder(1)
            .service(tv())
            .service(tv())
            .build()
            .unwrap_err(),
        Error::TestMux(MuxFault::DuplicateService(1))
    );
    assert_eq!(
        TestMux::builder(1)
            .service(tv())
            .data(0x0100, Data::None)
            .build()
            .unwrap_err(),
        Error::TestMux(MuxFault::BadDataPid(0x0100))
    );
    assert_eq!(
        TestMux::builder(1)
            .service(tv())
            .period(2)
            .build()
            .unwrap_err(),
        Error::TestMux(MuxFault::PeriodTooShort { need: 3 })
    );
    assert_eq!(
        TestMux::builder(1)
            .service(tv())
            .data_every(0)
            .build()
            .unwrap_err(),
        Error::TestMux(MuxFault::BadDataEvery)
    );
    // Many services: the tables take more than one packet each, and that is allowed.
    let mut b = TestMux::builder(1).period(200);
    for i in 0..40 {
        b = b.service(TestService::tv(
            i + 1,
            "a long service name for the test",
            i,
        ));
    }
    assert!(b.build().is_ok());
}

#[test]
fn a_file_source_aligns_and_loops() {
    let mut bytes = vec![0u8; 5]; // junk before the first packet
    for n in 0..4 {
        bytes.extend_from_slice(&Numbered::TERRESTRIAL.packet(n));
    }
    let mut f = FileSource::new(&bytes);
    assert_eq!(f.packets().len(), 4);
    let mut p = [0u8; TS_SIZE];
    for _ in 0..5 {
        f.next_packet(&mut p);
    }
    assert_eq!(Numbered::TERRESTRIAL.number_of(&p), Some(0));
    f.looping = false;
    for _ in 0..4 {
        f.next_packet(&mut p);
    }
    assert_eq!(p, null_packet());
}
