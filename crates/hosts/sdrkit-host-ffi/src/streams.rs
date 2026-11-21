//! The streams of a session: audio, transmit samples, media packets, pictures, spectrum frames, taps, and samples that the
//! caller pushes.

use crate::engine::session_of;
use crate::error::{stream_type, Error, SDRKIT_STREAM_NONE};
use crate::handle::{run, slice_in, slice_out, str_in, Handle, Out, What};
use crate::value::{Bytes, Id};
use sdrkit_core::{Complex32, MediaFormat, Packaging, StreamKind};
use sdrkit_graph::SampleType;
use sdrkit_host_engine::{MediaRead, Samples, TapId};
use std::ffi::{c_char, c_void};
use std::sync::Arc;

/// 32-bit floats, I then Q.
pub const SDRKIT_SAMPLE_CF32: i32 = 0;
/// Signed 16-bit integers, I then Q, little-endian.
pub const SDRKIT_SAMPLE_CS16: i32 = 1;
/// Signed 32-bit integers, I then Q, little-endian.
pub const SDRKIT_SAMPLE_CS32: i32 = 2;
/// Signed 8-bit integers, I then Q.
pub const SDRKIT_SAMPLE_CS8: i32 = 3;
/// Unsigned 8-bit integers with zero at 127.5, I then Q.
pub const SDRKIT_SAMPLE_CU8: i32 = 4;

/// H.264 video.
pub const SDRKIT_MEDIA_H264: i32 = 0;
/// HEVC video.
pub const SDRKIT_MEDIA_HEVC: i32 = 1;
/// MPEG-2 video.
pub const SDRKIT_MEDIA_MPEG2_VIDEO: i32 = 2;
/// AAC audio.
pub const SDRKIT_MEDIA_AAC: i32 = 3;
/// HE-AAC audio, v1 or v2.
pub const SDRKIT_MEDIA_HE_AAC: i32 = 4;
/// xHE-AAC audio.
pub const SDRKIT_MEDIA_XHE_AAC: i32 = 5;
/// AC-3 audio.
pub const SDRKIT_MEDIA_AC3: i32 = 6;
/// E-AC-3 audio.
pub const SDRKIT_MEDIA_EAC3: i32 = 7;
/// MPEG-1 Layer I audio.
pub const SDRKIT_MEDIA_MP1: i32 = 8;
/// MPEG-1 Layer II audio.
pub const SDRKIT_MEDIA_MP2: i32 = 9;
/// MPEG-1 Layer III audio.
pub const SDRKIT_MEDIA_MP3: i32 = 10;
/// A still picture, compressed or raw pixels. `fourcc` selects the format.
pub const SDRKIT_MEDIA_PICTURE: i32 = 11;
/// An MPEG transport stream: whole 188-byte transport packets.
pub const SDRKIT_MEDIA_MPEG_TS: i32 = 12;
/// Decoded video: one frame of raw pixels in display order. `fourcc` selects the pixel format.
pub const SDRKIT_MEDIA_RAW_VIDEO: i32 = 13;

/// Bare access units.
pub const SDRKIT_PACKAGING_RAW: i32 = 0;
/// H.264 or HEVC with start codes.
pub const SDRKIT_PACKAGING_ANNEX_B: i32 = 1;
/// H.264 or HEVC with a big-endian length of `length_size` bytes before each unit.
pub const SDRKIT_PACKAGING_LENGTH_PREFIXED: i32 = 2;
/// AAC with ADTS headers.
pub const SDRKIT_PACKAGING_ADTS: i32 = 3;
/// AAC in LATM / LOAS.
pub const SDRKIT_PACKAGING_LATM: i32 = 4;

/// Channels in an order that the library does not know.
pub const SDRKIT_CHANNEL_LAYOUT_UNKNOWN: i32 = 0;
/// One channel.
pub const SDRKIT_CHANNEL_LAYOUT_MONO: i32 = 1;
/// Left, right.
pub const SDRKIT_CHANNEL_LAYOUT_STEREO: i32 = 2;
/// 5.1: left, centre, right, left surround, right surround, low-frequency effects.
pub const SDRKIT_CHANNEL_LAYOUT_SURROUND_5_1: i32 = 3;

