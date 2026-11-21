//! Protocol 1 bytes: discovery, start and stop, the 1032-byte data packets, the receive sample layout, the status bytes and the
//! control registers. Pure functions over byte slices.
//! Standard: HPSDR USB data protocol, carried over UDP as the Metis documents describe it.

/// Every data packet is this long.
pub const PACKET: usize = 1032;
/// The discovery request length.
pub const DISCOVERY_LEN: usize = 63;
/// The start and stop command length.
pub const COMMAND_LEN: usize = 64;
/// One 512-byte frame of a data packet.
pub const FRAME: usize = 512;
/// Sample bytes in one frame.
pub const FRAME_SAMPLES: usize = 504;
/// Transmit samples per frame (63 slots of 8 bytes).
pub const TX_SLOTS: usize = 63;
/// Transmit samples per packet.
pub const TX_PER_PACKET: usize = 2 * TX_SLOTS;
/// The transmit rate.
pub const TX_RATE: u32 = 48_000;

/// The discovery request: `EF FE 02` then 60 zero bytes.
pub const fn discovery_request() -> [u8; DISCOVERY_LEN] {
    let mut b = [0u8; DISCOVERY_LEN];
    b[0] = 0xEF;
    b[1] = 0xFE;
    b[2] = 0x02;
    b
}

/// A start (`cmd` 1) or stop (`cmd` 0) command: `EF FE 04 <cmd>` then 60 zero bytes.
pub fn command(cmd: u8) -> [u8; COMMAND_LEN] {
    let mut b = [0u8; COMMAND_LEN];
    b[..4].copy_from_slice(&[0xEF, 0xFE, 0x04, cmd]);
    b
}

/// A discovery reply, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// Status `03`: streaming to some program.
    pub busy: bool,
    /// The MAC address.
    pub mac: [u8; 6],
    /// Gateware version (major, on the Hermes-Lite 2).
    pub gateware: u8,
    /// Gateware minor version (byte 0x15), when the reply is longer than 0x15 bytes.
    pub minor: Option<u8>,
    /// The board id.
    pub board: u8,
    /// The receivers the gateware has (byte 0x13, 1 to 12), when said.
    pub receivers: Option<u8>,
}

impl Reply {
    /// Reads a reply: at least 11 bytes, `EF FE`, status `02` or `03`.
    pub fn parse(b: &[u8]) -> Option<Reply> {
        if b.len() < 11 || b[0] != 0xEF || b[1] != 0xFE || !matches!(b[2], 2 | 3) {
            return None;
        }
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&b[3..9]);
        let receivers = (b.len() > 0x13 && (1..=12).contains(&b[0x13])).then(|| b[0x13]);
        let minor = (b.len() > 0x15).then(|| b[0x15]);
        Some(Reply {
            busy: b[2] == 3,
            mac,
            gateware: b[9],
            minor,
            board: b[10],
            receivers,
        })
    }
}

/// A MAC address as `00:1C:C0:A2:12:34`.
pub fn mac_text(mac: &[u8; 6]) -> String {
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Slots (sample instants) per frame with `n` receivers in the stream: floor(504 / (6n + 2)).
pub fn slots(n: usize) -> usize {
    FRAME_SAMPLES / (6 * n.max(1) + 2)
}

/// Samples per packet for each receiver.
pub fn per_packet(n: usize) -> usize {
    2 * slots(n)
}

/// The packet is an EP6 (receive samples) packet: at least 1032 bytes starting `EF FE 01 06`.
pub fn is_ep6(b: &[u8]) -> bool {
    b.len() >= PACKET && b[..4] == [0xEF, 0xFE, 0x01, 0x06]
}

/// The packet's 32-bit sequence number.
pub fn sequence(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[4], b[5], b[6], b[7]])
}

/// Frame `f` (0 or 1) of a data packet.
pub fn frame(b: &[u8], f: usize) -> &[u8] {
    &b[8 + f * FRAME..8 + (f + 1) * FRAME]
}

/// The frame starts with the sync bytes `7F 7F 7F`.
pub fn synced(frame: &[u8]) -> bool {
    frame[..3] == [0x7F, 0x7F, 0x7F]
}

/// Both frames of a packet start `7F 7F 7F`.
pub fn both_synced(b: &[u8]) -> bool {
    synced(frame(b, 0)) && synced(frame(b, 1))
}

