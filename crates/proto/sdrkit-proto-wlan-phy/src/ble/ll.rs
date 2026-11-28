//! The PDUs of a connection (Bluetooth Core Specification v5.4 Vol 6 Part B clause 2.4): the data physical channel PDU
//! header, the LL control PDUs (clause 2.4.2) with the fields of the procedures that change the connection, and the
//! L2CAP basic frame (Vol 3 Part A clause 3.1) that carries the Attribute Protocol (Vol 3 Part F clause 3.4).
//!
//! The parsers read PDUs sent in the clear. A PDU of an encrypted connection carries ciphertext and a MIC: its header is
//! readable, its payload is not, and these parsers do not try to read it.

/// The LLID of a data physical channel PDU (clause 2.4, table 2.21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Llid {
    /// An LL data PDU that continues an L2CAP message, or an empty PDU.
    Continuation,
    /// An LL data PDU that starts an L2CAP message (or holds a whole one).
    Start,
    /// An LL control PDU.
    Control,
}

/// The header of a data physical channel PDU (clause 2.4, figure 2.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DataHeader {
    /// The LLID.
    pub llid: Llid,
    /// Next expected sequence number.
    pub nesn: bool,
    /// Sequence number.
    pub sn: bool,
    /// More data.
    pub md: bool,
    /// A CTEInfo byte follows the header.
    pub cp: bool,
    /// The payload length (with the MIC on an encrypted connection).
    pub length: u8,
}

impl DataHeader {
    /// The header of `pdu`, or `None` when it is shorter than two bytes or its LLID is the reserved 0.
    pub fn parse(pdu: &[u8]) -> Option<DataHeader> {
        let (&h, &length) = (pdu.first()?, pdu.get(1)?);
        let llid = match h & 3 {
            1 => Llid::Continuation,
            2 => Llid::Start,
            3 => Llid::Control,
            _ => return None,
        };
        Some(DataHeader {
            llid,
            nesn: h & 0x04 != 0,
            sn: h & 0x08 != 0,
            md: h & 0x10 != 0,
            cp: h & 0x20 != 0,
            length,
        })
    }

    /// The payload of `pdu` after the header (and the CTEInfo byte when `cp` is set), at most `length` bytes.
    pub fn payload<'a>(&self, pdu: &'a [u8]) -> &'a [u8] {
        let at = if self.cp { 3 } else { 2 };
        let end = (2 + usize::from(self.length)).min(pdu.len());
        pdu.get(at..end).unwrap_or(&[])
    }
}

/// The name of the LL control opcode `op` (clause 2.4.2, table 2.22), as the specification writes it.
pub fn control_name(op: u8) -> &'static str {
    const NAMES: [&str; 42] = [
        "LL_CONNECTION_UPDATE_IND",
        "LL_CHANNEL_MAP_IND",
        "LL_TERMINATE_IND",
        "LL_ENC_REQ",
        "LL_ENC_RSP",
        "LL_START_ENC_REQ",
        "LL_START_ENC_RSP",
        "LL_UNKNOWN_RSP",
        "LL_FEATURE_REQ",
        "LL_FEATURE_RSP",
        "LL_PAUSE_ENC_REQ",
        "LL_PAUSE_ENC_RSP",
        "LL_VERSION_IND",
        "LL_REJECT_IND",
        "LL_PERIPHERAL_FEATURE_REQ",
        "LL_CONNECTION_PARAM_REQ",
        "LL_CONNECTION_PARAM_RSP",
        "LL_REJECT_EXT_IND",
        "LL_PING_REQ",
        "LL_PING_RSP",
        "LL_LENGTH_REQ",
        "LL_LENGTH_RSP",
        "LL_PHY_REQ",
        "LL_PHY_RSP",
        "LL_PHY_UPDATE_IND",
        "LL_MIN_USED_CHANNELS_IND",
        "LL_CTE_REQ",
        "LL_CTE_RSP",
        "LL_PERIODIC_SYNC_IND",
        "LL_CLOCK_ACCURACY_REQ",
        "LL_CLOCK_ACCURACY_RSP",
        "LL_CIS_REQ",
        "LL_CIS_RSP",
        "LL_CIS_IND",
        "LL_CIS_TERMINATE_IND",
        "LL_POWER_CONTROL_REQ",
        "LL_POWER_CONTROL_RSP",
        "LL_POWER_CHANGE_IND",
        "LL_SUBRATE_REQ",
        "LL_SUBRATE_IND",
        "LL_CHANNEL_REPORTING_IND",
        "LL_CHANNEL_STATUS_IND",
    ];
    NAMES.get(usize::from(op)).copied().unwrap_or("LL control (unknown opcode)")
}