/// Pixels whose colour matrix the library does not know, or no pixels.
pub const SDRKIT_COLOUR_MATRIX_UNKNOWN: i32 = 0;
/// ITU-R BT.601 (standard definition).
pub const SDRKIT_COLOUR_MATRIX_BT601: i32 = 1;
/// ITU-R BT.709 (high definition).
pub const SDRKIT_COLOUR_MATRIX_BT709: i32 = 2;
/// ITU-R BT.2020.
pub const SDRKIT_COLOUR_MATRIX_BT2020: i32 = 3;
/// RGB or grey: no matrix.
pub const SDRKIT_COLOUR_MATRIX_IDENTITY: i32 = 4;

/// The audio that `sdrkit_audio_read` copied.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AudioInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of frames copied, or 0 if no audio was ready.
    pub frames: usize,
    /// Frames per second.
    pub rate_hz: f64,
    /// Samples per frame.
    pub channels: u16,
    /// The order of the channels: one of `SDRKIT_CHANNEL_LAYOUT_*`.
    pub layout: i32,
}

/// The `SDRKIT_CHANNEL_LAYOUT_*` code of `layout`.
fn layout_code(layout: sdrkit_core::ChannelLayout) -> i32 {
    use sdrkit_core::ChannelLayout as L;
    match layout {
        L::Unknown => SDRKIT_CHANNEL_LAYOUT_UNKNOWN,
        L::Mono => SDRKIT_CHANNEL_LAYOUT_MONO,
        L::Stereo => SDRKIT_CHANNEL_LAYOUT_STEREO,
        L::Surround51 => SDRKIT_CHANNEL_LAYOUT_SURROUND_5_1,
    }
}

/// The complex samples that `sdrkit_transmit_read` copied.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IqInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of samples copied, or 0 if no samples were ready.
    pub samples: usize,
    /// Samples per second.
    pub rate_hz: f64,
    /// The frequency at 0 Hz of the samples.
    pub centre_hz: f64,
}

/// The media packet or picture that `sdrkit_media_read` or `sdrkit_picture_read` returned.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MediaInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// True if a packet follows the counter `after`. The other members describe it.
    pub found: bool,
    /// The counter of the packet. Pass it as `after` to the next call.
    pub counter: u64,
    /// The number of packets that the session dropped between `after` and this packet.
    pub missed: u64,
    /// An `SDRKIT_MEDIA_*` constant: the decoder that the packet needs.
    pub r#type: i32,
    /// The four-character code of the codec or of the pixel format.
    pub fourcc: [u8; 4],
    /// An `SDRKIT_PACKAGING_*` constant: how packets are framed.
    pub packaging: i32,
    /// For `SDRKIT_PACKAGING_LENGTH_PREFIXED`: the number of bytes of each length.
    pub length_size: u8,
    /// The decoder configuration.
    pub config: Bytes,
    /// The numerator of the unit of `pts` and `dts`, in seconds.
    pub timebase_num: u32,
    /// The denominator of that unit.
    pub timebase_den: u32,
    /// True if `pts` holds a presentation time.
    pub has_pts: bool,
    /// The presentation time.
    pub pts: i64,
    /// True if `dts` holds a decoding time.
    pub has_dts: bool,
    /// The decoding time.
    pub dts: i64,
    /// True if the packet starts a group that decodes alone.
    pub keyframe: bool,
    /// The time of the signal at which the session made the packet, in seconds.
    pub time: f64,
    /// The bytes of the packet.
    pub bytes: Bytes,
    /// For raw pixels: the width in pixels, else 0.
    pub width: u32,
    /// For raw pixels: the height in pixels, else 0.
    pub height: u32,
    /// For raw pixels: the bytes per row of each plane, 0 for a plane that does not exist.
    pub strides: [usize; 4],
    /// For video: the aspect ratio of the displayed picture, width to height (16 and 9). 0 and 0 if it is not known.
    pub aspect_num: u32,
    /// The height part of the aspect ratio.
    pub aspect_den: u32,
    /// For video: the frames per second as a fraction. 0 and 0 if the rate is not known.
    pub frame_rate_num: u32,
    /// The denominator of the frame rate.
    pub frame_rate_den: u32,
    /// For decoded video: true if the two fields of the frame are from different times.
    pub interlaced: bool,
    /// For decoded video: true if the top field is the first in time.
    pub top_field_first: bool,
    /// For decoded video: true if the stream lost or damaged pictures and the frame is the last good one again.
    pub damaged: bool,
    /// True if the packet is a keyframe after which nothing refers to data before it: an H.264 or HEVC IDR picture, an
    /// HEVC BLA picture, a decoded frame, an audio frame. A decoder that starts at a keyframe that is not a clean start
    /// (a recovery point, a CRA picture, an MPEG-2 I picture) can show damage until the next clean start. Some streams
    /// have no clean start at all (MPEG-2 video, H.264 with recovery points only): a host that waits for one in vain
    /// starts at a keyframe and drops the pictures with an earlier `pts` until the next keyframe.
    pub clean_start: bool,
    /// For raw pixels: one of `SDRKIT_COLOUR_MATRIX_*`. `SDRKIT_COLOUR_MATRIX_UNKNOWN` for coded media.
    pub colour_matrix: i32,
    /// For raw pixels: true for full range (0 to 255), false for video range (16 to 235).
    pub full_range: bool,
}

