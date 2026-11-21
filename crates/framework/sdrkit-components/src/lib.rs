//! The standard sdrkit components. Each one returns a type of the algorithm crates ports, formats, tags and settings, so a stack
//! can chain them in a graph. [`register`] adds every component under `<family>.<name>`.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod dsp;
pub mod sim;
pub mod testing;
pub mod tx;
pub mod voice;

pub mod bits;
pub mod fec;
pub mod modem;

use sdrkit_core::Benchmark;
use sdrkit_graph::Registry;

/// Adds every standard component to `r`, under `<family>.<name>`, and the standard adapters.
pub fn register(r: &mut Registry) {
    dsp::register(r);
    modem::register(r);
    fec::register(r);
    bits::register(r);
    tx::register(r);
}

/// The error of an algorithm crate that has no conversion of its own, as a configuration error.
pub(crate) fn stage(e: impl std::error::Error + Send + Sync + 'static) -> sdrkit_core::ConfigError {
    sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
}

/// A registry holding every standard component.
pub fn registry() -> Registry {
    let mut r = Registry::new();
    register(&mut r);
    r
}

/// The benchmarks of the hot paths of the algorithm crates, the graph, the error correction and the bit-level components.
pub fn benchmarks() -> Vec<Benchmark> {
    let mut v = sdrkit_algo_dsp::benchmarks();
    v.extend(sdrkit_algo_modem::benchmarks());
    v.extend(sdrkit_algo_sim::benchmarks());
    v.extend(sdrkit_graph::benchmarks());
    v.extend(fec::benchmarks());
    v.extend(bits::benchmarks());
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_name_is_family_dot_snake_case() {
        let r = super::registry();
        for n in r.names() {
            let (family, name) = n
                .split_once('.')
                .unwrap_or_else(|| panic!("{n} has no family"));
            for part in [family, name] {
                assert!(
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                    "{n} is not <family>.<snake_case>"
                );
            }
            let info = r.make(n).unwrap().info();
            assert_eq!(info.name, n, "{n} calls itself {}", info.name);
        }
    }

    #[test]
    fn every_component_starts_at_the_defaults_of_its_settings() {
        let r = super::registry();
        let mut off = Vec::new();
        for n in r.names() {
            let c = r.make(n).unwrap();
            let schema = c.info().settings;
            let v = c.values();
            for (i, d) in schema.settings().iter().enumerate() {
                let id = sdrkit_core::SettingId(i as u16);
                if v.value(id) != Some(&d.default) {
                    off.push(format!("{n}.{}", d.field.key));
                }
            }
        }
        assert!(off.is_empty(), "{off:?}");
    }

    #[test]
    fn benchmark_names_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for b in super::benchmarks() {
            assert!(seen.insert(b.name), "{} twice", b.name);
        }
    }
}