/// The payload length (opcode and CtrData) of the LL control PDU `op` sent in the clear (clause 2.4.2), for the opcodes
/// 0x00 to 0x19. A control PDU of an encrypted connection is 4 bytes longer (the MIC) and its opcode byte is ciphertext.
pub fn control_length(op: u8) -> Option<u8> {
    const LENGTHS: [u8; 26] = [
        12, 8, 2, 23, 13, 1, 1, 2, 9, 9, 1, 1, 6, 2, 9, 24, 24, 3, 1, 1, 9, 9, 3, 3, 5, 3,
    ];
    LENGTHS.get(usize::from(op)).copied()
}

/// The LL control opcodes that change the connection or start its encryption (clause 2.4.2).
pub mod opcode {
    /// LL_CONNECTION_UPDATE_IND.
    pub const CONNECTION_UPDATE_IND: u8 = 0x00;
    /// LL_CHANNEL_MAP_IND.
    pub const CHANNEL_MAP_IND: u8 = 0x01;
    /// LL_TERMINATE_IND.
    pub const TERMINATE_IND: u8 = 0x02;
    /// LL_ENC_REQ.
    pub const ENC_REQ: u8 = 0x03;
    /// LL_ENC_RSP.
    pub const ENC_RSP: u8 = 0x04;
    /// LL_START_ENC_REQ.
    pub const START_ENC_REQ: u8 = 0x05;
    /// LL_START_ENC_RSP.
    pub const START_ENC_RSP: u8 = 0x06;
    /// LL_PHY_UPDATE_IND.
    pub const PHY_UPDATE_IND: u8 = 0x18;
}

/// What an LL control PDU changes about the connection at its instant (clauses 2.4.2.1, 2.4.2.2 and 2.4.2.22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    /// LL_CONNECTION_UPDATE_IND: new timing from the event `instant` on.
    Update {
        /// WinSize, units of 1.25 ms.
        win_size: u8,
        /// WinOffset, units of 1.25 ms.
        win_offset: u16,
        /// Interval, units of 1.25 ms.
        interval: u16,
        /// Latency, events.
        latency: u16,
        /// Timeout, units of 10 ms.
        timeout: u16,
        /// The connection event counter at which it applies.
        instant: u16,
    },
    /// LL_CHANNEL_MAP_IND: a new channel map from the event `instant` on.
    ChannelMap {
        /// ChM.
        map: [u8; 5],
        /// The connection event counter at which it applies.
        instant: u16,
    },
    /// LL_PHY_UPDATE_IND: new PHYs from the event `instant` on (bit 0 LE 1M, bit 1 LE 2M, bit 2 LE Coded; 0 keeps the PHY).
    Phy {
        /// PHY_C_TO_P.
        central_to_peripheral: u8,
        /// PHY_P_TO_C.
        peripheral_to_central: u8,
        /// The connection event counter at which it applies.
        instant: u16,
    },
    /// LL_TERMINATE_IND with its error code.
    Terminate(u8),
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

/// The change that the LL control payload `p` (opcode first) makes, if it is one of [`Change`] and long enough.
pub fn change(p: &[u8]) -> Option<Change> {
    let (&op, f) = p.split_first()?;
    match op {
        opcode::CONNECTION_UPDATE_IND => Some(Change::Update {
            win_size: *f.first()?,
            win_offset: le16(f, 1)?,
            interval: le16(f, 3)?,
            latency: le16(f, 5)?,
            timeout: le16(f, 7)?,
            instant: le16(f, 9)?,
        }),
        opcode::CHANNEL_MAP_IND => Some(Change::ChannelMap {
            map: f.get(..5)?.try_into().ok()?,
            instant: le16(f, 5)?,
        }),
        opcode::PHY_UPDATE_IND => Some(Change::Phy {
            central_to_peripheral: *f.first()?,
            peripheral_to_central: *f.get(1)?,
            instant: le16(f, 2)?,
        }),
        opcode::TERMINATE_IND => Some(Change::Terminate(*f.first()?)),
        _ => None,
    }
}

/// The L2CAP channel of the Attribute Protocol on an LE connection (Vol 3 Part A clause 2.1).
pub const ATT_CID: u16 = 0x0004;

/// The L2CAP basic frame header at the start of an LL data PDU payload that starts a message: (length, channel id, the
/// information bytes this PDU holds).
pub fn l2cap(p: &[u8]) -> Option<(u16, u16, &[u8])> {
    Some((le16(p, 0)?, le16(p, 2)?, p.get(4..)?))
}

/// An Attribute Protocol PDU (Vol 3 Part F clause 3.4): its opcode, the handle it names and the value it carries, when it
/// has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Att<'a> {
    /// The opcode.
    pub opcode: u8,
    /// The attribute handle that the PDU names (the first handle of a range).
    pub handle: Option<u16>,
    /// The attribute value or the rest of the PDU after the handle.
    pub value: &'a [u8],
}

