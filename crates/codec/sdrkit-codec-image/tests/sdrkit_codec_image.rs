//! The tests of sdrkit-codec-image: one program, one module for each part.
//!
//! The files in `tests/data` come from an independent encoder and decoder (the macOS `sips` tool, Apple ImageIO): `grey.png`
//! and `colour.png` are the sources, `*_sips.jpg` and `*_sips.jp2` are its JPEG and JPEG 2000 files of them, and
//! `*_decoded.png` are its own decodings of those files.

use sdrkit_codec_image::{PixelFormat, Raster};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mse = a
        .iter()
        .zip(b)
        .map(|(x, y)| (*x as f64 - *y as f64).powi(2))
        .sum::<f64>()
        / a.len() as f64;
    10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn ramp(format: PixelFormat, w: u32, h: u32) -> Raster {
    let bpp = sdrkit_codec_image::raster::bytes_per_pixel(format).unwrap() as u32;
    let v = (0..w * h * bpp)
        .map(|i| {
            let (x, y, c) = ((i / bpp) % w, (i / bpp) / w, i % bpp);
            // A smooth pattern (a triangle wave, no jumps), so lossy coding keeps it well.
            let t = (x * 4 + y * 3 + c * 70) % 510;
            (if t > 255 { 510 - t } else { t }) as u8
        })
        .collect();
    Raster::from_vec(format, w, h, v).unwrap()
}

