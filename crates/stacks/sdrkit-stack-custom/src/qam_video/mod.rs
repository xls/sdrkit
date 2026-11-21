//! `qam_video`: a single-carrier link of 16-QAM or 64-QAM with LDPC at 700 to 1400 ksymbols a second, with a faint V-shaped
//! sweep as a signature, that carries typed packets (video, audio, text, pictures, data) in FEC groups of an erasure code.

pub mod container;
pub mod phy;
pub mod rx;
pub mod stack;

pub use stack::QAM_VIDEO;

use container::{fragments, largest_payload, Packer, Packet, DATA, TEXT};
use phy::{Burst, Waveform};
use sdrkit_codec_custom::video_units::{Synthetic, SyntheticParams};

/// What a transmission carries.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A file, as Data packets of stream 1.
    File(Vec<u8>),
    /// The synthetic test stream for this many seconds of media, video at this bit rate.
    Synthetic {
        /// Seconds of media.
        seconds: f64,
        /// Video bit rate.
        video_bps: f64,
    },
}

/// A transmission: the station, the profile and the payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// The profile and waveform.
    pub waveform: Waveform,
    /// The FEC profile index.
    pub fec: usize,
    /// The callsign (1 to 7 characters of A-Z, 0-9 and /).
    pub callsign: String,
    /// The bytes before " de " in the station identification text.
    pub tag: Vec<u8>,
    /// The payload.
    pub source: Source,
}

/// Why a transmission cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    /// The callsign is not 1 to 7 characters of A-Z, 0-9 and /.
    Callsign,
    /// The file is empty or larger than 64 MiB.
    File,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SessionError::Callsign => "the callsign must be 1 to 7 characters of A-Z, 0-9 and /",
            SessionError::File => "the file must hold 1 byte to 64 MiB",
        })
    }
}

impl std::error::Error for SessionError {}

/// The callsign as sent: upper case, no trailing spaces; an error when it is not 1 to 7 characters of A-Z, 0-9 and /.
pub fn callsign(c: &str) -> Result<String, SessionError> {
    let c = c.trim_end().to_ascii_uppercase();
    let ok = (1..=7).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/');
    if ok {
        Ok(c)
    } else {
        Err(SessionError::Callsign)
    }
}

/// The identification text: the tag, " de ", the callsign.
pub fn ident_text(tag: &[u8], call: &str) -> Vec<u8> {
    let mut t = tag.to_vec();
    t.extend_from_slice(b" de ");
    t.extend_from_slice(call.as_bytes());
    t
}

/// The callsign of an identification text: what follows " de " in the first 9 bytes.
pub fn ident_callsign(payload: &[u8]) -> Option<String> {
    let at = payload.windows(4).take(6).position(|w| w == b" de ")?;
    let c = &payload[at + 4..];
    (!c.is_empty()).then(|| String::from_utf8_lossy(c).into_owned())
}

/// The frame payloads of a session, and the packets in the order sent.
pub fn session_frames(s: &Session) -> Result<(Vec<Vec<u8>>, Vec<Packet>), SessionError> {
    let call = callsign(&s.callsign)?;
    let cell = s.waveform.cell();
    let mut pk = Packer::new(s.fec, cell);
    let frame_s = s.waveform.frame_symbols() as f64 / s.waveform.profile.symbol_rate();
    let ident_every = ((0.5 / frame_s).round() as usize).max(1);
    let max = largest_payload(s.fec, cell).min(1200);
    let mut sent = Vec::new();
    let mut seqs = [[0u16; 16]; 5];
    let mut text_seq = 0u16;
    let mut last_ident: Option<usize> = None;
    let add = |pk: &mut Packer, p: Packet, sent: &mut Vec<Packet>| {
        if pk.add(&p) {
            sent.push(p);
        }
    };
    let ident = |text_seq: &mut u16| {
        let p = Packet {
            kind: TEXT,
            stream: 0,
            seq: *text_seq,
            payload: ident_text(&s.tag, &call),
        };
        *text_seq = text_seq.wrapping_add(1);
        p
    };
    let mut maybe_ident = |pk: &mut Packer, sent: &mut Vec<Packet>, text_seq: &mut u16| {
        let frames = pk.cells.len() / 4;
        if last_ident.is_none_or(|l| frames >= l + ident_every) {
            last_ident = Some(frames);
            let p = ident(text_seq);
            add(pk, p, sent);
        }
    };
    let mut frames_out = Vec::new();
    match &s.source {
        Source::File(bytes) => {
            if bytes.is_empty() || bytes.len() > 64 << 20 {
                return Err(SessionError::File);
            }
            for chunk in bytes.chunks(max) {
                maybe_ident(&mut pk, &mut sent, &mut text_seq);
                let seq = &mut seqs[DATA as usize][1];
                let p = Packet {
                    kind: DATA,
                    stream: 1,
                    seq: *seq,
                    payload: chunk.to_vec(),
                };
                *seq = seq.wrapping_add(1);
                add(&mut pk, p, &mut sent);
                frames_out.extend(pk.frames());
            }
        }
        Source::Synthetic { seconds, video_bps } => {
            let mut syn = Synthetic::new(SyntheticParams {
                video_bps: *video_bps,
                ..SyntheticParams::default()
            })
            .map_err(|_| SessionError::File)?;
            let mut buf = Vec::new();
            loop {
                let u = syn.next_unit();
                if f64::from(u.time_ms) / 1000.0 >= *seconds {
                    break;
                }
                buf.resize(u.len, 0);
                let _ = u.fill(&mut buf);
                let kind = u.kind.packet_type();
                for f in fragments(&buf, u.time_ms, u.key, max) {
                    maybe_ident(&mut pk, &mut sent, &mut text_seq);
                    let seq = &mut seqs[kind as usize][0];
                    let p = Packet {
                        kind,
                        stream: 0,
                        seq: *seq,
                        payload: f,
                    };
                    *seq = seq.wrapping_add(1);
                    add(&mut pk, p, &mut sent);
                }
                frames_out.extend(pk.frames());
            }
        }
    }
    pk.close();
    frames_out.extend(pk.frames());
    Ok((frames_out, sent))
}

/// The samples of a session at 4 samples a symbol, with 96 symbols of silence before and after.
pub fn session_samples(s: &Session) -> Result<(Vec<sdrkit_core::Complex32>, Vec<Packet>), SessionError> {
    let (frames, sent) = session_frames(s)?;
    let b = Burst {
        waveform: s.waveform,
        fec: s.fec as u8,
        link: 0,
        first_seq: 0,
        first_sweep: 0,
        lead: 96,
        tail: 96,
        chirp: true,
    };
    Ok((phy::modulate(&b, &frames), sent))
}
