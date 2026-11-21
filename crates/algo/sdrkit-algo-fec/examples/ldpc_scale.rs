//! Measures the frame error rate of the min-sum LDPC decoder against its scale, on the DVB-S2 codes:
//! `cargo run --release -p sdrkit-algo-fec --example ldpc_scale [frames] [step_db] [points] [first_db] [name]`. The frames
//! are BPSK in white noise, quantised with the default [`Quantizer`], 50 iterations at most. Every scale of a line decodes
//! the same frames. Under each line of rates stands the mean count of wrong information bits in the frames that failed.
//! The Eb/N0 of the first line of a code is `first_db` (default -0.5) from the QPSK level of the standard
//! for the normal code of that rate (EN 302 307-1 table 13). `name` is a part of the code names to run (default
//! `normal`; `short` for the 16200-bit codes). [`MinSum::for_rate`] holds the outcome.

use sdrkit_algo_fec::ldpc::tables::{dvb_s2, DvbTable};
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::Quantizer;

const SCALES: [u8; 6] = [11, 12, 13, 14, 15, 16];
const MAX_ITER: u32 = 50;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in (0, 1].
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// A random code word of `code` after BPSK in white noise at `ebn0_db`: its 8-bit LLRs and its bits.
fn frame(code: &LdpcCode, ebn0_db: f64, rng: &mut Rng) -> (Vec<i8>, Vec<u8>) {
    let info: Vec<u8> = (0..code.k()).map(|_| (rng.next() >> 63) as u8).collect();
    let mut cw = vec![0u8; code.n()];
    code.encode(&info, &mut cw).unwrap();
    let rate = code.k() as f64 / code.n() as f64;
    let sigma2 = 1.0 / (2.0 * rate * 10f64.powf(ebn0_db / 10.0));
    let f: Vec<f32> = cw
        .iter()
        .map(|&b| {
            let x = if b == 0 { 1.0 } else { -1.0 };
            (2.0 * (x + sigma2.sqrt() * rng.gauss()) / sigma2) as f32
        })
        .collect();
    let mut out = vec![0i8; f.len()];
    Quantizer::default().quantize_auto(&f, &mut out).unwrap();
    (out, cw)
}

/// The lines of one code: for each Eb/N0 from `first_db` in steps of `step_db`, the share of failed frames of each scale
/// and the mean count of wrong information bits in them.
fn measure(code: &LdpcCode, first_db: f64, frames: usize, step_db: f64, points: usize) -> String {
    let mut decs: Vec<LdpcDecoder> = SCALES
        .iter()
        .map(|&s| LdpcDecoder::new(code, MinSum::with_scale(s)))
        .collect();
    let mut out = vec![0u8; code.n()];
    let mut text = format!(
        "{} (k/n {:.3})\n",
        code.name(),
        code.k() as f64 / code.n() as f64
    );
    for i in 0..points {
        let ebn0 = first_db + step_db * i as f64;
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (i as u64 + 1));
        let mut failed = [0usize; SCALES.len()];
        let mut wrong = [0usize; SCALES.len()];
        for _ in 0..frames {
            let (llr, cw) = frame(code, ebn0, &mut rng);
            for ((dec, n), w) in decs.iter_mut().zip(&mut failed).zip(&mut wrong) {
                if dec.decode(&llr, MAX_ITER, &mut out).is_err() {
                    *n += 1;
                    *w += out[..code.k()].iter().zip(&cw).filter(|(a, b)| a != b).count();
                }
            }
        }
        text += &format!("  {ebn0:5.2} dB:");
        for n in failed {
            text += &format!(" {:6.3}", n as f64 / frames as f64);
        }
        text += "\n     wrong:";
        for (w, n) in wrong.iter().zip(failed) {
            text += &format!(" {:6.0}", *w as f64 / n.max(1) as f64);
        }
        text.push('\n');
    }
    text
}

fn main() {
    let arg = |i: usize, d: f64| {
        std::env::args()
            .nth(i)
            .and_then(|a| a.parse().ok())
            .unwrap_or(d)
    };
    let (frames, step_db, points, first_db) = (
        arg(1, 40.0) as usize,
        arg(2, 0.1),
        arg(3, 16.0) as usize,
        arg(4, -0.5),
    );
    let name = std::env::args().nth(5).unwrap_or_else(|| "normal".into());
    let codes: [(&'static DvbTable, f64); 21] = [
        (&dvb_s2::S2_N_1_4, 0.66),
        (&dvb_s2::S2_N_1_3, 0.52),
        (&dvb_s2::S2_N_2_5, 0.67),
        (&dvb_s2::S2_N_1_2, 1.00),
        (&dvb_s2::S2_N_3_5, 1.44),
        (&dvb_s2::S2_N_2_3, 1.85),
        (&dvb_s2::S2_N_3_4, 2.27),
        (&dvb_s2::S2_N_4_5, 2.64),
        (&dvb_s2::S2_N_5_6, 2.96),
        (&dvb_s2::S2_N_8_9, 3.70),
        (&dvb_s2::S2_N_9_10, 3.87),
        (&dvb_s2::S2_S_1_4, 0.66),
        (&dvb_s2::S2_S_1_3, 0.52),
        (&dvb_s2::S2_S_2_5, 0.67),
        (&dvb_s2::S2_S_1_2, 1.00),
        (&dvb_s2::S2_S_3_5, 1.44),
        (&dvb_s2::S2_S_2_3, 1.85),
        (&dvb_s2::S2_S_3_4, 2.27),
        (&dvb_s2::S2_S_4_5, 2.64),
        (&dvb_s2::S2_S_5_6, 2.96),
        (&dvb_s2::S2_S_8_9, 3.70),
    ];
    println!("frame error rate of {frames} frames; Eb/N0, then scale/16 =");
    println!("            {}", SCALES.map(|s| format!("{s:6}")).join(" "));
    let texts: Vec<String> = std::thread::scope(|s| {
        let jobs: Vec<_> = codes
            .iter()
            .map(|&(t, db)| (LdpcCode::from_dvb(t).unwrap(), db))
            .filter(|(code, _)| code.name().contains(&name))
            .map(|(code, db)| {
                s.spawn(move || measure(&code, db + first_db, frames, step_db, points))
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    });
    for t in texts {
        print!("{t}");
    }
}
