//! The copy that the application keeps of the results of one object, brought up to date by change counter.

use crate::api::{Handle, Id, Result, ResultsDecl, State, Value};

/// One row of a table: its key and its cells by column id.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Row {
    /// The row key.
    pub key: String,
    /// The cells, by column id.
    pub cells: Vec<Value>,
}

/// The rows of one table, newest first.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Rows {
    /// The rows.
    pub rows: Vec<Row>,
}

impl Rows {
    /// Writes the cells of the row with the key `key`, or adds the row at the front. Returns the index of the row.
    pub fn upsert(&mut self, key: &str, columns: usize) -> usize {
        if let Some(i) = self.rows.iter().position(|r| r.key == key) {
            return i;
        }
        self.rows.insert(
            0,
            Row {
                key: key.to_owned(),
                cells: vec![Value::None; columns],
            },
        );
        0
    }
}

/// The results of one object as the application last read them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Results {
    /// The declaration: fields and tables.
    pub decl: ResultsDecl,
    /// The change counter of the last snapshot. 0 before the first.
    pub counter: u64,
    /// The state.
    pub state: State,
    /// The signal to noise ratio in dB, or NaN.
    pub snr_db: f32,
    /// The level in dB full scale, or NaN.
    pub level_dbfs: f32,
    /// The offset from the tuned frequency in Hz, or NaN.
    pub offset_hz: f64,
    /// One line for people.
    pub summary: String,
    /// The log, oldest line first.
    pub log: Vec<String>,
    /// The result fields, by id.
    pub fields: Vec<Value>,
    /// The tables, by id.
    pub tables: Vec<Rows>,
    /// A buffer for row keys.
    key: String,
}

impl Results {
    /// Returns the id of the table with the key `key`.
    pub fn table_id(&self, key: &str) -> Option<usize> {
        self.decl.tables.iter().position(|t| t.key == key)
    }

    /// Returns the id of column `key` of table `table`.
    pub fn column_id(&self, table: usize, key: &str) -> Option<usize> {
        self.decl
            .tables
            .get(table)?
            .columns
            .iter()
            .position(|c| c.key == key)
    }

    /// Reads what changed after the last snapshot from the object of `h`.
    ///
    /// # Errors
    ///
    /// The code of the library; `SDRKIT_E_NO_SUCH_OBJECT` once the object is gone.
    pub fn poll(&mut self, h: &Handle) -> Result<()> {
        if self.read(h)? {
            // A table lost rows: read everything again.
            self.counter = 0;
            self.read(h)?;
        }
        Ok(())
    }

    /// Reads one snapshot after `self.counter`. Returns true if a table holds more rows than the object has.
    fn read(&mut self, h: &Handle) -> Result<bool> {
        let since = self.counter;
        let info = h.snapshot_read(since, &mut self.summary)?;
        if since == 0 || info.declaration_changed {
            self.decl = h.results_decl()?;
            self.fields = vec![Value::None; self.decl.fields.len()];
            self.tables = vec![Rows::default(); self.decl.tables.len()];
        }
        self.state = info.state;
        self.snr_db = info.snr_db;
        self.level_dbfs = info.level_dbfs;
        self.offset_hz = info.offset_hz;
        self.log.resize_with(info.log_lines as usize, String::new);
        for (n, line) in (0..info.log_lines).zip(self.log.iter_mut()) {
            h.snapshot_log(n, line)?;
        }
        if info.fields_changed || since == 0 || info.declaration_changed {
            for (id, v) in (0..).zip(self.fields.iter_mut()) {
                h.snapshot_field(id, v)?;
            }
        }
        let mut lost = false;
        for (t, (decl, rows)) in self
            .decl
            .tables
            .iter()
            .zip(self.tables.iter_mut())
            .enumerate()
        {
            let t = t as Id;
            let r = h.snapshot_table(t)?;
            for n in (0..r.rows).rev() {
                h.snapshot_row(t, n, &mut self.key)?;
                let i = rows.upsert(&self.key, decl.columns.len());
                for (c, cell) in (0..).zip(rows.rows[i].cells.iter_mut()) {
                    h.snapshot_cell(t, n, c, cell)?;
                }
            }
            lost |= rows.rows.len() > r.total as usize;
        }
        self.counter = info.counter;
        Ok(lost)
    }
}
