//! Extended advertising followed through its pointers (Bluetooth Core Specification v5.4 Vol 6 Part B clauses 2.3.1.5 to
//! 2.3.1.8, 2.3.2.3 and 2.3.4.5): an AuxPtr places its auxiliary packet on a channel, a PHY and a time window. A packet on the
//! secondary channel that lands in such a window is the AUX_ADV_IND (or AUX_CHAIN_IND) it points to; its AdvData joins the
//! chain, and the whole AdvData is reported when the chain ends. An AuxPtr to a channel outside the capture is reported as such:
//! the stack does not ask the host to retune for it, because the auxiliary packet starts 0.3 ms to a few ms after its pointer,
//! less than a retune takes.
//! AUX_SYNC_IND packets come on the access address of their train, which a SyncInfo named.

use super::Advert;
use sdrkit_proto_wlan_phy::ble::{Addr, AdvPdu, Body, LinkKind, Phy, PduType, SyncInfo};

/// The slack around an auxiliary packet's window, s: clock drift and the receiver's timing.
const SLACK_S: f64 = 60e-6;
/// T_IFS, s (clause 4.1.1).
const T_IFS_S: f64 = 150e-6;
/// The most pointers and trains kept.
const CAP: usize = 64;
/// The longest AdvData of a chain kept, bytes (Vol 6 Part B clause 4.4.2.10 allows 1650 of host data).
const MAX_DATA: usize = 1650;

/// The role of a packet in extended advertising.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A packet of the primary channels, or legacy advertising.
    Primary,
    /// AUX_ADV_IND: pointed to by an ADV_EXT_IND.
    AuxAdv,
    /// AUX_CHAIN_IND: pointed to by an auxiliary packet.
    AuxChain,
    /// AUX_SCAN_RSP: the answer to an AUX_SCAN_REQ on its channel.
    AuxScanRsp,
    /// AUX_SYNC_IND: a packet of a periodic advertising train.
    AuxSync,
    /// An auxiliary packet whose pointer was not heard (its ADV_EXT_IND was outside the capture or lost).
    Unlinked,
}

impl Role {
    /// The PDU name of a packet of this role and type, as the specification calls it.
    pub fn pdu_name(self, t: PduType) -> &'static str {
        match (self, t) {
            (Role::AuxAdv, PduType::AdvExtInd) => "AUX_ADV_IND",
            (Role::AuxChain, PduType::AdvExtInd) => "AUX_CHAIN_IND",
            (Role::AuxScanRsp, PduType::AdvExtInd) => "AUX_SCAN_RSP",
            (Role::AuxSync, PduType::AdvExtInd) => "AUX_SYNC_IND",
            (Role::Unlinked, PduType::AdvExtInd) => "AUX (pointer not heard)",
            (Role::Primary, _) => t.name(),
            (_, PduType::ScanReq) => "AUX_SCAN_REQ",
            (_, PduType::ConnectInd) => "AUX_CONNECT_REQ",
            (_, t) => t.name(),
        }
    }
}

/// What the follower makes of one packet.
#[derive(Clone, Debug, PartialEq)]
pub struct Followed {
    /// The role.
    pub role: Role,
    /// The advertiser: AdvA of the packet or of the packet that pointed to it.
    pub owner: Option<Addr>,
    /// The AdvData of a whole chain, when this packet ends it.
    pub complete: Option<Vec<u8>>,
    /// The chain lost a part (a pointer whose packet did not come).
    pub incomplete: bool,
    /// The channel of an AuxPtr of this packet that lies outside the capture.
    pub outside: Option<u8>,
    /// An AuxPtr of this packet was not followed because the channels of the capture are not known.
    pub unfollowed: bool,
    /// The PHY of the auxiliary packets this packet points to.
    pub aux_phy: Option<Phy>,
    /// The SyncInfo of this packet.
    pub sync: Option<SyncInfo>,
    /// The advertising set id (SID).
    pub set_id: Option<u8>,
}

