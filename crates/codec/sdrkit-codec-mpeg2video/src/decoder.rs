//! The decoder above the slice: start codes, headers, the three frame stores, the order of output and concealment.

use crate::header::{self, Picture, Sequence, BOTTOM_FIELD, FRAME, TOP_FIELD};
use sdrkit_util_bits::BitReader;

/// The coding type of a picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureKind {
    /// Coded without another picture.
    I,
    /// Predicted from the I or P picture before it.
    P,
    /// Predicted from the I or P pictures before and after it in display order.
    B,
}

/// One decoded frame. The planes hold whole macroblocks: use `width` and `height` of each, and `stride` between rows.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    /// The luminance plane.
    pub y: &'a [u8],
    /// The Cb plane, half the size of the luminance in both directions.
    pub cb: &'a [u8],
    /// The Cr plane.
    pub cr: &'a [u8],
    /// The distance between rows of `y`.
    pub stride: usize,
    /// The distance between rows of `cb` and `cr`.
    pub chroma_stride: usize,
    /// The width of the picture in luminance samples.
    pub width: u32,
    /// The height of the picture in luminance rows.
    pub height: u32,
    /// The aspect ratio of the displayed picture (16:9, 4:3), width to height.
    pub aspect: (u32, u32),
    /// The frame rate as a fraction.
    pub frame_rate: (u32, u32),
    /// matrix_coefficients of the colour description of the sequence (ISO/IEC 13818-2 table 6-9: 1 BT.709, 5 and 6
    /// BT.601), if the stream has one. The samples are always of video range.
    pub matrix_coefficients: Option<u8>,
    /// The presentation time stamp in units of 1/90000 s: the one given with the picture, or the one before plus the
    /// duration of a frame. `None` until a picture came with a time stamp.
    pub pts: Option<i64>,
    /// The two fields of the frame are from different times.
    pub interlaced: bool,
    /// The top field is the first in time.
    pub top_field_first: bool,
    /// The first field is shown again after the second.
    pub repeat_first_field: bool,
    /// The coding type.
    pub kind: PictureKind,
    /// Macroblocks that were not decoded and are a copy of the picture before.
    pub concealed: u32,
    /// The frame is not what the stream coded: a part of it was lost or damaged, or a picture that it is predicted
    /// from was. The frames after a clean I picture are not damaged.
    pub damaged: bool,
    /// How much of the frame is not what the stream coded, in macroblocks: those concealed in this frame, and
    /// those of the most damaged picture that it is predicted from. All macroblocks of the frame after lost data.
    pub damaged_macroblocks: u32,
}

/// Counts since the decoder was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames given to the caller.
    pub frames: u64,
    /// Slices decoded, with and without an error.
    pub slices: u64,
    /// Slices that ended at an error.
    pub slice_errors: u64,
    /// Macroblocks that were concealed.
    pub concealed: u64,
    /// Pictures left out because a picture they are predicted from is missing.
    pub dropped: u64,
    /// Sequence headers of a format that the decoder does not take.
    pub unsupported: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct Info {
    pts: Option<i64>,
    top_field_first: bool,
    repeat_first_field: bool,
    progressive: bool,
    kind: u8,
    temporal_reference: u16,
    concealed: u32,
    damaged: bool,
    /// Macroblocks that are damaged in the pictures this one is predicted from.
    inherited: u32,
}

/// One frame store.
#[derive(Default)]
pub(crate) struct Planes {
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
    info: Option<Info>,
}

impl Planes {
    /// Marks the whole frame as damaged.
    fn mark_lost(&mut self) {
        if let Some(i) = &mut self.info {
            i.damaged = true;
            i.inherited = u32::MAX;
        }
    }

    /// Marks the frame as damaged.
    pub(crate) fn mark_damaged(&mut self) {
        if let Some(i) = &mut self.info {
            i.damaged = true;
        }
    }

    fn damaged(&self) -> bool {
        self.info.is_some_and(|i| i.damaged)
    }

    /// The macroblocks of the frame that are damaged, its own and those it has from other pictures.
    fn lost(&self) -> u32 {
        self.info
            .map_or(0, |i| i.concealed.saturating_add(i.inherited))
    }
}

