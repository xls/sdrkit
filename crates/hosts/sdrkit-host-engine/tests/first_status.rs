//! A session shows a status from its first copy on: until its stack reports, the stack is searching and its summary says
//! that it starts.

mod common;

use common::*;
use sdrkit_core::{LockState, StackDescriptor, Values};
use sdrkit_host_engine::{Input, OwnerRef};
use std::time::{Duration, Instant};

#[test]
fn no_status_of_a_stack_has_an_empty_summary_from_the_start_on() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let stacks: [&StackDescriptor; 3] = [
        &sdrkit_stack_fm::DESCRIPTOR,
        &sdrkit_stack_hfkbd::RTTY,
        &sdrkit_stack_general::DESCRIPTOR,
    ];
    for stack in stacks {
        let synth = e.find("driver.synth").unwrap();
        let radio = e
            .open_radio("synth", &format!("synth:{}", stack.id), &defaults(&e, synth))
            .unwrap();
        let values = Values::defaults(&(stack.settings)());
        let id = e
            .start_stack(stack.id, None, &values, Input::Radio(radio), None)
            .unwrap();
        let s = OwnerRef::Session(id);
        let status = || e.owner(s, |o| o.results().status().clone()).unwrap();
        let first = status();
        assert_eq!(first.lock_state, LockState::Searching, "{}", stack.id);
        assert_eq!(first.summary, format!("Starting {}…", stack.name));
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(3) {
            let now = status();
            assert!(!now.summary.is_empty(), "{}: {now:?}", stack.id);
            std::thread::sleep(Duration::from_millis(1));
        }
        e.stop_session(id).unwrap();
        e.close_radio(radio).unwrap();
    }
}
