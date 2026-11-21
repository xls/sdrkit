//! Flow statistics: packets and bytes per flow, UDP checksum failures per flow, IP versions, the busiest UDP ports, non-IP
//! protocol types and the byte entropy of non-IP PDUs. Only addresses, ports, protocols and counts are kept.

use crate::ip::{Datagram, Ipv6, TCP, UDP};
use std::collections::HashMap;
use std::net::IpAddr;

/// What names a flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FlowKey {
    /// The source address.
    pub src: IpAddr,
    /// The destination address.
    pub dst: IpAddr,
    /// The IP protocol.
    pub protocol: u8,
    /// The source port (0 when the protocol has none).
    pub src_port: u16,
    /// The destination port (0 when the protocol has none).
    pub dst_port: u16,
}

/// The counters of one flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    /// The flow.
    pub key: FlowKey,
    /// Packets.
    pub packets: u64,
    /// Bytes of IP payload.
    pub bytes: u64,
    /// UDP checksums that failed.
    pub bad_checksums: u64,
    /// Signal time of the first packet, seconds.
    pub first_s: f64,
    /// Signal time of the last packet, seconds.
    pub last_s: f64,
}

impl Flow {
    /// True when at least half of its UDP checksums failed while its IP headers were good (encrypted in transit).
    pub fn encrypted(&self) -> bool {
        self.bad_checksums > 0 && self.bad_checksums * 2 >= self.packets
    }
}

/// The flow table and the stream counters. It holds at most `cap` flows. When it is full, the flow heard least recently goes.
/// It allocates only in [`FlowTable::new`].
#[derive(Clone, Debug)]
pub struct FlowTable {
    flows: Vec<Flow>,
    index: HashMap<FlowKey, usize>,
    cap: usize,
    /// IPv4 datagrams counted.
    pub ipv4: u64,
    /// IPv6 packets counted.
    pub ipv6: u64,
    /// Flows pushed out when the table was full.
    pub evicted: u64,
    ports: Vec<(u16, u64)>,
    non_ip: Vec<(u16, u64)>,
    hist: [u64; 256],
    hist_n: u64,
}

const PORTS: usize = 64;
const NON_IP: usize = 32;

impl FlowTable {
    /// A table of at most `cap` flows (at least 1).
    pub fn new(cap: usize) -> FlowTable {
        let cap = cap.max(1);
        FlowTable {
            flows: Vec::with_capacity(cap),
            // Twice the flows, so that evictions never make the map grow.
            index: HashMap::with_capacity(cap * 2),
            cap,
            ipv4: 0,
            ipv6: 0,
            evicted: 0,
            ports: Vec::with_capacity(PORTS),
            non_ip: Vec::with_capacity(NON_IP),
            hist: [0; 256],
            hist_n: 0,
        }
    }

    /// Counts one whole IPv4 datagram (from [`crate::ip::Reassembler`]) at signal time `now`.
    pub fn datagram(&mut self, d: &Datagram<'_>, now: f64) {
        self.ipv4 += 1;
        let (sp, dp, bad) = ports(d.protocol, d.src.into(), d.dst.into(), d.payload);
        let key = FlowKey {
            src: d.src.into(),
            dst: d.dst.into(),
            protocol: d.protocol,
            src_port: sp,
            dst_port: dp,
        };
        self.count(key, d.payload.len(), bad, now);
    }

    /// Counts one IPv6 packet at signal time `now`. A packet that is not valid IPv6 is not counted.
    pub fn ipv6(&mut self, packet: &[u8], now: f64) {
        let Ok(p) = Ipv6::parse(packet) else {
            return;
        };
        self.ipv6 += 1;
        let (sp, dp, bad) = if p.fragment {
            (0, 0, false)
        } else {
            ports(p.protocol, p.src.into(), p.dst.into(), p.payload)
        };
        let key = FlowKey {
            src: p.src.into(),
            dst: p.dst.into(),
            protocol: p.protocol,
            src_port: sp,
            dst_port: dp,
        };
        self.count(key, p.payload.len(), bad, now);
    }