/// The ramp of three channels, as red, green and blue bytes.
fn ramp_rgb_bytes(w: u32, h: u32) -> Vec<u8> {
    let g = ramp(PixelFormat::Bgra, w, h);
    g.data()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

/// A colour picture of the ramp, alpha 255.
fn ramp_rgb(w: u32, h: u32) -> Raster {
    Raster::from_rgb(w, h, &ramp_rgb_bytes(w, h)).unwrap()
}

/// The red, green and blue bytes of a picture.
fn rgb_bytes(r: &Raster) -> Vec<u8> {
    r.rgb().flatten().collect()
}

mod colour {
    use sdrkit_codec_image::colour::*;

    #[test]
    fn colour_bars_match_bt601_and_bt709() {
        // 100 % and 75 % colour bars, 8-bit video range (BT.601-7 clause 2.5.3, BT.709-6 item 4.6).
        let c601 = Converter::new(BT601, Range::Video);
        let c709 = Converter::new(BT709, Range::Video);
        assert_eq!(c601.ycbcr([255, 255, 255]), [235, 128, 128]);
        assert_eq!(c601.ycbcr([0, 0, 0]), [16, 128, 128]);
        assert_eq!(c601.ycbcr([255, 0, 0]), [81, 90, 240]);
        assert_eq!(c601.ycbcr([191, 0, 0]), [65, 100, 212]);
        assert_eq!(c709.ycbcr([255, 0, 0]), [63, 102, 240]);
        assert_eq!(c709.ycbcr([191, 0, 0]), [51, 109, 212]);
        assert_eq!(c709.ycbcr([0, 0, 255]), [32, 240, 118]);
    }

    #[test]
    fn jfif_full_range_matches_t871() {
        // T.871 clause 7: Y = 0.299 R + 0.587 G + 0.114 B, Cb = -0.1687 R - 0.3313 G + 0.5 B + 128 ...
        let c = Converter::new(BT601, Range::Full);
        assert_eq!(c.ycbcr([255, 0, 0]), [76, 85, 255]);
        assert_eq!(c.ycbcr([0, 255, 0]), [150, 44, 21]);
        assert_eq!(c.ycbcr([0, 0, 255]), [29, 255, 107]);
        assert_eq!(c.ycbcr([128, 128, 128]), [128, 128, 128]);
        // R = Y + 1.402 (Cr - 128) and so on.
        assert_eq!(c.rgb([128, 128, 228]), [255, 57, 128]);
        let f = c.rgb_f32(100.0, 128.0, 228.0);
        assert!((f[0] * 255.0 - 240.2).abs() < 0.05, "{f:?}");
    }

    #[test]
    fn conversions_round_trip_within_one_level() {
        for m in [BT601, BT709, BT2020] {
            let c = Converter::new(m, Range::Full);
            for r in (0..=255).step_by(15) {
                for g in (0..=255).step_by(17) {
                    for b in (0..=255).step_by(51) {
                        let back = c.rgb(c.ycbcr([r as u8, g as u8, b as u8]));
                        for (x, y) in back.iter().zip([r, g, b]) {
                            assert!((*x as i32 - y).abs() <= 2, "{m:?} {r} {g} {b} -> {back:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn luma_and_analogue_differences() {
        assert_eq!(luma_601(255, 255, 255), 255);
        assert_eq!(luma_601(255, 0, 0), 76);
        let [y, u, v] = yuv_from_rgb(1.0, 0.0, 0.0);
        assert!(
            (y - 0.299).abs() < 1e-6
                && (u + 0.493 * 0.299).abs() < 1e-6
                && (v - 0.877 * 0.701).abs() < 1e-6
        );
        let rgb = rgb_from_differences(BT601, y, u / KU, v / KV);
        assert!((rgb[0] - 1.0).abs() < 1e-5 && rgb[1].abs() < 1e-5 && rgb[2].abs() < 1e-5);
    }
}

mod jpeg {
    use super::*;
    use sdrkit_codec_image::jpeg::tables::*;
    use sdrkit_codec_image::jpeg::*;

    fn bits_of(code: u16, len: u8) -> String {
        (0..len)
            .rev()
            .map(|k| if code >> k & 1 == 1 { '1' } else { '0' })
            .collect()
    }

    #[test]
    fn zigzag_is_figure_a6() {
        assert_eq!(&ZIGZAG[..10], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24]);
        assert_eq!(ZIGZAG[63], 63);
        let mut seen = [false; 64];
        for &z in &ZIGZAG {
            seen[z as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn quantisation_tables_are_k1_and_k2() {
        assert_eq!(&LUMA_Q[..8], &[16, 11, 10, 16, 24, 40, 51, 61]);
        assert_eq!(&LUMA_Q[56..], &[72, 92, 95, 98, 112, 100, 103, 99]);
        assert_eq!(&CHROMA_Q[..8], &[17, 18, 24, 47, 99, 99, 99, 99]);
        assert_eq!(scaled(&LUMA_Q, 50, BASELINE_MAX_Q), LUMA_Q);
        assert_eq!(scaled(&LUMA_Q, 100, BASELINE_MAX_Q), [1; 64]);
        assert_eq!(scaled(&LUMA_Q, 10, BASELINE_MAX_Q)[0], 80);
        // Quality 1 scales by 5000 %: held at 255 in a baseline file.
        assert_eq!(scaled(&LUMA_Q, 1, BASELINE_MAX_Q)[0], 255);
        assert_eq!(scaled(&LUMA_Q, 1, 32767)[0], 800);
    }

    #[test]
    fn huffman_codes_are_tables_k3_to_k6() {
        let t = StandardTables::get();
        let c = |h: &HuffmanTable, s: u8| {
            let (code, len) = h.code(s);
            bits_of(code, len)
        };
        // Table K.3: luminance DC.
        assert_eq!(c(&t.dc_luma, 0), "00");
        assert_eq!(c(&t.dc_luma, 1), "010");
        assert_eq!(c(&t.dc_luma, 5), "110");
        assert_eq!(c(&t.dc_luma, 6), "1110");
        assert_eq!(c(&t.dc_luma, 11), "111111110");
        // Table K.4: chrominance DC.
        assert_eq!(c(&t.dc_chroma, 0), "00");
        assert_eq!(c(&t.dc_chroma, 3), "110");
        assert_eq!(c(&t.dc_chroma, 11), "11111111110");
        // Table K.5: luminance AC.
        assert_eq!(c(&t.ac_luma, 0x00), "1010");
        assert_eq!(c(&t.ac_luma, 0x01), "00");
        assert_eq!(c(&t.ac_luma, 0x03), "100");
        assert_eq!(c(&t.ac_luma, 0x11), "1100");
        assert_eq!(c(&t.ac_luma, 0xF0), "11111111001");
        assert_eq!(c(&t.ac_luma, 0x0A), "1111111110000011");
        // Table K.6: chrominance AC.
        assert_eq!(c(&t.ac_chroma, 0x00), "00");
        assert_eq!(c(&t.ac_chroma, 0x01), "01");
        assert_eq!(c(&t.ac_chroma, 0xF0), "1111111010");
        // Every run/size symbol of baseline coding has a code.
        for h in [&t.ac_luma, &t.ac_chroma] {
            for run in 0..16u8 {
                for size in 1..=10u8 {
                    assert!(h.code(run << 4 | size).1 > 0);
                }
            }
        }
        assert!(HuffmanTable::new(
            &[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[1, 2, 3]
        )
        .is_err());
    }

    #[test]
    fn dct_matches_the_t81_equations() {
        // A flat block of 255: S(0,0) = 1/4 x 1/2 x 64 x 127 = 1016, every AC coefficient 0.
        let mut s = [0f32; 64];
        fdct_8x8(&[255u8; 64], 8, &mut s);
        assert!((s[0] - 1016.0).abs() < 1e-3, "{}", s[0]);
        assert!(s[1..].iter().all(|v| v.abs() < 1e-3));
        // One coefficient S(0,1) = 100: s(y,x) = 128 + 1/4 C(0) C(1) 100 cos((2x+1) pi / 16).
        let mut c = [0f32; 64];
        c[1] = 100.0;
        let mut out = [0u8; 64];
        idct_8x8(&c, &mut out, 8);
        for y in 0..8 {
            for x in 0..8 {
                let want = 128.0
                    + 0.25
                        * std::f64::consts::FRAC_1_SQRT_2
                        * 100.0
                        * ((2 * x + 1) as f64 * std::f64::consts::PI / 16.0).cos();
                assert_eq!(out[y * 8 + x], want.round() as u8, "{x} {y}");
            }
        }
        // FDCT then IDCT returns the original block.
        let px: Vec<u8> = (0..64).map(|i| ((i * 37 + 11) % 256) as u8).collect();
        fdct_8x8(&px, 8, &mut s);
        idct_8x8(&s, &mut out, 8);
        assert!(max_diff(&px, &out) <= 1);
    }

    #[test]
    fn entropy_reader_returns_its_bit_position_and_skips_to_the_next_byte() {
        let t = StandardTables::get();
        // T.81 Annex K: DC luminance category 0 is "00" (table K.3) and the AC luminance EOB is "1010" (table K.5), so a zero
        // block with a zero predictor is 6 bits.
        let zero = [0i16; 64];
        let mut b1 = [0i16; 64];
        b1[0] = 5;
        b1[2] = -3;
        // Two units, each on its own bytes, as in an SSDV packet: a zero block, then another block.
        let mut buf = Vec::new();
        let mut w = EntropyWriter::new(&mut buf, false);
        encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut 0, &zero).unwrap();
        w.align();
        let second = buf.len();
        let mut w = EntropyWriter::new(&mut buf, false);
        encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut 0, &b1).unwrap();
        // The writer counts the whole vector, so this is the end of the second unit.
        let bits_b1 = w.bit_len() as u64;
        w.align();
        assert_eq!(second, 1);
        let mut r = EntropyReader::new(&buf, false);
        let mut got = [0i16; 64];
        assert_eq!(r.bit_position(), 0);
        decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut 0, &mut got).unwrap();
        assert_eq!(r.bit_position(), 6);
        r.skip_to_byte();
        assert_eq!(r.bit_position(), 8);
        r.skip_to_byte();
        assert_eq!(r.bit_position(), 8);
        decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut 0, &mut got).unwrap();
        assert_eq!(got, b1);
        assert_eq!(r.bit_position(), bits_b1);
        r.skip_to_byte();
        assert_eq!(r.bit_position(), 8 * buf.len() as u64);
        // Past the end, the position stays at the end.
        assert!(r.bits(16).is_err());
        assert_eq!(r.bit_position(), 8 * buf.len() as u64);
        // With stuffing, the 0x00 after 0xFF does not count (T.81 F.1.2.3).
        let mut r = EntropyReader::new(&[0xFF, 0x00, 0xAB, 0xCD], true);
        assert_eq!(r.bits(8), Ok(0xFF));
        assert_eq!(r.bits(4), Ok(0xA));
        assert_eq!(r.bit_position(), 12);
        r.skip_to_byte();
        assert_eq!(r.bit_position(), 16);
        assert_eq!(r.bits(8), Ok(0xCD));
    }

    #[test]
    fn blocks_round_trip_through_the_entropy_layer() {
        let t = StandardTables::get();
        let mut blocks = Vec::new();
        for k in 0..20i16 {
            let mut b = [0i16; 64];
            b[0] = k * 37 - 300;
            b[1] = -k;
            b[5] = 511;
            b[40] = -1023;
            b[63] = 1;
            blocks.push(b);
        }
        let mut buf = Vec::new();
        let mut w = EntropyWriter::new(&mut buf, true);
        let mut pred = 0;
        for b in &blocks {
            encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut pred, b).unwrap();
        }
        w.align();
        let mut r = EntropyReader::new(&buf, true);
        let (mut pred, mut got) = (0, [0i16; 64]);
        for b in &blocks {
            decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut pred, &mut got).unwrap();
            assert_eq!(&got, b);
        }
        // Cut data is an error, not a wrong block.
        let mut r = EntropyReader::new(&buf[..buf.len() / 2], true);
        let mut pred = 0;
        let res: Result<Vec<()>, _> = blocks
            .iter()
            .map(|_| decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut pred, &mut got))
            .collect();
        assert_eq!(res, Err(JpegError::Truncated));
        // A coefficient beyond baseline range is refused.
        let mut big = [0i16; 64];
        big[3] = 2000;
        let mut v = Vec::new();
        let mut w = EntropyWriter::new(&mut v, true);
        assert_eq!(
            encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut 0, &big),
            Err(JpegError::ValueTooLarge)
        );
    }

    #[test]
    fn our_files_decode_the_same_in_an_independent_decoder() {
        for (format, sub) in [
            (PixelFormat::Grey8, Subsampling::S444),
            (PixelFormat::Bgra, Subsampling::S444),
            (PixelFormat::Bgra, Subsampling::S420),
        ] {
            let src = if format == PixelFormat::Grey8 {
                ramp(format, 37, 21)
            } else {
                ramp_rgb(37, 21)
            };
            let file = encode(&src, 85, sub).unwrap();
            let ours = decode(&file).unwrap();
            let mut z = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(&file));
            let theirs = z.decode().unwrap();
            let ours_rgb = rgb_bytes(&ours);
            assert_eq!(ours_rgb.len(), theirs.len());
            if sub == Subsampling::S444 {
                assert!(max_diff(&ours_rgb, &theirs) <= 2, "{format:?}");
            } else {
                // The decoders enlarge the chroma differently.
                assert!(
                    psnr(&ours_rgb, &theirs) > 30.0,
                    "{}",
                    psnr(&ours_rgb, &theirs)
                );
            }
            let p = psnr(&ours_rgb, &rgb_bytes(&src));
            assert!(p > 28.0, "{format:?} {sub:?} {p}");
        }
    }

    #[test]
    fn files_from_an_independent_encoder_decode() {
        // Grey: one component, so no colour step differs.
        let ours = decode(&data("grey_sips.jpg")).unwrap();
        let theirs = sdrkit_codec_image::png::decode(&data("grey_sips_jpg_decoded.png")).unwrap();
        assert_eq!(ours.format(), PixelFormat::Grey8);
        assert!(
            max_diff(ours.data(), theirs.data()) <= 2,
            "{}",
            max_diff(ours.data(), theirs.data())
        );
        // Colour 4:2:0.
        let ours = decode(&data("colour_sips.jpg")).unwrap();
        let theirs = sdrkit_codec_image::png::decode(&data("colour_sips_jpg_decoded.png")).unwrap();
        assert_eq!((ours.width(), ours.height()), (48, 32));
        assert!(
            psnr(ours.data(), theirs.data()) > 30.0,
            "{}",
            psnr(ours.data(), theirs.data())
        );
        let source = sdrkit_codec_image::png::decode(&data("colour.png")).unwrap();
        assert!(psnr(ours.data(), source.data()) > 25.0);
    }

    #[test]
    fn low_quality_files_stay_consistent() {
        // A table entry above 255 is held at 255 in the file and in the coding, so another decoder agrees.
        let src = ramp(PixelFormat::Grey8, 64, 16);
        let file = encode(&src, 2, Subsampling::S444).unwrap();
        let ours = decode(&file).unwrap();
        let theirs = zune_jpeg::JpegDecoder::new(std::io::Cursor::new(&file))
            .decode()
            .unwrap();
        let theirs: Vec<u8> = theirs.chunks(3).map(|p| p[0]).collect();
        assert!(max_diff(ours.data(), &theirs) <= 2);
    }

    #[test]
    fn restart_markers_and_damaged_data() {
        // A grey 16 x 8 file of two blocks with a restart interval of one MCU, written with the coefficient layer.
        let t = StandardTables::get();
        let hd = JfifHeader::grey(16, 8, 75);
        let mut head = Vec::new();
        hd.write(&mut head);
        // Put DRI (interval 1) before SOS.
        let sos = head.windows(2).rposition(|w| w == [0xFF, 0xDA]).unwrap();
        let mut file = head[..sos].to_vec();
        file.extend_from_slice(&[0xFF, 0xDD, 0, 4, 0, 1]);
        file.extend_from_slice(&head[sos..]);
        let mut a = [0i16; 64];
        a[0] = 20;
        let mut b = [0i16; 64];
        b[0] = -10;
        b[2] = 5;
        for (k, blk) in [a, b].iter().enumerate() {
            let mut seg = Vec::new();
            let mut w = EntropyWriter::new(&mut seg, true);
            encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut 0, blk).unwrap();
            w.align();
            file.extend_from_slice(&seg);
            if k == 0 {
                file.extend_from_slice(&[0xFF, 0xD0]);
            }
        }
        JfifHeader::finish(&mut file);
        let f = decode_coefficients(&file).unwrap();
        assert_eq!(f.damaged, None);
        assert_eq!(f.components[0].blocks[0], a);
        assert_eq!(f.components[0].blocks[1], b);
        // Cut after the first block: the frame decodes, with the damage reported.
        let cut = &file[..file.len() - 6];
        let f = decode_coefficients(cut).unwrap();
        assert!(f.damaged.is_some());
        assert_eq!(f.components[0].blocks[0], a);
        // Not JPEG, and unsupported processes.
        assert_eq!(decode_coefficients(b"\x89PNG"), Err(JpegError::NotJpeg));
        let mut prog = file.clone();
        let sof = prog.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        prog[sof + 1] = 0xC2;
        assert_eq!(
            decode_coefficients(&prog),
            Err(JpegError::Unsupported(JpegFeature::Progressive))
        );
    }

    #[test]
    fn ssdv_style_mcus_make_a_file() {
        let rgb = ramp_rgb_bytes(32, 32);
        let mcus = rgb_to_mcus_420(&rgb, 32, 32, 75);
        assert_eq!(mcus.len(), 4);
        let list: Vec<Option<Mcu420>> = vec![Some(mcus[0]), None, Some(mcus[2]), Some(mcus[3])];
        let file = write_mcus_420(32, 32, 75, &list).unwrap();
        let f = decode_coefficients(&file).unwrap();
        assert_eq!(f.damaged, None);
        // The coefficients come back exactly, block by block.
        assert_eq!(f.components[0].blocks[0], mcus[0][0]);
        assert_eq!(f.components[1].blocks[3], mcus[3][4]);
        assert_eq!(f.components[2].blocks[2], mcus[2][5]);
        // The missing MCU is flat mid grey.
        let r = f.to_raster().unwrap();
        let px = &r.data()[(4 * 32 + 20) * 4..(4 * 32 + 20) * 4 + 4];
        assert_eq!(px, &[128, 128, 128, 255]);
    }
}

