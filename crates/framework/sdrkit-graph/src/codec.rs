//! The guard around an audio codec plug-in ([`sdrkit_core::codec`]), and the decoding of the coded audio that leaves a graph.
//!
//! The guard runs each decoder state on its own thread and waits at most [`TIMEOUT`] for a call. A plug-in that panics or hangs
//! is stopped for good, and the caller turns it off in its [`CodecSet`].

use crate::media::SpeechPacket;
use sdrkit_core::codec::{AudioCodec, CodecEntry, CodecError, CodecSet, Pcm};
use sdrkit_core::media::{MediaFormat, MediaType};
use sdrkit_core::{FourCc, Output};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::Duration;

/// How long one call into a plug-in may take.
pub const TIMEOUT: Duration = Duration::from_millis(1500);

struct Job {
    frame: Vec<u8>,
    erasure: bool,
    pcm: Pcm,
}

type Answer = (Vec<u8>, Pcm, Result<(), CodecError>);

/// One decoder state behind the guard.
pub struct GuardedCodec {
    fourcc: FourCc,
    tx: Option<SyncSender<Job>>,
    rx: Receiver<Answer>,
    frame: Vec<u8>,
    pcm: Pcm,
    failed: Option<CodecError>,
    timeout: Duration,
}

impl std::fmt::Debug for GuardedCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardedCodec")
            .field("fourcc", &self.fourcc)
            .field("failed", &self.failed)
            .finish()
    }
}

impl GuardedCodec {
    /// Starts a thread with a decoder state of `entry` for a stream of `format`. If the plug-in does not decode that format,
    /// every call to [`GuardedCodec::decode`] returns [`CodecError::Unsupported`].
    pub fn start(entry: &CodecEntry, format: &MediaFormat) -> GuardedCodec {
        Self::start_with_timeout(entry, format, TIMEOUT)
    }

    /// The same with another time limit (for tests).
    pub fn start_with_timeout(
        entry: &CodecEntry,
        format: &MediaFormat,
        timeout: Duration,
    ) -> GuardedCodec {
        // One job and one answer at a time.
        let (tx, jobs) = sync_channel::<Job>(1);
        let (done, rx) = sync_channel::<Answer>(1);
        let factory = entry.factory.clone();
        let format = format.clone();
        let started = std::thread::Builder::new()
            .name("sdrkit-codec".into())
            .spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut codec: Option<Box<dyn AudioCodec>> = factory(&format);
                    while let Ok(mut job) = jobs.recv() {
                        job.pcm.clear();
                        let r = match codec.as_mut() {
                            Some(c) => c.decode(&job.frame, job.erasure, &mut job.pcm),
                            None => Err(CodecError::Unsupported),
                        };
                        if done.send((job.frame, job.pcm, r)).is_err() {
                            break;
                        }
                    }
                }));
            });
        GuardedCodec {
            fourcc: entry.fourcc,
            failed: started.is_err().then_some(CodecError::Crashed),
            tx: Some(tx),
            rx,
            frame: Vec::new(),
            pcm: Pcm::default(),
            timeout,
        }
    }

    /// The codec's four-character code.
    pub fn fourcc(&self) -> FourCc {
        self.fourcc
    }

    /// Why the guard stopped the plug-in, if it did.
    pub fn failed(&self) -> Option<&CodecError> {
        self.failed.as_ref()
    }

    /// Decodes one frame (or conceals an erasure) under the guard. The samples are valid until the next call. After a timeout, a
    /// crash or [`CodecError::Unsupported`], every later call returns the same error at once.
    pub fn decode(&mut self, frame: &[u8], erasure: bool) -> Result<&Pcm, CodecError> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        let mut f = std::mem::take(&mut self.frame);
        f.clear();
        f.extend_from_slice(frame);
        let job = Job {
            frame: f,
            erasure,
            pcm: std::mem::take(&mut self.pcm),
        };
        let sent = self.tx.as_ref().is_some_and(|t| t.send(job).is_ok());
        if !sent {
            return Err(self.stop(CodecError::Crashed));
        }
        match self.rx.recv_timeout(self.timeout) {
            Ok((f, pcm, r)) => {
                self.frame = f;
                self.pcm = pcm;
                if r == Err(CodecError::Unsupported) {
                    return Err(self.stop(CodecError::Unsupported));
                }
                r.map(|()| &self.pcm)
            }
            Err(RecvTimeoutError::Timeout) => Err(self.stop(CodecError::Timeout)),
            Err(RecvTimeoutError::Disconnected) => Err(self.stop(CodecError::Crashed)),
        }
    }

    /// Decodes one [`SpeechPacket`] under the guard, as [`GuardedCodec::decode`] does. An erasure goes to the plug-in's
    /// concealment with no payload. A damaged frame goes to the plug-in as received, because the speech packagings leave the
    /// FEC of the voice bits to the codec.
    pub fn decode_packet(&mut self, p: &SpeechPacket) -> Result<&Pcm, CodecError> {
        self.decode(p.payload(), p.erasure)
    }

    fn stop(&mut self, e: CodecError) -> CodecError {
        // Dropping the sender ends the thread once the plug-in returns (if it ever does).
        self.tx = None;
        self.failed = Some(e.clone());
        e
    }
}

