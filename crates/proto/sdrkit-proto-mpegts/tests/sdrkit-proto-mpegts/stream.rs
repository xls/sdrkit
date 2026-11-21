//! The transport stream item.

use sdrkit_graph::{Kind, Packet};
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use std::sync::Arc;

#[test]
fn the_stream_is_a_stream_of_transport_packet_items() {
    assert_eq!(stream::kind(), Kind::typed::<TsPacket>());
    assert_eq!(
        stream::port("in").format.kind.to_string(),
        stream::kind().to_string()
    );
}

#[test]
fn items_hold_one_packet_and_the_error_indicator() {
    let mut out = TsOut::default();
    let mut items = Vec::new();
    let p = sdrkit_proto_mpegts::source::Numbered::TERRESTRIAL.packet(3);
    assert!(out.push(&mut items, &p, 1.25, false));
    assert!(out.push(&mut items, &p, 1.5, true));
    assert_eq!(items[0].value.bytes(), &p);
    assert!(!items[0].value.has_error());
    assert!(items[1].value.has_error());
    assert_eq!((items[0].time, items[1].time), (1.25, 1.5));
    // Not a transport packet: wrong length or no sync byte.
    let short = Packet::new(Arc::from(&p[..100]), 0.0);
    assert!(TsPacket::new(short).is_none());
    let mut nosync = p;
    nosync[0] = 0;
    assert!(TsPacket::new(Packet::new(Arc::from(&nosync[..]), 0.0)).is_none());
    assert!(!out.push(&mut items, &nosync, 2.0, false));
    assert_eq!(items.len(), 2);
}
