//! The tests of sdrkit-util-objects, in one program.

mod alloc_count {
    //! An allocator that counts the allocations of the current thread only, so tests on other threads do not disturb a count.
    //! (A global allocator needs `unsafe impl GlobalAlloc`. The library itself forbids `unsafe`.)
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    /// Allocations made by `f` on this thread.
    pub fn count(f: impl FnOnce()) -> usize {
        let before = COUNT.with(Cell::get);
        f();
        COUNT.with(Cell::get) - before
    }
}

mod table {
    use sdrkit_util_objects::{Error, ObjectTable, Stamp};

    fn filled(n: u32) -> ObjectTable<u32, u32> {
        let mut t = ObjectTable::new(n as usize);
        for k in 0..n {
            t.upsert(&k, Stamp::at(k as f64), || k * 10).unwrap();
        }
        t
    }

    #[test]
    fn least_recently_heard_leaves_first() {
        let mut t = filled(3);
        t.upsert(&0, Stamp::at(10.0), || 0).unwrap(); // 0 is now the newest
        let up = t.upsert(&9, Stamp::at(11.0), || 90).unwrap();
        assert!(up.new);
        let e = up.evicted.unwrap();
        assert_eq!((e.key, e.record, e.seen.last_s), (1, 10, 1.0));
        let order: Vec<u32> = t.iter().map(|r| *r.key).collect();
        assert_eq!(order, [2, 0, 9]);
        let newest: Vec<u32> = t.iter_newest_first().map(|r| *r.key).collect();
        assert_eq!(newest, [9, 0, 2]);
    }

    #[test]
    fn seen_counts_and_keeps_the_latest_time() {
        let mut t: ObjectTable<u32, ()> = ObjectTable::new(4);
        t.upsert(&1, Stamp::at(5.0).with_wall(1000.0), || ())
            .unwrap();
        t.upsert(&1, Stamp::at(9.0).with_wall(1004.0), || ())
            .unwrap();
        t.upsert(&1, Stamp::at(7.0), || ()).unwrap(); // a late report
        let s = t.seen(&1).unwrap();
        assert_eq!((s.first_s, s.last_s, s.count), (5.0, 9.0, 3));
        assert_eq!((s.first_wall, s.last_wall), (Some(1000.0), Some(1004.0)));
    }

    #[test]
    fn favourites_never_leave_and_a_table_of_favourites_refuses_new_rows() {
        let mut t = filled(2);
        assert!(t.set_favourite(&0, true));
        let up = t.upsert(&5, Stamp::at(5.0), || 0).unwrap();
        assert_eq!(up.evicted.unwrap().key, 1); // 0 was older but is a favourite
        assert!(t.set_favourite(&5, true));
        assert_eq!(t.upsert(&6, Stamp::at(6.0), || 0).unwrap_err(), Error::Full);
        assert_eq!(t.len(), 2);
        assert_eq!(t.expire(1e6, 1.0), 0);
        t.clear();
        assert_eq!(t.len(), 2);
        t.clear_all();
        assert!(t.is_empty());
    }

    #[test]
    fn bad_times_are_refused_and_change_nothing() {
        let mut t = filled(2);
        assert_eq!(
            t.upsert(&7, Stamp::at(f64::NAN), || 0).unwrap_err(),
            Error::BadTime
        );
        assert_eq!(
            t.upsert(&0, Stamp::at(1.0).with_wall(f64::INFINITY), || 0)
                .unwrap_err(),
            Error::BadTime
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t.seen(&0).unwrap().count, 1);
        assert_eq!(t.expire(f64::NAN, 1.0), 0);
    }

    #[test]
    fn ageing_removes_the_least_recently_heard() {
        let mut t = filled(5);
        assert_eq!(t.expire(10.0, 7.5), 3);
        assert_eq!(t.iter().map(|r| *r.key).collect::<Vec<_>>(), [3, 4]);
        assert_eq!(t.iter().filter(|r| r.seen.within(10.0, 7.0)).count(), 2);
        assert_eq!(t.newest_s(), Some(4.0));
    }

    #[test]
    fn a_smaller_cap_drops_the_oldest() {
        let mut t = filled(5);
        t.set_favourite(&0, true);
        assert_eq!(t.set_cap(2), 3);
        let keys: Vec<u32> = t.iter().map(|r| *r.key).collect();
        assert_eq!(keys, [0, 4]);
        t.set_cap(10);
        for k in 10..20 {
            t.upsert(&k, Stamp::at(k as f64), || 0).unwrap();
        }
        assert_eq!(t.len(), 10);
    }

    #[test]
    fn string_keys_are_looked_up_by_str() {
        let mut t: ObjectTable<String, u32> = ObjectTable::new(3);
        *t.upsert("VH-OQA", Stamp::at(1.0), || 0).unwrap().row += 5;
        assert_eq!(t.get("VH-OQA"), Some(&5));
        *t.get_mut("VH-OQA").unwrap() += 1;
        assert_eq!(t.remove("VH-OQA").unwrap().record, 6);
        assert!(!t.contains("VH-OQA"));
    }

    #[test]
    fn retain_and_records_mut() {
        let mut t = filled(6);
        t.retain(|r| r.key % 2 == 0);
        assert_eq!(t.len(), 3);
        for (_, r) in t.records_mut() {
            *r += 1;
        }
        assert_eq!(t.get(&4), Some(&41));
        // Removing in the middle keeps the order of the others.
        let order: Vec<u32> = t.iter().rev().map(|r| *r.key).collect();
        assert_eq!(order, [4, 2, 0]);
    }

    #[test]
    fn adopt_keeps_the_own_cap_and_the_newest_rows() {
        let saved = filled(10);
        let mut t: ObjectTable<u32, u32> = ObjectTable::new(3);
        t.adopt(saved);
        let keys: Vec<u32> = t.iter().map(|r| *r.key).collect();
        assert_eq!(keys, [7, 8, 9]);
        assert_eq!(t.cap(), 3);
    }

