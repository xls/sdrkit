//! The burst waveforms of third-generation HF link automation (MIL-STD-188-141B Appendix C, section C.5.1): the known
//! sequences, Walsh symbols on PN sequences, the codes and interleavers of BW0 to BW4, the BW2 PN generator, and the symbols of
//! each burst.

use crate::serial::psk8;
use sdrkit_algo_fec::conv::{encode_tail_biting, ConvCode, Encoder};
use sdrkit_algo_fec::increments::{interleaver, Walk};
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;

/// The TLC/AGC sequence, 256 tribits (Table C-V).
pub const TLC: &str = "2616163060115006262162327643023527515176171544072262226333773245\
0747772316765705107624027554151567302766407432266747202721546232\
1607112622022367171715772220434206760507174123467220644664226534\
2357710003120162744325456425622470623725424155361132757073500120";
/// The BW0 preamble, 384 tribits (Table C-VI).
pub const BW0_PRE: &str = "7777543112027220134753774310115216004762236051761617666173047122\
3367717315033452525317215761253536207566014254117006675637402636\
4510045547151567335222723304141360726150141170740245300312646526\
0073534062723376710067315502342774521610471612403654544612513627\
2674730150534507327032706167714267742727376326566366410626415543\
3463524117537165466234233741445461346174135265542151612714423473";
/// The BW0 and BW1 PN, 256 tribits (Table C-IX).
pub const PN01: &str = "0243364576705543543707626246724755707333733142370277351014050000\
6501276552733321256134210123647522627652465472510077354214270340\
0077354214270340105260351051525632371220713642627437672317415154\
7112367766122417755477507375775066613444033214545311125171572006";
/// The BW1 preamble, 576 tribits (Table C-XI).
pub const BW1_PRE: &str = "4473771047166550640601622735154556602040702637152323711700074523\
2373103425666523276766101265651435265652520026704225025152123417\
4105114162652231710604623361504251467215414771541702624711306421\
7325404454677267114605166123253452041465263223077022166605134516\
7222137570665724030614340154515765647701435615715310550422252453\
6263504007351455252663760271435261206517106304760504157046617051\
6064661463321441467262461050405442527244736475656553234757271553\
0354213715443755547077105470467100345470332220327003053714235357\
1331011651515070257677310314235140217117450100366663732603751016";
/// The BW3 preamble, 640 tribits (Table C-XV).
pub const BW3_PRE: &str = "7235750675356354714577350516031027624420072023741130136301315456\
3252606604637170621552253332147002025757753622164471547275611500\
1405347362254027627165532325773732224007545350333064656427626610\
5107142760167561175100103174544543204166276446223035211627622571\
2556407172325202220366625556002367657224552573270301416257016016\
5136542044212611011357504315300456757615512750363527066065427560\
4170474731237226751667256403047162543606653344512673130774625207\
3676363144637764452254745626024633435507631602204267720513763720\
1635103754672400227126337177412254033561045671203162461567726376\
7234446043771571345663234401403673503071054544376115011146072543";
/// The BW3 PN, 32 tribits (Table C-XVII).
pub const PN3: &str = "00000246040406420000135726263175";
/// The BW4 PN, 1280 tribits (Table C-XIX).
pub const PN4: &str = "5207132504533073532374750410576526417166012216763713506466522352\
3572546061075202662460207301502162274311463513616330557174761717\
1537375336633466410553312621153247715573720407056414307630051344\
2151303023667600262771035100012021703635752652343155717216406247\
4207445354240452266436203270426071472603554211570232205220714304\
0170551231516661056126204444443764532551640423522107451252656020\
2717221665361444433635415043742753460451313010223236315336037114\
3631252363026234657550402704272255477443673416115317645472641130\
5057742515353375730055705302336632254575637115200053272417154723\
6610552446524465521617314652657072461124427445350715555625337057\
0310631072536652642055337343033536710203535276161562162424174222\
7077562053514071262755541407246675543062313120376621243114620422\
3223136027347535223033671300060322613602755633702335227231320440\
1360505200102365566317760525277233637607111371125073413470553261\
5641642743526411340127014466177025016436305033232477047326755310\
1536446545012137345542505474750017217050242531677643664272250140\
3751517352206321364515767066017375100707707615215512625620121140\
3654600337332562642172600500510066575410410001512611643760250410\
4615322712702761272446554715362440023241270217504427565230100233\
5445164020214474205162566317613402244504213662417426750056761455";

