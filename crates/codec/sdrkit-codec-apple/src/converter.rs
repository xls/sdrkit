//! A safe owner of one AudioToolbox `AudioConverter` that decodes compressed packets into 16-bit interleaved PCM. Every `unsafe` block of the crate is in this module.
//!
//! Standard: Apple SDK headers `CoreAudioTypes/CoreAudioBaseTypes.h` (`AudioStreamBasicDescription`, `AudioBuffer`,
//! `AudioBufferList`, `AudioStreamPacketDescription`, the format flags) and `AudioToolbox/AudioConverter.h`
//! (`AudioConverterNew`, `AudioConverterSetProperty`, `kAudioConverterDecompressionMagicCookie`,
//! `AudioConverterFillComplexBuffer`, `AudioConverterComplexInputDataProc`, `AudioConverterReset`, `AudioConverterDispose`).

use std::ffi::c_void;
use std::ptr;

/// `AudioStreamBasicDescription`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Asbd {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

/// `AudioBuffer`.
#[repr(C)]
struct AudioBuffer {
    number_channels: u32,
    data_byte_size: u32,
    data: *mut c_void,
}

/// `AudioBufferList` with one buffer.
#[repr(C)]
struct AudioBufferList {
    number_buffers: u32,
    buffers: [AudioBuffer; 1],
}

/// `AudioStreamPacketDescription`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PacketDescription {
    start_offset: i64,
    variable_frames_in_packet: u32,
    data_byte_size: u32,
}

type ConverterRef = *mut c_void;

type InputProc = unsafe extern "C" fn(
    ConverterRef,
    *mut u32,
    *mut AudioBufferList,
    *mut *mut PacketDescription,
    *mut c_void,
) -> i32;

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioConverterNew(
        source: *const Asbd,
        destination: *const Asbd,
        out: *mut ConverterRef,
    ) -> i32;
    fn AudioConverterDispose(converter: ConverterRef) -> i32;
    fn AudioConverterReset(converter: ConverterRef) -> i32;
    fn AudioConverterSetProperty(
        converter: ConverterRef,
        id: u32,
        size: u32,
        data: *const c_void,
    ) -> i32;
    fn AudioConverterFillComplexBuffer(
        converter: ConverterRef,
        input: InputProc,
        user: *mut c_void,
        packets: *mut u32,
        output: *mut AudioBufferList,
        descriptions: *mut PacketDescription,
    ) -> i32;
}

/// `kAudioFormatLinearPCM`.
const FORMAT_LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
/// `kAudioFormatFlagIsSignedInteger`.
const FLAG_SIGNED_INTEGER: u32 = 1 << 2;
/// `kAudioFormatFlagIsPacked`.
const FLAG_PACKED: u32 = 1 << 3;
/// `kAudioConverterDecompressionMagicCookie`.
const PROPERTY_MAGIC_COOKIE: u32 = u32::from_be_bytes(*b"dmgc");
/// The status the input callback returns when it has no more packets for this call.
const NO_MORE_INPUT: i32 = i32::from_be_bytes(*b"nomo");

/// The input of a converter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Input {
    /// The AudioToolbox format id (the codec's four-character code, or `lpcm` for 16-bit interleaved PCM).
    pub format_id: u32,
    /// The sample rate of the decoded audio, Hz.
    pub rate: f64,
    /// The channels of the decoded audio.
    pub channels: u32,
    /// The decoded frames in one packet, or 0 if the codec does not fix it. 1 for PCM.
    pub frames_per_packet: u32,
}

impl Input {
    /// 16-bit interleaved PCM at `rate` with `channels`.
    pub(crate) fn pcm(rate: f64, channels: u32) -> Input {
        Input {
            format_id: FORMAT_LINEAR_PCM,
            rate,
            channels,
            frames_per_packet: 1,
        }
    }

    fn is_pcm(&self) -> bool {
        self.format_id == FORMAT_LINEAR_PCM
    }

    fn description(&self) -> Asbd {
        if self.is_pcm() {
            let bytes = 2 * self.channels;
            Asbd {
                sample_rate: self.rate,
                format_id: FORMAT_LINEAR_PCM,
                format_flags: FLAG_SIGNED_INTEGER | FLAG_PACKED,
                bytes_per_packet: bytes,
                frames_per_packet: 1,
                bytes_per_frame: bytes,
                channels_per_frame: self.channels,
                bits_per_channel: 16,
                reserved: 0,
            }
        } else {
            Asbd {
                sample_rate: self.rate,
                format_id: self.format_id,
                frames_per_packet: self.frames_per_packet,
                channels_per_frame: self.channels,
                ..Asbd::default()
            }
        }
    }
}

/// The state the input callback reads: the input, which is passed to the converter once.
struct Feed {
    data: Vec<u8>,
    /// The packets in `data`.
    packets: u32,
    given: bool,
    channels: u32,
    /// The description of a compressed packet.
    description: PacketDescription,
}

/// One AudioToolbox converter to 16-bit interleaved PCM.
pub(crate) struct Converter {
    handle: ConverterRef,
    input: Input,
    output_rate: f64,
    feed: Box<Feed>,
}

// SAFETY: the converter handle is owned by this value alone and every call on it takes `&mut self`, so no two threads use it at
// the same time. A decoder is made and used on the one thread of its guard.
unsafe impl Send for Converter {}