/// The decoder. See the [crate documentation](crate).
pub struct Decoder {
    pub(crate) seq: Option<Sequence>,
    /// A sequence header that waits for its extension.
    pending: Option<Sequence>,
    /// The values of a picture header that waits for its coding extension, and its time stamp.
    header: Option<((u16, u8), Option<i64>)>,
    /// The picture whose slices are decoded.
    pub(crate) pic: Option<Picture>,
    pub(crate) frames: [Planes; 3],
    pub(crate) cur: usize,
    pub(crate) fwd: Option<usize>,
    pub(crate) bwd: Option<usize>,
    /// `bwd` was not given to the caller yet.
    held: bool,
    /// `cur` holds a frame that is not finished.
    open: bool,
    /// The structure of the field that completes the open frame.
    awaited: Option<u8>,
    /// Per macroblock: bit 0 the frame or the top field is decoded, bit 1 the bottom field.
    pub(crate) done: Vec<u8>,
    /// B pictures are left out until this many I or P pictures came.
    skip_b: u8,
    next_pts: Option<i64>,
    last_pts: Option<i64>,
    /// An I or P picture was lost before the pictures that follow: they are damaged until an I picture.
    tainted: bool,
    /// Data was lost, and the pictures since then did not show yet whether an I or P picture is missing.
    suspect: bool,
    /// Set at an I or P picture that came after lost data, until the next picture shows whether a picture before
    /// it is missing: the time stamp of the I or P picture before the loss, and whether the new one is an I picture.
    check: Option<(Option<i64>, bool)>,
    /// The B pictures before the next I or P picture have lost a picture that they are predicted from.
    b_lost: bool,
    /// The data of this call of `decode` is damaged.
    unit_damaged: bool,
    /// The structure of the first field of the last frame that came as two fields.
    first_field: Option<u8>,
    /// The open frame got its second field.
    paired: bool,
    pub(crate) stats: Stats,
    pub(crate) plain: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The place of the next start code prefix (00 00 01) in `data` at or after `from`.
fn start_code(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from + 2;
    while i < data.len() {
        match data[i] {
            1 if data[i - 1] == 0 && data[i - 2] == 0 => return Some(i - 2),
            0 => i += 1,
            _ => i += 3,
        }
    }
    None
}

impl Decoder {
    /// A decoder that waits for a sequence header.
    pub fn new() -> Self {
        Decoder {
            seq: None,
            pending: None,
            header: None,
            pic: None,
            frames: Default::default(),
            cur: 0,
            fwd: None,
            bwd: None,
            held: false,
            open: false,
            awaited: None,
            done: Vec::new(),
            skip_b: 0,
            next_pts: None,
            last_pts: None,
            tainted: false,
            suspect: false,
            check: None,
            b_lost: false,
            unit_damaged: false,
            first_field: None,
            paired: false,
            stats: Stats::default(),
            plain: false,
        }
    }

    /// Uses the portable arithmetic on every machine when `on`. The result is the same; only the speed differs.
    pub fn set_portable(&mut self, on: bool) {
        self.plain = on;
    }

