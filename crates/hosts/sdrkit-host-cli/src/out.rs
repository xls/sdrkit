//! Where a command writes: data to the output, the live status line and notes to the error stream, and one JSON value
//! per line with `--json`.

use crate::error::CliError;
use serde::Serialize;
use std::io::Write;

/// The two streams of a command and its output form.
pub struct Out<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    /// One JSON value per line instead of text.
    pub json: bool,
    /// A status line is on the error stream and must be cleared before the next line.
    status: bool,
    /// The error stream is a terminal, so the status line redraws in place.
    redraw: bool,
}

impl<'a> Out<'a> {
    /// Writes data to `out`, notes and the status to `err`. `redraw` is true when `err` is a terminal.
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write, json: bool, redraw: bool) -> Self {
        Out {
            out,
            err,
            json,
            status: false,
            redraw,
        }
    }

    fn clear(&mut self) -> Result<(), CliError> {
        if self.status {
            write!(self.err, "\r\x1b[2K")?;
            self.status = false;
        }
        Ok(())
    }

    /// Writes text, which ends with a line end if it has one.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`] if the output is closed.
    pub fn text(&mut self, s: &str) -> Result<(), CliError> {
        self.clear()?;
        self.out.write_all(s.as_bytes())?;
        self.out.flush()?;
        Ok(())
    }

    /// Writes one line.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`] if the output is closed.
    pub fn line(&mut self, s: &str) -> Result<(), CliError> {
        self.clear()?;
        writeln!(self.out, "{s}")?;
        self.out.flush()?;
        Ok(())
    }

    /// Writes `v` as one JSON line.
    ///
    /// # Errors
    ///
    /// [`CliError::Json`] or [`CliError::Io`].
    pub fn json(&mut self, v: &impl Serialize) -> Result<(), CliError> {
        self.clear()?;
        serde_json::to_writer(&mut *self.out, v)?;
        writeln!(self.out)?;
        self.out.flush()?;
        Ok(())
    }

    /// Writes a note to the error stream.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`].
    pub fn note(&mut self, s: &str) -> Result<(), CliError> {
        self.clear()?;
        writeln!(self.err, "{s}")?;
        Ok(())
    }

    /// Shows the live status line. On a terminal it redraws in place. Else it is a line of its own.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`].
    pub fn status(&mut self, s: &str) -> Result<(), CliError> {
        if self.redraw {
            write!(self.err, "\r\x1b[2K{s}")?;
            self.status = true;
        } else {
            writeln!(self.err, "{s}")?;
        }
        self.err.flush()?;
        Ok(())
    }

    /// Clears the status line at the end of a live command.
    ///
    /// # Errors
    ///
    /// [`CliError::Io`].
    pub fn done(&mut self) -> Result<(), CliError> {
        self.clear()
    }
}