#[cfg(feature = "jpeg2000")]
mod j2k {
    use super::*;
    use sdrkit_codec_image::j2k;

    #[test]
    fn files_from_an_independent_encoder_decode() {
        for (file, decoded) in [
            ("grey_sips.jp2", "grey_sips_jp2_decoded.png"),
            ("colour_sips.jp2", "colour_sips_jp2_decoded.png"),
        ] {
            let bytes = data(file);
            assert_eq!(j2k::find_stream(&bytes), Some((0, true)));
            let p = j2k::decode(&bytes).unwrap();
            assert_eq!((p.width, p.height, p.precision), (48, 32, 8));
            let ours = p.to_raster().unwrap();
            let theirs = sdrkit_codec_image::png::decode(&data(decoded)).unwrap();
            assert_eq!(ours.format(), theirs.format());
            assert!(
                max_diff(ours.data(), theirs.data()) <= 2,
                "{file} {}",
                max_diff(ours.data(), theirs.data())
            );
        }
        assert!(j2k::decode(b"not a picture").is_err());
        assert!(j2k::decode(&data("grey_sips.jp2")[..200]).is_err());
    }

    /// The SIZ marker segment after SOC (T.800 A.5.1): Lsiz, Rsiz, Xsiz, Ysiz, XOsiz, YOsiz, XTsiz, YTsiz, XTOsiz, YTOsiz, Csiz,
    /// then Ssiz, XRsiz, YRsiz for each component. Ssiz is the precision minus 1 (bit 7 clear for unsigned samples).
    #[cfg(feature = "jpeg2000-encode")]
    fn siz(stream: &[u8]) -> (u32, u32, u16, Vec<u8>) {
        assert_eq!(stream[..4], j2k::CODESTREAM_START);
        let be32 =
            |i: usize| u32::from_be_bytes([stream[i], stream[i + 1], stream[i + 2], stream[i + 3]]);
        let csiz = u16::from_be_bytes([stream[40], stream[41]]);
        let ssiz = (0..csiz as usize).map(|c| stream[42 + 3 * c]).collect();
        (be32(8), be32(12), csiz, ssiz)
    }

