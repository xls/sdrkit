//! meshtastic: receives a Meshtastic mesh, with the settings that the data folder holds for a country if one is given,
//! prints each new text message, and lists the nodes it heard.
//!
//! `cargo run -p sdrkit-examples --example meshtastic -- [source] [seconds] [country] [channel]`
//!
//! - `source`: `synth:meshtastic` (the default) is a test signal. A radio is an address such as
//!   `rtl_tcp:192.168.1.20:1234`.
//! - `seconds`: how long to listen, 20 by default.
//! - `country`: an ISO 3166 code such as `AU`. The presets helper then sets the frequency and the slots of its region.
//!   Without it, or with `-`, the stack starts with its defaults on the frequency of the input, which the test signal
//!   needs.
//! - `channel`: the name of a private channel to add. Its key is read from the environment variable
//!   `SDRKIT_MESHTASTIC_KEY`, in base64 or hex. Without a channel the stack reads the primary channel with the
//!   published default key of the protocol.
//!
//! The data folder is the environment variable `SDRKIT_DATA`, else the folder `data` of the repository.

use sdrkit_host_engine::OwnerRef;
use sdrkit_presets::{Data, Options, Region};
use sdrkit_stack_lora::typed::{MeshMessageRow, MeshNodeRow, MESH_MESSAGES, MESH_NODES};
use std::collections::HashSet;
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = |n: usize| std::env::args().nth(n);
    let source = arg(1).unwrap_or_else(|| "synth:meshtastic".into());
    let seconds: f64 = arg(2).map_or(Ok(20.0), |s| s.parse())?;
    let country = arg(3).filter(|c| c != "-");
    let channel = arg(4);
    let folder = std::env::var("SDRKIT_DATA")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../data").into());

    let sdr = sdrkit_host_catalog::engine()?;
    let radio = sdr.open(&source)?;
    let mesh = match country {
        // The preset of the stack in the country: plain settings and a frequency, from the files of the folder.
        Some(code) => {
            let data = Data::open(folder)?;
            let region = Region::new(&code)?;
            sdrkit_presets::start(
                &data,
                &sdr,
                radio,
                "meshtastic",
                &region,
                &Options::default(),
            )?
        }
        None => sdr.start("meshtastic", radio, None)?,
    };

    if let Some(name) = channel {
        let key = std::env::var("SDRKIT_MESHTASTIC_KEY")
            .map_err(|_| format!("the channel {name} needs its key in SDRKIT_MESHTASTIC_KEY"))?;
        sdr.call(
            mesh,
            "mt.channel.add",
            &[("name", name.into()), ("key", key.into())],
        )?;
    }

    let (mut seen, mut said) = (HashSet::new(), None);
    for _ in 0..(seconds * 5.0) as usize {
        // The rows of the message table as typed structs, newest first. Each key is printed once.
        let new: Vec<String> = sdr
            .owner(OwnerRef::Stack(mesh), |o| {
                MESH_MESSAGES
                    .rows::<MeshMessageRow>(o.results())
                    .filter(|(key, _)| !seen.contains(*key))
                    .map(|(key, m)| {
                        let from = m.sender.or(m.source).unwrap_or("?");
                        (key.to_owned(), format!("{from}: {}", m.text))
                    })
                    .collect::<Vec<_>>()
            })?
            .into_iter()
            .rev()
            .map(|(key, line)| {
                seen.insert(key);
                line
            })
            .collect();
        for line in new {
            println!("{line}");
        }
        // What is wrong with the session, printed when it changes.
        let why = sdr.problem(mesh)?;
        if why != said {
            if let Some(w) = &why {
                eprintln!("{w}");
            }
            said = why;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let nodes: Vec<String> = sdr.owner(OwnerRef::Stack(mesh), |o| {
        MESH_NODES
            .rows::<MeshNodeRow>(o.results())
            .map(|(_, n)| format!("node {} {}", n.id, n.name))
            .collect()
    })?;
    for line in &nodes {
        println!("{line}");
    }
    sdr.stop_session(mesh)?;
    println!("{} nodes heard", nodes.len());
    Ok(())
}
