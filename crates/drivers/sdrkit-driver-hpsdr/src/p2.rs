//! Protocol 2 bytes: discovery, the General, Receiver-specific, Transmitter-specific and High-priority packets, the DDC and DUC
//! streams and the high-priority status. Pure functions over byte slices.
//! Standard: openHPSDR Ethernet protocol (protocol 2).

/// Port offsets from the radio's base port (1024 on a real radio).
pub mod port {
    /// Discovery and General.
    pub const GENERAL: u16 = 0;
    /// Receiver-specific (host to radio), high-priority status (radio to host).
    pub const RX_SPECIFIC: u16 = 1;
    /// Transmitter-specific (host to radio), microphone (radio to host).
    pub const TX_SPECIFIC: u16 = 2;
    /// High-priority (host to radio), wideband ADC0 (radio to host).
    pub const HIGH_PRIORITY: u16 = 3;
    /// Receive audio to the codec.
    pub const AUDIO: u16 = 4;
    /// DUC0 transmit IQ.
    pub const DUC0: u16 = 5;
    /// DDC0 IQ (radio to host). DDC n is DDC0 + n.
    pub const DDC0: u16 = 11;
}

/// General, discovery and transmitter-specific packets are this long.
pub const SHORT: usize = 60;
/// Receiver-specific, high-priority, DDC and DUC packets are this long.
pub const LONG: usize = 1444;
/// Samples in a full DDC packet.
pub const DDC_SAMPLES: usize = 238;
/// Samples in a DUC packet.
pub const DUC_SAMPLES: usize = 240;
/// The DUC rate.
pub const TX_RATE: u32 = 192_000;
/// The most DDCs.
pub const MAX_DDC: usize = 80;

/// The discovery request: `00 00 00 00 02` then 55 zero bytes.
pub const fn discovery_request() -> [u8; SHORT] {
    let mut b = [0u8; SHORT];
    b[4] = 0x02;
    b
}

/// A protocol 2 discovery reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// Status `03`: streaming.
    pub busy: bool,
    /// MAC address.
    pub mac: [u8; 6],
    /// Board id (protocol 2 numbering).
    pub board: u8,
    /// Protocol version supported, times ten.
    pub protocol: u8,
    /// Firmware version.
    pub firmware: u8,
    /// DDCs, when byte 20 is 1 to 80.
    pub receivers: Option<u8>,
}

impl Reply {
    /// Reads a reply: `00 00 00 00` then status `02` or `03`, at least 21 bytes.
    pub fn parse(b: &[u8]) -> Option<Reply> {
        if b.len() < 21 || b[..4] != [0, 0, 0, 0] || !matches!(b[4], 2 | 3) {
            return None;
        }
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&b[5..11]);
        Some(Reply {
            busy: b[4] == 3,
            mac,
            board: b[11],
            protocol: b[12],
            firmware: b[13],
            receivers: (1..=80).contains(&b[20]).then_some(b[20]),
        })
    }
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// The General packet. `pa` sets the PA enable bit, `alex` the Alex enable bit.
pub fn general(b: &mut [u8; SHORT], seq: u32, pa: bool, alex: bool) {
    b.fill(0);
    put32(b, 0, seq);
    b[4] = 0x00;
    for (at, p) in [
        (5, 1025),
        (7, 1026),
        (9, 1027),
        (11, 1025),
        (13, 1028),
        (15, 1029),
        (17, 1035),
        (19, 1026),
        (21, 1027),
    ] {
        put16(b, at, p);
    }
    b[23] = 0;
    b[37] = 0x08;
    b[38] = 0x01;
    b[58] = pa as u8;
    b[59] = alex as u8;
}

/// One DDC's settings in the receiver-specific packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ddc {
    /// Enabled (its receiver is open).
    pub enabled: bool,
    /// Rate in ksps.
    pub rate_ksps: u16,
}

/// The Receiver-specific packet. `ddcs[n]` is DDC n. A DDC that is not enabled has all its fields 0.
pub fn rx_specific(b: &mut [u8; LONG], seq: u32, adcs: u8, ddcs: &[Ddc]) {
    b.fill(0);
    put32(b, 0, seq);
    b[4] = adcs;
    for (n, d) in ddcs.iter().enumerate().take(MAX_DDC) {
        if !d.enabled {
            continue;
        }
        b[7 + n / 8] |= 1 << (n % 8);
        let o = 17 + 6 * n;
        b[o] = 0;
        put16(b, o + 1, d.rate_ksps);
        b[o + 5] = 24;
    }
}