    #[cfg(feature = "jpeg2000-encode")]
    #[test]
    fn the_encoder_writes_what_the_decoder_reads_back() {
        use j2k::{J2kEncode, J2kPicture};
        // A GK-2A LRIT segment shape: 128 x 96, 10 bits, one component, lossless.
        let grey = J2kPicture {
            width: 128,
            height: 96,
            precision: 10,
            components: vec![(0..128 * 96u32)
                .map(|i| ((i * 37 + i / 128 * 11) % 1024) as u16)
                .collect()],
        };
        let stream = j2k::encode(&grey, &J2kEncode::default()).unwrap();
        assert_eq!(siz(&stream), (128, 96, 1, vec![9]));
        // The COD marker segment follows SIZ (T.800 A.6.1): Lcod 12 with no precincts, then Scod, SGcod (4 bytes) and SPcod:
        // levels, code-block width and height exponents, style, and the wavelet (1 is the reversible 5-3).
        let at = 4 + u16::from_be_bytes([stream[4], stream[5]]) as usize;
        assert_eq!(stream[at..at + 4], [0xFF, 0x52, 0, 12]);
        assert_eq!((stream[at + 9], stream[at + 13]), (5, 1));
        assert_eq!(stream[stream.len() - 2..], [0xFF, 0xD9]);
        assert_eq!(j2k::find_stream(&stream), Some((0, false)));
        assert_eq!(j2k::decode(&stream).unwrap(), grey);
        // Three components of 8 bits, lossless.
        let n = 40 * 24;
        let colour = J2kPicture {
            width: 40,
            height: 24,
            precision: 8,
            components: (0..3u32)
                .map(|c| {
                    (0..n as u32)
                        .map(|i| ((i * (c + 3)) % 256) as u16)
                        .collect()
                })
                .collect(),
        };
        let stream = j2k::encode(&colour, &J2kEncode::default()).unwrap();
        assert_eq!(siz(&stream), (40, 24, 3, vec![7, 7, 7]));
        assert_eq!(j2k::decode(&stream).unwrap(), colour);
        // Lossy 9-7: close to the picture, smooth content.
        let smooth = J2kPicture {
            width: 64,
            height: 64,
            precision: 8,
            components: vec![(0..64 * 64u32)
                .map(|i| ((i % 64) * 2 + (i / 64) * 2) as u16)
                .collect()],
        };
        let lossy = J2kEncode {
            lossless: false,
            levels: 3,
        };
        let back = j2k::decode(&j2k::encode(&smooth, &lossy).unwrap()).unwrap();
        let worst = back.components[0]
            .iter()
            .zip(&smooth.components[0])
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(worst <= 4, "worst error {worst}");
    }

    #[cfg(feature = "jpeg2000-encode")]
    #[test]
    fn the_encoder_refuses_bad_pictures() {
        use j2k::{J2kEncode, J2kPicture};
        let ok = J2kPicture {
            width: 4,
            height: 4,
            precision: 8,
            components: vec![vec![0; 16]],
        };
        let e = J2kEncode::default();
        let bad = |f: &dyn Fn(&mut J2kPicture)| {
            let mut p = ok.clone();
            f(&mut p);
            j2k::encode(&p, &e).is_err()
        };
        assert!(bad(&|p| p.precision = 0));
        assert!(bad(&|p| p.precision = 17));
        assert!(bad(&|p| p.components.clear()));
        assert!(bad(&|p| p.components[0].pop().map(drop).unwrap_or(())));
        assert!(bad(&|p| p.components[0][3] = 256));
        assert!(bad(&|p| p.width = 0));
        assert!(j2k::encode(
            &ok,
            &J2kEncode {
                lossless: true,
                levels: 33
            }
        )
        .is_err());
        assert!(j2k::encode(&ok, &e).is_ok());
    }
}

mod png {
    use super::*;
    use sdrkit_codec_image::png;

    #[test]
    fn files_carry_the_signature_and_the_fixed_iend_chunk() {
        let file = png::encode(&ramp(PixelFormat::Grey8, 5, 3)).unwrap();
        assert_eq!(&file[..8], &png::SIGNATURE);
        // IEND: length 0, type, and the CRC-32 of "IEND", AE 42 60 82 (PNG clause 11.2.5).
        assert_eq!(
            &file[file.len() - 12..],
            &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
        );
    }

    #[test]
    fn every_format_round_trips() {
        for r in [
            ramp(PixelFormat::Grey8, 17, 9),
            ramp_rgb(17, 9),
            ramp(PixelFormat::Bgra, 17, 9),
        ] {
            assert_eq!(png::decode(&png::encode(&r).unwrap()).unwrap(), r);
        }
        let pic = ramp(PixelFormat::Bgra, 4, 4).to_picture(
            "x",
            1,
            sdrkit_codec_image::PictureState::Finished,
        );
        assert_eq!(
            png::decode(&png::encode_picture(&pic).unwrap())
                .unwrap()
                .data(),
            &pic.data[..]
        );
    }

    #[test]
    fn an_independent_file_decodes_and_damage_is_an_error() {
        let r = png::decode(&data("grey.png")).unwrap();
        assert_eq!(
            (r.width(), r.height(), r.format()),
            (48, 32, PixelFormat::Grey8)
        );
        // grey.png's pixel (1, 0): (x / 8 + y / 8) is even, so 255 - 4 x.
        assert_eq!(r.data()[1], 251);
        let file = data("grey.png");
        assert!(png::decode(&file[..file.len() / 2]).is_err());
        assert!(png::decode(b"junk").is_err());
    }
}

mod line {
    use super::*;
    use sdrkit_codec_image::line::{LineImage, Overflow};
    use sdrkit_codec_image::ImageError;

    #[test]
    fn rows_grow_stop_and_scroll() {
        let mut stop = LineImage::new(PixelFormat::Grey8, 3, 2, Overflow::Stop).unwrap();
        stop.push_row(&[1, 2, 3]).unwrap();
        stop.push_row(&[4, 5, 6]).unwrap();
        assert_eq!(stop.push_row(&[7, 8, 9]), Err(ImageError::Full));
        assert!(matches!(
            stop.push_row(&[1, 2]),
            Err(ImageError::WrongLength { .. })
        ));
        assert_eq!(stop.to_raster().data(), &[1, 2, 3, 4, 5, 6]);
        let mut s = LineImage::new(PixelFormat::Bgra, 1, 2, Overflow::Scroll).unwrap();
        for v in 0..5u8 {
            s.push_row(&[v; 4]).unwrap();
        }
        assert_eq!(s.to_raster().data(), &[3, 3, 3, 3, 4, 4, 4, 4]);
        assert!(LineImage::new(PixelFormat::Grey8, 0, 2, Overflow::Stop).is_err());
    }

    #[test]
    fn slant_and_margin_fixes() {
        let mut img = LineImage::new(PixelFormat::Grey8, 4, 10, Overflow::Stop).unwrap();
        img.push_row(&[0, 100, 200, 100]).unwrap();
        img.push_row(&[0, 100, 200, 100]).unwrap();
        img.shift_row(0, 1.0);
        assert_eq!(img.row(0).unwrap(), &[100, 200, 100, 0]);
        img.shift_row(1, 0.5);
        assert_eq!(img.row(1).unwrap(), &[50, 150, 150, 50]);
        img.roll(1);
        assert_eq!(img.row(0).unwrap(), &[200, 100, 0, 100]);
        let v = img.version();
        img.deslant(-1.0); // row 1 shifts right by one
        assert!(img.version() > v);
        assert_eq!(img.row(1).unwrap(), &[50, 150, 150, 50]);
        img.clear();
        assert!(img.is_empty());
    }
}

