//! `attach` builds the same transport stream layer for every stack, and the graph inserts the demultiplexer where a consumer
//! requests coded media.

use sdrkit_core::media::{Codec, MediaSpec};
use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, Receiver, ResultsDecl, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Data, Format, GraphBuilder, GraphPlan, Io, Item, Kind,
    Port, PortRef, Registry, Timed,
};
use sdrkit_proto_mpegts::layer::{self, attach, TsLayer};
use sdrkit_proto_mpegts::publish::{
    PAGE, REGION_COUNT, SERVICE_LIST_EVENT, SUBTITLE_EVENT, TELETEXT_PAGE_EVENT,
};
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_proto_mpegts::stream::{self, TsOut, TsPacket};
use sdrkit_proto_mpegts::{SubtitleDisplay, TeletextDecoder, TeletextPage, TestMux};

/// A stack decoder's stand-in: samples in, a transport stream out (packets of the test multiplex, 100 per call).
struct TsSource {
    mux: Option<TestMux>,
    out: TsOut,
    t: f64,
}

impl TsSource {
    fn silent() -> Self {
        TsSource {
            mux: None,
            out: TsOut::default(),
            t: 0.0,
        }
    }
    fn test_mux() -> Self {
        TsSource {
            mux: Some(TestMux::satellite_cable(50).expect("a test multiplex")),
            ..TsSource::silent()
        }
    }
}