    #[test]
    fn serde_round_trip_and_refusals() {
        let mut t = filled(4);
        t.set_favourite(&2, true);
        let text = serde_json::to_string(&t).unwrap();
        let back: ObjectTable<u32, u32> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, t);
        assert!(back.row(&2).unwrap().favourite);
        let twice = r#"{"cap":4,"rows":[
            {"key":1,"seen":{"first_s":0,"last_s":0,"first_wall":null,"last_wall":null,"count":1},"record":0},
            {"key":1,"seen":{"first_s":0,"last_s":0,"first_wall":null,"last_wall":null,"count":1},"record":0}]}"#;
        assert!(serde_json::from_str::<ObjectTable<u32, u32>>(twice).is_err());
        let not_finite = r#"{"cap":4,"rows":[
            {"key":1,"seen":{"first_s":0,"last_s":null,"first_wall":null,"last_wall":null,"count":1},"record":0}]}"#;
        assert!(serde_json::from_str::<ObjectTable<u32, u32>>(not_finite).is_err());
    }
}

mod persist {
    use sdrkit_core::{MemStorage, StateBlob, StateError, Storage, StorePath};
    use sdrkit_util_geo::Position;
    use sdrkit_util_objects::{
        Error, Format, LogLine, LogRing, ObjectTable, Shift, Spacing, Stamp, Tables, Trail,
    };
    use serde::{Deserialize, Serialize};

    const FMT: Format = Format::new("test.heard", 2);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Ship {
        position_time_s: f64,
        trail: Trail,
    }

    impl Shift for Ship {
        fn shift_times(&mut self, by_s: f64) {
            self.position_time_s += by_s;
            self.trail.shift_times(by_s);
        }
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Heard {
        ships: ObjectTable<u32, Ship>,
        log: LogRing<LogLine>,
    }

    impl Shift for Heard {
        fn shift_times(&mut self, by_s: f64) {
            self.ships.shift_times(by_s);
            self.log.shift_times(by_s);
        }
    }

    impl Tables for Heard {
        const FORMAT: Format = FMT;
        fn clear(&mut self) {
            self.ships.clear();
            self.log.clear();
        }
        fn adopt(&mut self, saved: Self) {
            self.ships.adopt(saved.ships);
            self.log.adopt(saved.log);
        }
    }

    fn heard() -> Heard {
        let mut h = Heard {
            ships: ObjectTable::new(300),
            log: LogRing::new(100),
        };
        let mut trail = Trail::new(100, Spacing::metres(20.0));
        trail.push(90.0, Position::new(-33.85, 151.2), ()).unwrap();
        h.ships
            .upsert(
                &503_000_001,
                Stamp::at(100.0).with_wall(1_762_646_400.0),
                || Ship {
                    position_time_s: 90.0,
                    trail,
                },
            )
            .unwrap();
        h.log.push(LogLine {
            at: Stamp::at(95.0),
            text: "ship 503000001 heard".into(),
        });
        h
    }

