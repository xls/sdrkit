//! The `sdrkit` program: the command line on the standard streams, with Ctrl-C and the console lines of `run`.

use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;

fn main() -> ExitCode {
    if let Err(e) = ctrlc::set_handler(sdrkit_cli::request_stop) {
        eprintln!("Ctrl-C will not stop a live command cleanly: {e}");
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::Builder::new()
        .name("sdrkit-console".into())
        .spawn(move || {
            for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    let lines = reader.is_ok().then_some(rx);
    let redraw = std::io::stderr().is_terminal();
    let code = sdrkit_cli::run(
        std::env::args_os(),
        None,
        sdrkit_cli::Io {
            out: &mut std::io::stdout().lock(),
            err: &mut std::io::stderr(),
            redraw,
            lines,
        },
    );
    ExitCode::from(code)
}