/// The `SDRKIT_COLOUR_MATRIX_*` code of `m`.
fn matrix_code(m: sdrkit_core::media::ColourMatrix) -> i32 {
    use sdrkit_core::media::ColourMatrix as M;
    match m {
        M::Bt601 => SDRKIT_COLOUR_MATRIX_BT601,
        M::Bt709 => SDRKIT_COLOUR_MATRIX_BT709,
        M::Bt2020 => SDRKIT_COLOUR_MATRIX_BT2020,
        M::Identity => SDRKIT_COLOUR_MATRIX_IDENTITY,
    }
}

/// The spectrum frame that `sdrkit_spectrum_read` copied.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SpectrumFrame {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// True if a frame came after the last call. The other members describe it.
    pub found: bool,
    /// The number of bins copied to the buffer of the caller.
    pub bins: u32,
    /// The number of bins of the frame. It is more than `bins` if the buffer was too small.
    pub total: u32,
    /// The frequency at the middle bin, in Hz.
    pub centre_hz: f64,
    /// The width of the frame in Hz.
    pub span_hz: f64,
    /// Seconds of spectrum before this frame: the number of the frames that the session kept before it, divided by
    /// `frame_rate`.
    pub time: f64,
    /// Frames per second.
    pub frame_rate: f64,
}

/// The part of a tapped stream that `sdrkit_tap_read` copied.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TapInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of values copied, or 0 if nothing was ready.
    pub values: usize,
    /// The `SDRKIT_STREAM_*` type of the stream. For `SDRKIT_STREAM_IQ` the values are I then Q.
    pub r#type: i32,
    /// Items per second.
    pub rate_hz: f64,
    /// The frequency at 0 Hz of the stream.
    pub centre_hz: f64,
    /// The number of items that the tap dropped because the caller did not read in time.
    pub dropped: u64,
}

/// The stream buffers of one handle.
#[derive(Default)]
pub(crate) struct Buffers {
    spectrum: Vec<f32>,
    media: Vec<u8>,
    format: Option<Arc<MediaFormat>>,
}

fn media_type(k: StreamKind) -> i32 {
    match k {
        StreamKind::H264 => SDRKIT_MEDIA_H264,
        StreamKind::Hevc => SDRKIT_MEDIA_HEVC,
        StreamKind::Mpeg2Video => SDRKIT_MEDIA_MPEG2_VIDEO,
        StreamKind::Aac => SDRKIT_MEDIA_AAC,
        StreamKind::HeAac => SDRKIT_MEDIA_HE_AAC,
        StreamKind::XheAac => SDRKIT_MEDIA_XHE_AAC,
        StreamKind::Ac3 => SDRKIT_MEDIA_AC3,
        StreamKind::EAc3 => SDRKIT_MEDIA_EAC3,
        StreamKind::Mp1 => SDRKIT_MEDIA_MP1,
        StreamKind::Mp2 => SDRKIT_MEDIA_MP2,
        StreamKind::Mp3 => SDRKIT_MEDIA_MP3,
        StreamKind::Picture => SDRKIT_MEDIA_PICTURE,
        StreamKind::MpegTs => SDRKIT_MEDIA_MPEG_TS,
        StreamKind::RawVideo => SDRKIT_MEDIA_RAW_VIDEO,
    }
}