    /// Counts a PDU that is not IP, by its protocol type, and adds its bytes to the entropy histogram.
    pub fn non_ip(&mut self, protocol: u16, pdu: &[u8]) {
        bump(&mut self.non_ip, NON_IP, protocol);
        for &b in pdu {
            self.hist[b as usize] += 1;
        }
        self.hist_n += pdu.len() as u64;
    }

    fn count(&mut self, key: FlowKey, bytes: usize, bad: bool, now: f64) {
        if key.protocol == UDP {
            bump(&mut self.ports, PORTS, key.dst_port);
        }
        let k = match self.index.get(&key) {
            Some(&k) => k,
            None => {
                let fresh = Flow {
                    key,
                    packets: 0,
                    bytes: 0,
                    bad_checksums: 0,
                    first_s: now,
                    last_s: now,
                };
                if self.flows.len() < self.cap {
                    self.flows.push(fresh);
                    self.index.insert(key, self.flows.len() - 1);
                    self.flows.len() - 1
                } else {
                    let mut k = 0;
                    for (i, f) in self.flows.iter().enumerate() {
                        if f.last_s < self.flows[k].last_s {
                            k = i;
                        }
                    }
                    self.index.remove(&self.flows[k].key);
                    self.flows[k] = fresh;
                    self.index.insert(key, k);
                    self.evicted += 1;
                    k
                }
            }
        };
        let f = &mut self.flows[k];
        f.packets += 1;
        f.bytes += bytes as u64;
        f.bad_checksums += bad as u64;
        f.last_s = now;
    }

    /// The flows, in no set order.
    pub fn flows(&self) -> &[Flow] {
        &self.flows
    }

    /// The number of flows marked encrypted.
    pub fn encrypted_flows(&self) -> usize {
        self.flows.iter().filter(|f| f.encrypted()).count()
    }

    /// UDP destination ports and their packet counts (the first 64 ports seen).
    pub fn udp_ports(&self) -> &[(u16, u64)] {
        &self.ports
    }

    /// Non-IP protocol types and their PDU counts (the first 32 types seen).
    pub fn non_ip_types(&self) -> &[(u16, u64)] {
        &self.non_ip
    }

    /// The entropy of the bytes of non-IP PDUs in bits per byte, once `min_bytes` bytes are seen. Near 8 means encrypted or
    /// compressed data.
    pub fn non_ip_entropy(&self, min_bytes: u64) -> Option<f64> {
        if self.hist_n < min_bytes.max(1) {
            return None;
        }
        let n = self.hist_n as f64;
        Some(
            -self
                .hist
                .iter()
                .filter(|&&c| c > 0)
                .map(|&c| c as f64 / n * (c as f64 / n).log2())
                .sum::<f64>(),
        )
    }

    /// Empties the table and the counters.
    pub fn clear(&mut self) {
        self.flows.clear();
        self.index.clear();
        self.ports.clear();
        self.non_ip.clear();
        self.hist = [0; 256];
        self.hist_n = 0;
        self.ipv4 = 0;
        self.ipv6 = 0;
        self.evicted = 0;
    }
}

fn bump(v: &mut Vec<(u16, u64)>, cap: usize, key: u16) {
    if let Some(e) = v.iter_mut().find(|e| e.0 == key) {
        e.1 += 1;
    } else if v.len() < cap {
        v.push((key, 1));
    }
}

/// The ports of a UDP or TCP payload and whether a UDP checksum failed.
fn ports(protocol: u8, src: IpAddr, dst: IpAddr, payload: &[u8]) -> (u16, u16, bool) {
    if protocol != UDP && protocol != TCP || payload.len() < 4 {
        return (0, 0, false);
    }
    let sp = u16::from_be_bytes([payload[0], payload[1]]);
    let dp = u16::from_be_bytes([payload[2], payload[3]]);
    let bad = protocol == UDP
        && matches!(crate::ip::Udp::parse(src, dst, payload), Ok(u) if u.checksum_ok == Some(false));
    (sp, dp, bad)
}