    #[test]
    fn the_state_is_tagged_with_the_format() {
        let blob = FMT.save(130.5, &[1, 2]).unwrap();
        assert_eq!((blob.owner(), blob.version()), ("test.heard", 2));
        assert_eq!(blob.bytes(), br#"{"clock":130.5,"data":[1,2]}"#);
    }

    #[test]
    fn ages_are_kept_and_wall_times_are_not_moved() {
        let h = heard();
        let state = h.save_state(130.0).unwrap();
        let mut back = Heard {
            ships: ObjectTable::new(300),
            log: LogRing::new(100),
        };
        back.restore_state(&state, 7.0).unwrap();
        let shift = 7.0 - 130.0;
        let s = back.ships.seen(&503_000_001).unwrap();
        assert_eq!(s.age(7.0), 30.0);
        assert_eq!(s.last_wall, Some(1_762_646_400.0));
        let ship = back.ships.get(&503_000_001).unwrap();
        assert_eq!(ship.position_time_s, 90.0 + shift);
        assert_eq!(ship.trail.last().unwrap().t, 90.0 + shift);
        assert_eq!(back.log.iter().next().unwrap().at.t, 95.0 + shift);
        assert_eq!(back.log.cap(), 100);
    }

    #[test]
    fn the_state_survives_storage() {
        let storage = MemStorage::new();
        let path = StorePath::new("objects/heard.state").unwrap();
        let blob = heard().save_state(130.0).unwrap();
        storage
            .write(&path, &serde_json::to_vec(&blob).unwrap())
            .unwrap();
        let read: StateBlob = serde_json::from_slice(&storage.read(&path).unwrap()).unwrap();
        let mut back = heard();
        back.ships.clear_all();
        back.restore_state(&read, 130.0).unwrap();
        assert_eq!(back, heard());
    }

    #[test]
    fn a_refused_state_changes_nothing() {
        let h = heard();
        let mut other = heard();
        other.ships.clear_all();
        let wrong_format = Format::new("test.other", 2).save(1.0, &h).unwrap();
        assert_eq!(
            other.restore_state(&wrong_format, 0.0),
            Err(StateError::WrongOwner {
                found: "test.other".into()
            })
        );
        let wrong_version = Format::new("test.heard", 1).save(1.0, &h).unwrap();
        assert_eq!(
            other.restore_state(&wrong_version, 0.0),
            Err(StateError::WrongVersion {
                expected: 2,
                found: 1
            })
        );
        let damaged = StateBlob::new("test.heard", 2, b"{\"clock\":1}".to_vec());
        assert_eq!(other.restore_state(&damaged, 0.0), Err(StateError::Damaged));
        assert!(other.ships.is_empty());
    }

    #[test]
    fn bad_clocks() {
        assert_eq!(FMT.save(f64::NAN, &1).unwrap_err(), Error::BadTime);
        let ok = FMT.save(1.0, &Vec::<Stamp>::new()).unwrap();
        assert_eq!(
            FMT.restore::<Vec<Stamp>>(&ok, f64::INFINITY).unwrap_err(),
            Error::BadTime
        );
        let bad = StateBlob::new("test.heard", 2, br#"{"clock":null,"data":[]}"#.to_vec());
        assert!(FMT.restore::<Vec<Stamp>>(&bad, 0.0).is_err());
    }

    #[test]
    fn clear_keeps_favourites() {
        let mut h = heard();
        h.ships.set_favourite(&503_000_001, true);
        h.ships.upsert(&1, Stamp::at(1.0), h_ship).unwrap();
        h.clear();
        assert_eq!(h.ships.len(), 1);
        assert!(h.log.is_empty());
    }

    fn h_ship() -> Ship {
        Ship {
            position_time_s: 0.0,
            trail: Trail::new(4, Spacing::default()),
        }
    }
}

mod trail {
    use sdrkit_util_geo::{Position, EARTH_MEAN_RADIUS_M};
    use sdrkit_util_objects::{Error, Spacing, Trail};

    #[test]
    fn spacing_in_time_and_cap() {
        let mut t: Trail<i16> = Trail::new(3, Spacing::seconds(5.0));
        for i in 0..20 {
            t.push(i as f64, Position::new(10.0 + i as f64 * 0.01, 20.0), i)
                .unwrap();
        }
        let times: Vec<f64> = t.points().map(|p| p.t).collect();
        assert_eq!(times, [5.0, 10.0, 15.0]);
        assert_eq!(t.last().unwrap().extra, 15);
        assert_eq!(t.expire(17.0, 5.0), 2);
    }

    #[test]
    fn spacing_in_distance() {
        // 0.0002 degrees of latitude is 22.24 m on the mean sphere (R = 6 371 008.8 m).
        let step = 0.0002f64;
        let metres = EARTH_MEAN_RADIUS_M * step.to_radians();
        assert!((metres - 22.239).abs() < 1e-3);
        let mut t: Trail = Trail::new(100, Spacing::metres(22.0));
        assert!(t.push(0.0, Position::new(50.0, -1.0), ()).unwrap());
        assert!(!t
            .push(1.0, Position::new(50.0 + step / 2.0, -1.0), ())
            .unwrap());
        assert!(t.push(2.0, Position::new(50.0 + step, -1.0), ()).unwrap());
    }

    #[test]
    fn bad_input_is_refused() {
        let mut t: Trail = Trail::new(4, Spacing::default());
        assert_eq!(
            t.push(f64::NAN, Position::new(0.0, 0.0), ()).unwrap_err(),
            Error::BadTime
        );
        assert_eq!(
            t.push(0.0, Position::new(91.0, 0.0), ()).unwrap_err(),
            Error::BadPosition
        );
        assert!(t.is_empty());
        let bad = r#"{"cap":4,"spacing":{"min_s":0,"min_m":0},"points":[{"t":0,"position":{"lat_deg":95,"lon_deg":0,"alt_m":0},"extra":null}]}"#;
        assert!(serde_json::from_str::<Trail>(bad).is_err());
    }

    #[test]
    fn date_line_segments_and_serde() {
        let mut t: Trail = Trail::new(10, Spacing::default());
        for (i, lon) in [170.0, 179.0, -179.0, -170.0, 175.0]
            .into_iter()
            .enumerate()
        {
            t.push(i as f64, Position::new(0.0, lon), ()).unwrap();
        }
        let segs: Vec<(f64, f64)> = t
            .segments()
            .map(|(a, b)| (a.position.lon_deg, b.position.lon_deg))
            .collect();
        assert_eq!(segs, [(170.0, 179.0), (-179.0, -170.0)]);
        let back: Trail = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        let mut small: Trail = Trail::new(2, Spacing::default());
        small.adopt(back);
        assert_eq!(small.points().next().unwrap().position.lon_deg, -170.0);
    }
}

mod log {
    use sdrkit_util_objects::LogRing;

    #[test]
    fn caps_and_edits() {
        let mut l = LogRing::new(3);
        for i in 0..5 {
            l.push(i);
        }
        assert_eq!(l.iter().copied().collect::<Vec<_>>(), [2, 3, 4]);
        *l.find_recent_mut(2, |&x| x == 3).unwrap() = 30;
        assert!(l.find_recent_mut(1, |&x| x == 2).is_none());
        *l.newest_mut().unwrap() += 1;
        assert_eq!(l.newest_first().copied().collect::<Vec<_>>(), [5, 30, 2]);
        l.set_cap(1);
        assert_eq!(l.len(), 1);
        l.set_cap(0);
        assert_eq!(l.cap(), 1);
    }

    #[test]
    fn a_restored_log_keeps_its_cap() {
        // The cap of a saved log survives a restore.
        let mut l = LogRing::new(300);
        for i in 0..10 {
            l.push(i);
        }
        let back: LogRing<i32> = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back.cap(), 300);
        assert_eq!(back.len(), 10);
    }
}

mod dedup {
    use sdrkit_util_objects::{fingerprint, Dedup};

