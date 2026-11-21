//! `lora.modulate`: the modulator of the family. It turns each frame to send (`tx.request`) into complex samples for a
//! transmitter: LoRa, or AFSK 1200 or G3RUH 9600 on FM for an AX.25 frame.

use crate::aprs::rx::EMPHASIS_TAU_S;
use crate::mesh::types::{AfskTx, TxRequest};
use crate::typed::ModulateFields;
use sdrkit_algo_dsp::iir::PreEmphasis;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_components::tx::{end_mark, start_mark, BurstTiming};
use sdrkit_core::{Complex32, ConfigError, Results, ResultsDecl, Values};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Port, RunContext};
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio};
use sdrkit_proto_lora_phy::modulate::TxFrame;
use std::collections::VecDeque;

/// The peak level of a burst.
const AMPLITUDE: f32 = 0.7;
/// The FM peak deviation of an AX.25 frame.
pub const DEVIATION_HZ: f64 = 3000.0;
/// Where the pre-emphasis of the 1200 bit/s audio stops rising, Hz.
const PRE_EMPHASIS_TOP_HZ: f64 = 8000.0;

/// The modulator (`lora.modulate`). Input 0 (`in`) is the session's IQ input: its rate is the transmit rate. Input 1
/// (`frames`) holds the frames to send. Output 0 (`out`) is the IQ of the bursts, centred on the channel of the frame, for
/// `tx.sink`.
///
/// The bursts are written at the pace of the input: each pass writes at most as many samples as input 0 holds, so a long
/// burst takes many passes. Frames that arrive during a burst wait for it to end. If the channel of the next frame
/// differs from the output's centre, the output takes the new centre and the burst starts in the next pass. The last
/// sample of each burst carries the end mark ([`end_mark`]).
///
/// On a live radio whose host times bursts, the first sample of each burst states its time on air
/// ([`BurstTiming::start_queued`]): the time of the pass that writes it plus the lead of the host and two passes, and not
/// before the end on air of the burst before it. The frame was due when it arrived; the stated start is the
/// earliest time the host can put it on air.
pub struct LoraModulate {
    rate: f64,
    centre_hz: f64,
    waiting: VecDeque<TxRequest>,
    burst: Option<Burst>,
    /// The time on air of the burst in `burst`, UTC, until its first sample is written.
    start: Option<f64>,
    sent: i64,
    samples: i64,
    dropped: i64,
    results: Published,
    timing: BurstTiming,
}

/// The burst that the modulator writes.
enum Burst {
    /// A LoRa frame, the index of its next sample and its length in samples.
    Lora { frame: TxFrame, next: u64, len: u64 },
    /// The samples of an AX.25 frame and the index of the next one.
    Packet { iq: Vec<Complex32>, next: usize },
}

impl Burst {
    /// Appends at most `room` samples of the burst to `out` at `rate`, and returns the number appended.
    fn write(&mut self, out: &mut Vec<Complex32>, room: usize, rate: f64) -> usize {
        let start = out.len();
        match self {
            Burst::Lora { frame, next, len } => {
                let k = usize::try_from(*len - *next).map_or(room, |left| left.min(room));
                out.resize(start + k, Complex32::default());
                frame.add_into(&mut out[start..], rate, *next);
                *next += k as u64;
                k
            }
            Burst::Packet { iq, next } => {
                let k = (iq.len() - *next).min(room);
                out.extend(iq[*next..*next + k].iter().map(|v| *v * AMPLITUDE));
                *next += k;
                k
            }
        }
    }

    /// The length of the burst in samples.
    fn len(&self) -> u64 {
        match self {
            Burst::Lora { len, .. } => *len,
            Burst::Packet { iq, .. } => iq.len() as u64,
        }
    }

    /// Returns true when every sample of the burst is written.
    fn done(&self) -> bool {
        match self {
            Burst::Lora { next, len, .. } => next >= len,
            Burst::Packet { iq, next } => *next >= iq.len(),
        }
    }
}

impl Default for LoraModulate {
    fn default() -> Self {
        LoraModulate {
            rate: 0.0,
            centre_hz: 0.0,
            waiting: VecDeque::new(),
            burst: None,
            start: None,
            sent: 0,
            samples: 0,
            dropped: 0,
            results: Published::from_decl(std::sync::Arc::new(
                ResultsDecl::new().with_fields(ModulateFields::SHAPE.fields),
            )),
            timing: BurstTiming::default(),
        }
    }
}

