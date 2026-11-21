//! MPEG Layer II audio frames as DAB carries them: the ScF-CRC, the joining of two frames into one 24 kHz frame, and the PAD
//! at the end of each frame. The frame header and the codec are [`sdrkit_codec_mpa`].
//!
//! Standard: ETSI EN 300 401 clause 7.4 and annex B.3.

use sdrkit_codec_mpa::{Header, ScaleFactor};
use sdrkit_util_bits::crc::catalog::CRC8_GSM_A;

/// The length in bytes of the ScF-CRC at the end of a DAB Layer II frame (EN 300 401 clause 7.4.4): 4 at half sampling rates
/// or from 56 kbit/s per channel, else 2.
pub fn scf_crc_len(h: &Header) -> usize {
    if !h.mpeg1 || h.bitrate_kbps as usize / h.channels() >= 56 {
        4
    } else {
        2
    }
}

/// The ScF-CRC words over the scale factors of a frame (EN 300 401 annex B.3), in transmission order (highest group of
/// sub-bands first). Each word is the CRC-8 (generator 0x1D) of the three most significant bits of each scale factor index of
/// its group. The result fills the first [`scf_crc_len`] bytes of the array.
pub fn scf_crc(h: &Header, scf: &[ScaleFactor]) -> [u8; 4] {
    let groups: &[(u8, u8)] = if scf_crc_len(h) == 4 {
        if h.mpeg1 {
            &[(16, 26), (8, 15), (4, 7), (0, 3)]
        } else {
            &[(16, 29), (8, 15), (4, 7), (0, 3)]
        }
    } else {
        &[(4, 7), (0, 3)]
    };
    let mut words = [0u8; 4];
    for (w, &(a, b)) in words.iter_mut().zip(groups) {
        let mut d = CRC8_GSM_A.digest();
        for s in scf.iter().filter(|s| (a..=b).contains(&s.subband)) {
            let v = s.index;
            d.update_bits(&[(v >> 5) & 1, (v >> 4) & 1, (v >> 3) & 1]);
        }
        *w = d.finalize();
    }
    words
}

/// Joins DAB logical frames into Layer II frames. A logical frame whose header announces a frame of twice its size (24 kHz)
/// is held and given out with the next one. Every other logical frame is one frame.
#[derive(Clone, Debug, Default)]
pub struct Framer {
    buf: Vec<u8>,
    held: bool,
}

impl Framer {
    /// Forgets a held half frame.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.held = false;
    }

    /// True when the first half of a 24 kHz frame is held.
    pub fn holding(&self) -> bool {
        self.held
    }

    /// One logical frame in. Gives the frame when one is complete. An erased logical frame provides nothing and drops a held
    /// half frame.
    pub fn push(&mut self, lf: &[u8], erased: bool) -> Option<&[u8]> {
        if self.held {
            self.held = false;
            if erased {
                self.buf.clear();
                return None;
            }
            self.buf.extend_from_slice(lf);
            return Some(&self.buf);
        }
        self.buf.clear();
        if erased {
            return None;
        }
        self.buf.extend_from_slice(lf);
        if let Ok(h) = Header::parse(lf) {
            let n = h.frame_len();
            if n + 1 >= 2 * lf.len() && n <= 2 * lf.len() + 1 {
                self.held = true;
                return None;
            }
        }
        Some(&self.buf)
    }
}

/// The PAD of a DAB Layer II frame: the F-PAD (the last two bytes) is returned, and `xpad` gets the bytes before the ScF-CRC
/// in logical X-PAD order (reversed), at most the frame length less 6. `None` when the frame has no valid header or fewer than
/// 8 bytes.
pub fn pad_of_frame(frame: &[u8], xpad: &mut Vec<u8>) -> Option<[u8; 2]> {
    let h = Header::parse(frame).ok()?;
    let n = frame.len();
    if n < 8 {
        return None;
    }
    let end = n.checked_sub(2 + scf_crc_len(&h))?;
    let max = end.min(n - 6);
    xpad.clear();
    xpad.extend(frame[end - max..end].iter().rev());
    Some([frame[n - 2], frame[n - 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(b: [u8; 4]) -> Header {
        Header::parse(&b).unwrap()
    }

    #[test]
    fn scf_crc_length_of_clause_7_4_4() {
        assert_eq!(scf_crc_len(&header([0xFF, 0xFD, 0x84, 0x04])), 4); // 64 kbit/s per channel
        assert_eq!(scf_crc_len(&header([0xFF, 0xFD, 0x44, 0x04])), 2); // 32 kbit/s per channel
        assert_eq!(scf_crc_len(&header([0xFF, 0xF5, 0x84, 0xC4])), 4); // 24 kHz
    }

    #[test]
    fn scf_crc_words_go_highest_group_first() {
        // Annex B.3: the word of sub-bands 0 to 3 is sent last.
        let h = header([0xFF, 0xFD, 0x84, 0x04]);
        let sf = |subband, index| ScaleFactor {
            channel: 0,
            subband,
            index,
        };
        let a = scf_crc(&h, &[sf(2, 10), sf(20, 30)]);
        let b = scf_crc(&h, &[sf(2, 50), sf(20, 30)]);
        assert_eq!(&a[..3], &b[..3]);
        assert_ne!(a[3], b[3]);
        // Only the three most significant bits of an index count.
        assert_eq!(scf_crc(&h, &[sf(2, 8)]), scf_crc(&h, &[sf(2, 15)]));
    }

    #[test]
    fn framer_joins_24_khz_halves() {
        // 24 kHz, 64 kbit/s mono: a frame of 384 bytes in logical frames of 192 bytes.
        let mut lf = vec![0u8; 192];
        lf[..4].copy_from_slice(&[0xFF, 0xF5, 0x84, 0xC4]);
        let mut f = Framer::default();
        assert!(f.push(&lf, false).is_none());
        assert!(f.holding());
        assert_eq!(f.push(&[7; 192], false).map(|x| x.len()), Some(384));
        // An erased second half drops the frame.
        assert!(f.push(&lf, false).is_none());
        assert!(f.push(&[7; 192], true).is_none());
        assert!(!f.holding());
        // 48 kHz: each logical frame is a frame.
        let mut lf = vec![0u8; 384];
        lf[..4].copy_from_slice(&[0xFF, 0xFD, 0x84, 0x04]);
        assert_eq!(f.push(&lf, false).map(|x| x.len()), Some(384));
    }

    #[test]
    fn pad_is_read_backwards_before_the_scf_crc() {
        let mut frame = vec![0u8; 384];
        frame[..4].copy_from_slice(&[0xFF, 0xFD, 0x84, 0x04]);
        frame[384 - 2..].copy_from_slice(&[0x12, 0x34]);
        frame[384 - 2 - 4 - 1] = 0xAB;
        frame[384 - 2 - 4 - 2] = 0xCD;
        let mut x = Vec::new();
        assert_eq!(pad_of_frame(&frame, &mut x), Some([0x12, 0x34]));
        assert_eq!(&x[..2], &[0xAB, 0xCD]);
        assert_eq!(x.len(), 378);
        assert_eq!(pad_of_frame(&frame[..6], &mut x), None);
    }
}
