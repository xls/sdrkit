//! Pictures: pictures that grow line by line or strip by strip, views and look-up tables, colour conversion, thumbnails, PNG,
//! baseline JPEG, JPEG 2000, bitmap fonts, the `image.pictures` component, and the picture decoders and pixel converters that
//! the graph inserts as adapters.
//!
//! A picture leaves a graph as the standard [`Picture`] item: raw pixels (8-bit grey or BGRA) with their layout, or a compressed
//! file as the signal carries it. [`decode`] converts a compressed picture to pixels by its [`Codec`]. The host saves pictures
//! with [`png::encode`] or [`jpeg::encode`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod colour;
pub mod component;
mod convert;
pub mod enhance;
pub mod font;
pub mod geom;
pub mod history;
#[cfg(feature = "jpeg2000")]
pub mod j2k;
pub mod jpeg;
pub mod line;
pub mod lut;
pub mod png;
pub mod raster;
mod state;
pub mod strip;

mod bench;

pub use bench::benchmarks;
pub use convert::{Decode, PackedPixels, PixelConvert, PixelConvertSetting};
pub use raster::Raster;
pub use sdrkit_core::media::{Codec, Picture, PixelFormat};

/// Why a picture operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// A size of 0, or a picture larger than [`raster::MAX_BYTES`].
    #[error("picture size {width} x {height} is not allowed")]
    TooLarge {
        /// The requested width.
        width: u32,
        /// The requested height.
        height: u32,
    },
    /// Data of the wrong length for the size given.
    #[error("expected {expected} bytes, got {got}")]
    WrongLength {
        /// The bytes the size needs.
        expected: usize,
        /// The bytes given.
        got: usize,
    },
    /// A picture that holds its most rows and does not scroll.
    #[error("the picture is full")]
    Full,
    /// Data cut short.
    #[error("picture data cut short")]
    Truncated,
    /// A pixel format that a [`Raster`] does not hold: only 8-bit grey and BGRA are packed with whole bytes per pixel.
    #[error("{pixel:?} pixels are not held packed")]
    NotPacked {
        /// The format.
        pixel: PixelFormat,
    },
    /// A compressed picture where raw pixels are needed.
    #[error("a {codec} picture is not raw pixels")]
    NotPixels {
        /// Its codec.
        codec: Codec,
    },
    /// A raw picture without a pixel layout.
    #[error("the picture has no pixel layout")]
    NoLayout,
    /// A row stride shorter than one row of pixels.
    #[error("stride {stride} is shorter than a row of {row} bytes")]
    StrideShort {
        /// The stride of the layout.
        stride: usize,
        /// The bytes of one row.
        row: usize,
    },
    /// A codec that this crate does not decode.
    #[error("no decoder for {codec}")]
    NoDecoder {
        /// The codec.
        codec: Codec,
    },
    /// A TrueType font that this reader cannot use.
    #[error("not a TrueType font this reader can use: {0:?}")]
    Font(font::ttf::FontFault),
    /// A JPEG error.
    #[error("JPEG: {0}")]
    Jpeg(#[from] jpeg::JpegError),
    /// A PNG error.
    #[error("PNG: {0:?}")]
    Png(png::PngFault),
    /// A JPEG 2000 error.
    #[error("JPEG 2000: {0:?}")]
    J2k(J2kFault),
}

/// Why a JPEG 2000 picture did not decode or encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum J2kFault {
    /// The decoder refused the data: damaged, or a feature it does not have.
    Damaged,
    /// A component count other than 1 or 3 for a picture in 8 bits, or no component, or more than 255.
    Components {
        /// The count.
        count: usize,
    },
    /// A precision other than 1 to 16 bits.
    Precision {
        /// The precision given.
        bits: u8,
    },
    /// A component without `width` x `height` samples.
    SampleCount,
    /// A sample above the largest value of the precision.
    SampleAbovePrecision,
    /// More than 32 decomposition levels (T.800 Table A.15).
    Levels {
        /// The levels given.
        levels: u8,
    },
    /// The encoder refused the picture.
    Encoder,
}

/// The codecs that [`decode`] decodes: PNG, JPEG and, with the `jpeg2000` feature, JPEG 2000.
pub const DECODES: &[Codec] = &[
    Codec::Png,
    Codec::Jpeg,
    #[cfg(feature = "jpeg2000")]
    Codec::Jpeg2000,
];

/// Decodes the compressed picture `bytes` of `codec` to raw pixels. The decoder writes the pixels once, into the buffer that
/// the [`Raster`] shares.
///
/// # Errors
///
/// [`ImageError::NoDecoder`] for a codec outside [`DECODES`], else the error of the decoder.
pub fn decode(codec: Codec, bytes: &[u8]) -> Result<Raster, ImageError> {
    match codec {
        Codec::Png => png::decode(bytes),
        Codec::Jpeg => jpeg::decode(bytes),
        #[cfg(feature = "jpeg2000")]
        Codec::Jpeg2000 => j2k::decode(bytes)?.to_raster(),
        codec => Err(ImageError::NoDecoder { codec }),
    }
}

/// Adds the components of this crate to a registry: `image.pictures` ([`component::Pictures`] with [`component::Layout::any`]),
/// and the adapters `image.decode` (a compressed picture to pixels) and `image.pixels` (one pixel layout to another).
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add(
        "image.pictures",
        || Box::new(component::Pictures::default()),
    );
    convert::register(r);
}
