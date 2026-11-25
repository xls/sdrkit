//! Replays SigMF recordings (`ci16_le`) through the `dji_droneid` stack and prints the counts of each recording:
//! `SDRKIT_DRONEID_RECORDINGS=<file or folder> cargo run --release -p sdrkit-stack-drones --example droneid_replay`.
//! A folder is searched for `.sigmf-data` files. Only counts are printed: no position, serial number or session key.

use sdrkit_core::{Complex32, InputInfo, Output, Owner, StackConfig, Values};
use sdrkit_stack_drones::typed::{DjiDroneidAircraftRow, DjiDroneidFields, DJI_AIRCRAFT};
use sdrkit_stack_drones::DJI_DRONEID;
use std::io::Read;
use std::path::{Path, PathBuf};

fn data_files(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for e in entries {
            data_files(&e, out);
        }
    } else if p.to_string_lossy().ends_with(".sigmf-data") {
        out.push(p.to_owned());
    }
}

/// The rate and the centre of the recording, from its `.sigmf-meta` file.
fn meta(data: &Path) -> Option<(f64, f64)> {
    let text = std::fs::read_to_string(data.with_extension("sigmf-meta")).ok()?;
    let j: serde_json::Value = serde_json::from_str(&text).ok()?;
    let rate = j["global"]["core:sample_rate"].as_f64()?;
    let centre = j["captures"][0]["core:frequency"].as_f64().unwrap_or(0.0);
    Some((rate, centre))
}

fn replay(data: &Path) -> Option<()> {
    let (rate, centre) = meta(data)?;
    let settings = Values::defaults(&(DJI_DRONEID.settings)());
    let mut rx = (DJI_DRONEID.make_receiver)(&StackConfig::new(settings)).ok()?;
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .ok()?;
    let mut file = std::fs::File::open(data).ok()?;
    let mut raw = vec![0u8; 4 << 16];
    let mut x = Vec::with_capacity(1 << 16);
    let mut out = Output::default();
    let mut samples = 0u64;
    let started = std::time::Instant::now();
    loop {
        let mut got = 0;
        while got < raw.len() {
            match file.read(&mut raw[got..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => got += n,
            }
        }
        if got < 4 {
            break;
        }
        x.clear();
        x.extend(raw[..got].as_chunks::<4>().0.iter().map(|b| {
            Complex32::new(
                f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
                f32::from(i16::from_le_bytes([b[2], b[3]])) / 32768.0,
            )
        }));
        samples += x.len() as u64;
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx.feed(&[], &mut out);
    let spent = started.elapsed().as_secs_f64();
    let r = Owner::results(&*rx);
    let n = |f| r.fields().at(f).unwrap_or(0);
    let formats: Vec<String> = r
        .decl()
        .table_id(DJI_AIRCRAFT.def().key)
        .and_then(|id| r.table(id))
        .map(|t| {
            t.rows_newest_first()
                .filter_map(|(_, row)| row.at(&DjiDroneidAircraftRow::FORMAT))
                .map(|f| format!("{f:?}"))
                .collect()
        })
        .unwrap_or_default();
    let seconds = samples as f64 / rate;
    println!(
        "{}\n  {:.1} s at {:.3} MHz, {:.1}x real time; searched {}, pairs {}, bursts {}, attempts {}; frames: encrypted {}, \
         empty {}, readable {}, other {}; CRC failed {}, empty refused {}; aircraft {} {:?}",
        data.file_name()?.to_string_lossy(),
        seconds,
        centre / 1e6,
        seconds / spent,
        n(&DjiDroneidFields::DRONEID_SEARCHED),
        n(&DjiDroneidFields::DRONEID_PAIRS),
        n(&DjiDroneidFields::DRONEID_BURSTS),
        n(&DjiDroneidFields::DRONEID_TRIED),
        n(&DjiDroneidFields::DRONEID_ENCRYPTED),
        n(&DjiDroneidFields::DRONEID_EMPTY),
        n(&DjiDroneidFields::DRONEID_DECODED),
        n(&DjiDroneidFields::DRONEID_OTHER),
        n(&DjiDroneidFields::DRONEID_CRC_FAILED),
        n(&DjiDroneidFields::DRONEID_REFUSED),
        n(&DjiDroneidFields::AIRCRAFT),
        formats,
    );
    Some(())
}

fn main() {
    let Ok(root) = std::env::var("SDRKIT_DRONEID_RECORDINGS") else {
        eprintln!(
            "set SDRKIT_DRONEID_RECORDINGS to a .sigmf-data file or to a folder of recordings"
        );
        return;
    };
    let mut files = Vec::new();
    data_files(Path::new(&root), &mut files);
    for f in &files {
        if replay(f).is_none() {
            println!("{}: not read", f.display());
        }
    }
}