fn media_out(b: &mut Buffers, got: Option<MediaRead>, after: u64) -> MediaInfo {
    let mut r = MediaInfo {
        size: 0,
        found: false,
        counter: after,
        missed: 0,
        r#type: SDRKIT_MEDIA_PICTURE,
        fourcc: [0; 4],
        packaging: SDRKIT_PACKAGING_RAW,
        length_size: 0,
        config: Bytes::of(&[]),
        timebase_num: 0,
        timebase_den: 0,
        has_pts: false,
        pts: 0,
        has_dts: false,
        dts: 0,
        keyframe: false,
        time: 0.0,
        bytes: Bytes::of(&[]),
        width: 0,
        height: 0,
        strides: [0; 4],
        aspect_num: 0,
        aspect_den: 0,
        frame_rate_num: 0,
        frame_rate_den: 0,
        interlaced: false,
        top_field_first: false,
        damaged: false,
        clean_start: false,
        colour_matrix: SDRKIT_COLOUR_MATRIX_UNKNOWN,
        full_range: false,
    };
    let Some(m) = got else {
        return r;
    };
    let f = &*b.format.insert(m.format);
    (r.packaging, r.length_size) = match f.packaging {
        Packaging::Raw => (SDRKIT_PACKAGING_RAW, 0),
        Packaging::AnnexB => (SDRKIT_PACKAGING_ANNEX_B, 0),
        Packaging::LengthPrefixed(n) => (SDRKIT_PACKAGING_LENGTH_PREFIXED, n),
        Packaging::Adts => (SDRKIT_PACKAGING_ADTS, 0),
        Packaging::Latm => (SDRKIT_PACKAGING_LATM, 0),
    };
    if let Some(p) = &f.pixels {
        r.width = p.width;
        r.height = p.height;
        for (d, s) in r.strides.iter_mut().zip(&p.strides) {
            *d = *s;
        }
        r.colour_matrix = matrix_code(p.matrix);
        r.full_range = p.full_range;
    }
    if let Some(v) = &f.video {
        (r.aspect_num, r.aspect_den) = v.aspect.unwrap_or((0, 0));
        (r.frame_rate_num, r.frame_rate_den) = v.frame_rate.unwrap_or((0, 0));
        r.interlaced = v.interlaced;
        r.top_field_first = v.top_field_first;
        r.damaged = v.damaged;
    }
    r.found = true;
    r.counter = m.counter;
    r.missed = m.missed;
    r.r#type = media_type(m.kind);
    r.fourcc = f.fourcc().0;
    r.config = Bytes::of(&f.config);
    (r.timebase_num, r.timebase_den) = f.timebase;
    r.has_pts = m.pts.is_some();
    r.pts = m.pts.unwrap_or(0);
    r.has_dts = m.dts.is_some();
    r.dts = m.dts.unwrap_or(0);
    r.keyframe = m.keyframe;
    r.clean_start = m.clean_start;
    r.time = m.time;
    r.bytes = Bytes::of(&b.media);
    r
}

/// Copies the waiting audio of `session` into `out`, at most `capacity` floats, and fills `info`. The call copies whole
/// interleaved frames of one rate and one channel count. Audio that the caller does not read in time is lost, and the
/// result field `session.audio_lost` counts it.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_audio_read(
    session: Handle,
    out: *mut f32,
    capacity: usize,
    info: *mut AudioInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let (buf, info) = unsafe { (slice_out(out, capacity), Out::new(info)) };
    run(session, |e| {
        let (buf, info) = (buf?, info?);
        let got = e.host.engine.audio(session_of(e)?, buf)?;
        info.put(AudioInfo {
            size: 0,
            frames: got.map_or(0, |a| a.frames),
            rate_hz: got.map_or(0.0, |a| a.rate_hz),
            channels: got.map_or(0, |a| a.channels),
            layout: got.map_or(SDRKIT_CHANNEL_LAYOUT_UNKNOWN, |a| layout_code(a.layout)),
        });
        Ok(())
    })
}