/// The decoders of the coded audio streams that leave a graph as media packets. A packet whose codec has a plug-in in the run
/// context's [`CodecSet`] is decoded, and its samples are appended to the output's audio; the packet stays in the output's media
/// as it is.
#[derive(Debug, Default)]
pub(crate) struct MediaAudio {
    /// The set the decoders come from, as the run context holds it.
    from: Option<Arc<CodecSet>>,
    /// A copy of `from` in which failed plug-ins are turned off.
    codecs: CodecSet,
    /// One decoder per stream format; `None` once the plug-in refused the stream or failed.
    streams: Vec<(Arc<MediaFormat>, Option<GuardedCodec>)>,
    audio: Vec<f32>,
    notice: Option<String>,
}

impl MediaAudio {
    /// Decodes the packet `bytes` of a stream of `format` with a plug-in of `codecs`, and appends the samples to `out`. A
    /// packet the plug-in cannot decode adds no samples. A plug-in that stops or does not answer in time is turned off for
    /// this graph, and [`MediaAudio::notice`] says so.
    pub(crate) fn decode(
        &mut self,
        codecs: &Arc<CodecSet>,
        format: &Arc<MediaFormat>,
        bytes: &[u8],
        out: &mut Output,
    ) {
        if format.codec.media() != MediaType::Audio {
            return;
        }
        if !self.from.as_ref().is_some_and(|f| Arc::ptr_eq(f, codecs)) {
            self.from = Some(Arc::clone(codecs));
            self.codecs = CodecSet::clone(codecs);
            self.streams.clear();
        }
        let k = match self
            .streams
            .iter()
            .position(|(f, _)| Arc::ptr_eq(f, format))
        {
            Some(k) => k,
            // The same format in a new allocation: the stream goes on.
            None => match self.streams.iter().position(|(f, _)| **f == **format) {
                Some(k) => {
                    self.streams[k].0 = Arc::clone(format);
                    k
                }
                None => {
                    // A format that only this list still holds belongs to a stream that ended.
                    self.streams.retain(|(f, _)| Arc::strong_count(f) > 1);
                    let codec = self
                        .codecs
                        .find(format.fourcc(), "")
                        .map(|e| GuardedCodec::start(e, format));
                    self.streams.push((Arc::clone(format), codec));
                    self.streams.len() - 1
                }
            },
        };
        let Some(codec) = self.streams[k].1.as_mut() else {
            return;
        };
        match codec.decode(bytes, false) {
            Ok(pcm) => {
                self.audio.clear();
                self.audio
                    .extend(pcm.samples.iter().map(|&x| f32::from(x) / 32768.0));
                out.push_audio_in(
                    &self.audio,
                    f64::from(pcm.rate_hz),
                    u16::from(pcm.channels),
                    pcm.layout(),
                );
            }
            Err(CodecError::Frame) => {}
            Err(CodecError::Unsupported) => {
                self.streams[k].1 = None;
                self.notice = Some(format!(
                    "the {} plug-in does not decode this stream: it has no sound",
                    format.codec.name()
                ));
            }
            Err(e) => {
                let fourcc = format.fourcc();
                self.codecs.disable(fourcc, &e);
                for (f, c) in &mut self.streams {
                    if f.fourcc() == fourcc {
                        *c = None;
                    }
                }
                if let Some(n) = self.codecs.take_notices().pop() {
                    self.notice = Some(n);
                }
            }
        }
    }

    /// The last notice about a plug-in that was turned off.
    pub(crate) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::codec::{CodecFactory, CodecSet};
    use sdrkit_core::media::{Codec, Packaging};
    use std::sync::Arc;

    fn factory(make: fn() -> Box<dyn AudioCodec>) -> CodecFactory {
        Arc::new(move |_: &MediaFormat| Some(make()))
    }

