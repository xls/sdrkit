//! The headers above the slice: sequence, sequence extension, group of pictures, picture and its coding extension,
//! and the quantiser matrix extension.

use crate::tables::{DEFAULT_INTRA_MATRIX, ZIGZAG};
use sdrkit_util_bits::BitReader;

/// The largest picture the decoder takes, in samples.
pub(crate) const MAX_WIDTH: u32 = 1920;
pub(crate) const MAX_HEIGHT: u32 = 1152;

/// What a sequence header and its extension say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sequence {
    pub width: u32,
    pub height: u32,
    /// The size in macroblocks. `mb_h` is even when the sequence is not progressive.
    pub mb_w: usize,
    pub mb_h: usize,
    pub aspect_code: u8,
    pub rate_code: u8,
    /// frame_rate_extension_n and frame_rate_extension_d.
    pub rate_ext: (u8, u8),
    pub progressive: bool,
    pub low_delay: bool,
    /// The chroma format is 4:2:0.
    pub supported: bool,
    /// The quantiser matrices, along the rows.
    pub intra: [u8; 64],
    pub non_intra: [u8; 64],
    /// matrix_coefficients of the sequence display extension, if it has a colour description.
    pub matrix: Option<u8>,
}

fn matrix(r: &mut BitReader) -> [u8; 64] {
    let mut m = [0u8; 64];
    for z in ZIGZAG {
        // A weight of 0 is forbidden; 1 keeps the arithmetic defined.
        m[z as usize] = (r.u(8) as u8).max(1);
    }
    m
}

impl Sequence {
    /// The sequence header in `body` (the bytes after the start code). The extension completes it.
    pub fn parse(body: &[u8]) -> Option<Sequence> {
        let mut r = BitReader::new(body);
        let width = r.u(12) as u32;
        let height = r.u(12) as u32;
        let aspect_code = r.u(4) as u8;
        let rate_code = r.u(4) as u8;
        r.skip(18 + 1 + 10 + 1);
        let intra = if r.bit() {
            matrix(&mut r)
        } else {
            DEFAULT_INTRA_MATRIX
        };
        let non_intra = if r.bit() { matrix(&mut r) } else { [16; 64] };
        (!r.overrun()).then_some(Sequence {
            width,
            height,
            mb_w: 0,
            mb_h: 0,
            aspect_code,
            rate_code,
            rate_ext: (0, 0),
            progressive: false,
            low_delay: false,
            supported: false,
            intra,
            non_intra,
            matrix: None,
        })
    }

    /// Adds the sequence extension in `body` (after the 4 bits of the identifier). False when the result is no
    /// sequence that can be decoded.
    pub fn extend(&mut self, r: &mut BitReader) -> bool {
        r.skip(8);
        self.progressive = r.bit();
        self.supported = r.u(2) == 1;
        self.width |= (r.u(2) as u32) << 12;
        self.height |= (r.u(2) as u32) << 12;
        r.skip(12 + 1 + 8);
        self.low_delay = r.bit();
        self.rate_ext = (r.u(2) as u8, r.u(5) as u8);
        if r.overrun()
            || self.width == 0
            || self.height == 0
            || self.width > MAX_WIDTH
            || self.height > MAX_HEIGHT
        {
            return false;
        }
        self.mb_w = self.width.div_ceil(16) as usize;
        self.mb_h = if self.progressive {
            self.height.div_ceil(16) as usize
        } else {
            2 * self.height.div_ceil(32) as usize
        };
        true
    }

    /// Reads the sequence display extension in `r` (after the 4 bits of the identifier): video_format, then
    /// colour_primaries, transfer_characteristics and matrix_coefficients if colour_description is set.
    pub fn display(&mut self, r: &mut BitReader) {
        r.skip(3);
        if r.bit() {
            r.skip(16);
            let m = r.u(8) as u8;
            if !r.overrun() {
                self.matrix = Some(m);
            }
        }
    }

    /// Loads the matrices of a quantiser matrix extension.
    pub fn load_matrices(&mut self, r: &mut BitReader) {
        let intra = r.bit().then(|| matrix(r));
        let non_intra = r.bit().then(|| matrix(r));
        if r.overrun() {
            return;
        }
        if let Some(m) = intra {
            self.intra = m;
        }
        if let Some(m) = non_intra {
            self.non_intra = m;
        }
    }

