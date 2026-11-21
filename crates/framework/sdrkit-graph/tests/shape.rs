//! A receiver made from an interface description declares exactly what the description holds.

use sdrkit_core::interface::{Fields, StackShape, Table};
use sdrkit_core::{Command, Owner};
use sdrkit_graph::components::NullSink;
use sdrkit_graph::{CommandTarget, Graph, GraphBuilder, GraphError, StackGraph};

/// A row that no component of the test graph publishes.
#[derive(Fields)]
struct ShipRow<'a> {
    /// The name.
    #[field(label = "Name")]
    name: &'a str,
}

static SHIPS: Table<ShipRow<'static>> = Table::new("ships", "Ships", 8);
const CLEAR: Command = Command::new("clear", "Clear");

fn graph() -> Graph {
    GraphBuilder::new()
        .add("sink", NullSink::default())
        .input("sink")
        .build()
        .expect("the graph builds")
}

#[test]
fn a_receiver_has_the_id_settings_and_results_of_its_description() {
    let shape = StackShape::new("demo");
    let rx = StackGraph::of(&shape, graph())
        .build()
        .expect("the receiver");
    assert_eq!(**rx.results_decl(), shape.results_decl());
    assert!(Owner::schema(&rx).is_empty());
    assert_eq!(ShipRow::NAME.key, "name");
}

