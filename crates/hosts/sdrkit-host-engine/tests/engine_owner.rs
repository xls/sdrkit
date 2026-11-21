//! The engine owner: a stack on its test signal with default settings, and the typed description of the tables.

mod common;

use common::*;
use sdrkit_core::MemStorage;
use sdrkit_host_engine::{Engine, OwnerRef};
use std::sync::Arc;

/// An engine with the FM stack on `storage`.
fn fm_engine(storage: Arc<MemStorage>) -> Engine {
    Engine::new(sdrkit_host_engine::Setup {
        facilities: sdrkit_core::Facilities {
            storage,
            clock: sdrkit_core::Clock::fixed(1.7e9),
            random: sdrkit_core::Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&sdrkit_stack_fm::DESCRIPTOR],
        families: Vec::new(),
        components: sdrkit_stack_fm::graph::registry(),
    })
    .unwrap()
}

#[test]
fn an_engine_runs_fm_on_its_test_signal_with_default_settings() {
    let e = fm_engine(Arc::new(MemStorage::new()));
    let synth = e.find("driver.synth").unwrap();
    let r = e
        .open_radio("synth", "synth:fm", &defaults(&e, synth))
        .unwrap();
    let fm = e.find("stack.fm").unwrap();
    let s = e
        .start_stack(
            "fm",
            None,
            &defaults(&e, fm),
            sdrkit_host_engine::Input::Radio(r),
            None,
        )
        .unwrap();
    let locked = wait_for(30.0, || {
        e.owner(OwnerRef::Session(s), |o| o.results().status().lock_state)
            .ok()
            .filter(|l| *l == sdrkit_core::LockState::Locked)
    });
    assert!(locked.is_some(), "FM locks on default settings");
}

/// The typed description of the engine lists the tables of the engine owner, in its order and with its columns.
#[test]
fn the_engine_description_lists_the_tables_of_the_engine_owner() {
    let e = fm_engine(Arc::new(MemStorage::new()));
    let held = e
        .owner(OwnerRef::Engine, |o| o.results().decl().tables().to_vec())
        .unwrap();
    let families = sdrkit_host_engine::object_families();
    let described = families
        .iter()
        .find(|f| f.key == "engine")
        .expect("the description of the engine")
        .stacks[0]
        .results_decl();
    let keys = |t: &[sdrkit_core::TableDecl]| -> Vec<String> {
        t.iter().map(|t| t.key.to_string()).collect()
    };
    assert_eq!(keys(described.tables()), keys(&held));
    assert_eq!(described.tables(), held.as_slice());
}