    #[test]
    fn fnv1a_64_published_vectors() {
        // From the FNV test vectors published with the algorithm (Fowler, Noll, Vo).
        assert_eq!(fingerprint(b""), 0xcbf29ce484222325);
        assert_eq!(fingerprint(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fingerprint(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn a_full_memory_still_catches_repeats() {
        // A memory emptied all at once when full lets the next repeats through. This one drops only the oldest key.
        let mut d = Dedup::new(600.0, 4);
        for k in 0..4u64 {
            assert!(!d.seen(k, k as f64));
        }
        assert!(!d.seen(4, 4.0)); // key 0 leaves
        assert!(d.seen(3, 5.0));
        assert!(d.seen(1, 5.0));
        assert!(!d.seen(0, 5.0));
        assert_eq!(d.len(), 4);
    }

    #[test]
    fn the_window_runs_from_the_first_sighting() {
        let mut d = Dedup::new(10.0, 100);
        assert!(!d.seen(7u64, 0.0));
        assert!(d.seen(7, 9.0));
        assert!(d.seen(7, 10.0));
        assert!(!d.seen(7, 10.5));
        assert!(d.contains(&7, 11.0));
        assert!(!d.seen(8, f64::NAN));
        assert!(!d.contains(&8, 11.0));
        d.clear();
        assert!(d.is_empty());
    }
}

mod joiner {
    use sdrkit_util_objects::{JoinRules, Joined, Joiner, Numbering, Part};

    type J = Joiner<&'static str, String>;

    fn part(t: f64, message: u32, number: Numbering, last: bool, body: &str) -> Part<String> {
        Part {
            t,
            message,
            number,
            last,
            body: body.into(),
        }
    }

    #[test]
    fn three_parts_join() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push(
            "N1",
            part(1.0, 1, Numbering::Seq(0), false, "WX OMDB 1200Z "),
            &mut out,
        );
        j.push(
            "N1",
            part(2.0, 1, Numbering::Seq(1), false, "WIND 270/12 "),
            &mut out,
        );
        assert!(out.is_empty());
        j.push(
            "N1",
            part(3.0, 1, Numbering::Seq(2), true, "QNH 1008"),
            &mut out,
        );
        assert_eq!(out.len(), 1);
        let m = &out[0];
        assert_eq!(m.body, "WX OMDB 1200Z WIND 270/12 QNH 1008");
        assert_eq!((m.parts, m.complete, m.cut), (3, true, false));
        assert_eq!((m.first_t, m.last_t), (1.0, 3.0));
    }

    #[test]
    fn a_repeat_is_dropped_and_a_gap_marks_incomplete() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push(
            "N1",
            part(1.0, 1, Numbering::Seq(0), false, "AAA"),
            &mut out,
        );
        j.push(
            "N1",
            part(1.5, 1, Numbering::Seq(0), false, "AAA"),
            &mut out,
        );
        j.push("N1", part(2.0, 1, Numbering::Seq(2), true, "CCC"), &mut out);
        assert_eq!(out[0].body, "AAACCC");
        assert_eq!(out[0].parts, 2);
        assert!(!out[0].complete);
    }

    #[test]
    fn tags_show_repeats_only() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push(
            "N1|80",
            part(1.0, 0, Numbering::Tag(b'A' as u32), false, "FPL PART ONE "),
            &mut out,
        );
        j.push(
            "N1|80",
            part(1.5, 0, Numbering::Tag(b'A' as u32), false, "FPL PART ONE "),
            &mut out,
        );
        j.push(
            "N1|80",
            part(2.0, 0, Numbering::Tag(b'B' as u32), true, "PART TWO"),
            &mut out,
        );
        assert_eq!(out[0].body, "FPL PART ONE PART TWO");
        assert!(out[0].complete);
    }

    #[test]
    fn a_missing_first_part_marks_incomplete() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push("K", part(1.0, 0, Numbering::Seq(1), false, "B"), &mut out);
        j.push("K", part(2.0, 0, Numbering::Seq(2), true, "C"), &mut out);
        assert!(!out[0].complete);
        j.push("K", part(3.0, 0, Numbering::Seq(0), true, "only"), &mut out);
        assert!(out[1].complete && out[1].parts == 1);
    }

    #[test]
    fn a_new_message_number_ends_the_open_one() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push("K", part(1.0, 1, Numbering::Seq(0), false, "old"), &mut out);
        j.push(
            "K",
            part(2.0, 2, Numbering::Seq(0), false, "new "),
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert!(!out[0].complete && out[0].body == "old");
        j.push("K", part(3.0, 2, Numbering::Seq(1), true, "end"), &mut out);
        assert_eq!(out[1].body, "new end");
        assert!(out[1].complete);
    }

    #[test]
    fn caps_timeouts_and_a_clock_that_goes_back() {
        let rules = JoinRules {
            timeout_s: 45.0,
            max_open: 4,
            max_len: 10,
        };
        let mut j: Joiner<u32, String> = Joiner::new(rules);
        let mut out: Vec<Joined<u32, String>> = Vec::new();
        for k in 0..10 {
            j.push(k, part(1.0, 0, Numbering::Seq(0), false, "x"), &mut out);
        }
        assert_eq!(j.open_count(), 4);
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|m| !m.complete));
        assert_eq!(out[0].key, 0);
        out.clear();
        j.expire(100.0, &mut out);
        assert_eq!((out.len(), j.open_count()), (4, 0));
        out.clear();
        j.push(
            1,
            part(200.0, 0, Numbering::Seq(0), false, "123456"),
            &mut out,
        );
        j.push(
            1,
            part(201.0, 0, Numbering::Seq(1), false, "789012"),
            &mut out,
        ); // too long: cut
        j.push(1, part(202.0, 0, Numbering::Seq(2), true, "34"), &mut out);
        assert_eq!(out[0].body, "12345634");
        assert!(out[0].cut && !out[0].complete);
        out.clear();
        j.push(2, part(300.0, 0, Numbering::Seq(0), false, "a"), &mut out);
        j.push(3, part(298.0, 0, Numbering::Seq(0), false, "b"), &mut out); // the clock went back 2 s
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].key, 2);
    }

    #[test]
    fn off_bad_times_and_flush() {
        let mut j: J = Joiner::new(JoinRules::default());
        let mut out = Vec::new();
        j.push("K", part(1.0, 0, Numbering::None, false, "a"), &mut out);
        j.set_enabled(false, &mut out);
        assert_eq!(out.len(), 1);
        j.push("K", part(2.0, 0, Numbering::None, false, "b"), &mut out);
        assert_eq!(out.len(), 2);
        assert!(out[1].complete);
        j.set_enabled(true, &mut out);
        j.push(
            "K",
            part(f64::NAN, 0, Numbering::None, false, "c"),
            &mut out,
        );
        assert!(!out[2].complete);
        j.push("K", part(3.0, 0, Numbering::None, false, "d"), &mut out);
        j.push("K", part(4.0, 0, Numbering::None, false, "e"), &mut out);
        j.flush(&mut out);
        assert_eq!(out[3].body, "de");
    }
}

mod activity {
    use sdrkit_util_objects::{Activity, Touch};

