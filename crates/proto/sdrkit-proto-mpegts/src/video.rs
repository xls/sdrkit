//! Decoded video of a service: the access units of an MPEG-2 video stream to frames of raw pixels in display order,
//! for a host that has no decoder of MPEG-2 video.

use crate::es::EsPacket;
use sdrkit_codec_mpeg2video::{Decoder, Frame};
use sdrkit_core::media::{
    Codec, ColourMatrix, MediaFormat, Packaging, PixelFormat, PixelLayout, VideoProps,
};
use sdrkit_core::Packet;
use sdrkit_graph::{MediaPacket, PacketPool, Timed};
use std::sync::Arc;

/// The formats of the frames of one picture size and field order: of a frame as decoded, and of the last good frame
/// sent again.
struct Formats {
    video: VideoProps,
    matrix: ColourMatrix,
    clean: Arc<MediaFormat>,
    damaged: Arc<MediaFormat>,
    bytes: usize,
}

/// A frame leaves when at most one in this many of its macroblocks is damaged.
const HOLD_ABOVE: usize = 8;

/// The decoder of the video stream of one PID.
pub(crate) struct VideoDecoder {
    /// The PID of the stream.
    pub pid: u16,
    decoder: Decoder,
    pool: PacketPool,
    formats: Option<Formats>,
    /// The last frame that left as decoded.
    good: Option<Packet>,
}

/// What the stream says about `f`.
fn props(f: &Frame) -> VideoProps {
    VideoProps {
        width: f.width,
        height: f.height,
        frame_rate: Some(f.frame_rate),
        aspect: Some(f.aspect),
        interlaced: f.interlaced,
        top_field_first: f.top_field_first,
        damaged: false,
    }
}

/// The colour matrix of `f`: that of the colour description of the stream, else BT.601 for standard definition and
/// BT.709 above it. MPEG-2 video is always of video range.
fn matrix(f: &Frame) -> ColourMatrix {
    match f.matrix_coefficients {
        Some(1) => ColourMatrix::Bt709,
        Some(5 | 6) => ColourMatrix::Bt601,
        _ if f.height <= 576 => ColourMatrix::Bt601,
        _ => ColourMatrix::Bt709,
    }
}

/// The formats of frames with the properties `video` and the colour matrix `matrix`, in the time base `timebase`.
fn formats(video: VideoProps, matrix: ColourMatrix, timebase: (u32, u32)) -> Formats {
    let mut layout = PixelLayout::packed(PixelFormat::I420, video.width, video.height);
    layout.matrix = matrix;
    layout.full_range = false;
    let bytes = layout.frame_bytes();
    let mut f = MediaFormat::new(Codec::Pixels(PixelFormat::I420), Packaging::Raw);
    f.timebase = timebase;
    f.pixels = Some(layout);
    f.video = Some(video.clone());
    let clean = Arc::new(f.clone());
    if let Some(v) = f.video.as_mut() {
        v.damaged = true;
    }
    Formats {
        video,
        matrix,
        clean,
        damaged: Arc::new(f),
        bytes,
    }
}

/// Copies the picture of `f` to `to`: the three planes one after the other, each row as wide as the picture.
fn pack(f: &Frame, to: &mut [u8]) {
    let (w, h) = (f.width as usize, f.height as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let (luma, chroma) = to.split_at_mut(w * h);
    let (cb, cr) = chroma.split_at_mut(cw * ch);
    for (row, src) in luma.chunks_exact_mut(w).zip(f.y.chunks(f.stride)) {
        row.copy_from_slice(&src[..w]);
    }
    for (plane, src) in [(cb, f.cb), (cr, f.cr)] {
        for (row, src) in plane.chunks_exact_mut(cw).zip(src.chunks(f.chroma_stride)) {
            row.copy_from_slice(&src[..cw]);
        }
    }
}

impl VideoDecoder {
    /// A decoder for the stream of `pid`.
    pub fn new(pid: u16) -> Self {
        VideoDecoder {
            pid,
            decoder: Decoder::new(),
            pool: PacketPool::default(),
            formats: None,
            good: None,
        }
    }

    /// Decodes the access unit `m` and adds the frames that are ready to `out`. A frame with much damage does not
    /// leave: the last good frame leaves again in its place, with the time stamp of the damaged one and a format
    /// that says so. Before the first good frame a damaged one gives nothing.
    pub fn push(&mut self, m: &EsPacket, out: &mut Vec<Timed<MediaPacket>>) {
        if m.discontinuity {
            self.decoder.discontinuity();
        }
        if m.damaged {
            self.decoder.damaged();
        }
        let (time, timebase) = (m.packet.time, m.format.timebase);
        let (pool, known, good) = (&mut self.pool, &mut self.formats, &mut self.good);
        self.decoder
            .decode(m.packet.bytes(), m.packet.pts, &mut |f| {
                let (video, matrix) = (props(f), matrix(f));
                if known
                    .as_ref()
                    .is_none_or(|k| k.video != video || k.matrix != matrix)
                {
                    let new = formats(video, matrix, timebase);
                    // The last good frame of another picture size is no stand-in.
                    if known.as_ref().is_some_and(|k| k.bytes != new.bytes) {
                        *good = None;
                    }
                    *known = Some(new);
                }
                let Some(k) = known.as_ref() else { return };
                // A frame with little damage is shown: its lost parts are a copy of the picture before.
                let total = f.stride / 16 * (f.y.len() / f.stride / 16);
                let held = f.damaged_macroblocks as usize * HOLD_ABOVE > total;
                let (mut packet, format) = if held {
                    let Some(last) = good.clone() else { return };
                    (Packet { time, ..last }, &k.damaged)
                } else {
                    let packet = pool.make(k.bytes, time, |b| pack(f, b));
                    *good = Some(packet.clone());
                    (packet, &k.clean)
                };
                packet.pts = f.pts;
                packet.dts = None;
                packet.keyframe = true;
                packet.clean_start = true;
                out.push(Timed {
                    time,
                    value: MediaPacket::new(packet, Arc::clone(format)),
                });
            });
    }
}