mod strip {
    use sdrkit_codec_image::lut::{Lut, Palette};
    use sdrkit_codec_image::strip::*;
    use sdrkit_codec_image::PixelFormat;
    use std::sync::Arc;

    fn strip(ch: u16, row: u32, x: u32, v: u8) -> Strip {
        Strip {
            mask: 1 << (x / 112),
            ..Strip::new(ch, row, x, 8, 112, Arc::from(vec![v; 8 * 112]))
        }
    }

    #[test]
    fn strips_keep_their_place_and_views_compose() {
        let mut p = StripPicture::new(0, 1568, 12_000);
        for k in 0..14 {
            p.add(&strip(1, 0, k * 112, 100)).unwrap();
            p.add(&strip(2, 0, k * 112, 150)).unwrap();
        }
        p.add(&strip(1, 16, 0, 200)).unwrap(); // rows 8 to 15 lost
        assert_eq!(p.height, 24);
        assert_eq!(p.complete_strips(1, (1 << 14) - 1), 1);
        assert!(!p.planes[&1].has_row(9) && p.planes[&1].has_row(16));
        let g = render(
            &p,
            &View::Grey {
                channel: 1,
                invert: false,
            },
            Enhance::None,
        )
        .unwrap();
        assert_eq!(g.format(), PixelFormat::Grey8);
        assert_eq!(g.data()[0], 100);
        assert_eq!(g.data()[1568 * 9], 0);
        assert_eq!(g.data()[1568 * 16], 200);
        let auto = View::FirstOf(vec![
            View::Rgb([(1, false), (2, false), (3, false)]),
            View::Rgb([(1, false), (2, false), (5, false)]),
            View::FirstChannel,
        ]);
        // Only channels 1 and 2: the first channel in grey.
        assert_eq!(
            auto.resolve(&p),
            Some(View::Grey {
                channel: 1,
                invert: false
            })
        );
        let rgb = render(
            &p,
            &View::Rgb([(1, false), (2, false), (3, false)]),
            Enhance::None,
        )
        .unwrap();
        assert_eq!(&rgb.data()[..4], &[0, 150, 100, 255]);
        assert!(render(
            &p,
            &View::Grey {
                channel: 9,
                invert: false
            },
            Enhance::None
        )
        .is_none());
        let thermal = render(
            &p,
            &View::Grey {
                channel: 1,
                invert: false,
            },
            Enhance::Thermal,
        )
        .unwrap();
        assert_eq!(thermal.format(), PixelFormat::Bgra);
        assert_eq!(
            &thermal.data()[1568 * 9 * 4..1568 * 9 * 4 + 4],
            &[0, 0, 0, 255]
        );
    }

    #[test]
    fn a_strip_of_the_wrong_size_is_refused() {
        let mut p = StripPicture::new(0, 100, 100);
        let bad = Strip::new(1, 0, 0, 8, 100, Arc::from(vec![0u8; 10]));
        assert!(p.add(&bad).is_err());
        assert!(p.planes.is_empty());
        // Beyond the row limit: not kept, not an error.
        assert_eq!(
            p.add(&Strip::new(1, 200, 0, 1, 100, Arc::from(vec![0u8; 100]))),
            Ok(false)
        );
        // Wider than the picture: cut.
        assert_eq!(
            p.add(&Strip::new(1, 0, 90, 1, 20, Arc::from(vec![7u8; 20]))),
            Ok(true)
        );
        assert_eq!(p.planes[&1].data()[99], 7);
    }

    #[test]
    fn luts_and_palettes() {
        let s: Vec<u8> = (0..10_000).map(|i| (50 + i % 100) as u8).collect();
        let lut = Lut::stretch(&s, None);
        assert_eq!(lut.get(0), 0);
        assert!(lut.get(52) <= 2 && lut.get(149) >= 254);
        assert_eq!(Lut::stretch(&s[..100], None), Lut::identity());
        assert_eq!(Lut::invert().get(0), 0);
        assert_eq!(Lut::invert().get(1), 255);
        assert_eq!(Lut::invert().get(255), 1);
        assert_eq!(Lut::gamma(1.0), Lut::identity());
        assert_eq!(Lut::gamma(f64::NAN), Lut::identity());
        assert_eq!(Lut::invert().after(&Lut::invert()), Lut::identity());
        let t = Palette::thermal();
        assert_eq!(t.get(255), [255, 255, 255]);
        assert_eq!(t.get(0), [0, 0, 0]);
        assert!((138..=145).contains(&t.get(1)[2]));
        use sdrkit_core::Choice;
        assert_eq!(
            Enhance::CHOICES.iter().position(|c| c.key == "satellite"),
            Some(Enhance::Satellite.index() as usize)
        );
    }

    #[test]
    fn the_satellite_preset_repairs_a_line_and_keeps_no_data() {
        let (w, h) = (64, 32);
        let mut p: Vec<u8> = (0..w * h).map(|i| (40 + (i % w) * 2) as u8).collect();
        for x in 0..w {
            p[10 * w + x] = if x % 2 == 0 { 250 } else { 5 };
        }
        p[0] = 0;
        let out = sdrkit_codec_image::enhance::satellite(&p, w, h).unwrap();
        assert_eq!(out[0], 0);
        assert!(out[1..].iter().all(|&v| v > 0));
        let mut f: Vec<f32> = p.iter().map(|&v| v as f32).collect();
        sdrkit_codec_image::enhance::line_repair(&mut f, w, h, 0.35).unwrap();
        for x in 0..w {
            assert!((f[10 * w + x] - f[9 * w + x]).abs() < 1.0);
        }
        assert!(sdrkit_codec_image::enhance::satellite(&p[..10], w, h).is_err());
    }
}

mod geom {
    use super::*;
    use sdrkit_codec_image::geom::*;