impl Converter {
    /// Makes a converter from `input` to 16-bit interleaved PCM at `output_rate` with the same channel count, and sets
    /// `cookie` (the decoder configuration) if it is not empty. Returns the AudioToolbox status on error.
    pub(crate) fn new(input: Input, output_rate: f64, cookie: &[u8]) -> Result<Converter, i32> {
        let source = input.description();
        let destination = Input::pcm(output_rate, input.channels).description();
        let mut handle: ConverterRef = ptr::null_mut();
        // SAFETY: both descriptions are valid for the call, and `handle` is a valid place for the result.
        let status = unsafe { AudioConverterNew(&source, &destination, &mut handle) };
        if status != 0 || handle.is_null() {
            return Err(status);
        }
        let converter = Converter {
            handle,
            input,
            output_rate,
            feed: Box::new(Feed {
                data: Vec::new(),
                packets: 0,
                given: true,
                channels: input.channels,
                description: PacketDescription::default(),
            }),
        };
        if !cookie.is_empty() {
            let size = u32::try_from(cookie.len()).map_err(|_| -50)?;
            // SAFETY: `handle` is a live converter, and `cookie` is valid for `size` bytes during the call; the converter
            // copies it.
            let status = unsafe {
                AudioConverterSetProperty(
                    handle,
                    PROPERTY_MAGIC_COOKIE,
                    size,
                    cookie.as_ptr().cast(),
                )
            };
            if status != 0 {
                return Err(status);
            }
        }
        Ok(converter)
    }

    /// Decodes one compressed packet and appends its samples to `out`. A decoder with a start-up delay can append fewer
    /// samples than one packet holds, or none. Returns the AudioToolbox status on error.
    pub(crate) fn decode(&mut self, packet: &[u8], out: &mut Vec<i16>) -> Result<(), i32> {
        let feed = &mut *self.feed;
        feed.data.clear();
        feed.data.extend_from_slice(packet);
        feed.packets = 1;
        feed.description = PacketDescription {
            start_offset: 0,
            variable_frames_in_packet: 0,
            data_byte_size: u32::try_from(packet.len()).map_err(|_| -50)?,
        };
        let frames = match self.input.frames_per_packet {
            0 => 4096,
            n => n as usize,
        };
        self.run(frames, out)
    }

    /// Runs the converter on the feed until it takes no more input, with room for `frames` input frames per call.
    fn run(&mut self, frames: usize, out: &mut Vec<i16>) -> Result<(), i32> {
        self.feed.given = false;
        let channels = self.input.channels as usize;
        let capacity = (frames as f64 * self.output_rate / self.input.rate).ceil() as usize + 64;
        loop {
            let at = out.len();
            out.resize(at + capacity * channels, 0);
            let mut done = capacity as u32;
            let mut list = AudioBufferList {
                number_buffers: 1,
                buffers: [AudioBuffer {
                    number_channels: self.input.channels,
                    data_byte_size: (capacity * channels * 2) as u32,
                    data: out[at..].as_mut_ptr().cast(),
                }],
            };
            let feed: *mut Feed = &mut *self.feed;
            // SAFETY: `handle` is a live converter. `list` points at `capacity * channels` writable samples of `out`, which
            // is not touched until the call returns. `feed` points at the boxed feed, which outlives the call, and only
            // `input_proc` reads it during the call.
            let status = unsafe {
                AudioConverterFillComplexBuffer(
                    self.handle,
                    input_proc,
                    feed.cast(),
                    &mut done,
                    &mut list,
                    ptr::null_mut(),
                )
            };
            let done = (done as usize).min(capacity);
            out.truncate(at + done * channels);
            match status {
                0 if done > 0 => continue,
                0 | NO_MORE_INPUT => return Ok(()),
                e => {
                    // SAFETY: `handle` is a live converter.
                    unsafe { AudioConverterReset(self.handle) };
                    return Err(e);
                }
            }
        }
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        // SAFETY: `handle` is a live converter, and no reference to it remains after this call.
        unsafe { AudioConverterDispose(self.handle) };
    }
}

/// The input callback: passes the feed's data to the converter once, then reports that no more input is ready.
unsafe extern "C" fn input_proc(
    _: ConverterRef,
    packets: *mut u32,
    data: *mut AudioBufferList,
    descriptions: *mut *mut PacketDescription,
    user: *mut c_void,
) -> i32 {
    // SAFETY: `user` is the `Feed` pointer that `Converter::decode` passed, valid and not aliased for the whole call of
    // AudioConverterFillComplexBuffer. `packets` and `data` are valid pointers from the converter, and `descriptions` is
    // null or valid, as AudioConverter.h states for this callback.
    unsafe {
        let feed = &mut *user.cast::<Feed>();
        if feed.given || feed.data.is_empty() {
            *packets = 0;
            return NO_MORE_INPUT;
        }
        feed.given = true;
        let list = &mut *data;
        list.number_buffers = 1;
        list.buffers[0] = AudioBuffer {
            number_channels: feed.channels,
            data_byte_size: feed.data.len() as u32,
            data: feed.data.as_mut_ptr().cast(),
        };
        *packets = feed.packets;
        if !descriptions.is_null() && feed.description.data_byte_size > 0 {
            *descriptions = &mut feed.description;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(code: &[u8; 4], rate: f64, channels: u32, frames: u32) -> i32 {
        let input = Input {
            format_id: u32::from_be_bytes(*code),
            rate,
            channels,
            frames_per_packet: frames,
        };
        Converter::new(input, rate, &[]).err().unwrap_or(0)
    }

    #[test]
    fn audiotoolbox_has_decoders_for_layer_i_and_iii_ac3_eac3_and_usac() {
        let got = [
            starts(b".mp1", 48_000.0, 2, 384),
            starts(b".mp3", 48_000.0, 2, 1152),
            starts(b"ac-3", 48_000.0, 2, 1536),
            starts(b"ec-3", 48_000.0, 6, 1536),
            starts(b"usac", 48_000.0, 2, 0),
        ];
        assert_eq!(got, [0; 5]);
    }
}