#[derive(Clone, Debug)]
struct Pointer {
    channel: u8,
    phy: Option<Phy>,
    from_s: f64,
    to_s: f64,
    role: Role,
    owner: Option<Addr>,
    sid: Option<u8>,
    data: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Train {
    access_address: u32,
    owner: Option<Addr>,
    sid: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ScanReq {
    channel: u8,
    end_s: f64,
    adv_a: Addr,
}

/// The follower (module documentation).
#[derive(Default)]
pub struct Follower {
    pointers: Vec<Pointer>,
    trains: Vec<Train>,
    scan: Option<ScanReq>,
    /// Pointers whose window passed without their packet.
    pub missed: u64,
}

/// The time from the start of a packet to the end of its access address on `phy`, s.
pub fn start_to_aa_end_s(phy: Phy) -> f64 {
    match phy {
        Phy::Le1M => 40e-6,
        Phy::Le2M => 24e-6,
        Phy::LeCoded => 336e-6,
    }
}

impl Follower {
    /// Forgets every pointer and train.
    pub fn clear(&mut self) {
        self.pointers.clear();
        self.trains.clear();
        self.scan = None;
        self.missed = 0;
    }

    /// The owner of a periodic train, by its access address.
    pub fn train_owner(&self, access_address: u32) -> Option<Addr> {
        self.trains
            .iter()
            .find(|t| t.access_address == access_address)
            .and_then(|t| t.owner)
    }

    /// Takes a packet. `capture` holds bit `c` for each channel `c` inside the capture (0: not known, every channel counts as
    /// outside).
    pub fn take(&mut self, a: &Advert, capture: u64) -> Option<Followed> {
        let p = AdvPdu::parse(&a.pdu).ok()?;
        let start = a.time_s - start_to_aa_end_s(a.phy);
        self.expire(start);
        let t = p.header.pdu_type;
        let secondary = a.channel < 37;
        let mut f = Followed {
            role: Role::Primary,
            owner: p.adv_a(),
            complete: None,
            incomplete: false,
            outside: None,
            unfollowed: false,
            aux_phy: None,
            sync: None,
            set_id: None,
        };
        if t == PduType::ScanReq && secondary {
            if let Body::ScanReq { adv_a, .. } = p.body {
                let end = a.time_s + (p.header.length as f64 + 5.0) * 8.0 / a.phy.symbol_rate();
                self.scan = Some(ScanReq {
                    channel: a.channel,
                    end_s: end,
                    adv_a,
                });
            }
            return Some(f);
        }
        let Body::Extended { ext, data, .. } = p.body else {
            return Some(f);
        };
        f.set_id = ext.adi.map(|(_, sid)| sid);
        f.sync = ext.sync_info;
        let mut chain: Vec<u8> = Vec::new();
        let mut from_pointer = false;
        if a.kind == LinkKind::Periodic {
            f.role = Role::AuxSync;
            if let Some(tr) = self
                .trains
                .iter()
                .find(|tr| tr.access_address == a.access_address)
            {
                f.owner = f.owner.or(tr.owner);
                f.set_id = f.set_id.or(tr.sid);
            }
        }
        if let Some(i) = self.pointers.iter().position(|q| {
            q.channel == a.channel
                && q.phy.is_none_or(|ph| ph == a.phy)
                && start >= q.from_s
                && start <= q.to_s
        }) {
            let q = self.pointers.swap_remove(i);
            if f.role != Role::AuxSync || q.role == Role::AuxChain {
                f.role = q.role;
            }
            f.owner = f.owner.or(q.owner);
            f.set_id = f.set_id.or(q.sid);
            chain = q.data;
            from_pointer = true;
        } else if secondary && a.kind == LinkKind::Advertising && t == PduType::AdvExtInd {
            f.role = match self.scan {
                Some(s)
                    if s.channel == a.channel
                        && (start - s.end_s - T_IFS_S).abs() < SLACK_S
                        && f.owner.is_none_or(|o| o == s.adv_a) =>
                {
                    f.owner = f.owner.or(Some(s.adv_a));
                    Role::AuxScanRsp
                }
                _ => Role::Unlinked,
            };
        }
        // Each packet of an auxiliary chain carries a part of the AdvData; the primary packet carries none when it points on.
        if f.role != Role::Primary || ext.aux_ptr.is_none() {
            let room = MAX_DATA.saturating_sub(chain.len());
            chain.extend_from_slice(&data[..data.len().min(room)]);
        }
        if let (Some(s), Some(owner)) = (ext.sync_info, f.owner) {
            self.add_train(s.access_address, owner, f.set_id);
        }
        match ext.aux_ptr {
            Some(ap) if ap.offset > 0 => {
                f.aux_phy = ap.phy();
                let next = match f.role {
                    Role::Primary => Role::AuxAdv,
                    _ => Role::AuxChain,
                };
                if capture == 0 {
                    f.unfollowed = true;
                } else if capture & (1u64 << ap.channel.min(63)) != 0 {
                    if self.pointers.len() == CAP {
                        self.pointers.remove(0);
                        self.missed += 1;
                    }
                    let at = start + ap.offset_s();
                    self.pointers.push(Pointer {
                        channel: ap.channel,
                        phy: ap.phy(),
                        from_s: at - SLACK_S,
                        to_s: at + ap.window_s() + SLACK_S,
                        role: next,
                        owner: f.owner,
                        sid: f.set_id,
                        data: chain,
                    });
                } else {
                    f.outside = Some(ap.channel);
                    if f.role != Role::Primary {
                        f.complete = Some(chain);
                        f.incomplete = true;
                    }
                }
            }
            // An AuxPtr with offset 0: more data was meant but will not come (clause 2.3.4.5).
            Some(_) => {
                f.complete = Some(chain);
                f.incomplete = true;
            }
            None if f.role != Role::Primary || from_pointer => f.complete = Some(chain),
            None => {}
        }
        Some(f)
    }

    fn add_train(&mut self, access_address: u32, owner: Addr, sid: Option<u8>) {
        if let Some(t) = self
            .trains
            .iter_mut()
            .find(|t| t.access_address == access_address)
        {
            t.owner = Some(owner);
            t.sid = sid;
            return;
        }
        if self.trains.len() == CAP {
            self.trains.remove(0);
        }
        self.trains.push(Train {
            access_address,
            owner: Some(owner),
            sid,
        });
    }

    fn expire(&mut self, now_s: f64) {
        let before = self.pointers.len();
        self.pointers.retain(|q| q.to_s >= now_s - 0.01);
        self.missed += (before - self.pointers.len()) as u64;
    }
}