/// The tribits of a digit string.
pub fn tribits(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b - b'0').collect()
}

/// Chip `j` of Walsh row `n`: 4 when the parity of `n AND j` is 1, else 0.
pub fn walsh_chip(n: usize, j: usize) -> u8 {
    if (n & j).count_ones() % 2 == 1 {
        4
    } else {
        0
    }
}

/// The BW0 and BW3 code: K = 7, 133 and 171 (octal), 133 first.
pub const CODE_K7: ConvCode = ConvCode::new(7, &[0o133, 0o171]);
/// The BW1 code: K = 9, rate 1/3.
pub const CODE_BW1: ConvCode = ConvCode::new(9, &[0o711, 0o663, 0o557]);
/// The BW2 code: K = 8, rate 1/4.
pub const CODE_BW2: ConvCode = ConvCode::new(8, &[0o235, 0o275, 0o313, 0o357]);

/// The BW0 interleaver (4 x 13, Table C-VII).
pub fn bw0_interleaver() -> Permutation {
    interleaver(4, 13, Walk::new(0, 1, 1, 0), Walk::new(1, 0, 0, 1)).expect("a permutation")
}

/// The BW1 interleaver (16 x 9, Table C-XII).
pub fn bw1_interleaver() -> Permutation {
    interleaver(16, 9, Walk::new(0, 1, 1, 0), Walk::new(1, 0, 0, 1)).expect("a permutation")
}

/// The BW3 interleaver of an `n`-byte packet (Table C-XVI); `None` for the sizes whose printed parameters give no permutation.
pub fn bw3_interleaver(n: usize) -> Option<Permutation> {
    let (r, c) = match n {
        64 => (24, 24),
        128 => (32, 34),
        256 => (44, 48),
        512 => (64, 65),
        _ => return None,
    };
    interleaver(r, c, Walk::new(7, 0, 0, 1), Walk::new(1, -7, 0, -7)).ok()
}

/// The BW2 PN generator's feedback taps as drawn in Figure C-12: bits 15, 14, 5 and 0 of the register.
pub const BW2_TAPS_DRAWN: u16 = 0xC021;

/// The BW2 PN sequence of forward transmission `ft`: the register starts at `AB91 + ft`; for each symbol it is stepped 3 times
/// (the new bit, the parity of the tapped bits, enters bit 15 as the register shifts toward bit 0) and the low three bits read.
pub fn bw2_pn(ft: u32, taps: u16, n: usize) -> Vec<u8> {
    let mut reg = 0xAB91u16.wrapping_add(ft as u16);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        for _ in 0..3 {
            let b = ((reg & taps).count_ones() & 1) as u16;
            reg = (reg >> 1) | (b << 15);
        }
        out.push((reg & 7) as u8);
    }
    out
}

/// The BW2 gray mapping (Table C-XIV): value to tribit.
pub const BW2_GRAY: [u8; 8] = [0, 1, 3, 2, 7, 6, 4, 5];

/// The BW2 tribit of three coded bits `m` (first the most significant) in forward transmission `ft`: rotated right by
/// `ft mod 3` places, then gray coded.
pub fn bw2_tribit(m: u8, ft: u32) -> u8 {
    let mut v = m & 7;
    for _ in 0..ft % 3 {
        v = ((v & 1) << 2) | (v >> 1);
    }
    BW2_GRAY[usize::from(v)]
}

/// The 64 chips of a BW0 or BW1 Walsh symbol `n` at chip offset `at` of the PN.
fn walsh64(n: usize, at: usize, pn: &[u8], out: &mut Vec<Complex32>) {
    for j in 0..64 {
        out.push(psk8(walsh_chip(n, j % 16) + pn[(at + j) % pn.len()]));
    }
}