    #[test]
    fn quarter_turns_and_mirrors() {
        let r = Raster::from_vec(PixelFormat::Grey8, 3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(rotate(&r, 1).data(), &[4, 1, 5, 2, 6, 3]);
        assert_eq!(rotate(&r, -1).data(), &[3, 6, 2, 5, 1, 4]);
        assert_eq!(rotate(&r, 3), rotate(&r, -1));
        assert_eq!(rotate(&r, 2).data(), &[6, 5, 4, 3, 2, 1]);
        assert_eq!(rotate(&r, 4), r);
        assert_eq!(flip_horizontal(&r).data(), &[3, 2, 1, 6, 5, 4]);
        assert_eq!(flip_vertical(&r).data(), &[4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn thumbnails_average_by_area() {
        let r = Raster::from_vec(
            PixelFormat::Grey8,
            4,
            2,
            vec![0, 100, 200, 0, 50, 150, 250, 50],
        )
        .unwrap();
        let t = thumbnail(&r, 2, 2);
        assert_eq!((t.width(), t.height()), (2, 1));
        assert_eq!(t.data(), &[75, 125]);
        // Three to two: the middle pixel is shared half and half.
        let r = Raster::from_vec(PixelFormat::Grey8, 3, 1, vec![0, 90, 180]).unwrap();
        assert_eq!(resize_box(&r, 2, 1).data(), &[30, 150]);
        assert_eq!(thumbnail(&r, 10, 10), r);
    }

    #[test]
    fn the_row_shift_interpolates_and_wraps() {
        let mut row = [10u8, 20, 30, 40];
        let mut scratch = Vec::new();
        shift_row(&mut row, 0.5, &mut scratch);
        assert_eq!(row, [15, 25, 35, 25]);
        let mut row = [10u8, 20, 30, 40];
        shift_row(&mut row, -1.0, &mut scratch);
        assert_eq!(row, [40, 10, 20, 30]);
        shift_row(&mut row, f64::NAN, &mut scratch);
        assert_eq!(row, [40, 10, 20, 30]);
    }
}

mod font {
    use super::*;
    use sdrkit_codec_image::font::ttf::TrueTypeFont;
    use sdrkit_codec_image::font::*;

    #[test]
    fn the_fixed_font_draws_its_rows() {
        let g = (FIXED_5X7.glyph)('a').unwrap();
        assert_eq!(g, (FIXED_5X7.glyph)('A').unwrap());
        assert_eq!(g.rows[..7], [14, 17, 17, 31, 17, 17, 17]);
        assert!(g.pixel(0, 3) && g.pixel(4, 3) && !g.pixel(0, 0));
        assert_eq!(text_width(&FIXED_5X7, "AB", 2), 22);
        let mut r = Raster::new(PixelFormat::Grey8, 12, 7).unwrap();
        draw_text(&mut r, &FIXED_5X7, "T", 0, 0, 1, &[255]);
        assert_eq!(&r.row(0).unwrap()[..6], &[255, 255, 255, 255, 255, 0]);
        assert_eq!(&r.row(6).unwrap()[..6], &[0, 0, 255, 0, 0, 0]);
        // Off the picture: nothing drawn, no panic.
        draw_text(&mut r, &FIXED_5X7, "W", -100, 50, 3, &[255]);
        // A character the font lacks shows as '?'.
        assert_eq!(
            FIXED_5X7.glyph_or_missing('€'),
            (FIXED_5X7.glyph)('?').unwrap()
        );
    }

    #[test]
    fn hell_glyphs_fit_the_cell_and_cover_the_scripts() {
        for c in (32u8..127).map(|b| b as char).chain("ÀéÑöБЖЯΔΩАΣ".chars()) {
            if let Some(g) = hell::glyph(c) {
                assert_eq!(g.height, 5, "{c}");
                assert!(g.width >= 1 && g.width <= 6, "{c}");
                for x in 0..g.width {
                    assert!(g.column(x) < 1 << 5, "{c}");
                }
            }
        }
        assert!(hell::glyph('Ł').is_none());
        assert!(hell::glyph('Ж').is_some() && hell::glyph('ж').is_some());
        // 'A' drawn as columns, bottom row first: the left column is ink on its lower four rows.
        let a = hell::glyph('A').unwrap();
        assert_eq!(a.column(0), 0b01111);
        // 'É': the E in the lower four rows and an acute accent above the middle.
        let e = hell::glyph('É').unwrap();
        assert_eq!(e.rows[0], 0b0001);
        assert_eq!(e.rows[1], 0b1111);
        // 'А' (Cyrillic) is drawn as the Latin A.
        assert_eq!(hell::glyph('А'), hell::glyph('A'));
        assert_eq!(HELL_5X5.glyph_or_missing(' ').width, 3);
    }

    // A TrueType font with one glyph: a square from (100, 0) to (600, 500) for 'A', 1000 units to the em, descender -200.
    fn tiny_font() -> Vec<u8> {
        let be16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_be_bytes());
        let mut head = vec![0u8; 54];
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        let mut maxp = vec![0u8; 6];
        maxp[4..6].copy_from_slice(&2u16.to_be_bytes());
        let mut hhea = vec![0u8; 36];
        hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        let mut sub = Vec::new();
        for x in [
            4u16,
            32,
            0,
            4,
            4,
            1,
            0,
            b'A' as u16,
            0xFFFF,
            0,
            b'A' as u16,
            0xFFFF,
        ] {
            be16(&mut sub, x);
        }
        for x in [(1i32 - b'A' as i32) as u16, 1, 0, 0] {
            be16(&mut sub, x);
        }
        let mut cmap = Vec::new();
        for x in [0u16, 1, 3, 1] {
            be16(&mut cmap, x);
        }
        cmap.extend_from_slice(&12u32.to_be_bytes());
        cmap.extend_from_slice(&sub);
        let mut glyf = Vec::new();
        for x in [1i16, 100, 0, 600, 500, 3, 0] {
            be16(&mut glyf, x as u16);
        }
        glyf.extend_from_slice(&[1, 1, 1, 1]);
        for x in [100i16, 0, 500, 0, 0, 500, 0, -500] {
            be16(&mut glyf, x as u16);
        }
        let mut loca = Vec::new();
        for x in [0u16, 0, (glyf.len() / 2) as u16] {
            be16(&mut loca, x);
        }
        let tables: [(&[u8; 4], Vec<u8>); 6] = [
            (b"cmap", cmap),
            (b"glyf", glyf),
            (b"head", head),
            (b"hhea", hhea),
            (b"loca", loca),
            (b"maxp", maxp),
        ];
        let mut font = Vec::new();
        font.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        for x in [6u16, 0, 0, 0] {
            be16(&mut font, x);
        }
        let mut offset = 12 + 16 * tables.len();
        let mut body = Vec::new();
        for (tag, t) in &tables {
            font.extend_from_slice(*tag);
            font.extend_from_slice(&[0; 4]);
            font.extend_from_slice(&(offset as u32).to_be_bytes());
            font.extend_from_slice(&(t.len() as u32).to_be_bytes());
            body.extend_from_slice(t);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            offset = 12 + 16 * tables.len() + body.len();
        }
        font.extend_from_slice(&body);
        font
    }

    #[test]
    fn a_truetype_glyph_renders_where_its_outline_is() {
        let f = TrueTypeFont::parse(tiny_font()).unwrap();
        assert_eq!(f.glyph_index('A'), Some(1));
        assert_eq!(f.glyph_index('B'), None);
        let m = f.render('A', 10).unwrap();
        assert_eq!((m.width, m.height), (5, 10));
        for row in 0..10 {
            let ink = (3..=7).contains(&row);
            for x in 0..5 {
                assert_eq!(m.data[(row * 5 + x) as usize] == 255, ink, "row {row}");
            }
        }
        assert!(f.render('B', 10).is_none());
        assert!(TrueTypeFont::parse(b"OTTO\0\0\0\0".to_vec()).is_err());
        let mut cut = tiny_font();
        cut.truncate(100);
        assert!(TrueTypeFont::parse(cut).is_err());
    }
}

mod history {
    use sdrkit_codec_image::history::PictureHistory;

    #[test]
    fn numbers_grow_and_the_oldest_go() {
        let mut h = PictureHistory::new(2);
        h.seen_id(10);
        let a = h.next_id();
        assert_eq!(a, 11);
        h.finish(a, "a");
        for name in ["b", "c"] {
            let id = h.next_id();
            h.finish(id, name);
        }
        assert_eq!(h.len(), 2);
        assert_eq!(h.latest(), Some(&"c"));
        assert_eq!(h.get(11), None);
        assert_eq!(h.finished(), 3);
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.next_id(), 14);
    }
}

mod component {
    use sdrkit_codec_image::component::{
        Layout, NamedView, Pictures, PicturesCommand, PicturesSetting,
    };
    use sdrkit_codec_image::strip::{Enhance, Strip, View};
    use sdrkit_codec_image::{Codec, PixelFormat};
    use sdrkit_core::command::CommandSet;
    use sdrkit_core::media::Picture;
    use sdrkit_core::{Choice, StateBlob, Value, Values};
    use sdrkit_graph::stream::{Buffer, Data};
    use sdrkit_graph::{Component, Format, Io, Kind, Timed};
    use std::sync::Arc;

    use sdrkit_codec_image::component::PictureFields;

    fn run(c: &mut Pictures, strips: Vec<Strip>, tick: bool) -> Vec<Picture> {
        let mut input = Buffer::new(&Kind::typed::<Strip>());
        let items: Vec<Timed<Strip>> = strips
            .into_iter()
            .map(|value| Timed { time: 0.0, value })
            .collect();
        input.data = Data::Items(Some(Box::new(items)));
        let inputs = [input];
        let formats = [
            Format::new(Kind::typed::<Strip>(), 0.0),
            Format::new(Kind::typed::<Picture>(), 0.0),
        ];
        let mut outs = [Buffer::new(&Kind::typed::<Picture>())];
        {
            let mut io = Io::standalone(&inputs, &formats, &[0], &[1], &mut outs);
            io.set_tick(tick);
            c.process(&mut io);
        }
        match &outs[0].data {
            Data::Items(Some(b)) => b
                .as_any()
                .downcast_ref::<Vec<Timed<Picture>>>()
                .unwrap()
                .iter()
                .map(|t| t.value.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn strip(ch: u16, row: u32, v: u8) -> Strip {
        Strip {
            instrument: "MSU",
            ..Strip::new(ch, row, 0, 4, 16, Arc::from(vec![v; 64]))
        }
    }

    fn component() -> Pictures {
        Pictures::new(vec![Layout {
            name: "MSU",
            width: 16,
            max_rows: 100,
            full_mask: 1,
            views: vec![
                NamedView::new(
                    "ch1",
                    "Channel 1",
                    View::Grey {
                        channel: 1,
                        invert: false,
                    },
                ),
                NamedView::new(
                    "rgb",
                    "Colour",
                    View::Rgb([(1, false), (2, false), (3, false)]),
                ),
            ],
        }])
        .unwrap()
    }

    /// Applies `v` to the setting `key` after the schema check, as a host does.
    fn set(c: &mut Pictures, key: &str, v: Value) -> bool {
        let s = c.info().settings;
        let Ok(id) = s.id(key) else {
            return false;
        };
        s.check(id, &v).is_ok() && c.apply(id, &v).is_ok()
    }

    #[test]
    fn strips_in_pictures_out_on_ticks() {
        let mut c = component();
        c.configure(&[]).unwrap();
        assert!(run(&mut c, vec![strip(1, 0, 90)], false).is_empty());
        let out = run(&mut c, vec![], true);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].codec, Codec::Pixels(PixelFormat::Grey8));
        assert_eq!(out[0].name.as_deref(), Some("MSU-ch1"));
        assert_eq!(out[0].data.len(), 16 * 4);
        // No change: nothing new. A change of view: drawn at once.
        assert!(run(&mut c, vec![], true).is_empty());
        assert!(set(&mut c, "view", Value::Choice(1)));
        let out = run(&mut c, vec![], true);
        assert_eq!(out[0].codec, Codec::Pixels(PixelFormat::Bgra));
        // Growth: drawn after frame_every ticks.
        assert!(set(&mut c, "frame_every", Value::I64(2)));
        run(&mut c, vec![strip(1, 4, 50)], true);
        let out = run(&mut c, vec![], true);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].layout.as_ref().unwrap().height, 8);
        let r = c.results().unwrap();
        assert_eq!(r.fields().at(&PictureFields::ROWS), Some(8));
        assert_eq!(r.fields().at(&PictureFields::STRIPS), Some(2));
    }

    #[test]
    fn settings_are_strict() {
        let mut c = component();
        assert!(!set(&mut c, "view", Value::Choice(9)));
        assert!(!set(&mut c, "enhance", Value::Choice(99)));
        assert!(!set(&mut c, "frame_every", Value::I64(0)));
        assert!(!set(&mut c, "frame_every", Value::F64(f64::NAN)));
        assert!(!set(&mut c, "follow", Value::Text("maybe".into())));
        assert!(!set(&mut c, "save_png", Value::Bool(true)));
        assert!(set(
            &mut c,
            "enhance",
            Value::Choice(Enhance::Stretch.index())
        ));
        assert_eq!(
            c.values().value(PicturesSetting::Enhance.id()),
            Some(&Value::Choice(Enhance::Stretch.index()))
        );
    }

    #[test]
    fn a_view_of_another_instrument_is_refused() {
        let ch1 = |key: &'static str| {
            NamedView::new(
                key,
                key,
                View::Grey {
                    channel: 1,
                    invert: false,
                },
            )
        };
        let layout = |name, views| Layout {
            name,
            width: 16,
            max_rows: 10,
            full_mask: 1,
            views,
        };
        let mut c = Pictures::new(vec![
            layout("A", vec![ch1("a1")]),
            layout("B", vec![ch1("b1"), ch1("a1")]),
        ])
        .unwrap();
        let s = c.info().settings;
        let view = s.id("view").unwrap();
        assert_eq!(s.setting(view).unwrap().field.choice_index("b1"), Some(1));
        assert!(!set(&mut c, "view", Value::Choice(1)));
        assert!(set(&mut c, "instrument", Value::Choice(1)));
        assert!(set(&mut c, "view", Value::Choice(1)));
        assert!(set(&mut c, "view", Value::Choice(0)));
        assert_eq!(c.values().value(view), Some(&Value::Choice(0)));
    }

