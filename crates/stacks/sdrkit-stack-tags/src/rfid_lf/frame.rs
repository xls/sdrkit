//! The 64-bit frame of the EM4100 (EM Microelectronic-Marin, EM4100 Read Only Contactless Identification Device, data sheet,
//! Memory Array): nine header 1 bits, ten rows of 4 data bits each followed by its even row parity, four even column
//! parities and a stop bit 0. Bit 0 is sent first. The 40 data bits are 8 version (or customer) bits and a 32-bit number;
//! within a row the first bit sent is taken as the most significant (the data sheet does not say).

/// The bits of a frame.
pub const FRAME_BITS: usize = 64;
/// The header bits.
pub const HEADER_BITS: usize = 9;

/// The frame of the 40 data bits `data` (bits 39 to 0; bit 39 first).
pub fn encode(data: u64) -> [u8; FRAME_BITS] {
    let mut f = [0u8; FRAME_BITS];
    f[..HEADER_BITS].fill(1);
    let mut cols = [0u8; 4];
    for row in 0..10 {
        let nibble = ((data >> (36 - 4 * row)) & 0xF) as u8;
        let mut p = 0u8;
        for (c, col) in cols.iter_mut().enumerate() {
            let b = (nibble >> (3 - c)) & 1;
            f[HEADER_BITS + 5 * row + c] = b;
            p ^= b;
            *col ^= b;
        }
        f[HEADER_BITS + 5 * row + 4] = p;
    }
    f[59..63].copy_from_slice(&cols);
    f[63] = 0;
    f
}

/// Why a frame was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No header of nine 1 bits or no stop bit 0: not a frame.
    NoFrame,
    /// A row or a column parity fails.
    Parity,
}

/// The 40 data bits of `f` (64 bits, the first sent first), after the header, every row parity, every column parity and the
/// stop bit hold.
///
/// # Errors
///
/// [`Refused::NoFrame`] without the header or the stop bit, [`Refused::Parity`] when a parity fails.
pub fn decode(f: &[u8]) -> Result<u64, Refused> {
    if f.len() != FRAME_BITS || f[..HEADER_BITS].iter().any(|&b| b != 1) || f[63] != 0 {
        return Err(Refused::NoFrame);
    }
    let mut data = 0u64;
    let mut cols = [0u8; 4];
    for row in 0..10 {
        let r = &f[HEADER_BITS + 5 * row..HEADER_BITS + 5 * row + 5];
        if r.iter().fold(0, |a, &b| a ^ b) != 0 {
            return Err(Refused::Parity);
        }
        for (c, col) in cols.iter_mut().enumerate() {
            data = (data << 1) | u64::from(r[c]);
            *col ^= r[c];
        }
    }
    for (c, col) in cols.iter().enumerate() {
        if col ^ f[59 + c] != 0 {
            return Err(Refused::Parity);
        }
    }
    Ok(data)
}

/// The frame as hex, 16 digits, the first bit sent as the top bit.
pub fn hex(f: &[u8; FRAME_BITS]) -> String {
    let v = f.iter().fold(0u64, |a, &b| (a << 1) | u64::from(b));
    format!("{v:016X}")
}