    /// The frame rate as a fraction in lowest terms.
    pub fn frame_rate(&self) -> (u32, u32) {
        let (n, d) = match self.rate_code {
            1 => (24000, 1001),
            2 => (24, 1),
            3 => (25, 1),
            4 => (30000, 1001),
            5 => (30, 1),
            6 => (50, 1),
            7 => (60000, 1001),
            8 => (60, 1),
            _ => (25, 1),
        };
        reduce(
            n * (self.rate_ext.0 as u32 + 1),
            d * (self.rate_ext.1 as u32 + 1),
        )
    }

    /// The aspect ratio of the displayed picture as a fraction in lowest terms.
    pub fn aspect(&self) -> (u32, u32) {
        match self.aspect_code {
            2 => (4, 3),
            3 => (16, 9),
            4 => (221, 100),
            _ => reduce(self.width, self.height),
        }
    }
}

fn reduce(n: u32, d: u32) -> (u32, u32) {
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (n / a.max(1), d / a.max(1))
}

/// picture_structure: one field or a frame.
pub(crate) const TOP_FIELD: u8 = 1;
pub(crate) const BOTTOM_FIELD: u8 = 2;
pub(crate) const FRAME: u8 = 3;

/// What a picture header and its coding extension say.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Picture {
    /// picture_coding_type: 1 for I, 2 for P, 3 for B.
    pub kind: u8,
    pub temporal_reference: u16,
    /// f_code by direction (forward, backward) and by component (horizontal, vertical).
    pub f_code: [[u8; 2]; 2],
    pub dc_precision: u8,
    pub structure: u8,
    pub top_field_first: bool,
    pub frame_pred_frame_dct: bool,
    pub concealment_vectors: bool,
    pub non_linear_scale: bool,
    pub intra_vlc_one: bool,
    pub alternate_scan: bool,
    pub repeat_first_field: bool,
    pub progressive_frame: bool,
    /// The second field of a frame.
    pub second: bool,
}

/// The temporal_reference and the picture_coding_type of the picture header in `body`, when the decoder knows the
/// type.
pub(crate) fn picture_header(body: &[u8]) -> Option<(u16, u8)> {
    let mut r = BitReader::new(body);
    let temporal_reference = r.u(10) as u16;
    let kind = r.u(3) as u8;
    r.skip(16);
    (!r.overrun() && (1..=3).contains(&kind)).then_some((temporal_reference, kind))
}

/// The picture coding extension (after the 4 bits of the identifier) of the picture with the header values given.
pub(crate) fn coding_extension(
    (temporal_reference, kind): (u16, u8),
    r: &mut BitReader,
) -> Option<Picture> {
    let mut f_code = [[0u8; 2]; 2];
    for f in f_code.iter_mut().flatten() {
        *f = r.u(4) as u8;
    }
    let p = Picture {
        kind,
        temporal_reference,
        f_code,
        dc_precision: r.u(2) as u8,
        structure: r.u(2) as u8,
        top_field_first: r.bit(),
        frame_pred_frame_dct: r.bit(),
        concealment_vectors: r.bit(),
        non_linear_scale: r.bit(),
        intra_vlc_one: r.bit(),
        alternate_scan: r.bit(),
        repeat_first_field: r.bit(),
        progressive_frame: {
            r.skip(1);
            r.bit()
        },
        second: false,
    };
    (!r.overrun() && p.structure != 0).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence() -> Sequence {
        Sequence::parse(&[0x2D, 0x02, 0x40, 0x33, 0xFF, 0xFF, 0xE0, 0x18]).expect("a header")
    }

    #[test]
    fn the_colour_description_gives_the_matrix() {
        // The identifier 2, video_format 5, colour_description 1; primaries 5, transfer 5, matrix_coefficients 6.
        let display = |bytes: &[u8]| {
            let mut s = sequence();
            let mut r = BitReader::new(bytes);
            assert_eq!(r.u(4), 2);
            s.display(&mut r);
            s.matrix
        };
        assert_eq!(display(&[0x2B, 0x05, 0x05, 0x06, 0x00]), Some(6));
        // No colour description.
        assert_eq!(display(&[0x2A, 0x00, 0x00]), None);
    }
}
