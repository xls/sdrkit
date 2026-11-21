//! What the tests of the senders share: a host with a transmit stream and a microphone, a codec plug-in that keeps the
//! frames it is given, Codec 2 tables of the right shape, speech-like audio, and the commands of push-to-talk.

use sdrkit_codec_codec2::{Codebooks, Mode, Table};
use sdrkit_core::codec::{AudioCodec, CodecError, Pcm};
use sdrkit_core::{owner, CommandError, Complex32, Output, Receiver, Value};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The rate of the sessions of the sender tests.
pub const RATE: f64 = 96_000.0;

/// Codec 2 tables of `mode` with made-up values of the right shape: the frames are exact, not good speech.
pub fn made_up_books(mode: Mode) -> Arc<Codebooks> {
    let mut b = Codebooks::new();
    for &t in mode.tables() {
        let n = t.len();
        let v: Vec<f32> = match t {
            Table::NlpFir => {
                let h: Vec<f64> = (0..n)
                    .map(|i| {
                        let m = i as f64 - (n as f64 - 1.0) / 2.0;
                        let fc = 600.0 / 8000.0;
                        let s = (std::f64::consts::TAU * fc * m).sin() / (std::f64::consts::PI * m);
                        s * (0.54
                            - 0.46 * (std::f64::consts::TAU * i as f64 / (n as f64 - 1.0)).cos())
                    })
                    .collect();
                let sum: f64 = h.iter().sum();
                h.iter().map(|v| (v / sum) as f32).collect()
            }
            _ => (0..n).map(|j| 25.0 + 10.0 * j as f32).collect(),
        };
        b.set(t, &v).unwrap();
    }
    Arc::new(b)
}

/// A voiced sound with a moving pitch at `t` seconds.
pub fn speechlike(t: f64) -> f32 {
    let f0 = 140.0 + 30.0 * (std::f64::consts::TAU * 1.5 * t).sin();
    let v: f64 = (1..12)
        .map(|k| (std::f64::consts::TAU * f0 * k as f64 * t).sin() / k as f64)
        .sum();
    (0.25 * v * (0.6 + 0.4 * (std::f64::consts::TAU * 3.0 * t).sin())) as f32
}

/// A codec plug-in that keeps every frame it is given and answers with 20 ms of silence.
pub struct Keep(pub Arc<Mutex<Vec<Vec<u8>>>>);
impl AudioCodec for Keep {
    fn decode(&mut self, frame: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
        if !erasure {
            self.0.lock().unwrap().push(frame.to_vec());
        }
        pcm.rate_hz = 8000;
        pcm.channels = 1;
        pcm.samples.extend(std::iter::repeat_n(0, 160));
        Ok(())
    }
}

/// One write to the transmit stream: its samples, its centre, its end mark and its stated start.
pub type Write = (Vec<Complex32>, f64, bool, Option<f64>);

/// The host: the transmit stream, and the microphone with as much audio as the test allows.
#[derive(Default)]
pub struct Host {
    pub writes: Mutex<Vec<Write>>,
    pub lead: Duration,
    /// The microphone audio at 48 kHz, the samples read and the samples allowed.
    pub mic: Mutex<(Vec<f32>, usize, usize)>,
}

impl HostStreams for Host {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                centre_hz,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), centre_hz, end, start));
        }
    }
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        if port != HostPort::Microphone {
            return 0;
        }
        let mut m = self.mic.lock().unwrap();
        let (audio, read, allowed) = &mut *m;
        let k = out.len().min(*allowed - *read).min(audio.len() - *read);
        out[..k].copy_from_slice(&audio[*read..*read + k]);
        *read += k;
        k
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

/// One transmission: its samples, its centre, whether the end mark came, and the stated starts (place, UTC).
pub type Burst = (Vec<Complex32>, f64, bool, Vec<(usize, f64)>);

impl Host {
    /// The transmissions.
    pub fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, c, end, start) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push((Vec::new(), *c, false, Vec::new()));
            }
            let b = out.last_mut().unwrap();
            if let Some(utc) = start {
                b.3.push((b.0.len(), *utc));
            }
            b.0.extend_from_slice(s);
            b.2 = *end;
            open = !end;
        }
        out
    }
    pub fn allow(&self, n: usize) {
        self.mic.lock().unwrap().2 += n;
    }
}

pub fn call(rx: &mut dyn Receiver, key: &str, args: &[(&str, Value)]) -> Result<(), CommandError> {
    let id = rx.commands().id(key).unwrap();
    let mut a = rx.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

pub fn ptt(rx: &mut dyn Receiver, on: bool) -> Result<(), CommandError> {
    call(rx, "ptt", &[("on", Value::Bool(on))])
}

/// Feeds `seconds` of silence in pieces of 0.1 s, allowing the microphone audio of each piece first when `talk`.
pub fn idle(rx: &mut dyn Receiver, host: &Host, seconds: f64, talk: bool) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        if talk {
            host.allow(4800);
        }
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

/// The instantaneous frequency of `x` at `rate`, Hz: the turn between each two samples.
pub fn frequency(x: &[Complex32], rate: f64) -> Vec<f64> {
    x.windows(2)
        .map(|w| f64::from((w[1] * w[0].conj()).arg()) * rate / std::f64::consts::TAU)
        .collect()
}
