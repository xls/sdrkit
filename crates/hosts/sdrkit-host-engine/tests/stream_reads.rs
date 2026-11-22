//! The streams that a host pulls from a session (audio, media packets, pictures) while another thread of the host reads
//! results in a tight loop: the host receives every item that the stack produced.

mod common;

use common::*;
use sdrkit_core::media::{Codec, MediaFormat, Packaging, Packet};
use sdrkit_core::{
    Complex32, ConfigError, Elementary, Facilities, InputInfo, MemStorage, Output, Owner,
    Published, Random, Receiver, Results, ResultsDecl, SettingError, SettingId, StackDescriptor,
    StreamKind, Tuning, Value, Values,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, Setup, AUDIO_LOST};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The audio samples, the media packets and the pictures that the streamers produced.
static SAMPLES_MADE: AtomicU64 = AtomicU64::new(0);
static PACKETS_MADE: AtomicU64 = AtomicU64::new(0);
static PICTURES_MADE: AtomicU64 = AtomicU64::new(0);
/// The tests of this file run one after the other: they share the counters.
static ALONE: Mutex<()> = Mutex::new(());

/// The audio rate of the streamer.
const AUDIO_HZ: f64 = 48e3;
/// The audio comes in bursts of at least this many samples, as the frames of a PES packet do.
const BURST: usize = 12_000;
/// The media packets of one block.
const PACKETS: u64 = 20;

/// A stack that produces numbered audio samples in bursts, numbered media packets and one numbered picture per block.
struct Streamer {
    results: Published,
    values: Values,
    block: u64,
    /// The audio of the blocks since the last burst.
    held: Vec<f32>,
    sample: u64,
    packet: u64,
    audio: Arc<MediaFormat>,
    picture: Arc<MediaFormat>,
}

impl Receiver for Streamer {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        if iq.is_empty() {
            return;
        }
        self.block += 1;
        self.results.set(&N, self.block as i64);
        // As much audio as the block lasts: sample n has the value n.
        let n = (iq.len() as f64 * AUDIO_HZ / RATE) as u64;
        self.held
            .extend((self.sample..self.sample + n).map(|i| i as f32));
        self.sample += n;
        if self.held.len() >= BURST {
            out.push_audio(&self.held, AUDIO_HZ, 1);
            SAMPLES_MADE.fetch_add(self.held.len() as u64, Ordering::Relaxed);
            self.held.clear();
        }
        let time = self.block as f64 * 1e-3;
        for _ in 0..PACKETS {
            self.packet += 1;
            out.media.push(Elementary {
                kind: StreamKind::Mp2,
                packet: Packet::new(Arc::from(self.packet.to_le_bytes()), time),
                format: Arc::clone(&self.audio),
                picture: None,
            });
        }
        PACKETS_MADE.fetch_add(PACKETS, Ordering::Relaxed);
        out.media.push(Elementary {
            kind: StreamKind::Picture,
            packet: Packet::new(Arc::from(self.block.to_le_bytes()), time),
            format: Arc::clone(&self.picture),
            picture: None,
        });
        PICTURES_MADE.fetch_add(1, Ordering::Relaxed);
    }
}

impl Owner for Streamer {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.values.set(id, value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

const N: sdrkit_core::Field<sdrkit_core::publish::field::I64> =
    sdrkit_core::Field::new("n", "Blocks", "");

/// About 15 blocks a second.
const RATE: f64 = 250e3;

static STREAMER: StackDescriptor = StackDescriptor {
    id: "streamer",
    name: "Streamer",
    tuning: Tuning::anywhere(100e6, RATE, 1e3, 10e3),
    submodes: &[],
    make_receiver: |cfg| {
        Ok(Box::new(Streamer {
            results: Published::from_decl(Arc::new(ResultsDecl::new().with_fields(&[&N]))),
            values: cfg.settings.clone(),
            block: 0,
            held: Vec::new(),
            sample: 0,
            packet: 0,
            audio: Arc::new(MediaFormat::new(Codec::Mp2, Packaging::Raw)),
            picture: Arc::new(MediaFormat::new(Codec::Jpeg, Packaging::Raw)),
        }))
    },
    ..METER_DESC
};

/// An engine with a streamer session on the tone signal.
fn streamer_engine() -> (Engine, SessionId) {
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&STREAMER, &TONE],
        families: Vec::new(),
        components: sdrkit_graph::Registry::default(),
    })
    .unwrap();
    for made in [&SAMPLES_MADE, &PACKETS_MADE, &PICTURES_MADE] {
        made.store(0, Ordering::Relaxed);
    }
    let radio = tone_radio(&e, RATE);
    let values = Values::defaults(&(STREAMER.settings)());
    let id = e
        .start_stack("streamer", None, &values, Input::Radio(radio), None)
        .unwrap();
    (e, id)
}