    struct Tone;
    impl AudioCodec for Tone {
        fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
            pcm.rate_hz = 8000;
            pcm.channels = 1;
            let v = if erasure {
                0
            } else {
                *frame.first().ok_or(CodecError::Frame)? as i16 * 100
            };
            pcm.samples.extend(std::iter::repeat_n(v, 160));
            Ok(())
        }
    }
    struct Crash;
    impl AudioCodec for Crash {
        fn decode(&mut self, _: &[u8], _: bool, _: &mut Pcm) -> Result<(), CodecError> {
            panic!("bad plug-in")
        }
    }
    struct Hang;
    impl AudioCodec for Hang {
        fn decode(&mut self, _: &[u8], _: bool, _: &mut Pcm) -> Result<(), CodecError> {
            std::thread::sleep(Duration::from_millis(400));
            Ok(())
        }
    }

    #[test]
    fn a_plug_in_decodes_and_a_crashing_or_hanging_one_is_stopped() {
        let mut set = CodecSet::default();
        let ambe2 = FourCc::new("amb2");
        let imbe = FourCc::new("imbe");
        let acelp = FourCc::new("acel");
        set.register(ambe2, None, factory(|| Box::new(Tone)));
        set.register(imbe, None, factory(|| Box::new(Crash)));
        set.register(acelp, None, factory(|| Box::new(Hang)));
        let speech = |c| MediaFormat::new(c, Packaging::Raw);

        let mut g = GuardedCodec::start(set.find(ambe2, "dmr").unwrap(), &speech(Codec::Ambe2));
        let pcm = g.decode(&[3], false).unwrap();
        assert_eq!(
            (pcm.samples.len(), pcm.samples[0], pcm.rate_hz),
            (160, 300, 8000)
        );
        assert_eq!(g.decode(&[], false), Err(CodecError::Frame));
        assert_eq!(g.decode(&[], true).unwrap().samples[0], 0);

        let mut g = GuardedCodec::start(set.find(imbe, "p25").unwrap(), &speech(Codec::Imbe));
        assert_eq!(g.decode(&[1], false), Err(CodecError::Crashed));
        set.disable(imbe, g.failed().unwrap());
        assert!(!set.has(imbe));

        let mut g = GuardedCodec::start_with_timeout(
            set.find(acelp, "tetra").unwrap(),
            &speech(Codec::TetraAcelp),
            Duration::from_millis(50),
        );
        assert_eq!(g.decode(&[1], false), Err(CodecError::Timeout));
        // A late answer is never taken for the next frame's.
        assert_eq!(g.decode(&[1], false), Err(CodecError::Timeout));
        assert_eq!(
            set.take_notices(),
            ["the IMBE plug-in stopped and was turned off"]
        );

        let mut none = CodecSet::default();
        none.register(ambe2, None, Arc::new(|_: &MediaFormat| None));
        let mut g = GuardedCodec::start(none.find(ambe2, "dmr").unwrap(), &speech(Codec::Ambe2));
        assert_eq!(g.decode(&[1], false), Err(CodecError::Unsupported));
        assert_eq!(g.failed(), Some(&CodecError::Unsupported));
    }

    #[test]
    fn coded_audio_is_decoded_into_the_output_and_a_crashing_plug_in_is_turned_off() {
        use sdrkit_core::Output;
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Stereo;
        impl AudioCodec for Stereo {
            fn decode(&mut self, _: &[u8], _: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
                pcm.rate_hz = 48_000;
                pcm.channels = 2;
                pcm.samples.extend([16384i16; 8]);
                Ok(())
            }
        }
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let mut set = CodecSet::default();
        set.register(
            Codec::Aac.fourcc(),
            None,
            Arc::new(|_: &MediaFormat| {
                MADE.fetch_add(1, Ordering::Relaxed);
                Some(Box::new(Stereo) as Box<dyn AudioCodec>)
            }),
        );
        set.register(Codec::Mp2.fourcc(), None, factory(|| Box::new(Crash)));
        let set = Arc::new(set);
        let mut media = MediaAudio::default();
        let mut out = Output::default();
        let aac = Arc::new(MediaFormat::new(Codec::Aac, Packaging::Raw));
        media.decode(&set, &aac, &[1], &mut out);
        // The same format in a new allocation keeps its decoder.
        media.decode(
            &set,
            &Arc::new(MediaFormat::new(Codec::Aac, Packaging::Raw)),
            &[1],
            &mut out,
        );
        assert_eq!(MADE.load(Ordering::Relaxed), 1);
        assert_eq!(out.audio_format(), Some((48_000.0, 2)));
        assert_eq!(out.audio(), &[0.5; 16][..]);
        // Video is not decoded here.
        media.decode(
            &set,
            &Arc::new(MediaFormat::new(Codec::H264, Packaging::AnnexB)),
            &[1],
            &mut out,
        );
        let mp2 = Arc::new(MediaFormat::new(Codec::Mp2, Packaging::Raw));
        media.decode(&set, &mp2, &[1], &mut out);
        assert_eq!(
            media.notice(),
            Some("the MPEG-1 Layer II plug-in stopped and was turned off")
        );
        assert_eq!(out.audio().len(), 16);
    }
}