    #[test]
    fn hang_time_and_terminators() {
        let mut a = Activity::new(2.0, 4);
        assert_eq!(a.touch(1u8, 0.0), Touch::Started);
        assert_eq!(a.touch(2, 0.5), Touch::Started);
        assert_eq!(a.touch(1, 1.0), Touch::Continued);
        let c = a.close(2).unwrap();
        assert_eq!((c.start_s, c.last_s), (0.5, 0.5));
        let mut ended = Vec::new();
        assert_eq!(a.expire(2.9, |c| ended.push(c.key)), 0);
        assert_eq!(a.expire(3.1, |c| ended.push(c.key)), 1);
        assert_eq!(ended, [1]);
        a.touch(3, 4.0);
        a.close_all(|c| ended.push(c.key));
        assert_eq!(ended, [1, 3]);
        assert!(a.is_empty());
        assert!(!a.set_hang_s(-1.0));
        assert_eq!(a.touch(5, f64::NAN), Touch::Continued);
        assert!(a.is_empty());
    }
}

mod calls {
    use super::alloc_count::count;
    use sdrkit_util_objects::{CallEnd, CallEvent, CallLog, SameCall, Shift, Stamp};
    use serde::{Deserialize, Serialize};

    /// A voice call's identity, as a digital voice stack provides it.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Voice {
        source: u32,
        talkgroup: u32,
    }

    impl SameCall for Voice {
        fn same_call(&self, o: &Self) -> bool {
            self.source == o.source && self.talkgroup == o.talkgroup
        }
    }

    impl Shift for Voice {
        fn shift_times(&mut self, _by_s: f64) {}
    }

    fn voice(source: u32, talkgroup: u32) -> Voice {
        Voice { source, talkgroup }
    }

    #[test]
    fn calls_start_continue_and_end_by_terminator_or_hang_time() {
        // A terminator ends a call. 2 s without a frame ends it as signal lost.
        let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 2, 300);
        let mut ev = Vec::new();
        let c = calls.begin(
            1,
            Stamp::at(1.0).with_wall(1_762_646_400.0),
            false,
            voice(5, 91),
            |e| ev.push(e),
        );
        assert_eq!((c.id, c.is_active()), (1, true));
        assert!(calls.frame(1, 1.5).is_some());
        // The header again, inside the call: the same call goes on.
        calls.begin(1, Stamp::at(1.55), false, voice(5, 91), |e| ev.push(e));
        let c = calls.end(1, 1.6, |e| ev.push(e)).unwrap();
        assert_eq!(
            (c.end, c.frames, c.start.wall),
            (CallEnd::Terminator, 1, Some(1_762_646_400.0))
        );
        assert!((c.duration_s() - 0.6).abs() < 1e-12);
        assert!(calls.frame(1, 1.7).is_none());
        // A late entry on slot 2, silent after 2.0 s.
        calls.begin(2, Stamp::at(2.0), true, voice(6, 92), |e| ev.push(e));
        assert_eq!(calls.expire(4.0, |e| ev.push(e)), 0);
        assert_eq!(calls.expire(4.1, |e| ev.push(e)), 1);
        let c = calls.get(2).unwrap();
        assert_eq!(
            (c.end, c.late_entry, c.last_s),
            (CallEnd::HangTime, true, 2.0)
        );
        assert_eq!(
            ev,
            [
                CallEvent::Started { id: 1, channel: 1 },
                CallEvent::Ended {
                    id: 1,
                    channel: 1,
                    end: CallEnd::Terminator
                },
                CallEvent::Started { id: 2, channel: 2 },
                CallEvent::Ended {
                    id: 2,
                    channel: 2,
                    end: CallEnd::HangTime
                },
            ]
        );
        assert_eq!(calls.active().count(), 0);
    }

    #[test]
    fn a_new_identity_on_a_slot_replaces_the_call_and_slots_are_separate() {
        let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 2, 300);
        let mut ev = Vec::new();
        calls.begin(1, Stamp::at(0.0), false, voice(5, 91), |e| ev.push(e));
        calls.begin(2, Stamp::at(0.1), false, voice(5, 91), |e| ev.push(e));
        calls.frame(1, 0.4);
        calls.begin(1, Stamp::at(0.5), false, voice(7, 91), |e| ev.push(e));
        assert_eq!(calls.get(1).unwrap().end, CallEnd::Replaced);
        assert_eq!(calls.get(1).unwrap().last_s, 0.4);
        assert_eq!(calls.call_on(1).unwrap().record, voice(7, 91));
        assert_eq!(calls.call_on(2).unwrap().id, 2);
        // A third channel when room was made for two: the one silent longest stops being tracked.
        calls.begin(3, Stamp::at(0.6), false, voice(8, 93), |e| ev.push(e));
        assert_eq!(calls.get(2).unwrap().end, CallEnd::Closed);
        assert_eq!(calls.active().count(), 2);
        // A retune.
        calls.close_all(|e| ev.push(e));
        assert!(calls.iter().all(|c| !c.is_active()));
        assert_eq!(
            ev.iter()
                .filter(|e| matches!(e, CallEvent::Started { .. }))
                .count(),
            4
        );
        assert_eq!(
            ev.iter()
                .filter(|e| matches!(e, CallEvent::Ended { .. }))
                .count(),
            4
        );
    }

    #[test]
    fn singles_and_the_cap() {
        let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 4, 3);
        let mut ev = Vec::new();
        calls.begin(1, Stamp::at(0.0), false, voice(5, 91), |e| ev.push(e));
        for i in 0..3 {
            let c = calls.single(1, Stamp::at(1.0 + i as f64), voice(9, 0), |e| ev.push(e));
            assert_eq!(c.end, CallEnd::Single);
        }
        // The call on air was removed from the full log: it ended as closed and its tracking stopped. The single did not touch it.
        assert!(ev.contains(&CallEvent::Ended {
            id: 1,
            channel: 1,
            end: CallEnd::Closed
        }));
        assert_eq!(calls.len(), 3);
        assert!(calls.call_on(1).is_none());
        assert!(calls.frame(1, 4.0).is_none());
        assert_eq!(
            calls.newest_first().map(|c| c.id).collect::<Vec<_>>(),
            [4, 3, 2]
        );
        calls.set_cap(1);
        assert_eq!(calls.len(), 1);
        calls.clear();
        assert!(calls.is_empty());
        assert_eq!(
            calls
                .begin(1, Stamp::at(9.0), false, voice(1, 1), |_| {})
                .id,
            5
        );
        assert!(!calls.set_hang_s(f64::NAN));
        assert!(calls.set_hang_s(3.0));
        // A time that is not finite takes the channel's last time.
        let c = calls.begin(2, Stamp::at(f64::NAN), false, voice(2, 2), |_| {});
        assert_eq!(c.start.t, 0.0);
        assert_eq!(calls.expire(f64::INFINITY, |_| {}), 0);
    }

    #[test]
    fn a_restored_log_closes_its_active_calls_and_ids_go_on() {
        let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 2, 300);
        calls.begin(1, Stamp::at(10.0), false, voice(5, 91), |_| {});
        calls.end(1, 11.0, |_| {});
        calls.begin(2, Stamp::at(12.0), false, voice(6, 92), |_| {});
        let json = serde_json::to_string(&calls).unwrap();
        let mut back: CallLog<u8, Voice> = serde_json::from_str(&json).unwrap();
        back.shift_times(-12.0);
        assert_eq!(back.active().count(), 0);
        assert_eq!(back.get(2).unwrap().end, CallEnd::Closed);
        assert_eq!(
            (back.get(1).unwrap().start.t, back.get(1).unwrap().last_s),
            (-2.0, -1.0)
        );
        assert_eq!(
            back.begin(1, Stamp::at(0.0), false, voice(1, 1), |_| {}).id,
            3
        );
        let mut fresh: CallLog<u8, Voice> = CallLog::new(5.0, 2, 1);
        fresh.adopt(serde_json::from_str(&json).unwrap());
        assert_eq!((fresh.len(), fresh.hang_s()), (1, 5.0));
        assert_eq!(fresh.newest_first().next().unwrap().end, CallEnd::Closed);
        assert_eq!(
            fresh
                .begin(1, Stamp::at(0.0), false, voice(1, 1), |_| {})
                .id,
            3
        );
        let bad = json.replace("\"last_s\":12.0", "\"last_s\":null");
        assert!(serde_json::from_str::<CallLog<u8, Voice>>(&bad).is_err());
    }

    #[test]
    fn frames_and_expiry_do_not_allocate() {
        let mut calls: CallLog<u8, Voice> = CallLog::new(0.5, 4, 64);
        for ch in 0..4u8 {
            calls.begin(ch, Stamp::at(0.0), false, voice(ch as u32, 1), |_| {});
        }
        let n = count(|| {
            for i in 0..1000u32 {
                let t = i as f64 * 0.06;
                let ch = (i % 4) as u8;
                // Every 50 frames a new talker takes the channel, so the full log also replaces its oldest call.
                calls.begin(ch, Stamp::at(t), false, voice(i / 50, 1), |e| {
                    std::hint::black_box(e);
                });
                calls.frame(ch, t);
                calls.expire(t, |e| {
                    std::hint::black_box(e);
                });
            }
        });
        assert_eq!(n, 0, "allocations in the steady state");
        assert_eq!(calls.len(), 64);
    }
}

