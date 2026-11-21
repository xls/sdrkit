//! The benchmarks of the hot paths, for `sdrkit bench` and the C API's `sdrkit_benchmark`.

use crate::colour::{Converter, Range, BT601};
use crate::jpeg::{self, Subsampling};
use crate::line::{LineImage, Overflow};
use crate::{PixelFormat, Raster};
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};
use std::hint::black_box;

fn test_picture(format: PixelFormat, w: u32, h: u32) -> Raster {
    let bpp = crate::raster::bytes_per_pixel(format).unwrap_or(1) as u32;
    let data = (0..w * h * bpp)
        .map(|i| {
            let (x, y) = ((i / bpp) % w, (i / bpp) / w);
            ((x * 3 + y * 5 + (i % bpp) * 60) % 256) as u8
        })
        .collect();
    Raster::from_vec(format, w, h, data).unwrap_or_else(|_| unreachable!())
}

fn jpeg_decode(seconds: f64) -> BenchResult {
    let r = test_picture(PixelFormat::Bgra, 640, 480);
    let file = jpeg::encode(&r, 85, Subsampling::S420).unwrap_or_default();
    // Items: pixels. Real-time rate: one picture each second.
    measure(seconds, 640 * 480, 1.0, || {
        black_box(jpeg::decode(black_box(&file)).ok());
    })
}

fn jpeg_encode(seconds: f64) -> BenchResult {
    let r = test_picture(PixelFormat::Bgra, 640, 480);
    measure(seconds, 640 * 480, 1.0, || {
        black_box(jpeg::encode(black_box(&r), 85, Subsampling::S420).ok());
    })
}

fn idct(seconds: f64) -> BenchResult {
    let mut coefs = [0f32; 64];
    for (k, c) in coefs.iter_mut().enumerate() {
        *c = ((k * 37) % 23) as f32 - 11.0;
    }
    let mut out = [0u8; 64];
    // Items: blocks. Rate: the blocks of a 640 x 480 grey picture each second.
    measure(seconds, 1000, 1000.0 / 4800.0, || {
        for _ in 0..1000 {
            jpeg::idct_8x8(black_box(&coefs), &mut out, 8);
        }
        black_box(&out);
    })
}

fn push_row(seconds: f64) -> BenchResult {
    let mut img = LineImage::with_capacity(PixelFormat::Grey8, 1809, 2000, Overflow::Scroll)
        .unwrap_or_else(|_| unreachable!());
    let row = vec![128u8; 1809];
    // Items: rows. Rate: weather fax at 120 lines a minute.
    measure(seconds, 1000, 1000.0 / 2.0, || {
        for _ in 0..1000 {
            let _ = img.push_row(black_box(&row));
        }
    })
}

fn ycbcr_to_rgb(seconds: f64) -> BenchResult {
    let conv = Converter::new(BT601, Range::Full);
    let src = test_picture(PixelFormat::Bgra, 640, 480);
    let mut dst = vec![0u8; src.data().len()];
    // Items: pixels. Rate: 25 frames a second.
    measure(seconds, 640 * 480, 1.0 / 25.0, || {
        conv.rgb_row(black_box(src.data()), &mut dst);
        black_box(&dst);
    })
}

fn png_encode(seconds: f64) -> BenchResult {
    let r = test_picture(PixelFormat::Grey8, 1809, 600);
    measure(seconds, 1809 * 600, 1.0, || {
        black_box(crate::png::encode(black_box(&r)).ok());
    })
}

#[cfg(feature = "jpeg2000-encode")]
fn j2k_encode(seconds: f64) -> BenchResult {
    use crate::j2k::{encode, J2kEncode, J2kPicture};
    let p = J2kPicture {
        width: 1000,
        height: 250,
        precision: 10,
        components: vec![(0..250_000u32)
            .map(|i| ((i * 37 + i / 1000 * 11) % 1024) as u16)
            .collect()],
    };
    measure(seconds, 250_000, 1.0, || {
        black_box(encode(black_box(&p), &J2kEncode::default()).ok());
    })
}

/// The benchmarks: JPEG decoding and encoding of a 640 x 480 colour picture (items are pixels, real time is one picture a
/// second), the 8 x 8 IDCT (items are blocks), pushing fax rows into a line image (items are rows, real time is 120 lines a
/// minute), Y'CbCr to RGB (items are pixels, real time is 25 frames of 640 x 480 a second) and PNG encoding of a fax page. With
/// the `jpeg2000-encode` feature, lossless JPEG 2000 encoding of a 10-bit 1000 x 250 segment (items are pixels, real time is
/// one segment a second).
pub fn benchmarks() -> Vec<Benchmark> {
    #[allow(unused_mut)]
    let mut v = vec![
        Benchmark {
            name: "image.jpeg_decode_640x480",
            run: jpeg_decode,
        },
        Benchmark {
            name: "image.jpeg_encode_640x480",
            run: jpeg_encode,
        },
        Benchmark {
            name: "image.idct_8x8",
            run: idct,
        },
        Benchmark {
            name: "image.line_push_row",
            run: push_row,
        },
        Benchmark {
            name: "image.ycbcr_to_rgb",
            run: ycbcr_to_rgb,
        },
        Benchmark {
            name: "image.png_encode_fax",
            run: png_encode,
        },
    ];
    #[cfg(feature = "jpeg2000-encode")]
    v.push(Benchmark {
        name: "image.j2k_encode_lossless_1000x250",
        run: j2k_encode,
    });
    v
}
