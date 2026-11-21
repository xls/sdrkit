//! Helpers the tests share: runs through the harness, and typed reads of fields and tables.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CN0_DBHZ, GEN_NOISE};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::TableDef;
use sdrkit_core::record::Kind;
use sdrkit_core::{Record, Results, StackDescriptor};
use sdrkit_dev_test::{Harness, Run};

/// A harness for `d` with white noise at `cn0_dbhz` (C/N0 against the signal's declared power).
pub fn noisy(d: &'static StackDescriptor, cn0_dbhz: f64) -> Harness {
    Harness::new(d)
        .set(&GEN_NOISE, NoiseKind::Cn0)
        .set(&GEN_CN0_DBHZ, cn0_dbhz)
}

/// Runs `h`, which must not fail.
pub fn run(h: &Harness) -> Run {
    h.run().unwrap_or_else(|e| panic!("{h:?}: {e}"))
}

/// The value of the field of column `c`, if it has one.
pub fn get<'a, K: Kind>(r: &'a Results, c: &Column<K>) -> Option<K::Out<'a>> {
    r.fields().at(c)
}

/// The rows of table `t`, newest first.
pub fn rows<'a>(r: &'a Results, t: &TableDef) -> Vec<&'a Record> {
    r.decl()
        .table_id(t.key)
        .and_then(|id| r.table(id))
        .map(|t| t.rows_newest_first().map(|(_, row)| row).collect())
        .unwrap_or_default()
}
