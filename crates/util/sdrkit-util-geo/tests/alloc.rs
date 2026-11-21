//! Steady-state calls allocate nothing: every hot function runs 1 000 times under a counting allocator that must see zero
//! allocations on this thread.

use sdrkit_util_geo::coord::{
    format_angle, format_ddmm, parse_angle, parse_ddmm, parse_position, Axis, Style,
};
use sdrkit_util_geo::cpr::{decode_global, decode_local, encode, CprFormat};
use sdrkit_util_geo::nmea::{self, aivdm, Gga};
use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
use sdrkit_util_geo::orbit::{Satellite, Sgp4};
use sdrkit_util_geo::time::{self, gnss, Civil, LeapSeconds};
use sdrkit_util_geo::{maidenhead, Ellipsoid, Observer, Position};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ON.with(Cell::get) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ON.with(Cell::get) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations(name: &str, mut f: impl FnMut()) {
    f(); // warm-up outside the count
    COUNT.with(|c| c.set(0));
    ON.with(|o| o.set(true));
    for _ in 0..1_000 {
        f();
    }
    ON.with(|o| o.set(false));
    let n = COUNT.with(Cell::get);
    assert_eq!(n, 0, "{name} allocated {n} times in 1000 calls");
}

const ISS_1: &str = "1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927";
const ISS_2: &str = "2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";
const MOLNIYA_1: &str = "1 08195U 75081A   06176.33215444  .00000099  00000-0  11873-3 0   813";
const MOLNIYA_2: &str = "2 08195  64.1586 279.0717 6877146 264.7651  20.2257  2.00491383225656";

#[test]
fn hot_paths_do_not_allocate() {
    let a = Position::new(52.52, 13.405);
    let b = Position::new(48.8566, 2.3522);
    allocations("spherical", || {
        black_box(
            a.distance_m(&b)
                + a.initial_bearing_deg(&b)
                + a.destination(45.0, 1e5).lat_deg
                + a.cross_track_m(&b, &a),
        );
    });
    allocations("vincenty", || {
        black_box(Ellipsoid::WGS84.inverse(&a, &b).unwrap());
        black_box(Ellipsoid::WGS84.direct(&a, 45.0, 1e5).unwrap());
    });
    let obs = Observer::new(a);
    allocations("ecef and look", || {
        let e = black_box(b).to_ecef();
        black_box(e.to_position().unwrap());
        black_box(obs.look(&e));
    });
    allocations("maidenhead", || {
        let l = maidenhead::encode(black_box(&a), 10).unwrap();
        black_box(maidenhead::decode(&l).unwrap());
    });
    allocations("coordinates", || {
        black_box(format_angle(a.lat_deg, Axis::Latitude, Style::Dms, 2).unwrap());
        black_box(format_ddmm(a.lon_deg, Axis::Longitude, 4).unwrap());
        black_box(parse_angle("52°31'12.0\"N", Axis::Latitude).unwrap());
        black_box(parse_ddmm("4807.038", "N", Axis::Latitude).unwrap());
        black_box(parse_position("54.687N 25.280E").unwrap());
    });
    let f = CprFormat::AIRBORNE;
    allocations("cpr", || {
        let e = encode(&f, black_box(&a), false).unwrap();
        let o = encode(&f, &a, true).unwrap();
        black_box(decode_global(&f, &e, &o, true, None).unwrap());
        black_box(decode_local(&f, &o, &b).unwrap());
    });
    allocations("tle", || {
        black_box(
            Tle::from_lines(
                Some("ISS (ZARYA)"),
                black_box(ISS_1),
                ISS_2,
                Checksum::Require,
            )
            .unwrap(),
        );
    });
    let iss =
        Sgp4::from_tle(&Tle::from_lines(None, ISS_1, ISS_2, Checksum::Require).unwrap()).unwrap();
    let molniya =
        Sgp4::from_tle(&Tle::from_lines(None, MOLNIYA_1, MOLNIYA_2, Checksum::Require).unwrap())
            .unwrap();
    let mut t = 0.0;
    allocations("sgp4", || {
        t += 1.0;
        black_box(iss.propagate(t).unwrap());
        black_box(molniya.propagate(t).unwrap());
    });
    let sat = Satellite::new(iss.clone());
    let t0 = iss.epoch_unix();
    allocations("satellite look and doppler", || {
        black_box(sat.look(&obs, t0 + 100.0).unwrap());
        black_box(sat.doppler_hz(&obs, t0 + 100.0, 145.8e6).unwrap());
    });
    let leap = LeapSeconds::BUILTIN;
    allocations("time", || {
        let c = Civil::from_unix_f64(black_box(1.794e9)).unwrap();
        black_box(c.to_unix().unwrap());
        black_box(time::iso8601(1.794e9, 3).unwrap());
        black_box(time::parse_iso8601("2026-11-08T10:42:07.25+11:00").unwrap());
        black_box(gnss::gps_from_utc(1.794e9, &leap).unwrap());
        black_box(gnss::glonass_from_utc(1.794e9).unwrap());
        black_box(time::mjd_from_unix(1.794e9));
    });
    allocations("nmea", || {
        let s = nmea::parse(black_box(
            "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47",
        ))
        .unwrap();
        let g = Gga::from_sentence(&s).unwrap();
        black_box(g.write("GP", 1.794e9).unwrap());
    });
    let mut bits = Vec::with_capacity(2_048);
    let mut text = String::with_capacity(512);
    let mut asm = aivdm::Assembler::new();
    allocations("aivdm", || {
        let s = nmea::parse(black_box("!AIVDM,1,1,,B,177KQJ5000G?tO`K>RA1wUbN0TKH,0*5C")).unwrap();
        let v = aivdm::Vdm::from_sentence(&s).unwrap();
        black_box(asm.push(&v).unwrap());
        bits.clear();
        aivdm::dearmour(v.payload, v.fill, &mut bits).unwrap();
        text.clear();
        let fill = aivdm::armour(&bits, &mut text).unwrap();
        for f in aivdm::fragments("AIVDM", Some('B'), 0, &text, fill).unwrap() {
            black_box(f.unwrap());
        }
        let p1 = nmea::parse(
            "!AIVDM,2,1,1,A,55?MbV02;H;s<HtKR20EHE:0@T4@Dn2222222216L961O5Gf0NSQEp6ClRp8,0*1C",
        )
        .unwrap();
        let p2 = nmea::parse("!AIVDM,2,2,1,A,88888888880,2*25").unwrap();
        asm.push(&aivdm::Vdm::from_sentence(&p1).unwrap()).unwrap();
        black_box(asm.push(&aivdm::Vdm::from_sentence(&p2).unwrap()).unwrap());
    });
}