    #[test]
    fn state_round_trips_and_clear_starts_again() {
        let mut c = component();
        run(
            &mut c,
            vec![
                strip(1, 0, 90),
                strip(2, 8, 30),
                Strip {
                    mask: 0,
                    ..strip(1, 8, 70)
                },
            ],
            false,
        );
        let state = c.save_state().unwrap();
        let mut d = component();
        d.restore_state(&state).unwrap();
        let (a, b) = (&c.pictures()["MSU"], &d.pictures()["MSU"]);
        assert_eq!(
            (&a.planes, a.height, a.strips, a.id),
            (&b.planes, b.height, b.strips, b.id)
        );
        assert!(!b.planes[&2].has_row(0) && b.planes[&2].has_row(8));
        // Another owner, another version or damaged bytes are refused and change nothing.
        let other_version = StateBlob::new("image.pictures", 2, state.bytes().to_vec());
        assert!(d.restore_state(&other_version).is_err());
        assert!(d
            .restore_state(&StateBlob::new("x", 1, b"{}".to_vec()))
            .is_err());
        assert!(d
            .restore_state(&StateBlob::new("image.pictures", 1, b"[1]".to_vec()))
            .is_err());
        assert_eq!(d.pictures().len(), 1);
        d.invoke(
            PicturesCommand::Clear.id(),
            &Values::defaults(&Default::default()),
        )
        .unwrap();
        assert!(d.pictures().is_empty());
        assert!(d.save_state().is_none());
    }