/// Runs `pull` every 10 ms for `time` while a second thread reads the results of the session and of its stack without a
/// pause and writes a setting of the stack about every 100 ms, then stops the session and runs `pull` until `done` is true, for 5 s at most.
fn pull_beside_a_reader(
    e: &Engine,
    id: SessionId,
    time: Duration,
    mut pull: impl FnMut(),
    done: impl Fn() -> bool,
) {
    let on = AtomicBool::new(true);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut copy = Results::default();
            let gain = e
                .owner(OwnerRef::Stack(id), |o| o.schema().id(GAIN.key()))
                .unwrap()
                .unwrap();
            let (mut written, mut writes) = (Instant::now(), 0u32);
            while on.load(Ordering::Relaxed) {
                if written.elapsed() >= Duration::from_millis(100) {
                    writes += 1;
                    let v = Value::F64(1.0 + f64::from(writes % 2));
                    e.owner(OwnerRef::Stack(id), |o| o.apply(gain, &v))
                        .unwrap()
                        .unwrap();
                    written = Instant::now();
                }
                for r in [OwnerRef::Session(id), OwnerRef::Stack(id)] {
                    let _ = e.owner(r, |o| copy.clone_from(o.results()));
                }
            }
        });
        let start = Instant::now();
        while start.elapsed() < time {
            pull();
            std::thread::sleep(Duration::from_millis(10));
        }
        e.stop_session(id).unwrap();
        let stopped = Instant::now();
        // The session thread ends its block: the counters stand still after it.
        std::thread::sleep(Duration::from_millis(300));
        loop {
            pull();
            if done() || stopped.elapsed() > Duration::from_secs(5) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        on.store(false, Ordering::Relaxed);
    });
}

#[test]
fn a_host_receives_every_audio_sample_beside_a_reader_of_results() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, id) = streamer_engine();
    let heard = std::cell::Cell::new(0u64);
    let mut out_of_order = None;
    let mut buffer = vec![0.0f32; 4800];
    pull_beside_a_reader(
        &e,
        id,
        Duration::from_secs(10),
        || {
            while let Some(a) = e.audio(id, &mut buffer).unwrap() {
                assert_eq!((a.rate_hz, a.channels), (AUDIO_HZ, 1));
                for &x in &buffer[..a.frames] {
                    if x != heard.get() as f32 {
                        out_of_order.get_or_insert((heard.get(), x));
                    }
                    heard.set(heard.get() + 1);
                }
            }
        },
        || heard.get() == SAMPLES_MADE.load(Ordering::Relaxed),
    );
    let made = SAMPLES_MADE.load(Ordering::Relaxed);
    let lost = number(&e, OwnerRef::Session(id), AUDIO_LOST.key);
    eprintln!(
        "audio samples: {made} made, {} received, {lost} lost",
        heard.get()
    );
    assert!(
        made as f64 > 0.8 * 10.0 * AUDIO_HZ,
        "{made} samples in 10 s"
    );
    assert_eq!(heard.get(), made, "samples received against samples made");
    assert_eq!(
        out_of_order, None,
        "(position, value) of the first sample out of order"
    );
    assert_eq!(lost, 0.0);
}

/// Pulls every packet of one queue after counter `after` and checks its number, which follows `seen`.
fn pull_numbered(
    read: impl Fn(u64, &mut Vec<u8>) -> Option<sdrkit_host_engine::MediaRead>,
    after: &mut u64,
    seen: &std::cell::Cell<u64>,
    missed: &mut u64,
) {
    let mut bytes = Vec::new();
    while let Some(m) = read(*after, &mut bytes) {
        *after = m.counter;
        *missed += m.missed;
        let number = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        if number != seen.get() + 1 {
            *missed += number.saturating_sub(seen.get() + 1).max(1);
        }
        seen.set(number);
    }
}

#[test]
fn a_host_receives_every_media_packet_beside_a_reader_of_results() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, id) = streamer_engine();
    let (mut after, seen, mut missed) = (0, std::cell::Cell::new(0), 0);
    pull_beside_a_reader(
        &e,
        id,
        Duration::from_secs(10),
        || {
            pull_numbered(
                |a, out| e.media(id, a, out).unwrap(),
                &mut after,
                &seen,
                &mut missed,
            )
        },
        || seen.get() == PACKETS_MADE.load(Ordering::Relaxed),
    );
    let made = PACKETS_MADE.load(Ordering::Relaxed);
    eprintln!(
        "media packets: {made} made, {} received, {missed} missed",
        seen.get()
    );
    assert!(made > 2000, "{made} packets in 10 s");
    assert_eq!(
        (seen.get(), missed),
        (made, 0),
        "(last packet received, missed) of {made} made"
    );
}

#[test]
fn a_host_receives_every_picture_beside_a_reader_of_results() {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let (e, id) = streamer_engine();
    let (mut after, seen, mut missed) = (0, std::cell::Cell::new(0), 0);
    pull_beside_a_reader(
        &e,
        id,
        Duration::from_secs(10),
        || {
            pull_numbered(
                |a, out| e.picture(id, a, out).unwrap(),
                &mut after,
                &seen,
                &mut missed,
            )
        },
        || seen.get() == PICTURES_MADE.load(Ordering::Relaxed),
    );
    let made = PICTURES_MADE.load(Ordering::Relaxed);
    eprintln!(
        "pictures: {made} made, {} received, {missed} missed",
        seen.get()
    );
    assert!(made > 100, "{made} pictures in 10 s");
    assert_eq!(
        (seen.get(), missed),
        (made, 0),
        "(last picture received, missed) of {made} made"
    );
}
