//! The recorders as graph components: started and saved by command, writing through the host's storage.

mod support;

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Clock, Complex32, MemStorage, Output, StorePath, Value};
use sdrkit_graph::stream::Kind;
use sdrkit_graph::stream::{AudioFrames, Data, Timed};
use sdrkit_graph::{Buffer, Component, Format, GraphBuilder, Io, Registry};
use sdrkit_svc_record::{read_recording, AudioRecorder, GateBlock, IqRecorder, RewindRecorder};
use std::sync::Arc;
use support::{call, text};

fn list(storage: &MemStorage, ext: &str) -> Vec<String> {
    let mut v: Vec<String> =
        sdrkit_core::Storage::list(storage, &StorePath::new("recordings").unwrap())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.name.ends_with(ext))
            .map(|e| format!("recordings/{}", e.name))
            .collect();
    v.sort();
    v
}

fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if f() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out");
}

#[test]
fn a_rewind_in_a_graph_saves_on_a_command() {
    let storage = Arc::new(MemStorage::new());
    let mut g = GraphBuilder::new()
        .add(
            "rw",
            RewindRecorder::new(storage.clone(), Clock::fixed(1_735_732_800.0)),
        )
        .input("rw")
        .build()
        .unwrap();
    g.configure_input(Format::iq(48_000.0, 7.1e6, 7.1e6))
        .unwrap();
    let iq = vec![Complex32::new(0.25, -0.25); 4800];
    let mut out = Output::default();
    for _ in 0..20 {
        g.process(&iq, &mut out);
    }
    call(&mut g, "rw.save", &[("pre_s", Value::Duration(0.5))]).unwrap();
    g.process(&iq, &mut out);
    wait_for(|| list(&storage, ".sigmf-meta").len() == 1);
    let data = list(&storage, ".sigmf-data");
    let r = read_recording(&*storage, &StorePath::new(&data[0]).unwrap()).unwrap();
    assert_eq!(
        (r.format.rate, r.format.centre_hz, r.frames),
        (48_000.0, 7.1e6, 24_000)
    );
    g.refresh_results();
    let res = g.component_results("rw").unwrap();
    assert!(res.decl().field_id("rewind_s").is_some());
}

#[test]
fn the_iq_recorder_component_records_between_start_and_stop() {
    let storage = Arc::new(MemStorage::new());
    let mut g = GraphBuilder::new()
        .add(
            "rec",
            IqRecorder::new(storage.clone(), Clock::fixed(1_735_732_800.0)),
        )
        .input("rec")
        .build()
        .unwrap();
    g.configure_input(Format::iq(48_000.0, 7.1e6, 7.1e6))
        .unwrap();
    let iq = vec![Complex32::new(0.25, -0.25); 4800];
    let mut out = Output::default();
    g.process(&iq, &mut out);
    call(&mut g, "rec.start", &[]).unwrap();
    for _ in 0..10 {
        g.process(&iq, &mut out);
    }
    call(&mut g, "rec.stop", &[]).unwrap();
    g.process(&iq, &mut out);
    wait_for(|| {
        list(&storage, ".sigmf-data")
            .first()
            .and_then(|d| read_recording(&*storage, &StorePath::new(d).unwrap()).ok())
            .is_some_and(|r| r.frames == 48_000)
    });
    let id = g.schema().id("rec.folder").unwrap();
    assert!(owner::apply(&mut g, id, &text("x/../y")).is_err());
    let mut reg = Registry::default();
    sdrkit_svc_record::register(&mut reg, storage, Clock::fixed(0.0));
    let _ = Kind::Iq;
}

#[test]
fn the_audio_recorder_component_records_each_transmission_by_its_gate() {
    let storage = Arc::new(MemStorage::new());
    let mut c = AudioRecorder::new(storage.clone(), Clock::fixed(1_735_732_800.0));
    let schema = c.info().settings;
    for (k, v) in [
        ("split", Value::Choice(1)),
        ("preroll", Value::Duration(0.0)),
        ("hang", Value::Duration(0.0)),
        ("min_length", Value::Duration(0.5)),
    ] {
        let id = schema.id(k).unwrap();
        schema.check(id, &v).unwrap();
        c.apply(id, &v).unwrap();
    }
    let formats = [
        Format::audio(8000.0, 1),
        Format::new(Kind::typed::<GateBlock>(), 0.0),
        Format::iq(8000.0, 0.0, 0.0),
    ];
    c.configure(&formats).unwrap();
    let start = c.commands().id("start").unwrap();
    c.invoke(
        start,
        &c.commands()
            .check(start, &c.commands().def(start).unwrap().new_args())
            .unwrap(),
    )
    .unwrap();
    // One second open, one closed, one open: two files of one second.
    for k in 0..30 {
        let open = (k / 10) % 2 == 0;
        let mut audio = Buffer::new(&Kind::Audio);
        audio.data = Data::Audio(AudioFrames {
            channels: 1,
            samples: vec![0.25; 800],
        });
        let mut gate = Buffer::new(&Kind::typed::<GateBlock>());
        gate.data = Data::Items(Some(Box::new(vec![Timed {
            time: 0.0,
            value: GateBlock { open },
        }])));
        let iq = Buffer::new(&Kind::Iq);
        let inputs = [audio, gate, iq];
        let mut io = Io::standalone(&inputs, &formats, &[0, 1, 2], &[], &mut []);
        c.process(&mut io);
    }
    let stop = c.commands().id("stop").unwrap();
    c.invoke(
        stop,
        &c.commands()
            .check(stop, &c.commands().def(stop).unwrap().new_args())
            .unwrap(),
    )
    .unwrap();
    wait_for(|| list(&storage, ".wav").len() == 2);
    for f in list(&storage, ".wav") {
        let r = read_recording(&*storage, &StorePath::new(&f).unwrap()).unwrap();
        // The gate moves at the end of a 10 ms level window.
        assert_eq!(r.format.channels, 1);
        assert!(r.frames.abs_diff(8000) <= 80, "{f} {}", r.frames);
    }
}
