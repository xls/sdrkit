use sdrkit_proto_link::csp::{
    append_crc32, check_crc32, Ax100, CrcScope, Header, FLAG_CRC32, FLAG_RS, FLAG_SCRAMBLER,
};

#[test]
fn the_header_layout_of_csp_1() {
    // priority 2, source 1, destination 10, destination port 31, source port 7, CRC32 flag.
    let h = Header {
        priority: 2,
        source: 1,
        destination: 10,
        dest_port: 31,
        source_port: 7,
        flags: FLAG_CRC32,
    };
    assert_eq!(h.to_bytes(), 0x82A7_C701u32.to_be_bytes());
    assert_eq!(Header::parse(&h.to_bytes()).unwrap(), h);
    assert!(Header::parse(&[1, 2, 3]).is_err());
}

#[test]
fn the_crc32_is_crc32c_big_endian() {
    // CRC-32C check value of "123456789" (RFC 3720 appendix B.4) is 0xE3069283.
    let mut p = b"123456789".to_vec();
    append_crc32(&mut p, CrcScope::HeaderAndData).unwrap();
    assert_eq!(&p[9..], &[0xE3, 0x06, 0x92, 0x83]);
    assert!(check_crc32(&p, CrcScope::HeaderAndData));
    // Data scope: the header is not covered.
    let mut q = vec![0xAA, 0xBB, 0xCC, 0xDD];
    q.extend_from_slice(b"123456789");
    append_crc32(&mut q, CrcScope::Data).unwrap();
    assert_eq!(&q[13..], &[0xE3, 0x06, 0x92, 0x83]);
    q[0] ^= 1;
    assert!(check_crc32(&q, CrcScope::Data));
    q[5] ^= 1;
    assert!(!check_crc32(&q, CrcScope::Data));
    assert!(!check_crc32(&[1, 2], CrcScope::Data));
}

#[test]
fn the_ax100_golay_code_is_the_extended_golay_code() {
    let ax = Ax100::new().unwrap();
    // Every code word of the extended Golay (24,12) code has weight 0, 8, 12, 16 or 24, with 759 words of weight 8.
    let mut w8 = 0;
    for v in 0..4096u16 {
        let w = ax.golay_encode(v).count_ones();
        assert!(matches!(w, 0 | 8 | 12 | 16 | 24), "{v} {w}");
        w8 += (w == 8) as u32;
    }
    assert_eq!(w8, 759);
    // Up to 3 errors corrected, 4 detected.
    let c = ax.golay_encode(0xABC);
    for a in 0..24 {
        for b in a..24 {
            for d in b..24 {
                let e = (1 << a) | (1 << b) | (1 << d);
                assert_eq!(ax.golay_decode(c ^ e).unwrap().0, 0xABC);
            }
        }
    }
    assert!(ax.golay_decode(c ^ 0xF0).is_err());
}

#[test]
fn ax100_mode5_and_mode6_round_trip_through_errors() {
    let mut ax = Ax100::new().unwrap();
    let mut packet = Header::default().to_bytes().to_vec();
    packet.extend((0..100u8).map(|i| i.wrapping_mul(13)));
    let mut f = Vec::new();
    ax.mode5_encode(&packet, &mut f).unwrap();
    let (len, flags, n) = ax.mode5_length([f[0], f[1], f[2]]).unwrap();
    assert_eq!(
        (len, flags, n),
        (packet.len() + 32, FLAG_RS | FLAG_SCRAMBLER, 0)
    );
    // The coded part is randomised: CCSDS sequence FF 48 0E C0 ... over the RS word.
    let mut coded = f[3..].to_vec();
    for k in 0..16 {
        coded[k * 7] ^= 0x55;
    }
    let (n, fixed) = ax.mode5_decode(&mut coded, flags).unwrap();
    assert_eq!((&coded[..n], fixed), (&packet[..], 16));
    let mut coded = f[3..].to_vec();
    for k in 0..17 {
        coded[k * 7] ^= 0x55;
    }
    assert!(ax.mode5_decode(&mut coded, flags).is_err());
    // The length field survives three bit errors.
    let field = u32::from_be_bytes([0, f[0], f[1], f[2]]) ^ 0b1000_0000_0001_0000_0000_0010;
    let b = field.to_be_bytes();
    assert_eq!(ax.mode5_length([b[1], b[2], b[3]]).unwrap().2, 3);
    // Mode 6.
    let mut p6 = Vec::new();
    ax.mode6_encode(&packet, &mut p6).unwrap();
    p6[3] ^= 0xFF;
    let (n, fixed) = ax.mode6_decode(&mut p6).unwrap();
    assert_eq!((&p6[..n], fixed), (&packet[..], 1));
    assert!(ax.mode5_encode(&[0u8; 224], &mut Vec::new()).is_err());
    assert!(ax.mode6_decode(&mut [0u8; 20]).is_err());
}

#[test]
fn rs_of_zeros_is_zeros_and_decoding_does_not_allocate() {
    let mut ax = Ax100::new().unwrap();
    let mut z = Vec::new();
    ax.mode6_encode(&[0u8; 223], &mut z).unwrap();
    assert!(z.iter().all(|&b| b == 0));
    let mut f = Vec::new();
    ax.mode5_encode(&[1u8; 60], &mut f).unwrap();
    let coded = f[3..].to_vec();
    let allocs = crate::allocations(|| {
        for _ in 0..1000 {
            let mut c = [0u8; 92];
            c.copy_from_slice(&coded);
            c[5] ^= 1;
            let _ = ax.mode5_decode(&mut c, FLAG_RS | FLAG_SCRAMBLER).unwrap();
        }
    });
    assert_eq!(allocs, 0);
}