#[test]
fn a_table_of_the_description_that_no_component_publishes_is_refused() {
    let shape = StackShape::new("demo").table(&SHIPS);
    match StackGraph::of(&shape, graph()).build() {
        Err(GraphError::Shape {
            stack: "demo",
            what,
        }) => assert!(what.contains("table ships"), "{what}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_command_of_the_description_that_the_graph_lacks_is_refused() {
    let shape = StackShape::new("demo").command(&CLEAR);
    let built = StackGraph::of(&shape, graph()).build();
    assert!(
        matches!(built, Err(GraphError::Shape { stack: "demo", .. })),
        "{built:?}"
    );
    let named = StackGraph::of(&shape, graph())
        .command("clear", CommandTarget::every("clear"))
        .build();
    assert!(
        matches!(named, Err(GraphError::UnresolvedCommand { .. })),
        "{named:?}"
    );
}

#[test]
fn another_declaration_than_the_description_is_refused() {
    let shape = StackShape::new("demo");
    let other = StackShape::new("demo").table(&SHIPS).results_decl();
    let built = StackGraph::of(&shape, graph()).declare(&other).build();
    match built {
        Err(GraphError::Shape { what, .. }) => assert!(what.contains("table ships"), "{what}"),
        other => panic!("{other:?}"),
    }
}

mod components {
    use sdrkit_core::interface::{Fields, StackShape, Table};
    use sdrkit_core::{
        Complex32, ConfigError, InputInfo, Output, Owner, Published, Receiver, Value,
    };
    use sdrkit_graph::{
        CommandTarget, Decoder, DecoderComponent, DecoderFields, Format, GraphBuilder, GraphError,
        Outbox, Port, StackGraph,
    };

    /// The count of a decoder of the test.
    #[derive(Fields)]
    struct HeardFields {
        /// The samples seen.
        #[field(label = "Seen")]
        seen: i64,
    }

    /// A row of the table of a decoder of the test.
    #[derive(Fields)]
    struct HeardRow {
        /// The samples seen.
        #[field(label = "Seen")]
        seen: i64,
    }

    static HEARD: Table<HeardRow> = Table::new("heard", "Heard", 4);

    /// Counts its samples into a field and a table row, and forgets the count on `clear`.
    #[derive(Default)]
    struct Heard(i64);

    impl Decoder for Heard {
        type Input = Complex32;
        fn name(&self) -> &'static str {
            "demo.heard"
        }
        fn outputs(&self) -> Vec<Port> {
            Vec::new()
        }
        fn values(&self) -> sdrkit_core::Values {
            sdrkit_graph::no_values()
        }
        fn results(&self) -> Published {
            Published::new(<HeardFields as Fields>::SHAPE.fields).with_tables(&[&HEARD])
        }
        fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
            Ok(Vec::new())
        }
        fn process(&mut self, x: &[Complex32], _first: u64, _out: &mut Outbox) {
            self.0 += x.len() as i64;
        }
        fn report(&mut self, p: &mut Published) {
            p.put(&HeardFields { seen: self.0 });
            if self.0 == 0 {
                p.clear_rows(&HEARD);
            } else {
                HEARD.put(p, "all", &HeardRow { seen: self.0 });
            }
        }
        fn clear(&mut self) {
            self.0 = 0;
        }
    }

    /// The stack of the test: the status decoder "main", and the decoder "egc" whose results the stack shows under
    /// its name.
    fn shape() -> StackShape {
        StackShape::new("demo")
            .fields::<HeardFields>()
            .fields::<DecoderFields>()
            .component("egc", |c| c.fields::<HeardFields>().dropped().table(&HEARD))
            .table(&HEARD)
            .command(&sdrkit_graph::decoder::CLEAR)
    }

    fn receiver(shape: &StackShape) -> Result<StackGraph, GraphError> {
        let graph = GraphBuilder::new()
            .add("main", DecoderComponent::new(Heard::default()))
            .add("egc", DecoderComponent::new(Heard::default()))
            .input("main")
            .input("egc")
            .status_from("main")
            .build()?;
        StackGraph::of(shape, graph)
            .command("clear", CommandTarget::every("clear"))
            .build()
    }

    fn seen(rx: &StackGraph, key: &str) -> Option<i64> {
        let fields = Owner::results(rx).fields();
        match fields.value(fields.decl().id(key)?) {
            Some(Value::I64(n)) => Some(*n),
            _ => None,
        }
    }

    fn fed(shape: &StackShape) -> StackGraph {
        let mut rx = receiver(shape).expect("the receiver");
        rx.configure(&InputInfo::new(48_000.0, 100e6, 100e6))
            .expect("the receiver takes the input");
        let mut out = Output::default();
        rx.feed(&vec![Complex32::new(0.0, 0.0); 9600], &mut out);
        rx.finish(&mut out);
        rx
    }

    #[test]
    fn the_results_of_a_named_component_have_its_name_before_their_keys() {
        let shape = shape();
        let rx = fed(&shape);
        assert_eq!(**rx.results_decl(), shape.results_decl());
        let keys: Vec<&str> = rx
            .results_decl()
            .fields()
            .fields()
            .iter()
            .map(|f| f.key.as_ref())
            .collect();
        assert_eq!(keys[..4], ["seen", "dropped", "egc.seen", "egc.dropped"]);
        assert_eq!(seen(&rx, "seen"), Some(9600));
        assert_eq!(seen(&rx, "egc.seen"), Some(9600));
        let of_egc: Vec<i64> = HEARD
            .rows_of::<HeardRow>("egc", Owner::results(&rx))
            .map(|(_, r)| r.seen)
            .collect();
        assert_eq!(of_egc, [9600]);
        assert_eq!(shape.decoders(), ["egc"]);
    }

    #[test]
    fn a_result_of_a_component_that_does_not_publish_it_is_refused() {
        let shape = shape().decoder("nobody");
        match receiver(&shape) {
            Err(GraphError::Shape { what, .. }) => {
                assert!(what.contains("nobody.dropped"), "{what}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_change_of_frequency_by_the_host_clears_what_the_description_does_not_keep() {
        // Nothing is kept: both decoders forget.
        let mut rx = fed(&shape());
        rx.forget();
        assert_eq!(seen(&rx, "seen"), Some(0));
        assert_eq!(seen(&rx, "egc.seen"), Some(0));
        assert_eq!(HEARD.rows::<HeardRow>(Owner::results(&rx)).count(), 0);
        assert_eq!(
            HEARD
                .rows_of::<HeardRow>("egc", Owner::results(&rx))
                .count(),
            0
        );
        // The table of the status decoder is kept. That decoder does not implement `forget`, so it keeps all its
        // results. The other forgets.
        let mut rx = fed(&shape().kept(&["heard"]));
        rx.forget();
        assert_eq!(seen(&rx, "seen"), Some(9600));
        assert_eq!(HEARD.rows::<HeardRow>(Owner::results(&rx)).count(), 1);
        assert_eq!(seen(&rx, "egc.seen"), Some(0));
        assert_eq!(
            HEARD
                .rows_of::<HeardRow>("egc", Owner::results(&rx))
                .count(),
            0
        );
        // A retune alone clears nothing.
        let mut rx = fed(&shape());
        rx.retuned(&InputInfo::new(48_000.0, 101e6, 101e6));
        rx.refresh_results();
        assert_eq!(seen(&rx, "seen"), Some(9600));
    }

    #[test]
    fn a_kept_item_that_the_description_does_not_hold_is_refused() {
        let built = receiver(&shape().kept(&["stations"]));
        match built {
            Err(GraphError::Shape { what, .. }) => assert!(what.contains("stations"), "{what}"),
            other => panic!("{other:?}"),
        }
    }

    /// A decoder with a log that the stack can keep and a level of the present signal, and with no command of its
    /// own beyond the `clear` of every decoder component.
    #[derive(Default)]
    struct Logger {
        log: i64,
        level: i64,
    }

    /// The results of [`Logger`].
    #[derive(Fields)]
    struct LoggerFields {
        /// The entries of the log.
        #[field(label = "Logged")]
        logged: i64,
        /// The level of the present signal.
        #[field(label = "Level")]
        level: i64,
        /// The good frames.
        #[field(label = "Frames good")]
        frames_ok: i64,
    }

    static LOG: Table<HeardRow> = Table::new("log", "Log", 4);

    impl Decoder for Logger {
        type Input = Complex32;
        fn name(&self) -> &'static str {
            "demo.logger"
        }
        fn outputs(&self) -> Vec<Port> {
            Vec::new()
        }
        fn values(&self) -> sdrkit_core::Values {
            sdrkit_graph::no_values()
        }
        fn results(&self) -> Published {
            Published::new(<LoggerFields as Fields>::SHAPE.fields).with_tables(&[&LOG])
        }
        fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
            Ok(Vec::new())
        }
        fn process(&mut self, x: &[Complex32], _first: u64, _out: &mut Outbox) {
            self.log += x.len() as i64;
            self.level += x.len() as i64;
        }
        fn report(&mut self, p: &mut Published) {
            p.put(&LoggerFields {
                logged: self.log,
                level: self.level,
                frames_ok: self.log,
            });
            if self.log == 0 {
                p.clear_rows(&LOG);
            } else {
                LOG.put(p, "all", &HeardRow { seen: self.log });
            }
        }
        fn clear(&mut self) {
            *self = Logger::default();
        }
        fn forget(&mut self, kept: &sdrkit_graph::Kept) {
            self.level = 0;
            if !kept.table(LOG.key) {
                self.log = 0;
            }
        }
    }

    /// The totals of the stack of [`Logger`].
    #[derive(Fields)]
    struct TotalFields {
        /// The good frames of every decoder.
        #[field(label = "Frames good")]
        frames_ok: i64,
    }

    /// The stack of the status decoder "main" (a [`Heard`]) and the decoder "link" (a [`Logger`]), whose good frames
    /// are the field `frames_ok` of the stack.
    fn log_shape() -> StackShape {
        StackShape::new("demo")
            .fields::<HeardFields>()
            .fields::<TotalFields>()
            .fields::<DecoderFields>()
            .receiver_fields::<sdrkit_graph::GuardFields>()
            .component("link", |c| {
                c.fields::<LoggerFields>()
                    .shares(&["frames_ok"])
                    .dropped()
                    .table(&LOG)
            })
            .table(&HEARD)
            .command(&sdrkit_graph::decoder::CLEAR)
    }

    fn log_receiver(shape: &StackShape) -> Result<StackGraph, GraphError> {
        let graph = GraphBuilder::new()
            .add("main", DecoderComponent::new(Heard::default()))
            .add("link", DecoderComponent::new(Logger::default()))
            .input("main")
            .input("link")
            .status_from("main")
            .build()?;
        StackGraph::of(shape, graph)
            .command("clear", CommandTarget::every("clear"))
            .build()
    }

    fn log_fed(shape: &StackShape) -> StackGraph {
        let mut rx = log_receiver(shape).expect("the receiver");
        rx.configure(&InputInfo::new(48_000.0, 100e6, 100e6))
            .expect("the receiver takes the input");
        let mut out = Output::default();
        rx.feed(&vec![Complex32::new(0.0, 0.0); 9600], &mut out);
        rx.finish(&mut out);
        rx
    }

    #[test]
    fn a_kept_table_survives_and_the_other_results_of_its_component_are_cleared() {
        let logged = |rx: &StackGraph| LOG.rows_of::<HeardRow>("link", Owner::results(rx)).count();
        // The log of "link" is kept, its level is not.
        let mut rx = log_fed(&log_shape().kept(&["link.log"]));
        assert_eq!((seen(&rx, "link.level"), logged(&rx)), (Some(9600), 1));
        rx.forget();
        assert_eq!(seen(&rx, "link.logged"), Some(9600));
        assert_eq!(seen(&rx, "link.level"), Some(0));
        assert_eq!(logged(&rx), 1);
        // The other decoder keeps nothing.
        assert_eq!(seen(&rx, "seen"), Some(0));
        // Nothing is kept: the log goes too.
        let mut rx = log_fed(&log_shape());
        rx.forget();
        assert_eq!((seen(&rx, "link.logged"), logged(&rx)), (Some(0), 0));
        // One field of a result set is kept by `<Struct>.<key>`.
        let shape = log_shape().kept(&["HeardFields.seen"]);
        assert_eq!(shape.kept_keys().expect("the keys"), ["seen"]);
        let mut rx = log_fed(&shape);
        rx.forget();
        assert_eq!(seen(&rx, "seen"), Some(9600));
        assert_eq!(seen(&rx, "link.level"), Some(0));
    }

    /// A retune that the host did not order clears a component of the retune list through `forget`: its kept log
    /// stays. With nothing kept the `clear` command of the list runs.
    #[test]
    fn a_retune_command_keeps_what_the_description_keeps() {
        let logged = |rx: &StackGraph| LOG.rows_of::<HeardRow>("link", Owner::results(rx)).count();
        let retuned = |shape: &StackShape| {
            let graph = GraphBuilder::new()
                .add("main", DecoderComponent::new(Heard::default()))
                .add("link", DecoderComponent::new(Logger::default()))
                .input("main")
                .input("link")
                .status_from("main")
                .build()
                .expect("the graph");
            let mut rx = StackGraph::of(shape, graph)
                .command("clear", CommandTarget::every("clear"))
                .on_retune(CommandTarget::of("link", "clear"))
                .expect("the component has the command")
                .build()
                .expect("the receiver");
            rx.configure(&InputInfo::new(48_000.0, 100e6, 100e6))
                .expect("the receiver takes the input");
            let mut out = Output::default();
            rx.feed(&vec![Complex32::new(0.0, 0.0); 9600], &mut out);
            rx.finish(&mut out);
            rx.retuned(&InputInfo::new(48_000.0, 101e6, 101e6));
            rx.refresh_results();
            rx
        };
        let rx = retuned(&log_shape().kept(&["link.log"]));
        assert_eq!(seen(&rx, "link.logged"), Some(9600));
        assert_eq!(seen(&rx, "link.level"), Some(0));
        assert_eq!(logged(&rx), 1);
        // The decoder that the list does not name is not cleared.
        assert_eq!(seen(&rx, "seen"), Some(9600));
        let rx = retuned(&log_shape());
        assert_eq!((seen(&rx, "link.logged"), logged(&rx)), (Some(0), 0));
    }

    #[test]
    fn a_shared_field_has_the_key_of_the_stack_and_the_count_of_the_guard_is_there_from_the_start()
    {
        let shape = log_shape();
        let rx = log_receiver(&shape).expect("the receiver");
        // The receiver writes the count of its guard before the first sample, and with 0.
        assert_eq!(seen(&rx, "bad_samples"), Some(0));
        let rx = log_fed(&shape);
        assert_eq!(**rx.results_decl(), shape.results_decl());
        let keys: Vec<&str> = rx
            .results_decl()
            .fields()
            .fields()
            .iter()
            .map(|f| f.key.as_ref())
            .collect();
        assert_eq!(
            keys,
            [
                "seen",
                "frames_ok",
                "dropped",
                "bad_samples",
                "link.logged",
                "link.level",
                "link.dropped"
            ]
        );
        // The status decoder does not write the shared field, so the value of "link" is shown.
        assert_eq!(seen(&rx, "frames_ok"), Some(9600));
        assert_eq!(seen(&rx, "bad_samples"), Some(0));
        // A shared field that the component does not publish is refused.
        let wrong = StackShape::new("demo")
            .fields::<HeardFields>()
            .fields::<TotalFields>()
            .fields::<DecoderFields>()
            .component("main", |c| {
                c.fields::<LoggerFields>().shares(&["frames_ok"])
            });
        assert!(matches!(
            log_receiver(&wrong),
            Err(GraphError::Shape { .. })
        ));
    }

    #[test]
    fn a_table_of_the_stack_names_its_writers() {
        // Both decoders write the table `heard` of the stack.
        let shape = StackShape::new("demo")
            .fields::<HeardFields>()
            .fields::<DecoderFields>()
            .table(&HEARD)
            .component("main", |c| c.writes_table(&HEARD))
            .component("egc", |c| {
                c.fields::<HeardFields>().dropped().writes_table(&HEARD)
            })
            .command(&sdrkit_graph::decoder::CLEAR);
        assert_eq!(shape.table_writers, [("heard", vec!["main", "egc"])]);
        let rx = fed(&shape);
        assert_eq!(HEARD.rows::<HeardRow>(Owner::results(&rx)).count(), 1);
        assert!(rx.results_decl().table_id("egc.heard").is_none());
        // A writer that does not publish the table is refused.
        let wrong = shape.component("nobody", |c| c.writes_table(&HEARD));
        match receiver(&wrong) {
            Err(GraphError::Shape { what, .. }) => assert!(what.contains("nobody"), "{what}"),
            other => panic!("{other:?}"),
        }
    }

    /// A decoder whose count of entries has the key of its table.
    #[derive(Default)]
    struct Journal {
        entries: i64,
    }

    /// The results of [`Journal`], and the totals of its stack.
    #[derive(Fields)]
    struct JournalFields {
        /// The entries of the log.
        #[field(label = "Log")]
        log: i64,
    }

    impl Decoder for Journal {
        type Input = Complex32;
        fn name(&self) -> &'static str {
            "demo.journal"
        }
        fn outputs(&self) -> Vec<Port> {
            Vec::new()
        }
        fn values(&self) -> sdrkit_core::Values {
            sdrkit_graph::no_values()
        }
        fn results(&self) -> Published {
            Published::new(<JournalFields as Fields>::SHAPE.fields).with_tables(&[&LOG])
        }
        fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
            Ok(Vec::new())
        }
        fn process(&mut self, x: &[Complex32], _first: u64, _out: &mut Outbox) {
            self.entries += x.len() as i64;
        }
        fn report(&mut self, p: &mut Published) {
            p.put(&JournalFields { log: self.entries });
            LOG.put(p, "all", &HeardRow { seen: self.entries });
        }
    }

    #[test]
    fn a_shared_field_can_have_the_key_of_a_table_of_its_component() {
        // The field `log` of "journal" is the field of the stack, and its table `log` is `journal.log`.
        let shape = StackShape::new("demo")
            .fields::<HeardFields>()
            .fields::<JournalFields>()
            .fields::<DecoderFields>()
            .component("journal", |c| {
                c.fields::<JournalFields>()
                    .shares(&["log"])
                    .dropped()
                    .table(&LOG)
            })
            .table(&HEARD)
            .command(&sdrkit_graph::decoder::CLEAR);
        let graph = GraphBuilder::new()
            .add("main", DecoderComponent::new(Heard::default()))
            .add("journal", DecoderComponent::new(Journal::default()))
            .input("main")
            .input("journal")
            .status_from("main")
            .build()
            .expect("the graph");
        let mut rx = StackGraph::of(&shape, graph)
            .command("clear", CommandTarget::every("clear"))
            .build()
            .expect("the receiver");
        assert_eq!(**rx.results_decl(), shape.results_decl());
        rx.configure(&InputInfo::new(48_000.0, 100e6, 100e6))
            .expect("the receiver takes the input");
        let mut out = Output::default();
        rx.feed(&vec![Complex32::new(0.0, 0.0); 9600], &mut out);
        rx.finish(&mut out);
        assert_eq!(seen(&rx, "log"), Some(9600));
        assert_eq!(
            LOG.rows_of::<HeardRow>("journal", Owner::results(&rx))
                .count(),
            1
        );
    }
}