mod stats {
    use sdrkit_util_objects::{Stats, Tally};

    #[test]
    fn running_statistics_and_counts() {
        let mut s = Stats::default();
        assert_eq!(s.mean(), None);
        assert!(!s.add(f64::INFINITY));
        s.add(-3.0);
        s.add(5.0);
        assert_eq!((s.min, s.max, s.mean()), (Some(-3.0), Some(5.0), Some(1.0)));
        let mut t = Tally::new(2);
        t.add("a");
        t.add("b");
        for _ in 0..4 {
            t.add("c");
        }
        t.add("a");
        assert_eq!((t.get(&"a"), t.get(&"c"), t.other, t.total()), (2, 0, 4, 7));
        assert_eq!(t.iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["a", "b"]);
    }
}

mod publish {
    use sdrkit_core::command::Command;
    use sdrkit_core::command::{CommandError, CommandSet};
    use sdrkit_core::publish::field::Text;
    use sdrkit_core::publish::{field, Declare, Field, Published, TableDef};
    use sdrkit_core::results::MapPoint;
    use sdrkit_core::settings::Argument;
    use sdrkit_core::{EventDef, Record, Results, Value};
    use sdrkit_util_geo::Position;
    use sdrkit_util_objects::commands;

    const NODE_ROW: Argument<Text> = commands::row_of("nodes");
    const REMOVE: Command = commands::remove_command(&[&NODE_ROW]);
    use sdrkit_util_objects::publish::{self, columns, Cells, Object, Point};
    use sdrkit_util_objects::{LogRing, ObjectTable, Seen, Stamp};

    struct Node {
        name: &'static str,
        at: Option<Position>,
        level_dbfs: f64,
    }

    const NAME: Field<field::Text> = Field::new("name", "Name", "");
    const LEVEL: Field<field::F64> = Field::new("level_dbfs", "Level", "dBFS");
    const COLUMNS: &[&dyn Declare] = &[&NAME, &LEVEL, &columns::HEARD, &columns::LAST_HEARD];
    const NODES: TableDef = TableDef {
        key: "nodes",
        label: "Nodes",
        columns: COLUMNS,
        cap: 150,
        marks: sdrkit_core::RowMarks::NONE,
    };
    static NODE_HEARD: EventDef = EventDef::new("test.node", "Node heard", COLUMNS);

