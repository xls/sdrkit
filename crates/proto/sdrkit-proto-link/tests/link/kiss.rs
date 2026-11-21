use sdrkit_proto_link::kiss::{self, Command, Decoder, FEND, FESC, TFEND, TFESC};

fn enc(port: u8, c: Command, d: &[u8]) -> Vec<u8> {
    let mut o = Vec::new();
    kiss::encode(port, c, d, &mut o).unwrap();
    o
}

#[test]
fn frames_and_escapes_of_the_kiss_paper() {
    // Chepponis and Karn 1987: FEND type data FEND. FEND in data is FESC TFEND, FESC is FESC TFESC.
    assert_eq!(
        enc(0, Command::Data, b"TEST"),
        [0xC0, 0x00, 0x54, 0x45, 0x53, 0x54, 0xC0]
    );
    assert_eq!(
        enc(0, Command::Data, &[0x01, FEND, 0x02, FESC, 0x03]),
        [FEND, 0x00, 0x01, FESC, TFEND, 0x02, FESC, TFESC, 0x03, FEND]
    );
    // Commands and ports: TXDELAY 50 (500 ms) on port 3, persistence, slot time, tail, duplex, hardware, return.
    assert_eq!(enc(3, Command::TxDelay, &[50]), [0xC0, 0x31, 50, 0xC0]);
    assert_eq!(enc(0, Command::Persistence, &[63]), [0xC0, 0x02, 63, 0xC0]);
    assert_eq!(enc(0, Command::SlotTime, &[10]), [0xC0, 0x03, 10, 0xC0]);
    assert_eq!(enc(0, Command::TxTail, &[1]), [0xC0, 0x04, 1, 0xC0]);
    assert_eq!(enc(1, Command::FullDuplex, &[1]), [0xC0, 0x15, 1, 0xC0]);
    assert_eq!(enc(0, Command::SetHardware, &[9]), [0xC0, 0x06, 9, 0xC0]);
    assert_eq!(enc(5, Command::Return, &[]), [0xC0, 0xFF, 0xC0]);
    // Port 12 data: the type byte 0xC0 is FEND itself and is escaped.
    assert_eq!(
        enc(12, Command::Data, b"x"),
        [FEND, FESC, TFEND, b'x', FEND]
    );
    assert!(kiss::encode(16, Command::Data, b"", &mut Vec::new()).is_err());
}

#[test]
fn the_decoder_takes_a_stream_in_pieces() {
    let mut stream = vec![FEND, FEND]; // empty frames are skipped
    for (p, c, d) in [
        (0, Command::Data, vec![1, FEND, 2, FESC]),
        (12, Command::Data, b"twelve".to_vec()),
        (2, Command::TxDelay, vec![25]),
        (0, Command::Return, vec![]),
        (1, Command::Other(9), vec![7]),
    ] {
        kiss::encode(p, c, &d, &mut stream).unwrap();
    }
    let mut got = Vec::new();
    for chunk in [1, 3, 7, 1000] {
        let mut d = Decoder::new(64);
        got.clear();
        for piece in stream.chunks(chunk) {
            d.push_bytes(piece, |f| got.push((f.port, f.command, f.data.to_vec())));
        }
        assert_eq!(got.len(), 5);
    }
    assert_eq!(got[0], (0, Command::Data, vec![1, FEND, 2, FESC]));
    assert_eq!(got[1], (12, Command::Data, b"twelve".to_vec()));
    assert_eq!(got[2], (2, Command::TxDelay, vec![25]));
    assert_eq!(got[3], (0, Command::Return, vec![]));
    assert_eq!(got[4], (1, Command::Other(9), vec![7]));
    // A frame over the limit is dropped, the next one comes through.
    let mut d = Decoder::new(4);
    let mut s = Vec::new();
    kiss::encode(0, Command::Data, b"too long", &mut s).unwrap();
    kiss::encode(0, Command::Data, b"ok", &mut s).unwrap();
    let mut n = Vec::new();
    d.push_bytes(&s, |f| n.push(f.data.to_vec()));
    assert_eq!(n, [b"ok".to_vec()]);
    // No allocation after new.
    let mut d = Decoder::new(64);
    let allocs = crate::allocations(|| {
        for _ in 0..1000 {
            d.push_bytes(&stream, |_| {});
        }
    });
    assert_eq!(allocs, 0);
}
