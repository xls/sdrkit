//! The picture settings of the stacks mean the same view and instrument as the `pictures` component they go to.

use sdrkit_core::{Owner, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Graph;

/// The choice keys of setting `key` of the stack schema of `d` and of the `pictures` component of its graph.
fn both(d: &'static StackDescriptor, key: &str) -> (Vec<String>, Vec<String>) {
    let keys = |s: &sdrkit_core::Schema| -> Vec<String> {
        let id = s.id(key).expect("the setting");
        s.setting(id)
            .expect("its declaration")
            .field
            .choices
            .iter()
            .map(|c| c.key.to_string())
            .collect()
    };
    let stack = keys(&(d.settings)());
    let mut rx = Harness::new(d).receiver().unwrap();
    let graph = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    let full = format!("pictures.{key}");
    let schema = Owner::schema(graph);
    let id = schema.id(&full).expect("the component setting");
    let comp = schema
        .setting(id)
        .expect("its declaration")
        .field
        .choices
        .iter()
        .map(|c| c.key.to_string())
        .collect();
    (stack, comp)
}

#[test]
fn view_and_instrument_choices_match_the_pictures_component() {
    use sdrkit_stack_satimg::modes::{fy3, hrpt, meteor};
    for (d, key) in [
        (&meteor::DESCRIPTOR, "view"),
        (&hrpt::DESCRIPTOR, "view"),
        (&fy3::AHRPT, "view"),
        (&fy3::AHRPT, "instrument"),
        (&fy3::MPT, "view"),
        (&fy3::MPT, "instrument"),
    ] {
        let (stack, comp) = both(d, key);
        assert!(!stack.is_empty(), "{} {key}", d.id);
        assert_eq!(stack, comp, "{} {key}", d.id);
    }
}

/// Every stack and every submode makes its receiver: each graph declares what its description holds.
#[test]
fn every_submode_makes_its_receiver() {
    for d in sdrkit_stack_satimg::descriptors() {
        let ids = std::iter::once(None).chain(d.submodes.iter().map(|m| Some(m.id)));
        for id in ids {
            let h = match id {
                Some(id) => Harness::new(d).submode(id),
                None => Harness::new(d),
            };
            if let Err(e) = h.receiver() {
                panic!("{} {id:?}: {e}", d.id);
            }
        }
    }
}