/// The 4-bit Walsh indices of coded bits (first bit the most significant).
fn indices(bits: &[u8]) -> Vec<usize> {
    bits.chunks(4)
        .map(|c| c.iter().fold(0usize, |a, &b| (a << 1) | usize::from(b)))
        .collect()
}

/// The coded, interleaved bits of a tail-biting burst: `info` encoded with `code` (the register loaded with the last bits), then
/// interleaved by `p`.
fn tail_biting(code: &ConvCode, info: &[u8], p: &Permutation) -> Vec<u8> {
    let mut coded = vec![0u8; info.len() * code.n()];
    encode_tail_biting(code, info, &mut coded).expect("the output fits");
    let mut out = vec![0u8; coded.len()];
    p.apply(&coded, &mut out).expect("the sizes agree");
    out
}

/// The 52 coded and interleaved bits of a BW0 burst carrying 26 bits.
pub fn bw0_bits(bits: &[u8]) -> Vec<u8> {
    tail_biting(&CODE_K7, bits, &bw0_interleaver())
}

/// The symbols of a BW0 burst carrying 26 bits (first-sent first).
pub fn bw0(bits: &[u8]) -> Vec<Complex32> {
    let mut out: Vec<Complex32> = tribits(TLC).iter().map(|&t| psk8(t)).collect();
    out.extend(tribits(BW0_PRE).iter().map(|&t| psk8(t)));
    let pn = tribits(PN01);
    for (i, n) in indices(&bw0_bits(bits)).into_iter().enumerate() {
        walsh64(n, 64 * i, &pn, &mut out);
    }
    out
}

/// The symbols of a BW1 burst carrying 48 bits.
pub fn bw1(bits: &[u8]) -> Vec<Complex32> {
    let mut out: Vec<Complex32> = tribits(TLC).iter().map(|&t| psk8(t)).collect();
    out.extend(tribits(BW1_PRE).iter().map(|&t| psk8(t)));
    let pn = tribits(PN01);
    for (i, n) in indices(&tail_biting(&CODE_BW1, bits, &bw1_interleaver()))
        .into_iter()
        .enumerate()
    {
        walsh64(n, 64 * i, &pn, &mut out);
    }
    out
}

/// Frames of a BW2 packet.
pub const BW2_FRAMES: usize = 20;

/// The symbols of a BW2 burst of packets of 1913 bits each, forward transmission `ft`.
pub fn bw2(packets: &[Vec<u8>], ft: u32, taps: u16) -> Vec<Complex32> {
    let mut out: Vec<Complex32> = tribits(TLC)[..240].iter().map(|&t| psk8(t)).collect();
    let n = 64 + packets.len() * BW2_FRAMES * 48;
    let pn = bw2_pn(ft, taps, n);
    out.extend(pn[..64].iter().map(|&t| psk8(t)));
    let mut k = 64;
    for p in packets {
        let mut e = Encoder::new(CODE_BW2);
        let mut sent = Vec::with_capacity(1920);
        for &b in p.iter().chain(std::iter::repeat_n(&0u8, 7)) {
            let l = e.push_bit(b);
            sent.push((l >> (ft % 4)) & 1);
        }
        for f in 0..BW2_FRAMES {
            for i in 0..32 {
                let c = &sent[3 * (32 * f + i)..3 * (32 * f + i) + 3];
                let m = (c[0] << 2) | (c[1] << 1) | c[2];
                out.push(psk8(bw2_tribit(m, ft) + pn[k]));
                k += 1;
            }
            for _ in 0..16 {
                out.push(psk8(pn[k]));
                k += 1;
            }
        }
    }
    out
}