    /// The counts since the decoder was made.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Decodes `data`, which holds whole access units (each a picture with the headers before it), and calls `sink`
    /// for every frame that is ready, in display order. `pts` is the time stamp of the first picture in `data`, in
    /// units of 1/90000 s. A frame is valid during the call of `sink` only.
    pub fn decode(&mut self, data: &[u8], pts: Option<i64>, sink: &mut dyn FnMut(&Frame<'_>)) {
        self.next_pts = pts;
        let mut at = start_code(data, 0);
        while let Some(i) = at {
            let Some(&code) = data.get(i + 3) else { break };
            at = start_code(data, i + 4);
            self.unit(code, &data[i + 4..at.unwrap_or(data.len())], sink);
        }
        let field_missing = self.awaited.is_some() && !self.pic.is_some_and(|p| p.second);
        if self.open && !field_missing {
            self.finish(sink);
        } else if self.open && self.unit_damaged {
            self.frames[self.cur].mark_damaged();
        }
        self.unit_damaged = false;
    }

    /// Says that the data of the next call of [`Decoder::decode`] is damaged: its pictures are marked as damaged.
    pub fn damaged(&mut self) {
        self.unit_damaged = true;
    }

    /// Says that data was lost before the next call of [`Decoder::decode`]. If the time stamps of the next pictures
    /// show that an I or P picture is missing, or cannot show that none is, the pictures are marked as damaged
    /// until an I picture comes. The loss of B pictures alone damages nothing.
    pub fn discontinuity(&mut self) {
        self.suspect = true;
    }

    /// Gives the frames that the decoder holds back for the order of display. Call it at the end of the stream.
    pub fn flush(&mut self, sink: &mut dyn FnMut(&Frame<'_>)) {
        if self.open {
            self.finish(sink);
        }
        self.release(sink);
    }

    /// Forgets the pictures decoded so far, after a break in the stream. Decoding starts again at the next I picture.
    pub fn reset(&mut self) {
        self.pic = None;
        self.header = None;
        self.open = false;
        self.awaited = None;
        self.held = false;
        self.fwd = None;
        self.bwd = None;
        self.last_pts = None;
        self.tainted = false;
        self.suspect = false;
        self.check = None;
        self.b_lost = false;
        self.first_field = None;
    }

    fn unit(&mut self, code: u8, body: &[u8], sink: &mut dyn FnMut(&Frame<'_>)) {
        match code {
            0x00 => {
                self.pic = None;
                self.header = header::picture_header(body).map(|h| (h, self.next_pts.take()));
            }
            0x01..=0xaf => self.slice(code, body),
            0xb3 => {
                if self.open {
                    self.finish(sink);
                }
                self.pending = Sequence::parse(body);
            }
            0xb5 => self.extension(body, sink),
            0xb7 => self.flush(sink),
            0xb8 => {
                if self.open {
                    self.finish(sink);
                }
                // time_code (25 bits), closed_gop, broken_link.
                if body.get(3).is_some_and(|b| b & 0x20 != 0) {
                    self.skip_b = 2;
                }
            }
            _ => {}
        }
    }

    fn extension(&mut self, body: &[u8], sink: &mut dyn FnMut(&Frame<'_>)) {
        let mut r = BitReader::new(body);
        match r.u(4) {
            1 => {
                let Some(mut s) = self.pending.take() else {
                    return;
                };
                if !s.extend(&mut r) || !s.supported {
                    self.stats.unsupported += 1;
                    self.release(sink);
                    self.reset();
                    self.seq = None;
                    return;
                }
                let same = self
                    .seq
                    .as_ref()
                    .is_some_and(|o| (o.mb_w, o.mb_h) == (s.mb_w, s.mb_h));
                if !same {
                    self.release(sink);
                    self.reset();
                    let luma = s.mb_w * s.mb_h * 256;
                    for f in &mut self.frames {
                        f.y = vec![16; luma];
                        f.cb = vec![128; luma / 4];
                        f.cr = vec![128; luma / 4];
                        f.info = None;
                    }
                    self.done = vec![0; s.mb_w * s.mb_h];
                }
                self.seq = Some(s);
            }
            2 => {
                if let Some(s) = &mut self.seq {
                    s.display(&mut r);
                }
            }
            3 => {
                if let Some(s) = &mut self.seq {
                    s.load_matrices(&mut r);
                }
            }
            8 => {
                let Some((values, pts)) = self.header.take() else {
                    return;
                };
                if let Some(p) = header::coding_extension(values, &mut r) {
                    self.begin(p, pts, sink);
                }
            }
            _ => {}
        }
    }

    /// Starts the picture `p`: the second field of the open frame, or a new frame in a free store.
    fn begin(&mut self, mut p: Picture, pts: Option<i64>, sink: &mut dyn FnMut(&Frame<'_>)) {
        let Some(seq) = &self.seq else { return };
        if p.structure != FRAME && seq.progressive {
            return;
        }
        // The two fields of a frame have the same temporal reference.
        if let (true, Some(first)) = (
            self.open && self.awaited == Some(p.structure),
            self.frames[self.cur].info,
        ) {
            let pair = first.temporal_reference == p.temporal_reference;
            if pair && (first.kind == p.kind || (first.kind == 1 && p.kind == 2)) {
                p.second = true;
                self.paired = true;
                self.first_field = Some(FRAME - p.structure);
                self.pic = Some(p);
                return;
            }
        }
        if self.open {
            self.finish(sink);
        }
        // After lost data the time stamps show whether an I or P picture is missing. The frames between two such
        // pictures follow the later one, so the picture after an I or P picture is one frame later than the I or P
        // picture before the loss, or the two are one frame apart. A B picture is earlier than the last I or P
        // picture.
        let rate = self.seq.as_ref().map_or((25, 1), Sequence::frame_rate);
        let step = 90_000 * rate.1 as i64 / rate.0.max(1) as i64;
        let follows = |a: Option<i64>, b: Option<i64>| a.zip(b).is_some_and(|(a, b)| b - a == step);
        let anchor = self
            .bwd
            .and_then(|b| self.frames[b].info)
            .and_then(|i| i.pts);
        if let Some((before, intra)) = self.check.take() {
            let whole = follows(before, if p.kind == 3 { pts } else { anchor });
            if !whole && intra {
                self.b_lost = true;
            } else if !whole {
                self.tainted = true;
                if let Some(b) = self.bwd {
                    self.frames[b].mark_lost();
                }
            }
        }
        if p.kind == 3 {
            self.tainted |= self.suspect && !anchor.zip(pts).is_some_and(|(a, b)| b < a);
        } else {
            self.b_lost = false;
            if self.suspect {
                self.check = Some((anchor, p.kind == 1));
            }
        }
        self.suspect = false;
        let mut damaged = false;
        let mut inherited = 0;
        if p.kind == 3 {
            let (Some(f), Some(b)) = (self.fwd, self.bwd) else {
                self.stats.dropped += 1;
                return;
            };
            if self.skip_b > 0 {
                self.stats.dropped += 1;
                return;
            }
            self.cur = (0..3).find(|i| *i != f && *i != b).unwrap_or(0);
            damaged =
                self.tainted || self.b_lost || self.frames[f].damaged() || self.frames[b].damaged();
            inherited = self.frames[f].lost().max(self.frames[b].lost());
        } else {
            self.skip_b = self.skip_b.saturating_sub(1);
            self.release(sink);
            if p.kind == 2 && self.bwd.is_none() {
                self.stats.dropped += 1;
                return;
            }
            self.fwd = self.bwd.take();
            self.cur = (0..3).find(|i| Some(*i) != self.fwd).unwrap_or(0);
            if p.kind == 1 {
                self.tainted = false;
            } else {
                damaged = self.tainted || self.fwd.is_some_and(|f| self.frames[f].damaged());
                inherited = self.fwd.map_or(0, |f| self.frames[f].lost());
            }
        }
        let top_field_first = if p.structure == FRAME {
            p.top_field_first
        } else {
            p.structure == TOP_FIELD
        };
        self.frames[self.cur].info = Some(Info {
            pts,
            top_field_first,
            repeat_first_field: p.repeat_first_field,
            progressive: p.progressive_frame,
            kind: p.kind,
            temporal_reference: p.temporal_reference,
            concealed: 0,
            damaged,
            inherited: if self.tainted || (self.b_lost && p.kind == 3) {
                u32::MAX
            } else {
                inherited
            },
        });
        self.done.fill(0);
        self.paired = false;
        self.open = true;
        self.awaited = match p.structure {
            TOP_FIELD => Some(BOTTOM_FIELD),
            BOTTOM_FIELD => Some(TOP_FIELD),
            _ => None,
        };
        self.pic = Some(p);
    }

    /// Ends the open frame: conceals what is missing, then gives a B frame to the caller and keeps another frame for
    /// prediction.
    fn finish(&mut self, sink: &mut dyn FnMut(&Frame<'_>)) {
        self.open = false;
        self.pic = None;
        let awaited = self.awaited.take();
        let fields = awaited.is_some();
        // A frame that has only the field that came second before has lost its first field: it gets the time and
        // the field order of the frame that was coded.
        let first = self
            .first_field
            .filter(|f| !self.paired && awaited == Some(*f));
        if let (Some(first), Some(seq)) = (first, &self.seq) {
            let rate = seq.frame_rate();
            let field = 45_000 * rate.1 as i64 / rate.0.max(1) as i64;
            if let Some(i) = &mut self.frames[self.cur].info {
                i.pts = i.pts.map(|t| t.saturating_sub(field));
                i.top_field_first = first == TOP_FIELD;
            }
        }
        self.conceal(fields);
        if self.unit_damaged {
            self.frames[self.cur].mark_damaged();
        }
        let kind = self.frames[self.cur].info.map_or(0, |i| i.kind);
        if kind == 3 {
            self.emit(self.cur, sink);
        } else {
            self.bwd = Some(self.cur);
            if self.seq.as_ref().is_some_and(|s| s.low_delay) {
                self.emit(self.cur, sink);
            } else {
                self.held = true;
            }
        }
    }

    /// Gives the frame that is held back to the caller.
    fn release(&mut self, sink: &mut dyn FnMut(&Frame<'_>)) {
        if let (true, Some(b)) = (std::mem::take(&mut self.held), self.bwd) {
            self.emit(b, sink);
        }
    }

    /// Copies every macroblock of the open frame that was not decoded from the frame before, when there is one.
    fn conceal(&mut self, fields: bool) {
        let Some(seq) = &self.seq else { return };
        let (mb_w, stride) = (seq.mb_w, seq.mb_w * 16);
        let rows = if fields { seq.mb_h / 2 } else { seq.mb_h };
        let mut count = 0u32;
        let from = self.fwd.filter(|f| *f != self.cur);
        for (i, d) in self.done[..mb_w * rows].iter().enumerate() {
            let (x, y) = (i % mb_w, i / mb_w);
            // Per field of a field picture: the first row and the step between rows, in frame rows.
            let parts: [Option<(usize, usize)>; 2] = if fields {
                [
                    (d & 1 == 0).then_some((32 * y, 2)),
                    (d & 2 == 0).then_some((32 * y + 1, 2)),
                ]
            } else {
                [(d & 1 == 0).then_some((16 * y, 1)), None]
            };
            for (row, step) in parts.into_iter().flatten() {
                count += 1;
                let Some(from) = from else { continue };
                let [a, b, c] = &mut self.frames;
                let (dst, src) = match (self.cur, from) {
                    (0, 1) => (a, &*b),
                    (0, 2) => (a, &*c),
                    (1, 0) => (b, &*a),
                    (1, 2) => (b, &*c),
                    (2, 0) => (c, &*a),
                    _ => (c, &*b),
                };
                for k in 0..16 {
                    let at = (row + k * step) * stride + 16 * x;
                    dst.y[at..at + 16].copy_from_slice(&src.y[at..at + 16]);
                }
                let crow = if fields { 16 * y + (row & 1) } else { 8 * y };
                for k in 0..8 {
                    let at = (crow + k * step) * (stride / 2) + 8 * x;
                    dst.cb[at..at + 8].copy_from_slice(&src.cb[at..at + 8]);
                    dst.cr[at..at + 8].copy_from_slice(&src.cr[at..at + 8]);
                }
            }
        }
        self.stats.concealed += count as u64;
        if let Some(i) = &mut self.frames[self.cur].info {
            i.damaged |= count > 0;
            i.concealed = count;
        }
    }

    fn emit(&mut self, index: usize, sink: &mut dyn FnMut(&Frame<'_>)) {
        let (Some(seq), Some(info)) = (&self.seq, self.frames[index].info) else {
            return;
        };
        let rate = seq.frame_rate();
        let fields = if info.repeat_first_field { 3 } else { 2 };
        let duration = 45_000 * fields * rate.1 as i64 / rate.0.max(1) as i64;
        let pts = info.pts.or(self.last_pts);
        self.last_pts = pts.map(|p| p.saturating_add(duration));
        let f = &self.frames[index];
        self.stats.frames += 1;
        sink(&Frame {
            y: &f.y,
            cb: &f.cb,
            cr: &f.cr,
            stride: seq.mb_w * 16,
            chroma_stride: seq.mb_w * 8,
            width: seq.width,
            height: seq.height,
            aspect: seq.aspect(),
            frame_rate: rate,
            matrix_coefficients: seq.matrix,
            pts,
            interlaced: !info.progressive,
            top_field_first: info.top_field_first,
            repeat_first_field: info.repeat_first_field,
            kind: match info.kind {
                1 => PictureKind::I,
                2 => PictureKind::P,
                _ => PictureKind::B,
            },
            concealed: info.concealed,
            damaged: info.damaged,
            damaged_macroblocks: if info.damaged {
                let all = (seq.mb_w * seq.mb_h) as u32;
                info.concealed.saturating_add(info.inherited).clamp(1, all)
            } else {
                0
            },
        });
    }
}
