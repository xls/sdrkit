//! bzip2: a decoder of one or more joined streams, which checks every block CRC and the stream CRC and stops at an output
//! limit, and an encoder for test data. Both work on whole buffers.
//!
//! A stream is "BZh", a level from '1' to '9' (blocks of at most level x 100 000 bytes), then blocks and an end marker. A block
//! holds its CRC, the origin pointer of the Burrows-Wheeler transform, the map of the bytes it uses, 2 to 6 Huffman tables, the
//! selector of a table for each group of 50 symbols (move-to-front coded), the code lengths (delta coded), and the symbols:
//! move-to-front indices with runs of the front byte coded as RUNA and RUNB, ended by an end-of-block symbol. The decoder
//! undoes the move-to-front coding, the Burrows-Wheeler transform and the first run-length step (four equal bytes and a
//! count). The CRC is CRC-32/BZIP2; the stream CRC is the stream CRC so far rotated left by one bit, XOR each block CRC.
//!
//! Standard: the bzip2 file format as published with the bzip2 program (version 1.0).
//!
//! ```
//! let data: Vec<u8> = (0..5000u32).map(|i| (i % 7 * 31 + i / 100) as u8).collect();
//! let packed = sdrkit_codec_bzip2::compress(&data, 9);
//! assert_eq!(&packed[..4], b"BZh9");
//! assert_eq!(sdrkit_codec_bzip2::decompress(&packed, 1 << 20).unwrap(), data);
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod decode;
mod encode;

pub use decode::decompress;
pub use encode::compress;

/// The 48-bit marker that starts a block (the digits of pi).
pub const BLOCK_MAGIC: u64 = 0x3141_5926_5359;
/// The 48-bit marker that ends a stream (the digits of the square root of pi).
pub const END_MAGIC: u64 = 0x1772_4538_5090;
/// The largest number of selectors a block may use (2 + 900 000 / 50).
pub const MAX_SELECTORS: usize = 18_002;
/// The longest Huffman code.
pub const MAX_CODE_LEN: u32 = 20;

/// Why a buffer did not decode. Any of them fails the whole buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// No "BZh" stream in the buffer.
    #[error("no bzip2 stream")]
    NotBzip2,
    /// The data ends inside a stream.
    #[error("the stream is cut short")]
    Truncated,
    /// A marker that is neither a block nor the end of the stream.
    #[error("a damaged block marker")]
    BadMagic,
    /// The block uses the randomised mode, which this decoder does not take.
    #[error("a randomised block")]
    Randomised,
    /// The block uses no byte, or holds more bytes than its level allows.
    #[error("a block of a wrong size")]
    BlockSize,
    /// The origin pointer is beyond the block.
    #[error("the origin pointer is beyond the block")]
    Origin,
    /// The number of Huffman tables is not 2 to 6, or there are no selectors, or a selector names no table.
    #[error("damaged Huffman tables or selectors")]
    Tables,
    /// A code length is not 1 to 20.
    #[error("a code length out of range")]
    CodeLength,
    /// A code that no symbol has, or a symbol beyond the bytes in use.
    #[error("a damaged symbol")]
    Symbol,
    /// The CRC of a block is wrong.
    #[error("a block CRC mismatch")]
    BlockCrc,
    /// The CRC of the stream is wrong.
    #[error("a stream CRC mismatch")]
    StreamCrc,
    /// The output would pass the limit.
    #[error("the output passes the limit")]
    TooLong,
}