/// The symbols of a BW3 burst of an `n`-byte packet (`8 n + 57` bits), forward transmission `ft`. `None` for a size without an
/// interleaver.
pub fn bw3(bits: &[u8], n: usize, ft: u32) -> Option<Vec<Complex32>> {
    let p = bw3_interleaver(n)?;
    let mut e = Encoder::new(CODE_K7);
    let mut sent = Vec::with_capacity(8 * n + 64);
    for &b in bits.iter().chain(std::iter::repeat_n(&0u8, 7)) {
        let l = e.push_bit(b);
        sent.push((l >> (ft % 2)) & 1);
    }
    let mut il = vec![0u8; sent.len()];
    p.apply(&sent, &mut il).ok()?;
    let mut out: Vec<Complex32> = tribits(BW3_PRE).iter().map(|&t| psk8(t)).collect();
    let pn = tribits(PN3);
    for (i, w) in indices(&il).into_iter().enumerate() {
        for j in 0..16 {
            out.push(psk8(walsh_chip(w, j) + pn[(16 * i + j) % 32]));
        }
    }
    Some(out)
}

/// The symbols of a BW4 burst of the two bits `b1 b0`.
pub fn bw4(word: usize) -> Vec<Complex32> {
    let mut out: Vec<Complex32> = tribits(TLC).iter().map(|&t| psk8(t)).collect();
    let pn = tribits(PN4);
    for (j, &p) in pn.iter().enumerate() {
        out.push(psk8(walsh_chip(word & 3, j % 4) + p));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digits(v: &[u8]) -> String {
        v.iter().map(|d| char::from(b'0' + d)).collect()
    }

    #[test]
    fn tables_walsh_and_pn() {
        assert_eq!(tribits(TLC).len(), 256);
        assert_eq!(tribits(BW0_PRE).len(), 384);
        assert_eq!(tribits(PN01).len(), 256);
        assert_eq!(tribits(BW1_PRE).len(), 576);
        assert_eq!(tribits(BW3_PRE).len(), 640);
        assert_eq!(tribits(PN4).len(), 1280);
        let row = |n| (0..16).map(|j| walsh_chip(n, j)).collect::<Vec<u8>>();
        assert_eq!(digits(&row(5)), "0404404004044040");
        assert_eq!(digits(&row(14)), "0044440044000044");
        assert_eq!(digits(&bw2_pn(0, BW2_TAPS_DRAWN, 16)), "2652502116235655");
        assert_eq!(bw2_tribit(0b001, 1), 7);
    }

    #[test]
    fn bw0_coding() {
        let il = bw0_interleaver();
        let mut x = 0x2545_F491u32;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        for i in 0..200 {
            let info: Vec<u8> = (0..26).map(|_| (next() & 1) as u8).collect();
            let mut soft: Vec<f32> = bw0_bits(&info)
                .iter()
                .map(|&b| if b == 0 { 1.0 } else { -1.0 })
                .collect();
            if i < 50 {
                for _ in 0..4 {
                    let at = next() as usize % 52;
                    soft[at] = -soft[at];
                }
            }
            let got = crate::ale3g::rx::decode_tail_biting(&CODE_K7, &il, &soft, 26);
            assert_eq!(got, info, "PDU {i}");
        }
    }

    #[test]
    fn interleavers_and_lengths() {
        let p = bw0_interleaver();
        let f: Vec<usize> = (0..8).map(|t| p.src(t).unwrap()).collect();
        assert_eq!(f, [0, 13, 26, 39, 1, 14, 27, 40]);
        assert_eq!(p.src(51), Some(51));
        assert!(bw3_interleaver(64).is_some() && bw3_interleaver(128).is_some());
        assert!(bw3_interleaver(256).is_none() && bw3_interleaver(512).is_none());
        assert_eq!(bw0(&[0; 26]).len(), 1472);
        assert_eq!(bw1(&[0; 48]).len(), 3136);
        assert_eq!(bw2(&[vec![0; 1913]], 0, BW2_TAPS_DRAWN).len(), 304 + 960);
        assert_eq!(
            bw3(&vec![0; 8 * 64 + 57], 64, 0).unwrap().len(),
            640 + 16 * 144
        );
        assert_eq!(bw4(3).len(), 1536);
    }
}
