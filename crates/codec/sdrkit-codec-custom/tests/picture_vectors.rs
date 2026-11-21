//! The picture codec against the codec vectors: exact decoding, exact encoding under the unfused rule,
//! and the quality bounds.

mod common;

use sdrkit_codec_custom::picture::{levels_for, Decoder, Encoder, Header};

struct Stream {
    name: &'static str,
    input: &'static str,
    budget: usize,
    w: u32,
    h: u32,
    l: u32,
    top: i32,
    d: u32,
    bytes: usize,
    psnr: f64,
}

const STREAMS: &[Stream] = &[
    Stream {
        name: "synth_8x8_full",
        input: "synth_8x8",
        budget: 1_000_000,
        w: 8,
        h: 8,
        l: 3,
        top: 7,
        d: 749,
        bytes: 95,
        psnr: 23.540,
    },
    Stream {
        name: "synth_17x3_full",
        input: "synth_17x3",
        budget: 1_000_000,
        w: 17,
        h: 3,
        l: 1,
        top: 7,
        d: 611,
        bytes: 78,
        psnr: 20.790,
    },
    Stream {
        name: "synth_64x48_full",
        input: "synth_64x48",
        budget: 1_000_000,
        w: 64,
        h: 48,
        l: 5,
        top: 10,
        d: 23046,
        bytes: 1879,
        psnr: 34.824,
    },
    Stream {
        name: "synth_64x48_b200",
        input: "synth_64x48",
        budget: 200,
        w: 64,
        h: 48,
        l: 5,
        top: 10,
        d: 4141,
        bytes: 196,
        psnr: 23.455,
    },
    Stream {
        name: "synth_1920x1080_full",
        input: "synth_1920x1080",
        budget: 1_000_000_000,
        w: 1920,
        h: 1080,
        l: 5,
        top: 11,
        d: 7_249_149,
        bytes: 273_089,
        psnr: 46.897,
    },
    Stream {
        name: "synth_1920x1080_P0cap",
        input: "synth_1920x1080",
        budget: 258_438,
        w: 1920,
        h: 1080,
        l: 5,
        top: 11,
        d: 7_127_486,
        bytes: 258_434,
        psnr: 46.840,
    },
];

fn header(s: &Stream) -> Header {
    Header {
        width: s.w,
        height: s.h,
        levels: s.l,
        top_plane: s.top,
        decisions: s.d,
    }
}

fn wanted() -> Vec<&'static Stream> {
    STREAMS.iter().collect()
}

#[test]
fn decoder_matches_every_stream() {
    let Some(dir) = common::vectors() else { return };
    let pics = common::Pictures::find(&dir);
    let mut dec = Decoder::new();
    let mut rgb = Vec::new();
    for s in wanted() {
        let stream = std::fs::read(pics.stream(s.name)).unwrap();
        assert_eq!(stream.len(), s.bytes, "{}", s.name);
        let n = dec.decode(&header(s), &stream, &mut rgb).unwrap();
        let (_, _, want) = common::read_ppm(&pics.dir.join(format!("{}_decoded.ppm", s.name)));
        let (_, _, input) = common::read_ppm(&pics.dir.join(format!("{}_input.ppm", s.input)));
        let bad = rgb.iter().zip(&want).filter(|(a, b)| a != b).count();
        eprintln!(
            "{:24} decisions {n} of {}, {bad} bytes differ, PSNR {:.3}",
            s.name,
            s.d,
            common::psnr(&rgb, &input)
        );
        assert_eq!(bad, 0, "{}", s.name);
        assert!(
            (common::psnr(&rgb, &input) - s.psnr).abs() < 0.0005,
            "{}",
            s.name
        );
    }
}

