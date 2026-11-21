//! SSDP and DNS-SD on a fake network.

use crate::discovery::*;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use sdrkit_core::net::{
    Conn, Datagram, DatagramOptions, FakeTransport, Interface, Ipv4Net, Listener, NetError,
    Transport,
};
use std::time::{Duration, Instant};

const ANSWER: &str = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nEXT:\r\nLOCATION: tcp://192.0.2.1:55133\r\nSERVER: Linux UPnP/1.1 \
                      SoapyRemote/0.5.2\r\nST: urn:schemas-pothosware-com:service:soapyRemote:1\r\nUSN: \
                      uuid:2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6::urn:schemas-pothosware-com:service:soapyRemote:1\r\n\r\n";

fn local() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

#[test]
fn the_search_request() {
    let m = m_search();
    assert!(m.starts_with("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: urn:schemas-pothosware-com:service:soapyRemote:1\r\nUSER-AGENT: "));
    assert!(m.contains(" UPnP/1.1 sdrkit/"));
    assert!(m.ends_with("\r\n\r\n"));
}

#[test]
fn an_answer_is_read_from_its_source_address() {
    let m = parse_ssdp(ANSWER).unwrap();
    assert_eq!(m.kind, SsdpKind::Response);
    assert_eq!(m.uuid, "2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6");
    assert_eq!(m.port, 55133);
    assert_eq!(m.server, "Linux UPnP/1.1 SoapyRemote/0.5.2");
    assert_eq!(m.max_age, Duration::from_secs(120));
    let mut c = Collector::default();
    let now = Instant::now();
    c.ssdp(&m, local(), now);
    c.ssdp(&m, local(), now);
    c.ssdp(&m, "::1".parse().unwrap(), now);
    let l = c.list();
    assert_eq!(l.len(), 1);
    assert_eq!(
        l[0].address(),
        "127.0.0.1:55133",
        "the datagram's source, not 192.0.2.1"
    );
    // Another ST: ignored. Lower-case names: accepted. No LOCATION: 55132.
    assert!(parse_ssdp(&ANSWER.replace("soapyRemote:1\r\nUSN", "other:1\r\nUSN")).is_none());
    let lower = ANSWER
        .replace("LOCATION: tcp://192.0.2.1:55133\r\n", "")
        .replace("CACHE-CONTROL:", "cache-control:")
        .replace("SERVER:", "server:")
        .replace("ST:", "st:")
        .replace("USN:", "usn:");
    let m2 = parse_ssdp(&lower).unwrap();
    assert_eq!(m2.port, 55132);
    // A byebye with the same USN removes it, and entries age out.
    let bye = "NOTIFY * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nNT: urn:schemas-pothosware-com:service:soapyRemote:1\r\nNTS: ssdp:byebye\r\nUSN: uuid:2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6::urn:schemas-pothosware-com:service:soapyRemote:1\r\n\r\n";
    let b = parse_ssdp(bye).unwrap();
    assert_eq!(b.kind, SsdpKind::Byebye);
    c.ssdp(&b, local(), now);
    assert!(c.list().is_empty());
    c.ssdp(&m, local(), now);
    c.expire(now + Duration::from_secs(121));
    assert!(c.list().is_empty());
    let alive = bye.replace("ssdp:byebye", "ssdp:alive");
    assert_eq!(parse_ssdp(&alive).unwrap().kind, SsdpKind::Alive);
    assert_eq!(parse_ssdp(&alive).unwrap().max_age, DEFAULT_MAX_AGE);
}

#[test]
fn a_search_against_a_responder_on_the_group() {
    let net = FakeTransport::new();
    let mut resp = net
        .bind(&DatagramOptions::at(SocketAddr::new(local(), 1900)))
        .unwrap();
    std::thread::spawn(move || {
        let mut b = [0u8; 2048];
        let _ = resp.set_read_timeout(Some(Duration::from_secs(3)));
        while let Ok((n, from)) = resp.recv_from(&mut b) {
            let req = String::from_utf8_lossy(&b[..n]).to_string();
            if req.starts_with("M-SEARCH") && req.contains(SEARCH_TARGET) {
                let _ = resp.send_to(ANSWER.as_bytes(), from);
            }
        }
    });
    let found = ssdp_search(&net, &[SSDP_V4], Duration::from_millis(600)).unwrap();
    assert_eq!(found.len(), 1, "answered twice, one entry");
    assert_eq!(found[0].address(), "127.0.0.1:55133");
    assert_eq!(found[0].server, "Linux UPnP/1.1 SoapyRemote/0.5.2");
}

fn name(out: &mut Vec<u8>, n: &str) {
    for l in n.split('.') {
        out.push(l.len() as u8);
        out.extend_from_slice(l.as_bytes());
    }
    out.push(0);
}

fn rr(out: &mut Vec<u8>, n: &str, ty: u16, data: &[u8]) {
    name(out, n);
    out.extend_from_slice(&ty.to_be_bytes());
    out.extend_from_slice(&[0x80, 0x01, 0, 0, 0x11, 0x94]);
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(data);
}