/// Gives `session` `frames` frames of audio at `rate_hz`, such as a microphone, for the host audio input (`host.in`)
/// of its graph. A frame has `channels` floats: 1 for mono, or 2 for stereo as left then right, so `samples` holds
/// `frames` times `channels` floats. `samples` can be NULL when `frames` is 0. The engine converts the audio to the
/// rate and the channel count of that input: mono audio goes to both channels of a stereo input, and stereo audio
/// becomes the mean of left and right for a mono input. The channel count and the rate can change from call to call;
/// the audio from before that the graph has not taken is then left out. The session keeps 32768 floats (0.68 s of
/// mono or 0.34 s of stereo at 48 kHz): audio that the graph does not take in time is lost, oldest first, and the
/// result field `session.audio_in_lost` counts it. A session that transmits asks for audio: the field
/// `session.audio_in_rate_hz` has a value while it can take it, and `session.audio_in_wanted` is true while a
/// transmission that sends it runs or push-to-talk is pressed. Open the microphone only then.
///
/// The call never waits and allocates nothing, so a real-time audio thread can make it, also while another thread uses
/// the handle. It takes only the lock of the handle table of the library, which no thread of the engine takes, and no
/// lock of the engine or of the session. One thread at a time writes the audio of a session; stop the writes before
/// the handle or the engine is freed. Returns `SDRKIT_E_UNSUPPORTED` for a channel count that is not 1 or 2, with
/// the count in the member `value` of the error detail, and `SDRKIT_E_CONFIG` for a rate that is not above 0 or not
/// finite, with the rate in the member `value`; an error is kept as the last error of the engine, which takes a lock
/// and can allocate.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_session_write_audio(
    session: Handle,
    samples: *const f32,
    frames: u32,
    channels: u16,
    rate_hz: f64,
) -> i32 {
    // For a channel count that the engine refuses, no samples are read.
    let count = if (1..=2).contains(&channels) {
        frames as usize * usize::from(channels)
    } else {
        0
    };
    // SAFETY: the caller's contract.
    let samples = unsafe { slice_in(samples, count) };
    run(session, |e| {
        let samples = samples?;
        let input = e.audio_in.as_ref().ok_or(Error::WrongHandle)?;
        Ok(input.write(samples, channels, rate_hz)?)
    })
}

/// Copies the samples that the component `tx.sink` of the graph of `session` wrote into `out`, at most `capacity` samples of
/// two floats (I then Q), and fills `info`. Samples that the caller does not read in time are lost, and the result field
/// `session.transmit_lost` counts them. Returns `SDRKIT_E_TO_RADIO` if the engine sends the samples to the radio.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_transmit_read(
    session: Handle,
    out: *mut f32,
    capacity: usize,
    info: *mut IqInfo,
) -> i32 {
    // SAFETY: the caller's contract. `Complex32` is two `f32` in a `repr(C)` struct, aligned as `f32`.
    let (buf, info) = unsafe { (slice_out(out.cast::<Complex32>(), capacity), Out::new(info)) };
    run(session, |e| {
        let (buf, info) = (buf?, info?);
        let got = e.host.engine.transmit(session_of(e)?, buf)?;
        info.put(IqInfo {
            size: 0,
            samples: got.map_or(0, |r| r.samples),
            rate_hz: got.map_or(0.0, |r| r.rate_hz),
            centre_hz: got.map_or(0.0, |r| r.centre_hz),
        });
        Ok(())
    })
}

/// Writes `size` bytes of samples of the `SDRKIT_SAMPLE_*` format `format` to the host feed `feed`, for every session that
/// reads it. The call does not wait. Returns `SDRKIT_E_PART_SAMPLE`, or `SDRKIT_E_BEHIND` if a session of the feed has
/// not used the previous samples: the call then writes no samples, and the caller tries again later.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_samples_push(
    feed: Handle,
    format: i32,
    data: *const c_void,
    size: usize,
) -> i32 {
    // SAFETY: the caller's contract.
    let bytes = unsafe { slice_in(data.cast::<u8>(), size) };
    run(feed, |e| {
        let bytes = bytes?;
        let sample = match format {
            SDRKIT_SAMPLE_CF32 => SampleType::Cf32,
            SDRKIT_SAMPLE_CS16 => SampleType::Cs16,
            SDRKIT_SAMPLE_CS32 => SampleType::Cs32,
            SDRKIT_SAMPLE_CS8 => SampleType::Cs8,
            SDRKIT_SAMPLE_CU8 => SampleType::Cu8,
            _ => return Err(Error::BadArgument),
        };
        let What::Feed(id) = e.what else {
            return Err(Error::WrongHandle);
        };
        Ok(e.host.engine.push(id, Samples::Raw { sample, bytes })?)
    })
}

/// Fills `info` with the first media packet of `session` after the counter `after`. Pass 0 for the oldest packet. The
/// session keeps the coded packets of the last ten seconds, at most 32 MiB, and decoded video frames of the last half
/// second, at most 16 MiB; the result field `session.media_lost` counts those that left unread. Pictures are not media
/// packets.
/// The arrays and strings of `info` are views that the library owns, valid until the next call on `session`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_media_read(
    session: Handle,
    after: u64,
    info: *mut MediaInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(session, |e| {
        let out = out?;
        let id = session_of(e)?;
        let mut st = e.state();
        let b = &mut st.streams;
        let got = e.host.engine.media(id, after, &mut b.media)?;
        out.put(media_out(b, got, after));
        Ok(())
    })
}

