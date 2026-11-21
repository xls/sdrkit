//! Times the hot paths: `cargo run --release -p sdrkit-util-objects --example bench`. Each line is the best of several runs, in
//! millions of operations per second.

use sdrkit_core::bench::measure;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::{
    fingerprint, CallLog, Dedup, JoinRules, Joined, Joiner, Numbering, ObjectTable, Part, SameCall,
    Spacing, Stamp, Trail,
};
use std::hint::black_box;

const N: usize = 100_000;

/// A call's identity: the source.
struct Voice(u32);

impl SameCall for Voice {
    fn same_call(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

fn show(name: &str, ops: usize, step: impl FnMut()) {
    let r = measure(0.5, ops, 0.0, step);
    println!("{name:48} {:10.1} M/s", r.items_per_second / 1e6);
}

fn main() {
    let mut table: ObjectTable<u32, u64> = ObjectTable::new(1000);
    for k in 0..1000u32 {
        table.upsert(&k, Stamp::at(0.0), || 0).unwrap();
    }
    let mut t = 0.0;
    show("table upsert, row exists (1000 rows)", N, || {
        for i in 0..N as u32 {
            t += 1e-3;
            *table.upsert(&(i % 1000), Stamp::at(t), || 0).unwrap().row += 1;
        }
    });
    let mut next = 1000u32;
    show("table upsert, new row replaces the oldest", N, || {
        for _ in 0..N {
            t += 1e-3;
            next = next.wrapping_add(1);
            black_box(table.upsert(&next, Stamp::at(t), || 0).unwrap().evicted);
        }
    });
    let mut big: ObjectTable<String, u64> = ObjectTable::new(1000);
    let keys: Vec<String> = (0..1000).map(|i| format!("{i:06X}")).collect();
    for k in &keys {
        big.upsert(k.as_str(), Stamp::at(0.0), || 0).unwrap();
    }
    show("table upsert, text key, row exists", N, || {
        for i in 0..N {
            t += 1e-3;
            *big.upsert(keys[i % 1000].as_str(), Stamp::at(t), || 0)
                .unwrap()
                .row += 1;
        }
    });
    show("table expire scan (1000 rows)", 1000, || {
        black_box(table.expire(t, 1e9));
    });

    let mut dedup = Dedup::new(600.0, 4000);
    let frames: Vec<[u8; 32]> = (0..N as u32)
        .map(|i| {
            let mut b = [0u8; 32];
            b[..4].copy_from_slice(&(i % 6000).to_le_bytes());
            b
        })
        .collect();
    show("fingerprint (32 bytes) + dedup check", N, || {
        for f in &frames {
            t += 1e-3;
            black_box(dedup.seen(fingerprint(f), t));
        }
    });

    let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 8, 300);
    show("call log frame + expire (8 slots)", N, || {
        for i in 0..N as u32 {
            t += 1e-3;
            let ch = (i % 8) as u8;
            if i % 400 < 8 {
                calls.begin(ch, Stamp::at(t), false, Voice(i / 400), |e| {
                    black_box(e);
                });
            }
            black_box(calls.frame(ch, t));
            calls.expire(t, |e| {
                black_box(e);
            });
        }
    });

    let mut trail: Trail = Trail::new(80, Spacing::metres(20.0));
    show("trail push with distance spacing", N, || {
        for i in 0..N {
            t += 1.0;
            let _ = trail.push(
                t,
                Position::new(-33.0, 151.0 + (i % 1000) as f64 * 1e-3),
                (),
            );
        }
    });

    let mut joiner: Joiner<u32, Vec<u8>> = Joiner::new(JoinRules::default());
    let mut out: Vec<Joined<u32, Vec<u8>>> = Vec::with_capacity(64);
    show("joiner part (4 parts of 32 bytes per message)", N, || {
        for i in 0..N as u32 {
            t += 1e-3;
            if out.len() > 32 {
                out.clear();
            }
            let n = i / 16 % 4;
            joiner.push(
                i % 16,
                Part {
                    t,
                    message: 0,
                    number: Numbering::Seq(n),
                    last: n == 3,
                    body: vec![0u8; 32],
                },
                &mut out,
            );
        }
    });
}