/// The Transmitter-specific packet.
pub fn tx_specific(b: &mut [u8; SHORT], seq: u32) {
    b.fill(0);
    put32(b, 0, seq);
    b[4] = 1;
    put16(b, 14, (TX_RATE / 1000) as u16);
    b[16] = 24;
}

/// What the high-priority packet carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighPriority {
    /// Streaming.
    pub run: bool,
    /// DUC0 PTT.
    pub ptt: bool,
    /// Phase words of DDC 0 to 79.
    pub ddc: [u32; MAX_DDC],
    /// DUC0's phase word.
    pub duc: u32,
    /// Drive level.
    pub drive: u8,
    /// The Alex0 word.
    pub alex: u32,
    /// ADC0 (and ADC1) attenuation, dB.
    pub atten: u8,
}

impl Default for HighPriority {
    fn default() -> Self {
        HighPriority {
            run: false,
            ptt: false,
            ddc: [0; MAX_DDC],
            duc: 0,
            drive: 0,
            alex: 0,
            atten: 0,
        }
    }
}

/// Writes the high-priority packet.
pub fn high_priority(b: &mut [u8; LONG], seq: u32, hp: &HighPriority) {
    b.fill(0);
    put32(b, 0, seq);
    b[4] = hp.run as u8 | (hp.ptt as u8) << 1;
    for (n, w) in hp.ddc.iter().enumerate() {
        put32(b, 9 + 4 * n, *w);
    }
    put32(b, 329, hp.duc);
    b[345] = hp.drive;
    put32(b, 1432, hp.alex);
    b[1442] = hp.atten;
    b[1443] = hp.atten;
}

/// A DDC packet's sample count, when the packet is taken: at least 16 bytes, 24 bits per sample, and 16 + 6 x count within
/// its length.
pub fn ddc_count(b: &[u8]) -> Option<usize> {
    if b.len() < 16 || u16::from_be_bytes([b[12], b[13]]) != 24 {
        return None;
    }
    let n = u16::from_be_bytes([b[14], b[15]]) as usize;
    (16 + 6 * n <= b.len()).then_some(n)
}

/// The packet's sequence number.
pub fn sequence(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// DDC sample `i` as (I, Q) in 32-bit words: I then Q on the wire. `swap` takes them the other way round.
#[inline]
pub fn ddc_sample(b: &[u8], i: usize, swap: bool) -> [i32; 2] {
    let o = 16 + 6 * i;
    crate::p1::sample(&b[o + 3..o + 6], &b[o..o + 3], swap)
}

/// Writes DUC sample `i` of a DUC packet.
pub fn duc_sample(b: &mut [u8], i: usize, iv: i32, qv: i32) {
    let o = 4 + 6 * i;
    b[o..o + 3].copy_from_slice(&iv.to_be_bytes()[1..]);
    b[o + 3..o + 6].copy_from_slice(&qv.to_be_bytes()[1..]);
}

/// The high-priority status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// PTT input.
    pub ptt: bool,
    /// Dot.
    pub dot: bool,
    /// Dash.
    pub dash: bool,
    /// PLL locked.
    pub pll: bool,
    /// ADC0 or ADC1 overload.
    pub overload: bool,
    /// Exciter power.
    pub exciter: u16,
    /// Forward power.
    pub fwd: u16,
    /// Reverse power.
    pub rev: u16,
    /// Supply voltage, raw.
    pub supply: u16,
}

impl Status {
    /// Reads a status packet of at least 51 bytes.
    pub fn parse(b: &[u8]) -> Option<Status> {
        if b.len() < 51 {
            return None;
        }
        let w = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
        Some(Status {
            ptt: b[4] & 1 != 0,
            dot: b[4] & 2 != 0,
            dash: b[4] & 4 != 0,
            pll: b[4] & 0x10 != 0,
            overload: b[5] & 3 != 0,
            exciter: w(6),
            fwd: w(14),
            rev: w(22),
            supply: w(49),
        })
    }
}