    impl Object for Node {
        fn write(&self, seen: Option<&Seen>, _now_s: f64, out: &mut impl Cells) {
            out.set(&NAME, self.name);
            out.set(&LEVEL, self.level_dbfs);
            out.opt(&columns::HEARD, columns::heard(seen));
            out.opt(&columns::LAST_HEARD, columns::last_heard(seen));
        }
        fn point(&self) -> Option<Point<'_>> {
            self.at.map(|p| Point::new(p, self.name).heading(90.0))
        }
    }

    fn nodes() -> ObjectTable<u32, Node> {
        let mut t = ObjectTable::new(10);
        let rows = [
            (1, "Alpha", Some(Position::new(-33.0, 151.0)), -40.0),
            (2, "Bravo", None, -80.0),
            (3, "Charlie", Some(Position::new(-34.0, 150.0)), -60.0),
        ];
        for (k, name, at, level_dbfs) in rows {
            t.upsert(&k, Stamp::at(k as f64).with_wall(1000.0 + k as f64), || {
                Node {
                    name,
                    at,
                    level_dbfs,
                }
            })
            .unwrap();
        }
        t
    }

    fn published() -> Published {
        Published::new(&[]).with_tables(&[&NODES])
    }

    fn table(p: &Published) -> &sdrkit_core::Table {
        let r = p.results();
        r.table(r.decl().table_id("nodes").unwrap()).unwrap()
    }

    #[test]
    fn rows_are_typed_and_newest_first() {
        let t = nodes();
        let mut p = published();
        publish::rows(&t, &NODES, 10.0, &mut p);
        let ids: Vec<&str> = table(&p).rows_newest_first().map(|(id, _)| id).collect();
        assert_eq!(ids, ["3", "2", "1"]);
        let row = table(&p).row("1").unwrap();
        assert_eq!(row.get(&columns::HEARD), Some(1));
        assert_eq!(row.get(&columns::LAST_HEARD), Some(1001.0));
        publish::rows_where(&t, &NODES, 10.0, &mut p, |r| r.record.level_dbfs >= -70.0);
        let ids: Vec<&str> = table(&p).rows_newest_first().map(|(id, _)| id).collect();
        assert_eq!(ids, ["3", "1"]);
    }

    #[test]
    fn only_changed_rows_move_their_counters() {
        let mut t = nodes();
        let mut p = published();
        publish::rows(&t, &NODES, 10.0, &mut p);
        let before = p.results().change_counter();
        let table_before = table(&p).change_counter();
        publish::rows(&t, &NODES, 11.0, &mut p);
        assert_eq!(p.results().change_counter(), before);
        assert_eq!(table(&p).change_counter(), table_before);
        t.get_mut(&2).unwrap().level_dbfs = -75.0;
        publish::rows(&t, &NODES, 12.0, &mut p);
        let changed: Vec<&str> = table(&p)
            .rows_changed_since(before)
            .map(|(k, _)| k)
            .collect();
        assert_eq!(changed, ["2"]);
        assert!(table(&p).change_counter() > table_before);
        assert!(p.results().change_counter() > before);
    }

    #[test]
    fn rows_of_objects_that_left_are_removed() {
        let mut t = nodes();
        let mut p = published();
        publish::rows(&t, &NODES, 10.0, &mut p);
        t.remove(&2);
        publish::rows(&t, &NODES, 10.0, &mut p);
        assert!(table(&p).row("2").is_none());
        assert_eq!(table(&p).len(), 2);
    }

    #[test]
    fn remove_takes_a_row_key() {
        let mut t = nodes();
        let mut p = published();
        publish::rows(&t, &NODES, 10.0, &mut p);
        let def = REMOVE.def();
        let mut given = def.new_args();
        given
            .set_value(sdrkit_core::FieldId(0), &Value::Text("2".into()))
            .unwrap();
        let args = def.check(sdrkit_core::CommandId(0), &given).unwrap();
        let id = p.results().decl().table_id("nodes").unwrap();
        commands::remove(&mut t, id, &mut p, &args, &NODE_ROW).unwrap();
        assert!(!t.contains(&2));
        assert!(table(&p).row("2").is_none());
        assert_eq!(
            commands::remove(&mut t, id, &mut p, &args, &NODE_ROW),
            Err(CommandError::NoSuchRow {
                table: id,
                key: "2".into()
            })
        );
    }

    #[test]
    fn a_stack_lists_the_object_commands() {
        sdrkit_core::commands! {
            enum NodeCommand {
                Clear = sdrkit_util_objects::commands::CLEAR,
                Remove = REMOVE,
            }
        }
        let c = NodeCommand::commands();
        assert_eq!(c.id("remove").unwrap(), NodeCommand::Remove.id());
        let def = c.def(NodeCommand::Remove.id()).unwrap();
        let row = &def.args.settings()[0].field;
        assert_eq!(row.refers_to.as_ref().unwrap().table, "nodes");
        let missing = def.new_args();
        assert!(matches!(
            c.check(NodeCommand::Remove.id(), &missing),
            Err(CommandError::MissingArgument { .. })
        ));
    }

    #[test]
    fn an_event_has_the_columns_of_its_object() {
        let t = nodes();
        let r = t.row(&3).unwrap();
        let e = publish::event(r.record, Some(r.seen), &NODE_HEARD, 10.0, 156.8e6);
        assert!(e.is(&NODE_HEARD));
        assert_eq!(e.get(&NAME), Some("Charlie"));
        assert_eq!(e.get(&columns::LAST_HEARD), Some(1003.0));
        let mut fields = Record::new(NODE_HEARD.record_decl().unwrap().clone());
        r.record.write(None, 0.0, &mut fields);
        assert_eq!(fields.get(&LEVEL), Some(-60.0));
    }

    #[test]
    fn points_of_one_table_leave_the_others() {
        let mut t = nodes();
        let mut r = Results::default();
        publish::points(&t, "nodes/", &mut r);
        r.set_point(
            "ships/9",
            &MapPoint {
                id: "ships/9".into(),
                lat: 0.0,
                lon: 0.0,
                label: String::new(),
                heading: None,
                row: None,
            },
        );
        assert_eq!(r.points().len(), 3);
        assert_eq!(r.points()["nodes/1"].heading, Some(90.0));
        let before = r.change_counter();
        publish::points(&t, "nodes/", &mut r);
        assert_eq!(r.change_counter(), before);
        t.remove(&1);
        publish::points(&t, "nodes/", &mut r);
        assert!(!r.points().contains_key("nodes/1"));
        assert!(r.points().contains_key("ships/9"));
        assert!(r.points().contains_key("nodes/3"));
    }

    #[test]
    fn a_point_of_a_row_states_its_table_and_its_row_key() {
        let t = nodes();
        let mut p = Published::new(&[]).with_tables(&[&NODES]);
        publish::rows(&t, &NODES, 0.0, &mut p);
        publish::row_points(&t, &NODES, "nodes/", p.results_mut());
        let r = p.results();
        let table = r.decl().table_id(NODES.key).unwrap();
        let row = r.points()["nodes/1"].row.clone().unwrap();
        assert_eq!((row.table, row.key.as_str()), (table, "1"));
        // The row of each point is a row of the table.
        for point in r.points().values() {
            let link = point.row.as_ref().unwrap();
            assert!(r.table(link.table).unwrap().row(&link.key).is_some());
        }
        // Nothing changes when the same points are written again.
        let before = r.change_counter();
        publish::row_points(&t, &NODES, "nodes/", p.results_mut());
        assert_eq!(p.results().change_counter(), before);
        // A point that is written without a table states no row.
        let mut plain = Results::default();
        publish::points(&t, "nodes/", &mut plain);
        assert!(plain.points()["nodes/1"].row.is_none());
    }

    #[test]
    fn log_rows_are_keyed_by_sequence_and_have_no_heard_time() {
        let mut l = LogRing::new(2);
        let node = |name| Node {
            name,
            at: None,
            level_dbfs: 0.0,
        };
        l.push(node("a"));
        l.push(node("b"));
        let mut p = published();
        publish::log_rows(&l, &NODES, 10.0, &mut p);
        assert_eq!(table(&p).rows_newest_first().next().unwrap().0, "000001");
        assert_eq!(
            table(&p).row("000000").unwrap().get(&columns::LAST_HEARD),
            None
        );
        let before = p.results().change_counter();
        l.push(node("c"));
        publish::log_rows(&l, &NODES, 10.0, &mut p);
        let changed: Vec<&str> = table(&p)
            .rows_changed_since(before)
            .map(|(k, _)| k)
            .collect();
        assert_eq!(changed, ["000002"]);
        assert!(table(&p).row("000000").is_none());
    }

    #[test]
    fn rows_of_a_slice_child_rows_and_the_newest_rows_kept() {
        let node = |name, level_dbfs| Node {
            name,
            at: None,
            level_dbfs,
        };
        let list = [node("a", -1.0), node("b", -2.0), node("c", -3.0)];
        let mut p = published();
        publish::rows_of(&list, |n, w| w.write_str(n.name), &NODES, 10.0, &mut p);
        let ids: Vec<&str> = table(&p).rows_newest_first().map(|(id, _)| id).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        publish::rows_of(&list[..1], |n, w| w.write_str(n.name), &NODES, 10.0, &mut p);
        assert_eq!(table(&p).len(), 1);

        let mut p = published();
        let kids = [("x", &list[0]), ("y", &list[1])];
        publish::child_rows(&7, kids.iter().map(|(k, n)| (*k, *n)), &NODES, 10.0, &mut p);
        publish::child_rows(&8, kids.iter().map(|(k, n)| (*k, *n)), &NODES, 10.0, &mut p);
        assert_eq!(table(&p).len(), 4);
        publish::child_rows(
            &7,
            kids[1..].iter().map(|(k, n)| (*k, *n)),
            &NODES,
            10.0,
            &mut p,
        );
        let mut ids: Vec<&str> = table(&p).rows_newest_first().map(|(id, _)| id).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["7/y", "8/x", "8/y"]);

        publish::keep_newest_rows(&NODES, 1, &mut p);
        let ids: Vec<&str> = table(&p).rows_newest_first().map(|(id, _)| id).collect();
        assert_eq!(ids, ["7/y"]);
    }

    #[test]
    fn a_log_returns_each_entry_its_sequence_number() {
        let mut l = LogRing::new(2);
        for x in ["a", "b", "c"] {
            l.push(x);
        }
        let seq: Vec<(u64, &&str)> = l.iter_seq().collect();
        assert_eq!(seq, [(1, &"b"), (2, &"c")]);
    }
}