#[test]
fn encoder_matches_every_stream() {
    let Some(dir) = common::vectors() else { return };
    let pics = common::Pictures::find(&dir);
    let mut enc = Encoder::new();
    let mut out = Vec::new();
    for s in wanted() {
        let (w, h, input) = common::read_ppm(&pics.dir.join(format!("{}_input.ppm", s.input)));
        assert_eq!((w as u32, h as u32), (s.w, s.h));
        let hd = enc.encode(&input, s.w, s.h, s.budget, &mut out).unwrap();
        let want = std::fs::read(pics.stream(s.name)).unwrap();
        eprintln!(
            "{:24} header {:?}, {} bytes, exact {}",
            s.name,
            hd,
            out.len(),
            out == want
        );
        assert_eq!(hd, header(s), "{}", s.name);
        assert_eq!(out, want, "{}", s.name);
    }
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// The two smallest vectors in the spec text, as hex.
#[test]
fn spec_text_vectors() {
    let input = hex("3CBEE64ED9E661E5E674DDE687C3E699A7E6AC96E6BF9BE63CB1D414141E14141E14141E14141E999AC9AC8AC7BF8FC53CA5B54EC0B0C1DC6574C4A887AAA3998E9FAC7D9ABF82963C98AA4EB3A490CC7F74B797879E909C7F87C16070BF767C3C8CC14EA7B861B3B074ABA787919E997595AC648DBF69844D5F324D65324D77324D7B324D6B324D5E324D67324D7832555C32555132555B32556C32507D52555A32555132555D324F4F2445341B513926625837676C3C6160375B48305C4532");
    let stream = hex("0002B7F4F7DD544F59F226354F60144FAE0BFD4448E411680BD2676430DDBCC1087ABE1BAD08F0846055A906D616EC7ED7A820691EA45F361676B3C74AE5B18C3F84D1EAEECE7FD13FDAB5BC5E5C3BE42E8D95C0EB305A3386153CF3D4453A");
    let decoded = hex("55B5CE71C9DD8DD1DD9ACCDA9CBAD8A1A7D3AE99D4BF9DDF4CAEC1002E39002A27002423011C2E9A9BBEA98DBCBC91C640A5AC5EB8B29ADFBE8CBA9A90A6A2988F9EAA7F98BD819F409A995BAD9E88C99E85B2868B9D8D9086869B7279B37A844C8E8964A39575B08E7FAB8885978587837A9078719E7C773A62533E66524A735452725254644A565743645746725F4E505C3F4A56374F5C3959664266724E54593957503562553E504E293C3A1543411D595C36636C435B623B514D2E544A2F");
    let mut out = Vec::new();
    let hd = Encoder::new()
        .encode(&input, 8, 8, 1_000_000, &mut out)
        .unwrap();
    assert_eq!(
        hd,
        Header {
            width: 8,
            height: 8,
            levels: 3,
            top_plane: 7,
            decisions: 749
        }
    );
    assert_eq!(out, stream);
    let mut rgb = Vec::new();
    assert_eq!(Decoder::new().decode(&hd, &stream, &mut rgb).unwrap(), 749);
    assert_eq!(rgb, decoded);

    let input = hex("3CBEE644CBE64DD7E656E0E65FE5E668E5E670E0E679D6E682CAE68BBCE694AEE69DA3E6A59AE6AE96E6B797E6C09CE6C9A6E63C9CAA44AAA74DB6A47EC889D9DE50BAD76674BF9779B59782A9948B9B91948D8EB76B75E9414CB86C7CB77583C07B81C9847E505C32507732506032506632507532505A32507132506B32505C32507732506032506632507532505A32507132506C32505C32");
    let stream = hex("958CD5C0AFC4412A06E64E74FA2AE2F7936B4B28A658ADE5DD54EA9CC0C9BE8DF7CD23AF15FFEAC8DE5C6A57643DB54C7FDD5BD4F84CE663C7FCA163CE4A90FA284BC9B96A6F44A7029F05E20F4F");
    let decoded = hex("49BCD053C6D160D2CC72D9BE88DCA890DCAA89DBC289D3CD91C7CC9CB9C9A8AAC5B79CC1C890BBCF8DBBCB94C0CE9CC3D8A5C53F9C9D4BA8A257B1A075C7A6A3E7B39BDAA778BD9C76B5A27BAAA0819D9C8A90988C7D8C8C677BA0758AA07E91A58595AF8D993667424776503C6740426A3E4E733F3E642F487241456D4041623956704B4E5E3F575F4363664A5457395E66455C634357573C");
    let hd = Encoder::new()
        .encode(&input, 17, 3, 1_000_000, &mut out)
        .unwrap();
    assert_eq!(
        hd,
        Header {
            width: 17,
            height: 3,
            levels: 1,
            top_plane: 7,
            decisions: 611
        }
    );
    assert_eq!(out, stream);
    Decoder::new().decode(&hd, &stream, &mut rgb).unwrap();
    assert_eq!(rgb, decoded);
}

#[test]
fn level_rule() {
    assert_eq!(levels_for(8, 8), 3);
    assert_eq!(levels_for(17, 3), 1);
    assert_eq!(levels_for(1920, 1080), 5);
    assert_eq!(levels_for(1, 1), 1);
}

// The P0cap layer table: the decode of the first N layers of the padded stream.
#[test]
fn layer_prefixes_match() {
    let Some(dir) = common::vectors() else { return };
    let pics = common::Pictures::find(&dir);
    let padded = std::fs::read(pics.stream("synth_1920x1080_P0cap_padded")).unwrap();
    let hd = Header {
        width: 1920,
        height: 1080,
        levels: 5,
        top_plane: 11,
        decisions: 7_127_486,
    };
    let (_, _, input) = common::read_ppm(&pics.dir.join("synth_1920x1080_input.ppm"));
    let layers = [
        (1, 952, 18.117),
        (2, 1666, 20.678),
        (3, 3332, 22.335),
        (4, 5950, 25.128),
        (5, 12376, 28.343),
        (6, 31516, 32.026),
        (7, 104786, 40.460),
        (8, 258438, 46.840),
    ];
    let mut dec = Decoder::new();
    let mut rgb = Vec::new();
    for (n, bytes, psnr) in layers {
        dec.decode(&hd, &padded[..bytes], &mut rgb).unwrap();
        let (_, _, want) = common::read_ppm(
            &pics.dir.join(format!("synth_1920x1080_P0cap_layers{n}_decoded.ppm")),
        );
        let p = common::psnr(&rgb, &input);
        eprintln!("layers {n}: PSNR {p:.3} (table {psnr})");
        assert!(rgb == want, "layer {n}");
        assert!((p - psnr).abs() < 0.0005);
    }
}

fn crop(input: &[u8], iw: usize, w: usize, h: usize) -> Vec<u8> {
    let mut o = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        o.extend_from_slice(&input[3 * y * iw..3 * (y * iw + w)]);
    }
    o
}

