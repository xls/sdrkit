//! The encoder of the coders: analysis, quantisation, bit vectors, FEC, modulation and interleaving.

use crate::analysis::{Analyser, Bands};
use crate::model::{Params, FRAME, MAX_K};
use crate::{dstar, full, half, Coder};

/// A voice encoder for one coder. Its analysis looks two frames ahead, so the frame it returns is the one whose input
/// came two calls earlier. It sends only voice frames. It allocates nothing after construction.
pub struct Encoder {
    coder: Coder,
    ana: Analyser,
    full_amp: full::AmpState,
    half_amp: half::AmpState,
    sync: u32,
    params: Params,
    bands: Bands,
    rec: Params,
}

impl Encoder {
    /// An encoder in the initial state of the coder.
    pub fn new(coder: Coder) -> Self {
        Encoder {
            coder,
            ana: Analyser::new(),
            full_amp: full::AmpState::default(),
            half_amp: match coder {
                Coder::Dstar => dstar::BOOKS.start(),
                _ => half::BOOKS.start(),
            },
            sync: 0,
            params: Params::silent(0.0, 0),
            bands: Bands {
                k: 0,
                v: [false; MAX_K + 1],
            },
            rec: Params::silent(0.0, 0),
        }
    }

    /// The coder.
    pub fn coder(&self) -> Coder {
        self.coder
    }

    /// The parameters the analysis found for the last frame (amplitudes before quantisation).
    pub fn analysed(&self) -> &Params {
        &self.params
    }

    /// The parameters the last frame carries, as a decoder rebuilds them before enhancement.
    pub fn coded(&self) -> &Params {
        &self.rec
    }

    /// Encodes 160 samples into one frame of channel bits, one bit per byte in the order sent. Returns the number of bits
    /// written, [`Coder::frame_bits`], or 0 when `out` is shorter (the input is still taken).
    pub fn encode(&mut self, pcm: &[i16; FRAME], out: &mut [u8]) -> usize {
        match self.coder {
            Coder::FullRate => {
                let mut sb = [0u8; full::SPEECH_BITS];
                self.full_speech(pcm, &mut sb);
                if out.len() < full::FRAME_BITS {
                    return 0;
                }
                full::encode_channel(&sb, out);
                full::FRAME_BITS
            }
            Coder::HalfRate => {
                let mut sb = [0u8; half::SPEECH_BITS];
                self.half_speech(pcm, &mut sb);
                if out.len() < half::FRAME_BITS {
                    return 0;
                }
                half::encode_channel(&sb, out);
                half::FRAME_BITS
            }
            Coder::Dstar => {
                let mut sb = [0u8; dstar::SPEECH_BITS];
                self.dstar_speech(pcm, &mut sb);
                if out.len() < dstar::FRAME_BITS {
                    return 0;
                }
                dstar::encode_channel(&sb, out);
                dstar::FRAME_BITS
            }
        }
    }

    /// Encodes 160 samples into the speech bits of one frame (the format of [`crate::Decoder::decode_speech`]). Returns
    /// the number of bits written, [`Coder::speech_bits`], or 0 when `out` is shorter.
    pub fn encode_speech(&mut self, pcm: &[i16; FRAME], out: &mut [u8]) -> usize {
        match self.coder {
            Coder::FullRate => {
                let mut sb = [0u8; full::SPEECH_BITS];
                self.full_speech(pcm, &mut sb);
                copy_bits(&sb, out)
            }
            Coder::HalfRate => {
                let mut sb = [0u8; half::SPEECH_BITS];
                self.half_speech(pcm, &mut sb);
                copy_bits(&sb, out)
            }
            Coder::Dstar => {
                let mut sb = [0u8; dstar::SPEECH_BITS];
                self.dstar_speech(pcm, &mut sb);
                copy_bits(&sb, out)
            }
        }
    }

    /// Encodes a tone frame of tone index `id` and amplitude `amplitude` into channel bits: a half-rate frame (Table 10,
    /// amplitude 0..=127) or a D-STAR frame (the 8-bit level; the places of some of its bits are not confirmed). Returns
    /// the number of bits written, or 0 for the full-rate coder or when `out` is shorter. The analysis does not see this
    /// frame.
    pub fn encode_tone(&mut self, id: u8, amplitude: u8, out: &mut [u8]) -> usize {
        match self.coder {
            Coder::HalfRate if out.len() >= half::FRAME_BITS => {
                let mut sb = [0u8; half::SPEECH_BITS];
                half::tone_bits(half::Tone { id, ad: amplitude }, &mut sb);
                half::encode_channel(&sb, out);
                half::FRAME_BITS
            }
            Coder::Dstar if out.len() >= dstar::FRAME_BITS => {
                let mut sb = [0u8; dstar::SPEECH_BITS];
                dstar::tone_bits(dstar::Tone { id, level: amplitude }, &mut sb);
                dstar::encode_channel(&sb, out);
                dstar::FRAME_BITS
            }
            _ => 0,
        }
    }

    fn full_speech(&mut self, pcm: &[i16; FRAME], sb: &mut [u8; full::SPEECH_BITS]) {
        let w0 = self.ana.push(pcm);
        let b0 = full::b0_of(w0);
        let l = full::l_of(full::w0_of(b0));
        self.ana.harmonics(w0, w0, l, &mut self.params, &mut self.bands);
        let mut v = full::Values::default();
        full::quantise_frame(&self.params, &self.bands.v, &mut self.full_amp, &mut v, &mut self.rec);
        // The sync bit: 0 in the first frame, then alternating (Equation 80).
        v.b[l + 2] = self.sync;
        self.sync ^= 1;
        full::pack(&v, l, sb);
    }

    fn half_speech(&mut self, pcm: &[i16; FRAME], sb: &mut [u8; half::SPEECH_BITS]) {
        let analysed = self.ana.push(pcm);
        let mut b = [0u32; 9];
        self.quantise(&half::BOOKS, analysed, &mut b);
        half::pack(&b, sb);
    }

    fn dstar_speech(&mut self, pcm: &[i16; FRAME], sb: &mut [u8; dstar::SPEECH_BITS]) {
        let analysed = self.ana.push(pcm);
        let mut b = [0u32; 9];
        self.quantise(&dstar::BOOKS, analysed, &mut b);
        dstar::pack(&b, sb);
    }

    /// b0..b8 of the frame whose analysed fundamental is `analysed`, with the tables `k`.
    fn quantise(&mut self, k: &half::Books, analysed: f64, b: &mut [u32; 9]) {
        let b0 = k.b0_of(analysed);
        let (w0, l) = measure_point(k, b0);
        self.ana.harmonics(analysed, w0, l, &mut self.params, &mut self.bands);
        half::quantise_frame(k, b0, &self.params, &self.bands.v, &mut self.half_amp, b, &mut self.rec);
    }
}

/// Where the half-rate and D-STAR encoders measure the amplitudes: the table fundamental and L of the chosen b0, which
/// the decoder uses, rather than the analysis values (TIA-102.BABA-1 does not say which; for 36 entries the L differ).
/// The voicing measure keeps the analysed fundamental: the table is up to 1.1 % off it, which moves the upper harmonics
/// far enough to make a voiced band measure as unvoiced.
fn measure_point(k: &half::Books, b0: u32) -> (f64, usize) {
    ((k.w0)(b0), (k.l)(b0))
}

fn copy_bits(sb: &[u8], out: &mut [u8]) -> usize {
    if out.len() < sb.len() {
        return 0;
    }
    out[..sb.len()].copy_from_slice(sb);
    sb.len()
}