mod steady_state {
    use super::alloc_count::count;
    use sdrkit_util_geo::Position;
    use sdrkit_util_objects::{
        fingerprint, Activity, Dedup, JoinRules, Joinable, Joined, Joiner, LogRing, Numbering,
        ObjectTable, Part, Spacing, Stamp, Stats, Trail,
    };

    #[derive(Clone, Copy, Default)]
    struct Fixed {
        len: usize,
    }

    impl Joinable for Fixed {
        fn joined_len(&self) -> usize {
            self.len
        }
        fn append(&mut self, next: Self) {
            self.len += next.len;
        }
    }

    #[derive(Clone, Copy, Default)]
    struct Rec {
        snr: Stats,
    }

    #[test]
    fn the_streaming_paths_do_not_allocate() {
        let mut table: ObjectTable<u32, Rec> = ObjectTable::new(256);
        let mut dedup = Dedup::new(60.0, 512);
        let mut trail: Trail = Trail::new(80, Spacing::seconds(1.0));
        let mut log = LogRing::new(64);
        let mut joiner: Joiner<u32, Fixed> = Joiner::new(JoinRules::default());
        let mut joined: Vec<Joined<u32, Fixed>> = Vec::with_capacity(16);
        let mut activity = Activity::new(2.0, 8);
        // Warm up: fill the table, so the timed part both updates rows and replaces the oldest.
        for k in 0..256u32 {
            table.upsert(&k, Stamp::at(0.0), Rec::default).unwrap();
        }
        let n = count(|| {
            for i in 0..1000u32 {
                let t = i as f64 * 0.1;
                let k = (i * 7) % 300; // some keys are new, and replace the least recently heard
                let up = table.upsert(&k, Stamp::at(t), Rec::default).unwrap();
                up.row.snr.add(t);
                let key = fingerprint(&i.to_le_bytes());
                dedup.seen(key % 700, t);
                trail
                    .push(t, Position::new(10.0, (i % 300) as f64 * 0.001), ())
                    .unwrap();
                log.push(i);
                joined.clear();
                joiner.push(
                    i % 8,
                    Part {
                        t,
                        message: 0,
                        number: Numbering::Seq(i / 8 % 4),
                        last: i / 8 % 4 == 3,
                        body: Fixed { len: 4 },
                    },
                    &mut joined,
                );
                activity.touch(i % 6, t);
                activity.expire(t, |_| {});
                if i % 100 == 0 {
                    table.expire(t, 5.0);
                }
            }
        });
        assert_eq!(n, 0, "allocations in the steady state");
    }
}
