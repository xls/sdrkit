//! The DAB frequency blocks of ETSI EN 300 401 (clause 6 and annex B): Band III blocks 5A to 13F and L-band blocks LA to LW,
//! each 1.536 MHz wide.

/// One DAB frequency block: its name and centre frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DabBlock {
    /// The name ("12B").
    pub name: &'static str,
    /// The centre frequency, Hz.
    pub centre_hz: f64,
}

/// The width of a DAB block, Hz.
pub const BLOCK_WIDTH_HZ: f64 = 1.536e6;

/// The 38 Band III blocks, 5A to 13F, lowest first.
pub const BAND_III: [DabBlock; 38] = [
    DabBlock {
        name: "5A",
        centre_hz: 174.928e6,
    },
    DabBlock {
        name: "5B",
        centre_hz: 176.64e6,
    },
    DabBlock {
        name: "5C",
        centre_hz: 178.352e6,
    },
    DabBlock {
        name: "5D",
        centre_hz: 180.064e6,
    },
    DabBlock {
        name: "6A",
        centre_hz: 181.936e6,
    },
    DabBlock {
        name: "6B",
        centre_hz: 183.648e6,
    },
    DabBlock {
        name: "6C",
        centre_hz: 185.36e6,
    },
    DabBlock {
        name: "6D",
        centre_hz: 187.072e6,
    },
    DabBlock {
        name: "7A",
        centre_hz: 188.928e6,
    },
    DabBlock {
        name: "7B",
        centre_hz: 190.64e6,
    },
    DabBlock {
        name: "7C",
        centre_hz: 192.352e6,
    },
    DabBlock {
        name: "7D",
        centre_hz: 194.064e6,
    },
    DabBlock {
        name: "8A",
        centre_hz: 195.936e6,
    },
    DabBlock {
        name: "8B",
        centre_hz: 197.648e6,
    },
    DabBlock {
        name: "8C",
        centre_hz: 199.36e6,
    },
    DabBlock {
        name: "8D",
        centre_hz: 201.072e6,
    },
    DabBlock {
        name: "9A",
        centre_hz: 202.928e6,
    },
    DabBlock {
        name: "9B",
        centre_hz: 204.64e6,
    },
    DabBlock {
        name: "9C",
        centre_hz: 206.352e6,
    },
    DabBlock {
        name: "9D",
        centre_hz: 208.064e6,
    },
    DabBlock {
        name: "10A",
        centre_hz: 209.936e6,
    },
    DabBlock {
        name: "10B",
        centre_hz: 211.648e6,
    },
    DabBlock {
        name: "10C",
        centre_hz: 213.36e6,
    },
    DabBlock {
        name: "10D",
        centre_hz: 215.072e6,
    },
    DabBlock {
        name: "11A",
        centre_hz: 216.928e6,
    },
    DabBlock {
        name: "11B",
        centre_hz: 218.64e6,
    },
    DabBlock {
        name: "11C",
        centre_hz: 220.352e6,
    },
    DabBlock {
        name: "11D",
        centre_hz: 222.064e6,
    },
    DabBlock {
        name: "12A",
        centre_hz: 223.936e6,
    },
    DabBlock {
        name: "12B",
        centre_hz: 225.648e6,
    },
    DabBlock {
        name: "12C",
        centre_hz: 227.36e6,
    },
    DabBlock {
        name: "12D",
        centre_hz: 229.072e6,
    },
    DabBlock {
        name: "13A",
        centre_hz: 230.784e6,
    },
    DabBlock {
        name: "13B",
        centre_hz: 232.496e6,
    },
    DabBlock {
        name: "13C",
        centre_hz: 234.208e6,
    },
    DabBlock {
        name: "13D",
        centre_hz: 235.776e6,
    },
    DabBlock {
        name: "13E",
        centre_hz: 237.488e6,
    },
    DabBlock {
        name: "13F",
        centre_hz: 239.2e6,
    },
];

/// The 23 L-band blocks, LA to LW, lowest first.
pub const L_BAND: [DabBlock; 23] = [
    DabBlock {
        name: "LA",
        centre_hz: 1.45296e9,
    },
    DabBlock {
        name: "LB",
        centre_hz: 1.454672e9,
    },
    DabBlock {
        name: "LC",
        centre_hz: 1.456384e9,
    },
    DabBlock {
        name: "LD",
        centre_hz: 1.458096e9,
    },
    DabBlock {
        name: "LE",
        centre_hz: 1.459808e9,
    },
    DabBlock {
        name: "LF",
        centre_hz: 1.46152e9,
    },
    DabBlock {
        name: "LG",
        centre_hz: 1.463232e9,
    },
    DabBlock {
        name: "LH",
        centre_hz: 1.464944e9,
    },
    DabBlock {
        name: "LI",
        centre_hz: 1.466656e9,
    },
    DabBlock {
        name: "LJ",
        centre_hz: 1.468368e9,
    },
    DabBlock {
        name: "LK",
        centre_hz: 1.47008e9,
    },
    DabBlock {
        name: "LL",
        centre_hz: 1.471792e9,
    },
    DabBlock {
        name: "LM",
        centre_hz: 1.473504e9,
    },
    DabBlock {
        name: "LN",
        centre_hz: 1.475216e9,
    },
    DabBlock {
        name: "LO",
        centre_hz: 1.476928e9,
    },
    DabBlock {
        name: "LP",
        centre_hz: 1.47864e9,
    },
    DabBlock {
        name: "LQ",
        centre_hz: 1.480352e9,
    },
    DabBlock {
        name: "LR",
        centre_hz: 1.482064e9,
    },
    DabBlock {
        name: "LS",
        centre_hz: 1.483776e9,
    },
    DabBlock {
        name: "LT",
        centre_hz: 1.485488e9,
    },
    DabBlock {
        name: "LU",
        centre_hz: 1.4872e9,
    },
    DabBlock {
        name: "LV",
        centre_hz: 1.488912e9,
    },
    DabBlock {
        name: "LW",
        centre_hz: 1.490624e9,
    },
];