/// Fills `info` with the first picture of `session` after the counter `after`. Pass 0 for the oldest picture. A picture is
/// compressed bytes, or raw pixels in the layout of `width`, `height` and `strides`. The session keeps its last 16
/// pictures.
/// The arrays and strings of `info` are views that the library owns, valid until the next call on `session`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_picture_read(
    session: Handle,
    after: u64,
    info: *mut MediaInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(session, |e| {
        let out = out?;
        let id = session_of(e)?;
        let mut st = e.state();
        let b = &mut st.streams;
        let got = e.host.engine.picture(id, after, &mut b.media)?;
        out.put(media_out(b, got, after));
        Ok(())
    })
}

/// Copies the newest spectrum frame of `session` into `bins`, lowest frequency first, if a frame came after the last
/// call, and fills `frame`. `bins` holds `capacity` values in dB. If the frame has more bins, the call copies the
/// first `capacity` and writes the number of bins of the frame to `frame.total`: read the next frame with a buffer of
/// that size. The setting `spectrum` of the session turns the spectrum on, and `spectrum_size` sets the number of
/// bins. `bins` can be NULL with a `capacity` of 0.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_spectrum_read(
    session: Handle,
    bins: *mut f32,
    capacity: u32,
    frame: *mut SpectrumFrame,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(frame) };
    run(session, |e| {
        let out = out?;
        if bins.is_null() && capacity > 0 {
            return Err(Error::Null);
        }
        let id = session_of(e)?;
        let mut st = e.state();
        let values = &mut st.streams.spectrum;
        let got = e.host.engine.spectrum(id, values)?;
        let total = got.map_or(0, |f| f.bins);
        let copied = total.min(capacity as usize);
        if copied > 0 {
            // SAFETY: `bins` holds `capacity` values, and `copied` is at most `capacity`.
            unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), bins, copied) };
        }
        out.put(SpectrumFrame {
            size: 0,
            found: got.is_some(),
            bins: crate::value::count(copied),
            total: crate::value::count(total),
            centre_hz: got.map_or(0.0, |f| f.centre_hz),
            span_hz: got.map_or(0.0, |f| f.span_hz),
            time: got.map_or(0.0, |f| f.time_s),
            frame_rate: got.map_or(0.0, |f| f.frame_rate),
        });
        Ok(())
    })
}

/// Opens a tap on the output `port` (`"<component>.<port>"`, or `"input"`) of the graph of `session`, and writes its id to
/// `tap`. A tap copies the stream at that point into at most `chunks` buffers until the caller reads them. Returns
/// `SDRKIT_E_NO_GRAPH` or `SDRKIT_E_NO_SUCH_KEY`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_tap_open(
    session: Handle,
    port: *const c_char,
    chunks: usize,
    tap: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    let (port, out) = unsafe { (str_in(port), Out::new(tap)) };
    run(session, |e| {
        let (port, out) = (port?, out?);
        let t = e.host.engine.open_tap(session_of(e)?, port, chunks)?;
        out.put(t.0);
        Ok(())
    })
}

/// Copies the waiting values of tap `tap` of `session` into `out`, at most `capacity` floats, and fills `info`. Returns
/// `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_tap_read(
    session: Handle,
    tap: Id,
    out: *mut f32,
    capacity: usize,
    info: *mut TapInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let (buf, info) = unsafe { (slice_out(out, capacity), Out::new(info)) };
    run(session, |e| {
        let (buf, info) = (buf?, info?);
        let got = e.host.engine.read_tap(session_of(e)?, TapId(tap), buf)?;
        info.put(TapInfo {
            size: 0,
            values: got.map_or(0, |t| t.values),
            r#type: got.map_or(SDRKIT_STREAM_NONE, |t| stream_type(t.kind)),
            rate_hz: got.map_or(0.0, |t| t.rate_hz),
            centre_hz: got.map_or(0.0, |t| t.centre_hz),
            dropped: got.map_or(0, |t| t.dropped),
        });
        Ok(())
    })
}

/// Closes tap `tap` of `session`. Returns `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
#[no_mangle]
pub extern "C" fn sdrkit_tap_close(session: Handle, tap: Id) -> i32 {
    run(session, |e| {
        Ok(e.host.engine.close_tap(session_of(e)?, TapId(tap))?)
    })
}

crate::handle::sized_structs!(AudioInfo, IqInfo, MediaInfo, SpectrumFrame, TapInfo);