impl LoraModulate {
    /// Writes the counts.
    fn publish(&mut self) {
        self.results.put(&ModulateFields {
            tx_bursts: self.sent,
            tx_samples: self.samples,
            tx_dropped: self.dropped,
        });
    }
    /// Returns the burst of `r` at this rate and centre, or `None` if `r` cannot be modulated.
    fn burst(&self, r: &TxRequest) -> Option<Burst> {
        let off = r.freq_hz - self.centre_hz;
        match &r.lora {
            Some(p) => {
                if off.abs() + p.bw / 2.0 > self.rate / 2.0 {
                    return None;
                }
                let frame = TxFrame::new(p, &r.payload)
                    .ok()?
                    .carrier(off)
                    .amplitude(AMPLITUDE);
                let len = (frame.duration_s() * self.rate).ceil() as u64 + 1;
                Some(Burst::Lora {
                    frame,
                    next: 0,
                    len,
                })
            }
            None => {
                let afsk = r.afsk.unwrap_or_default();
                let base = packet_iq(&r.payload, &afsk, DEVIATION_HZ, self.rate)?;
                let mut iq = vec![Complex32::default(); base.len()];
                Nco::new(self.rate, off).mix(&base, &mut iq);
                Some(Burst::Packet { iq, next: 0 })
            }
        }
    }
}

impl Component for LoraModulate {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "lora.modulate",
            vec![Port::iq("in"), Port::typed::<TxRequest>("frames")],
            vec![Port::iq("out")],
        )
    }

    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.timing.attach(ctx);
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let input = &inputs[0];
        self.rate = input.rate;
        if self.centre_hz == 0.0 {
            self.centre_hz =
                super::channels::known_tuning(input.tuned_hz).unwrap_or(input.centre_hz);
        }
        Ok(vec![Format::iq(self.rate, self.centre_hz, self.centre_hz)])
    }

    fn process(&mut self, io: &mut Io) {
        self.waiting
            .extend(io.items::<TxRequest>(1).iter().map(|r| r.value.clone()));
        let mut room = io.input_len(0);
        let pass = room as f64 / self.rate;
        while room > 0 {
            if self.burst.is_none() {
                let Some(r) = self.waiting.front() else {
                    break;
                };
                if r.freq_hz != self.centre_hz {
                    self.centre_hz = r.freq_hz;
                    // The output keeps its format until the end of the pass: the burst starts in the next pass.
                    io.request_reformat();
                    break;
                }
                self.burst = self.waiting.pop_front().and_then(|r| self.burst(&r));
                let Some(len) = self.burst.as_ref().map(Burst::len) else {
                    self.dropped += 1;
                    self.publish();
                    continue;
                };
                self.start = self.timing.start_queued(pass, len as f64 / self.rate);
            }
            let Some(b) = &mut self.burst else {
                break;
            };
            let first = io.iq_out(0).len();
            let k = b.write(io.iq_out(0), room, self.rate);
            if k > 0 {
                if let Some(unix_s) = self.start.take() {
                    start_mark(io, 0, first, unix_s);
                }
            }
            room -= k;
            self.samples = self
                .samples
                .saturating_add(i64::try_from(k).unwrap_or(i64::MAX));
            self.results.put(&ModulateFields {
                tx_bursts: self.sent,
                tx_samples: self.samples,
                tx_dropped: self.dropped,
            });
            if b.done() {
                self.burst = None;
                end_mark(io, 0);
                self.sent += 1;
                self.publish();
            }
        }
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// The FM signal at 0 Hz and unit amplitude of one AX.25 frame (bytes without the FCS), as `afsk` says: the flags of its TX
/// delay, the frame with its FCS, bit stuffing and NRZI, and closing flags, as continuous-phase AFSK of 1200 bit/s (1200 Hz
/// mark, 2200 Hz space) or G3RUH 9600 audio, on an FM carrier whose peak deviation is `deviation_hz`. Pre-emphasised
/// AFSK audio has its 2200 Hz tone at the peak deviation. `None` if the frame is too short or too long, or the rate does not
/// suit the modulator.
pub fn packet_iq(
    frame: &[u8],
    afsk: &AfskTx,
    deviation_hz: f64,
    rate: f64,
) -> Option<Vec<Complex32>> {
    let audio = packet_audio(frame, afsk, rate)?;
    let mut iq = vec![Complex32::default(); audio.len()];
    Fm::new(rate, deviation_hz)
        .ok()?
        .process(&audio, &mut iq)
        .ok()?;
    Some(iq)
}

/// The audio of [`packet_iq`] before the FM, with peak 1, at `rate`.
pub fn packet_audio(frame: &[u8], afsk: &AfskTx, rate: f64) -> Option<Vec<f32>> {
    let mut audio = Vec::new();
    if afsk.baud == 9600 {
        g3ruh9600_audio(&[frame], afsk.flags(), rate, &mut audio).ok()?;
    } else {
        afsk1200_audio(&[frame], afsk.flags(), rate, &mut audio).ok()?;
        if afsk.pre_emphasis {
            // The boost levels off at 8 kHz as in a radio; the 2200 Hz tone keeps peak 1, and the limiter holds the
            // keying edges to the peak deviation.
            let mut pre = PreEmphasis::bilinear(EMPHASIS_TAU_S, PRE_EMPHASIS_TOP_HZ, rate).ok()?;
            let gain = 10f64.powf(-pre.response_db(rate, 2200.0) / 20.0) as f32;
            pre.process(&mut audio);
            audio
                .iter_mut()
                .for_each(|x| *x = (*x * gain).clamp(-1.0, 1.0));
        }
    }
    Some(audio)
}