// The codec acceptance tests: prefixes of the full stream do not lose quality, a budget of 30000 bytes, short prefixes, and
// other sizes. The other sizes use crops of the 1920 x 1080 input, so only the size test and "no failure" apply.
#[test]
fn acceptance_bounds() {
    let Some(dir) = common::vectors() else { return };
    let pics = common::Pictures::find(&dir);
    let (iw, ih, input) = common::read_ppm(&pics.dir.join("synth_1920x1080_input.ppm"));
    let full = std::fs::read(pics.stream("synth_1920x1080_full")).unwrap();
    let hd = Header {
        width: 1920,
        height: 1080,
        levels: 5,
        top_plane: 11,
        decisions: 7_249_149,
    };
    let mut dec = Decoder::new();
    let mut rgb = Vec::new();
    let mut last = 0.0;
    for n in [
        500, 1000, 2000, 4000, 8000, 16000, 32000, 64000, 128000, 256000,
    ] {
        dec.decode(&hd, &full[..n], &mut rgb).unwrap();
        let p = common::psnr(&rgb, &input);
        assert!(p >= last - 0.05, "prefix {n}: {p} after {last}");
        last = p;
    }
    let mut enc = Encoder::new();
    let mut out = Vec::new();
    {
        let hd30 = enc
            .encode(&input, iw as u32, ih as u32, 30000, &mut out)
            .unwrap();
        assert!(out.len() <= 30000);
        dec.decode(&hd30, &out, &mut rgb).unwrap();
        assert!(common::psnr(&rgb, &input) > 20.0);
        for n in [0, 3, 10, 100] {
            dec.decode(&hd30, &out[..n], &mut rgb).unwrap();
            assert_eq!(rgb.len(), iw * ih * 3);
        }
    }
    for (w, h) in [(37, 23), (64, 48), (1, 1), (17, 3), (200, 151)] {
        let pic = crop(&input, iw, w, h);
        let hd = enc
            .encode(&pic, w as u32, h as u32, 1_000_000, &mut out)
            .unwrap();
        dec.decode(&hd, &out, &mut rgb).unwrap();
        assert_eq!(rgb.len(), w * h * 3);
        eprintln!("crop {w}x{h}: PSNR {:.2}", common::psnr(&rgb, &pic));
    }
}

#[test]
fn bad_input_is_an_error() {
    let mut out = Vec::new();
    assert!(Encoder::new().encode(&[0; 5], 1, 1, 100, &mut out).is_err());
    assert!(Encoder::new().encode(&[], 0, 1, 100, &mut out).is_err());
    let mut rgb = Vec::new();
    let hd = Header {
        width: 4,
        height: 4,
        levels: 9,
        top_plane: 3,
        decisions: 10,
    };
    assert!(Decoder::new().decode(&hd, &[1, 2, 3], &mut rgb).is_err());
    let hd = Header { levels: 2, ..hd };
    assert_eq!(Decoder::new().decode(&hd, &[], &mut rgb).unwrap(), 0);
    assert!(rgb.iter().all(|&v| v == 128));
}

#[test]
fn random_streams_never_panic() {
    let mut s = 5u32;
    let mut next = || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        s >> 8
    };
    let mut dec = Decoder::new();
    let mut rgb = Vec::new();
    for _ in 0..200 {
        let hd = Header {
            width: 1 + next() % 70,
            height: 1 + next() % 50,
            levels: 1 + next() % 8,
            top_plane: (next() % 16) as i32 - 1,
            decisions: next() % 5000,
        };
        let stream: Vec<u8> = (0..next() % 400).map(|_| next() as u8).collect();
        dec.decode(&hd, &stream, &mut rgb).unwrap();
        assert_eq!(rgb.len(), (hd.width * hd.height * 3) as usize);
    }
}
