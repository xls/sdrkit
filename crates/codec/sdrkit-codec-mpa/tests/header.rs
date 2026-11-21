//! Known answers of the frame header from ISO/IEC 11172-3 clause 2.4.2.3 and ISO/IEC 13818-3.

use sdrkit_codec_mpa::{Error, Header};

#[test]
fn header_known_answers() {
    // ISO/IEC 11172-3 2.4.2.3. MPEG-1 Layer II, 128 kbit/s, 48 kHz, stereo, no CRC: 384 bytes, 1152 samples.
    let h = Header::parse(&[0xFF, 0xFD, 0x84, 0x04]).unwrap();
    assert_eq!(
        (
            h.mpeg1,
            h.layer,
            h.crc,
            h.bitrate_kbps,
            h.sample_rate,
            h.mode
        ),
        (true, 2, false, 128, 48_000, 0)
    );
    assert_eq!((h.frame_len(), h.samples(), h.channels()), (384, 1152, 2));
    assert!(h.original_copy);
    assert_eq!(h.bytes().unwrap(), [0xFF, 0xFD, 0x84, 0x04]);
    // MPEG-1 Layer III 128 kbit/s 44.1 kHz joint stereo with padding: 418 bytes.
    let h = Header::parse(&[0xFF, 0xFB, 0x92, 0x64]).unwrap();
    assert_eq!((h.layer, h.mode, h.frame_len()), (3, 1, 418));
    // MPEG-2 Layer II at 24 kHz, 64 kbit/s, mono (DAB half rate): 384 bytes.
    let h = Header::parse(&[0xFF, 0xF5, 0x84, 0xC4]).unwrap();
    assert_eq!(
        (
            h.mpeg1,
            h.sample_rate,
            h.bitrate_kbps,
            h.frame_len(),
            h.channels()
        ),
        (false, 24_000, 64, 384, 1)
    );
    // MPEG-2 Layer III at 22.05 kHz, 64 kbit/s: 576 samples, 72 × 64000 / 22050 = 208 bytes.
    let h = Header::parse(&[0xFF, 0xF3, 0x80, 0xC4]).unwrap();
    assert_eq!((h.samples(), h.frame_len()), (576, 208));
    // MPEG-1 Layer I, 384 kbit/s, 48 kHz: (12 × 384000 / 48000) × 4 = 384 bytes.
    let h = Header::parse(&[0xFF, 0xFF, 0xC4, 0x00]).unwrap();
    assert_eq!(
        (h.layer, h.bitrate_kbps, h.frame_len(), h.samples()),
        (1, 384, 384, 384)
    );
}

#[test]
fn refused_headers() {
    for b in [
        [0xFF, 0xFD, 0x04, 0x04], // free format
        [0xFF, 0xFD, 0xF4, 0x04], // bitrate index 15
        [0xFF, 0xFD, 0x8C, 0x04], // sampling 3
        [0xFF, 0xF9, 0x84, 0x04], // layer 0
        [0xFF, 0xFD, 0x84, 0x06], // emphasis 2
        [0xFF, 0xE5, 0x84, 0x04], // MPEG-2.5
    ] {
        assert!(Header::parse(&b).is_err(), "{b:02X?}");
    }
    assert_eq!(Header::parse(&[0xFF]), Err(Error::Truncated));
    assert_eq!(Header::parse(&[0xFF, 0xE5, 0x84, 0x04]), Err(Error::Mpeg25));
    assert_eq!(
        Header::parse(&[0xFF, 0xFD, 0x04, 0x04]),
        Err(Error::BitRate)
    );
    assert_eq!(Header::parse(&[0x47, 0xFD, 0x84, 0x04]), Err(Error::Sync));
}