impl Component for TsSource {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.ts_source",
            vec![Port::new("in", Kind::Iq)],
            vec![stream::port("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(stream::kind(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(mux) = self.mux.as_mut() else { return };
        for _ in 0..100 {
            let mut p = [0u8; 188];
            mux.next_packet(&mut p);
            self.t += 1e-4;
            assert!(self
                .out
                .push(io.items_out::<TsPacket>(0), &p, self.t, false));
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

fn input() -> InputInfo {
    InputInfo::new(1e6, 0.0, 0.0).recording()
}

#[test]
fn every_part_builds() {
    for parts in [TsLayer::basic(), TsLayer::all()] {
        let b = GraphBuilder::new()
            .add("fec", TsSource::silent())
            .input("fec");
        let g = attach(b, "fec.out", &parts).build();
        assert!(g.is_ok(), "{:?}", g.err());
        let g = g.unwrap();
        let ids = g.order();
        for id in [layer::DEMUX, layer::SI] {
            assert!(ids.contains(&id), "{id}");
        }
        assert_eq!(ids.contains(&layer::TELETEXT), parts.teletext);
        assert_eq!(ids.contains(&layer::SUBTITLE), parts.subtitles);
    }
}

/// Every part configures inside a receiver, and the service that plays is read inside as a transport stream.
#[test]
fn every_part_configures_and_the_service_stream_is_read_inside() {
    for parts in [TsLayer::basic(), TsLayer::all()] {
        let b = GraphBuilder::new()
            .add("fec", TsSource::test_mux())
            .input("fec");
        let mut g = attach(b, "fec.out", &parts).build().unwrap();
        Receiver::configure(&mut g, &input()).unwrap_or_else(|e| panic!("{parts:?}: {e}"));
        let mut out = Output::default();
        let mut service_ts = 0;
        for _ in 0..40 {
            out.clear();
            g.feed(&[Complex32::new(0.0, 0.0); 16], &mut out);
            if let Some(b) = g.buffer(layer::SERVICE_TS) {
                if let Data::Items(Some(v)) = &b.data {
                    let v = v
                        .as_any()
                        .downcast_ref::<Vec<Timed<TsPacket>>>()
                        .expect("transport packets");
                    service_ts += v.len();
                }
            }
        }
        assert!(service_ts > 0, "{parts:?}");
    }
}

/// Every event that the layer's graph outputs give is declared by `layer::declare`, and items become events with their
/// fields.
#[test]
fn the_layer_events_are_declared() {
    let parts = TsLayer::all();
    let decl = layer::declare(ResultsDecl::new(), &parts);
    let declared = |kind: &str| decl.events().iter().any(|e| e.kind == kind);
    let b = GraphBuilder::new()
        .add("fec", TsSource::test_mux())
        .input("fec");
    let mut g = attach(b, "fec.out", &parts).build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
    let mut out = Output::default();
    let mut kinds: Vec<String> = Vec::new();
    for _ in 0..40 {
        out.clear();
        g.feed(&[Complex32::new(0.0, 0.0); 16], &mut out);
        for e in &out.events {
            assert!(declared(e.kind()), "{}", e.kind());
            kinds.push(e.kind().to_owned());
        }
    }
    assert!(
        kinds.iter().any(|k| k == SERVICE_LIST_EVENT.kind),
        "{kinds:?}"
    );

    // The test multiplex has no teletext or subtitles: their items become events the same way.
    use sdrkit_util_text::teletext::{packet, PageNo};
    let page = PageNo::from_decimal(100).unwrap();
    let pk = [
        packet::build_header(page, 0, Default::default(), 0, b"SDRKIT"),
        packet::build_row(1, 1, b"Hello"),
    ];
    let mut pes = Vec::new();
    packet::build_pes(&pk, &mut pes);
    let mut d = TeletextDecoder::new();
    d.push_pes(&pes);
    d.push_pes(&pes);
    let done: Vec<(u16, u16)> = d.drain_completed().collect();
    assert!(!done.is_empty());
    assert!(declared(TeletextPage::KIND));
    for (no, sub) in done {
        let p = d.page(no, Some(sub)).expect("a held page");
        let mut e = TELETEXT_PAGE_EVENT.event(0.0, 0.0);
        p.write_fields(&mut e.fields);
        assert_eq!(e.get(&PAGE), Some(100));
    }
    let sub = SubtitleDisplay {
        pts: Some(90_000),
        timeout_s: 5,
        width: 720,
        height: 576,
        seq: 1,
        regions: Vec::new(),
    };
    assert!(declared(SubtitleDisplay::KIND));
    let mut e = SUBTITLE_EVENT.event(0.0, 0.0);
    sub.write_fields(&mut e.fields);
    assert_eq!(e.get(&REGION_COUNT), Some(0));
    // Without the optional parts, their kinds are not declared.
    let basic = layer::declare(ResultsDecl::new(), &TsLayer::basic());
    assert!(basic
        .events()
        .iter()
        .any(|e| e.kind == SERVICE_LIST_EVENT.kind));
    assert!(!basic.events().iter().any(|e| e.kind == TeletextPage::KIND));
    assert!(!basic
        .events()
        .iter()
        .any(|e| e.kind == SubtitleDisplay::KIND));
}

/// Every component is registered once, as `mpegts.<name>`, and names itself so.
#[test]
fn the_registry_names_have_the_mpegts_prefix() {
    let mut r = Registry::new();
    let before = r.names().count();
    sdrkit_proto_mpegts::register(&mut r);
    let names: Vec<String> = r.names().map(str::to_owned).collect();
    let ours: Vec<&String> = names.iter().filter(|n| n.starts_with("mpegts.")).collect();
    assert_eq!(ours.len(), names.len() - before);
    assert_eq!(ours.len(), 14);
    for name in ours {
        assert_eq!(r.make(name).unwrap().info().name, name.as_str());
    }
}

/// A decoder of MPEG audio that takes coded media.
struct Mp2Sink;

impl Component for Mp2Sink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "test.mp2_sink",
            vec![Port::media("in", &[MediaSpec::any(Codec::Mp2)])],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

/// A consumer of coded media connected to a transport stream gets the shared demultiplexer, made for its codec.
#[test]
fn the_graph_inserts_the_demultiplexer_for_coded_media() {
    let mut r = Registry::new();
    sdrkit_proto_mpegts::register(&mut r);
    r.add("test.ts_source", || Box::new(TsSource::test_mux()));
    r.add("test.mp2_sink", || Box::new(Mp2Sink));
    let mut plan = GraphPlan::new();
    let src = plan.add(&r, "fec", "test.ts_source").unwrap();
    let sink = plan.add(&r, "audio", "test.mp2_sink").unwrap();
    plan.input(PortRef::new(src, 0)).unwrap();
    plan.connect(PortRef::new(src, 0), PortRef::new(sink, 0))
        .unwrap();
    let mut g = plan.build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
    assert!(
        g.order().iter().any(|id| id.starts_with("~media_demux")),
        "{:?}",
        g.order()
    );
}

/// A consumer of MPEG-4 access units.
struct AuSink;

impl Component for AuSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "test.au_sink",
            vec![Port::typed::<sdrkit_proto_mpegts::demux::sl::AccessUnit>(
                "in",
            )],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

/// The catalog converts a transport stream to MPEG-4 access units with `mpegts.sl_demux`, and the component joins a transport
/// stream to a consumer of access units.
#[test]
fn the_sl_demultiplexer_returns_access_units() {
    use sdrkit_proto_mpegts::demux::sl::AccessUnit;
    let mut r = Registry::new();
    sdrkit_proto_mpegts::register(&mut r);
    assert!(r
        .catalog()
        .adapters()
        .iter()
        .any(|a| a.kind == "mpegts.sl_demux"
            && a.from == stream::kind()
            && a.to == Kind::typed::<AccessUnit>()));
    let b = GraphBuilder::new()
        .add("fec", TsSource::test_mux())
        .add(
            "sl",
            sdrkit_proto_mpegts::components::SlDemuxComponent::default(),
        )
        .add("units", AuSink)
        .input("fec")
        .connect("fec.out", "sl.in")
        .connect("sl.out", "units.in");
    let mut g = b.build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
}

/// The layer publishes the services and tracks under their own names, writes a table only when a value changed, and takes
/// `service.select` with a row key of `services`.
#[test]
fn the_tables_are_published_and_rows_are_selected() {
    use sdrkit_core::command::CommandError;
    use sdrkit_core::{owner, Owner, Value};
    use sdrkit_proto_mpegts::publish::{
        SERVICES, SERVICE_ID, SERVICE_ROW, SERVICE_SELECT_KIND, TRACKS,
    };
    let parts = TsLayer::basic();
    let b = GraphBuilder::new()
        .add("fec", TsSource::test_mux())
        .input("fec")
        .declare(&layer::declare(ResultsDecl::new(), &parts));
    let mut g = attach(b, "fec.out", &parts).build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
    let mut out = Output::default();
    let mut feed = |g: &mut sdrkit_graph::Graph, n: usize| {
        for _ in 0..n {
            out.clear();
            g.feed(&[Complex32::new(0.0, 0.0); 16], &mut out);
        }
        g.refresh_results();
    };
    feed(&mut g, 40);
    let r = Owner::results(&g);
    let services = r.table(r.decl().table_id(SERVICES.key).unwrap()).unwrap();
    assert!(!services.is_empty());
    let (key, row) = services.rows_newest_first().next().unwrap();
    assert_eq!(
        row.get(&SERVICE_ID).map(|s| s.to_string()).as_deref(),
        Some(key)
    );
    let key = key.to_owned();
    assert!(r.decl().table_id(TRACKS.key).is_some());
    // The multiplex repeats the same tables: the services do not change.
    let since = services.change_counter();
    feed(&mut g, 40);
    let r = Owner::results(&g);
    let services = r.table(r.decl().table_id(SERVICES.key).unwrap()).unwrap();
    assert_eq!(services.rows_changed_since(since).count(), 0);

    let id = g.commands().id(SERVICE_SELECT_KIND.command.key).unwrap();
    let mut given = g.commands().def(id).map(|d| d.new_args()).unwrap();
    let row = given.decl().id(SERVICE_ROW.field.key).unwrap();
    given.set_value(row, &Value::Text("9999".into())).unwrap();
    assert!(matches!(
        owner::invoke(&mut g, id, &given),
        Err(CommandError::NoSuchRow { .. })
    ));
    given.set_value(row, &Value::Text(key)).unwrap();
    assert_eq!(owner::invoke(&mut g, id, &given), Ok(None));
}

/// `service.select` with an empty row plays the first service that can play, and a service id plays that service.
#[test]
fn an_empty_service_row_plays_the_first_service() {
    use sdrkit_core::command::CommandError;
    use sdrkit_proto_mpegts::components::TsDemuxComponent;
    use sdrkit_proto_mpegts::publish::SERVICE_ROW;
    let b = GraphBuilder::new()
        .add("fec", TsSource::test_mux())
        .input("fec");
    let mut g = attach(b, "fec.out", &TsLayer::basic()).build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
    let mut out = Output::default();
    for _ in 0..10 {
        out.clear();
        g.feed(&[Complex32::new(0.0, 0.0); 16], &mut out);
    }
    let select = |g: &mut sdrkit_graph::Graph, row: &str| {
        let c = g.component_mut(layer::DEMUX).unwrap();
        let id = c.commands().id("service.select").unwrap();
        let mut args = c.commands().def(id).unwrap().new_args();
        args.set(&SERVICE_ROW.field, row);
        let args = c.commands().check(id, &args)?;
        c.invoke(id, &args).map(|_| ())
    };
    let demux = |g: &sdrkit_graph::Graph| {
        let d = g
            .component_as::<TsDemuxComponent>(layer::DEMUX)
            .unwrap()
            .demux();
        (d.auto_select, d.selection.service, d.si.first_playable())
    };
    select(&mut g, "1").unwrap();
    assert_eq!(demux(&g), (false, Some(1), Some(1)));
    select(&mut g, "").unwrap();
    assert_eq!(demux(&g), (true, Some(1), Some(1)));
    assert!(matches!(
        select(&mut g, "0"),
        Err(CommandError::NoSuchRow { .. })
    ));
}

/// The service information has the country of the `country` setting, and none when the setting is empty.
#[test]
fn the_setting_gives_the_country() {
    use sdrkit_proto_mpegts::components::{PsiSi, TsDemuxComponent};
    let b = GraphBuilder::new()
        .add("fec", TsSource::silent())
        .input("fec");
    let mut g = attach(b, "fec.out", &TsLayer::basic()).build().unwrap();
    Receiver::configure(&mut g, &input()).unwrap();
    let countries = |g: &sdrkit_graph::Graph| {
        let demux = g.component_as::<TsDemuxComponent>(layer::DEMUX).unwrap();
        let si = g.component_as::<PsiSi>(layer::SI).unwrap();
        (demux.demux().si.country.clone(), si.db().country.clone())
    };
    assert_eq!(countries(&g), (String::new(), String::new()));
    for id in ["ts_demux.country", "psi_si.country"] {
        let t = g.target(id).unwrap();
        g.apply_to(t, &sdrkit_core::Value::Text("AUS".into()))
            .unwrap();
    }
    assert_eq!(countries(&g), ("AUS".into(), "AUS".into()));
    for id in ["ts_demux.country", "psi_si.country"] {
        let t = g.target(id).unwrap();
        g.apply_to(t, &sdrkit_core::Value::Text(String::new()))
            .unwrap();
    }
    assert_eq!(countries(&g), (String::new(), String::new()));
}