    #[test]
    fn the_components_are_registered() {
        let mut r = sdrkit_graph::Registry::new();
        sdrkit_codec_image::register(&mut r);
        for kind in ["image.pictures", "image.decode", "image.pixels"] {
            assert_eq!(r.make(kind).unwrap().info().name, kind);
        }
        let kinds: Vec<&str> = r.catalog().adapters().iter().map(|a| a.kind).collect();
        assert!(kinds.contains(&"image.decode") && kinds.contains(&"image.pixels"));
    }
}

mod adapters {
    use sdrkit_codec_image::{png, Codec, Decode, PackedPixels, PixelConvert, PixelFormat, Raster};
    use sdrkit_core::media::Picture;
    use sdrkit_graph::stream::{Buffer, Data};
    use sdrkit_graph::{Component, Format, Io, Kind, Timed};
    use std::sync::Arc;

    fn run(c: &mut dyn Component, pictures: Vec<Picture>) -> Vec<Picture> {
        let kind = Kind::typed::<Picture>();
        let mut input = Buffer::new(&kind);
        let items: Vec<Timed<Picture>> = pictures
            .into_iter()
            .map(|value| Timed { time: 1.0, value })
            .collect();
        input.data = Data::Items(Some(Box::new(items)));
        let inputs = [input];
        let formats = [Format::new(kind, 0.0), Format::new(kind, 0.0)];
        let mut outs = [Buffer::new(&kind)];
        {
            let mut io = Io::standalone(&inputs, &formats, &[0], &[1], &mut outs);
            c.process(&mut io);
        }
        match &outs[0].data {
            Data::Items(Some(b)) => b
                .as_any()
                .downcast_ref::<Vec<Timed<Picture>>>()
                .unwrap()
                .iter()
                .map(|t| t.value.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn decode_returns_pixels_and_drops_damage() {
        let r = Raster::from_vec(PixelFormat::Grey8, 3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let file: Arc<[u8]> = png::encode(&r).unwrap().into();
        let compressed = Picture {
            codec: Codec::Png,
            layout: None,
            name: Some("a".into()),
            id: 1,
            state: sdrkit_codec_image::PictureState::Finished,
            data: file,
        };
        let damaged = Picture {
            data: Arc::from(&b"junk"[..]),
            ..compressed.clone()
        };
        let raw = r.to_picture("raw", 1, sdrkit_codec_image::PictureState::Finished);
        let out = run(
            &mut Decode::default(),
            vec![compressed, damaged, raw.clone()],
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].codec, Codec::Pixels(PixelFormat::Grey8));
        assert_eq!(&out[0].data[..], r.data());
        assert_eq!(out[0].name.as_deref(), Some("a"));
        // A raw picture passes and shares its bytes.
        assert!(Arc::ptr_eq(&out[1].data, &raw.data));
    }

    #[test]
    fn pixels_convert_and_share_when_they_fit() {
        let r = Raster::from_vec(PixelFormat::Grey8, 2, 1, vec![10, 200]).unwrap();
        let grey = r.to_picture("", 1, sdrkit_codec_image::PictureState::Finished);
        let out = run(
            &mut PixelConvert::new(PackedPixels::Bgra),
            vec![grey.clone()],
        );
        assert_eq!(out[0].codec, Codec::Pixels(PixelFormat::Bgra));
        assert_eq!(&out[0].data[..], &[10, 10, 10, 255, 200, 200, 200, 255]);
        let same = run(
            &mut PixelConvert::new(PackedPixels::Grey8),
            vec![grey.clone()],
        );
        assert!(Arc::ptr_eq(&same[0].data, &grey.data));
    }

    #[test]
    fn a_decoded_picture_shares_its_pixels_with_the_raster() {
        let r = Raster::from_vec(PixelFormat::Grey8, 4, 4, vec![9; 16]).unwrap();
        let back = png::decode(&png::encode(&r).unwrap()).unwrap();
        let p = back.to_picture("x", 1, sdrkit_codec_image::PictureState::Finished);
        assert!(Arc::ptr_eq(&p.data, back.shared()));
        let again = Raster::from_picture(&p).unwrap();
        assert!(Arc::ptr_eq(again.shared(), &p.data));
    }
}

// Counts the allocations of the calling thread only, so the other tests of this program, which run at the same time on other
// threads, do not count. (A counting allocator needs `unsafe impl GlobalAlloc`; the library itself forbids `unsafe`.)
mod alloc {
    use super::*;
    use sdrkit_codec_image::colour::{Converter, Range, BT601};
    use sdrkit_codec_image::jpeg::{self, StandardTables};
    use sdrkit_codec_image::line::{LineImage, Overflow};
    use sdrkit_codec_image::lut::{Lut, Palette};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    struct Counting;

    thread_local! {
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    fn count(f: impl FnOnce()) -> usize {
        let before = ALLOCS.with(Cell::get);
        f();
        ALLOCS.with(Cell::get) - before
    }

    #[test]
    fn steady_state_paths_do_not_allocate() {
        // Line images: 1,000 rows into a sized picture that scrolls, with slant fixes.
        let mut img =
            LineImage::with_capacity(PixelFormat::Grey8, 1809, 300, Overflow::Scroll).unwrap();
        let row = vec![77u8; 1809];
        img.push_row(&row).unwrap();
        img.shift_row(0, 0.5);
        assert_eq!(
            count(|| {
                for k in 0..1000 {
                    img.push_row(&row).unwrap();
                    img.next_row().unwrap()[0] = k as u8;
                    img.shift_row(0, 0.25);
                }
            }),
            0
        );
        // The coefficient layer and the transforms.
        let t = StandardTables::get();
        let mut blk = [0i16; 64];
        blk[0] = 40;
        blk[1] = -7;
        blk[9] = 3;
        let mut buf = Vec::with_capacity(100_000);
        let q = jpeg::tables::LUMA_Q;
        let (mut f, mut px, mut zz) = ([0f32; 64], [0u8; 64], [0i16; 64]);
        assert_eq!(
            count(|| {
                let mut w = jpeg::EntropyWriter::new(&mut buf, true);
                let mut pred = 0;
                for _ in 0..1000 {
                    jpeg::encode_block(&mut w, &t.dc_luma, &t.ac_luma, &mut pred, &blk).unwrap();
                }
                w.align();
                let mut r = jpeg::EntropyReader::new(&buf, true);
                let mut pred = 0;
                for _ in 0..1000 {
                    jpeg::decode_block(&mut r, &t.dc_luma, &t.ac_luma, &mut pred, &mut zz).unwrap();
                    jpeg::dequantise(&zz, &q, &mut f);
                    jpeg::idct_8x8(&f, &mut px, 8);
                    jpeg::fdct_8x8(&px, 8, &mut f);
                    jpeg::quantise(&f, &q, &mut zz);
                }
            }),
            0
        );
        // Colour conversion, look-up tables and palettes over rows.
        let conv = Converter::new(BT601, Range::Full);
        let src = vec![100u8; 640 * 3];
        let mut dst = vec![0u8; 640 * 4];
        let lut = Lut::gamma(0.7);
        let pal = Palette::thermal();
        assert_eq!(
            count(|| {
                for _ in 0..1000 {
                    conv.rgb_row(&src, &mut dst[..640 * 3]);
                    conv.ycbcr_row(&src, &mut dst[..640 * 3]);
                    lut.apply(&mut dst);
                    pal.to_bgra(&src[..640], &mut dst);
                }
            }),
            0
        );
    }
}