fn mdns_answer(with_uuid: bool) -> Vec<u8> {
    let mut m = vec![0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0];
    let mut ptr = Vec::new();
    name(&mut ptr, "lab._soapy._tcp.local");
    rr(&mut m, "_soapy._tcp.local", 12, &ptr);
    let mut srv = vec![0, 0, 0, 0, 0xd7, 0x9c];
    name(&mut srv, "labpc.local");
    rr(&mut m, "lab._soapy._tcp.local", 33, &srv);
    let mut txt = Vec::new();
    let kv = if with_uuid {
        "uuid=2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6"
    } else {
        "x=1"
    };
    txt.push(kv.len() as u8);
    txt.extend_from_slice(kv.as_bytes());
    rr(&mut m, "lab._soapy._tcp.local", 16, &txt);
    rr(&mut m, "labpc.local", 1, &[127, 0, 0, 1]);
    m
}

#[test]
fn dns_sd_answers() {
    let q = mdns_query();
    assert_eq!(&q[12..13], &[6]);
    assert!(q.windows(6).any(|w| w == b"_soapy"));
    let s = parse_mdns(&mdns_answer(true));
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].port, 55196);
    assert_eq!(s[0].host, "labpc.local");
    assert_eq!(s[0].ips, vec![local()]);
    let f = service_found(&s[0], "10.0.0.9".parse().unwrap(), Instant::now()).unwrap();
    assert_eq!(f.uuid, "2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6");
    assert_eq!(f.address(), "127.0.0.1:55196");
    // Without a uuid: ignored.
    let s = parse_mdns(&mdns_answer(false));
    assert!(service_found(&s[0], local(), Instant::now()).is_none());
    // A responder on the group.
    let net = FakeTransport::new();
    let mut resp = net
        .bind(&DatagramOptions::at(SocketAddr::new(
            local(),
            MDNS_V4.port(),
        )))
        .unwrap();
    std::thread::spawn(move || {
        let mut b = [0u8; 512];
        let _ = resp.set_read_timeout(Some(Duration::from_secs(3)));
        if let Ok((_, from)) = resp.recv_from(&mut b) {
            let _ = resp.send_to(&mdns_answer(true), from);
        }
    });
    let found = dnssd_search(&net, &[MDNS_V4], Duration::from_millis(400)).unwrap();
    assert_eq!(found.len(), 1);
    // Merged with the SSDP entry of the same server: one entry, the SSDP text kept.
    let m = parse_ssdp(ANSWER).unwrap();
    let mut c = Collector::default();
    c.ssdp(&m, local(), Instant::now());
    let merged = merge(&c.list(), &found);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].server, "Linux UPnP/1.1 SoapyRemote/0.5.2");
}

#[test]
fn a_compressed_name_is_followed() {
    let mut m = vec![0, 0, 0x84, 0, 0, 0, 0, 2, 0, 0, 0, 0];
    let start = m.len();
    let mut ptr = Vec::new();
    name(&mut ptr, "lab._soapy._tcp.local");
    rr(&mut m, "_soapy._tcp.local", 12, &ptr);
    // SRV owner "lab" + pointer to "_soapy._tcp.local" at `start`.
    m.extend_from_slice(&[3, b'l', b'a', b'b', 0xc0, start as u8]);
    m.extend_from_slice(&33u16.to_be_bytes());
    m.extend_from_slice(&[0, 1, 0, 0, 0, 10]);
    let mut srv = vec![0, 0, 0, 0, 0x00, 0x50];
    name(&mut srv, "h.local");
    m.extend_from_slice(&(srv.len() as u16).to_be_bytes());
    m.extend_from_slice(&srv);
    let s = parse_mdns(&m);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].port, 80);
}

/// A fake network that lists `0` as its interfaces.
struct WithInterfaces(Vec<Interface>, FakeTransport);

impl Transport for WithInterfaces {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        self.1.connect(host, port, tls, timeout)
    }
    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError> {
        self.1.datagram(host, port)
    }
    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError> {
        self.1.bind(options)
    }
    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError> {
        self.1.listen(host, port)
    }
    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError> {
        self.1.resolve(host, port)
    }
    fn interfaces(&self) -> Vec<Interface> {
        self.0.clone()
    }
}

#[test]
fn a_search_sends_from_one_socket_per_interface() {
    let two = vec![
        Interface {
            name: "a".into(),
            index: 1,
            v4: Some(Ipv4Net {
                addr: Ipv4Addr::LOCALHOST,
                netmask: Ipv4Addr::new(255, 0, 0, 0),
            }),
            v6: false,
        },
        Interface {
            name: "b".into(),
            index: 2,
            v4: None,
            v6: true,
        },
        Interface {
            name: "c".into(),
            index: 3,
            v4: Some(Ipv4Net {
                addr: Ipv4Addr::LOCALHOST,
                netmask: Ipv4Addr::new(255, 0, 0, 0),
            }),
            v6: true,
        },
    ];
    let net = WithInterfaces(two, FakeTransport::new());
    // An IPv4 group: the two interfaces with an IPv4 address.
    assert_eq!(sockets_for(&net, SSDP_V4).len(), 2);
    assert_eq!(sockets_for(&net, MDNS_V4).len(), 2);
    // No interface known: the default interface.
    assert_eq!(sockets_for(&FakeTransport::new(), SSDP_V4).len(), 1);
    // A plain address: one socket, whatever the interfaces.
    let lo = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1900);
    assert_eq!(sockets_for(&net, lo).len(), 1);
}
