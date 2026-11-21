//! A terminal application on the sdrkit C API: it lists the stacks and the drivers, opens radios, runs a stack with its
//! spectrum, results, tables and sound, and edits the settings and runs the commands of any object.

// The C API's symbols come from this crate. The application calls them only through the declarations in `sys`.
extern crate sdrkit;

pub mod api;
pub mod app;
pub mod entry;
pub mod find;
pub mod format;
pub mod known;
pub mod prefs;
pub mod radio_form;
pub mod results;
pub mod sound;
mod sys;
pub mod theme;
pub mod ui;
pub mod views;
pub mod waterfall;