/// The name of the ATT opcode `op` (Vol 3 Part F clause 3.4.8, table 3.37), as the specification writes it.
pub fn att_name(op: u8) -> &'static str {
    match op {
        0x01 => "ATT_ERROR_RSP",
        0x02 => "ATT_EXCHANGE_MTU_REQ",
        0x03 => "ATT_EXCHANGE_MTU_RSP",
        0x04 => "ATT_FIND_INFORMATION_REQ",
        0x05 => "ATT_FIND_INFORMATION_RSP",
        0x06 => "ATT_FIND_BY_TYPE_VALUE_REQ",
        0x07 => "ATT_FIND_BY_TYPE_VALUE_RSP",
        0x08 => "ATT_READ_BY_TYPE_REQ",
        0x09 => "ATT_READ_BY_TYPE_RSP",
        0x0A => "ATT_READ_REQ",
        0x0B => "ATT_READ_RSP",
        0x0C => "ATT_READ_BLOB_REQ",
        0x0D => "ATT_READ_BLOB_RSP",
        0x0E => "ATT_READ_MULTIPLE_REQ",
        0x0F => "ATT_READ_MULTIPLE_RSP",
        0x10 => "ATT_READ_BY_GROUP_TYPE_REQ",
        0x11 => "ATT_READ_BY_GROUP_TYPE_RSP",
        0x12 => "ATT_WRITE_REQ",
        0x13 => "ATT_WRITE_RSP",
        0x16 => "ATT_PREPARE_WRITE_REQ",
        0x17 => "ATT_PREPARE_WRITE_RSP",
        0x18 => "ATT_EXECUTE_WRITE_REQ",
        0x19 => "ATT_EXECUTE_WRITE_RSP",
        0x1B => "ATT_HANDLE_VALUE_NTF",
        0x1D => "ATT_HANDLE_VALUE_IND",
        0x1E => "ATT_HANDLE_VALUE_CFM",
        0x20 => "ATT_READ_MULTIPLE_VARIABLE_REQ",
        0x21 => "ATT_READ_MULTIPLE_VARIABLE_RSP",
        0x23 => "ATT_MULTIPLE_HANDLE_VALUE_NTF",
        0x52 => "ATT_WRITE_CMD",
        0xD2 => "ATT_SIGNED_WRITE_CMD",
        _ => "ATT (unknown opcode)",
    }
}

/// The ATT PDU `p`: the handle of the PDUs that start with one (requests and notifications by handle, the error
/// response's handle), and the bytes after it.
pub fn att(p: &[u8]) -> Option<Att<'_>> {
    let (&opcode, rest) = p.split_first()?;
    let handle_first = matches!(
        opcode,
        0x04 | 0x06 | 0x08 | 0x0A | 0x0C | 0x10 | 0x12 | 0x16 | 0x1B | 0x1D | 0x52 | 0xD2
    );
    Some(if opcode == 0x01 {
        Att {
            opcode,
            handle: le16(rest, 1),
            value: rest.get(3..).unwrap_or(&[]),
        }
    } else if handle_first {
        Att {
            opcode,
            handle: le16(rest, 0),
            value: rest.get(2..).unwrap_or(&[]),
        }
    } else {
        Att {
            opcode,
            handle: None,
            value: rest,
        }
    })
}

/// The groups of an ATT_READ_BY_GROUP_TYPE_RSP body (after the opcode): (first handle, last handle, 16-bit UUID) for each
/// group with a 16-bit UUID. Groups with 128-bit UUIDs are left out.
pub fn groups(body: &[u8]) -> Vec<(u16, u16, u16)> {
    let Some((&len, list)) = body.split_first() else {
        return Vec::new();
    };
    if len != 6 {
        return Vec::new();
    }
    list.as_chunks::<6>()
        .0
        .iter()
        .filter_map(|g| Some((le16(g, 0)?, le16(g, 2)?, le16(g, 4)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_payload_control_and_att() {
        let pdu = [0x0F, 0x06, 0x0C, 0x0D, 0x02, 0x00, 0x34, 0x12];
        let h = DataHeader::parse(&pdu).unwrap();
        assert_eq!((h.llid, h.nesn, h.sn, h.md, h.length), (Llid::Control, true, true, false, 6));
        assert_eq!(control_name(h.payload(&pdu)[0]), "LL_VERSION_IND");
        assert_eq!(DataHeader::parse(&[0x00, 0x00]), None);
        let upd = [0x00, 1, 2, 0, 24, 0, 0, 0, 100, 0, 0x34, 0x12];
        assert_eq!(
            change(&upd),
            Some(Change::Update {
                win_size: 1,
                win_offset: 2,
                interval: 24,
                latency: 0,
                timeout: 100,
                instant: 0x1234
            })
        );
        let l2 = [7, 0, 4, 0, 0x0A, 0x08, 0x00];
        let (len, cid, info) = l2cap(&l2).unwrap();
        assert_eq!((len, cid), (7, ATT_CID));
        let a = att(info).unwrap();
        assert_eq!((att_name(a.opcode), a.handle), ("ATT_READ_REQ", Some(8)));
        let rsp = [6, 1, 0, 5, 0, 0x00, 0x18, 6, 0, 9, 0, 0x0F, 0x18];
        assert_eq!(groups(&rsp), vec![(1, 5, 0x1800), (6, 9, 0x180F)]);
    }
}