/// A frame fits the layout of `n` receivers: it is synced and every one of its slots' microphone words is zero.
pub fn fits(frame: &[u8], n: usize) -> bool {
    if !synced(frame) {
        return false;
    }
    let size = 6 * n + 2;
    (0..slots(n)).all(|s| {
        let m = 8 + s * size + 6 * n;
        frame[m] == 0 && frame[m + 1] == 0
    })
}

/// Both frames of a packet fit the layout of `n` receivers.
pub fn packet_fits(b: &[u8], n: usize) -> bool {
    fits(frame(b, 0), n) && fits(frame(b, 1), n)
}

/// A 24-bit big-endian two's complement word in the top 24 bits of a 32-bit word: full scale is full scale of an `i32`.
#[inline]
pub fn word32(b: &[u8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], 0])
}

/// One receive sample from its words A and B, as (I, Q) in 32-bit words. Without `swap`, I is word B and Q is word A.
#[inline]
pub fn sample(a: &[u8], b: &[u8], swap: bool) -> [i32; 2] {
    let (wa, wb) = (word32(a), word32(b));
    if swap {
        [wa, wb]
    } else {
        [wb, wa]
    }
}

/// Sample `i` (0 to 2 x slots(n) - 1) of receiver `k` (0-based) in a packet of `n` receivers: frame 0's slots, then frame 1's.
#[inline]
pub fn rx_sample(pkt: &[u8], n: usize, k: usize, i: usize, swap: bool) -> [i32; 2] {
    let sl = slots(n);
    let (f, s) = (i / sl, i % sl);
    let o = 8 + f * FRAME + 8 + s * (6 * n + 2) + 6 * k;
    sample(&pkt[o..o + 3], &pkt[o + 3..o + 6], swap)
}

/// The status bytes of one EP6 frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameStatus {
    /// C0 bits 0 to 2: PTT, dash, dot.
    pub ptt: bool,
    /// Dash.
    pub dash: bool,
    /// Dot.
    pub dot: bool,
    /// `C0[7:3]`.
    pub address: u8,
    /// C1 to C4.
    pub c: [u8; 4],
    /// C0 bit 7 on a Hermes-Lite 2: an acknowledgement, not status.
    pub ack: bool,
}

/// Reads the C0 to C4 bytes of an EP6 frame. `hl2`: C0 bit 7 marks an acknowledgement.
pub fn frame_status(frame: &[u8], hl2: bool) -> FrameStatus {
    let c0 = frame[3];
    FrameStatus {
        ptt: c0 & 1 != 0,
        dash: c0 & 2 != 0,
        dot: c0 & 4 != 0,
        address: c0 >> 3,
        c: [frame[4], frame[5], frame[6], frame[7]],
        ack: hl2 && c0 & 0x80 != 0,
    }
}

/// The C0 byte of a register (address, MOX).
pub fn c0(address: u8, mox: bool) -> u8 {
    address << 1 | mox as u8
}

/// The register address of receiver `n`'s NCO (1-based): RX1 to RX7 are 0x02 to 0x08, RX8 to RX12 (Hermes-Lite 2) 0x12 to 0x16.
pub fn nco_register(n: usize) -> u8 {
    if n <= 7 {
        0x01 + n as u8
    } else {
        0x12 + (n - 8) as u8
    }
}

/// Writes the header of a host-to-radio (EP2) packet with sequence `seq`.
pub fn ep2_header(b: &mut [u8], seq: u32) {
    b[..4].copy_from_slice(&[0xEF, 0xFE, 0x01, 0x02]);
    b[4..8].copy_from_slice(&seq.to_be_bytes());
}

/// Writes frame `f` of an EP2 packet: sync, C0 for `address` and `mox`, the 32-bit `value` in C1 to C4. The sample bytes are left
/// as they are (the caller zeroes them or fills transmit samples).
pub fn ep2_frame(b: &mut [u8], f: usize, address: u8, mox: bool, value: u32) {
    let o = 8 + f * FRAME;
    b[o..o + 3].copy_from_slice(&[0x7F, 0x7F, 0x7F]);
    b[o + 3] = c0(address, mox);
    b[o + 4..o + 8].copy_from_slice(&value.to_be_bytes());
}

/// Writes transmit slot `s` of frame `f`: left and right audio 0, then I and Q, 16-bit big endian.
pub fn tx_slot(b: &mut [u8], f: usize, s: usize, i: i16, q: i16) {
    let o = 8 + f * FRAME + 8 + s * 8;
    b[o..o + 4].fill(0);
    b[o + 4..o + 6].copy_from_slice(&i.to_be_bytes());
    b[o + 6..o + 8].copy_from_slice(&q.to_be_bytes());
}
