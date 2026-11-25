//! The commands for USB radios and for serving, run in the process: `devices` on a scripted USB bus, `serve` with a real
//! rtl_tcp client on 127.0.0.1, and `usb record` on a scripted dongle. No USB device of the computer is opened.

use sdrkit_cli::{run, run_on, Io};
use sdrkit_core::MemStorage;
use sdrkit_driver_usb::{BulkItem, FakeUsb, Script, UsbId};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// An output that another thread can read while the command runs.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Shared {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// Runs `args` on the USB bus `usb` and returns the exit code, the output and the error text.
fn sdrkit_on(args: &[&str], usb: FakeUsb) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv = std::iter::once("sdrkit")
        .chain(args.iter().copied())
        .map(Into::into);
    let code = run_on(
        argv,
        Some(Arc::new(MemStorage::new())),
        Io {
            out: &mut out,
            err: &mut err,
            redraw: false,
            lines: None,
        },
        Arc::new(usb),
    );
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

/// A scripted bus with a dongle that streams, a HackRF and a device that no driver takes.
fn bus() -> FakeUsb {
    let text = "sdrkit-usb 1\nmode lenient\n\
        device 1 0bda:2838 bus=\"1\" ports=3 manufacturer=\"Realtek\" product=\"RTL2838UHIDIR\" serial=\"0001\"\n\
        device 2 1d50:6089 bus=\"2\" ports=1.4 product=\"HackRF One\" serial=\"0000000000000000a1b2c3d4e5f60718\"\n\
        device 3 046d:c52b bus=\"2\" ports=2 product=\"Receiver\"\n\
        out 1 40 00 00c8 0610 02 -> !stall\n\
        out 1 40 00 00c6 0610 00 -> !stall\n\
        in 1 c0 00 0120 000a 1 -> 09\n\
        in 1 c0 00 0034 0600 1 -> 69\n\
        in 1 c0 00 0034 0600 5 -> 69005a0014\n\
        in 1 c0 00 0034 0600 3 -> 69005a\n\
        in 1 c0 00 0034 0600 4 -> 69005a00\n";
    let mut s = Script::parse(text).unwrap();
    let data: Vec<u8> = (0..16_384u32).map(|i| (i * 37 % 251) as u8).collect();
    s.bulk(BulkItem {
        after: Duration::from_millis(4),
        repeat: 2000,
        ..BulkItem::data(UsbId(1), 0x81, data)
    });
    FakeUsb::new(s)
}

#[test]
fn devices_lists_the_usb_radios_with_their_addresses() {
    let (code, out, err) = sdrkit_on(&["devices"], bus());
    assert_eq!(code, 0, "{err}");
    let line = |text: &str| out.lines().find(|l| l.contains(text)).map(str::to_owned);
    let rtl = line("usb:rtl:0001").unwrap_or_else(|| panic!("no dongle in:\n{out}"));
    assert!(rtl.contains("rtlusb") && rtl.contains("Free"), "{rtl}");
    let hackrf = line("usb:hackrf:0000000000000000a1b2c3d4e5f60718")
        .unwrap_or_else(|| panic!("no HackRF in:\n{out}"));
    assert!(
        hackrf.contains("HackRF One") && hackrf.contains("2-1.4"),
        "{hackrf}"
    );
    assert!(!out.contains("Receiver"), "{out}");
    // The JSON form has the same rows.
    let (code, out, err) = sdrkit_on(&["devices", "--json"], bus());
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
    assert_eq!(v["table"], "devices");
    assert_eq!(v["rows"].as_array().map(Vec::len), Some(2));
}

/// Runs `args` with no USB bus and returns the exit code, the output and the error text.
fn sdrkit(args: &[&str]) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv = std::iter::once("sdrkit")
        .chain(args.iter().copied())
        .map(Into::into);
    let code = run(
        argv,
        Some(Arc::new(MemStorage::new())),
        Io {
            out: &mut out,
            err: &mut err,
            redraw: false,
            lines: None,
        },
    );
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[test]
fn without_usb_no_device_is_listed_and_the_drivers_say_why() {
    let (code, out, err) = sdrkit(&["devices"]);
    assert_eq!(code, 0, "{err}");
    assert!(!out.contains("usb:"), "{out}");
    let (code, out, err) = sdrkit(&["show", "engine", "drivers"]);
    assert_eq!(code, 0, "{err}");
    let row = |key: &str| {
        out.lines()
            .find(|l| l.split_whitespace().next() == Some(key))
            .unwrap_or_else(|| panic!("no driver {key} in:\n{out}"))
            .to_owned()
    };
    assert!(row("hackrf").contains("No USB access"), "{}", row("hackrf"));
    assert!(row("rtlusb").contains("No USB access"), "{}", row("rtlusb"));
    assert!(
        !row("rtl_tcp").contains("No USB access"),
        "{}",
        row("rtl_tcp")
    );
    // An open says the same.
    let (code, _, err) = sdrkit(&["serve", "usb:hackrf:", "--seconds", "0.1"]);
    assert_ne!(code, 0);
    assert!(err.contains("no USB access on this platform"), "{err}");
}

#[test]
fn serve_gives_an_rtl_tcp_client_the_test_signal() {
    let out = Shared::default();
    let err = Shared::default();
    let (o, e) = (out.clone(), err.clone());
    let command = std::thread::spawn(move || {
        let (mut o, mut e) = (o, e);
        let args = [
            "sdrkit",
            "serve",
            "synth:fm",
            "--rate",
            "1.024M",
            "--rtl-tcp",
            "127.0.0.1:0",
            "--seconds",
            "4",
        ];
        run(
            args.iter().map(Into::into),
            Some(Arc::new(MemStorage::new())),
            Io {
                out: &mut o,
                err: &mut e,
                redraw: false,
                lines: None,
            },
        )
    });
    // The command prints the port that the system gave it.
    let t0 = Instant::now();
    let port = loop {
        let text = out.text();
        let found = text
            .lines()
            .find_map(|l| l.strip_prefix("rtl_tcp on 127.0.0.1:"))
            .and_then(|p| p.trim().parse::<u16>().ok());
        if let Some(p) = found {
            break p;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "no listener: {text}\n{}",
            err.text()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut greeting = [0u8; 12];
    client.read_exact(&mut greeting).unwrap();
    assert_eq!(&greeting[..4], b"RTL0");
    let mut samples = vec![0u8; 200_000];
    client.read_exact(&mut samples).unwrap();
    // Unsigned 8-bit samples of a signal: around the middle, and not all equal.
    let mean = samples.iter().map(|b| f64::from(*b)).sum::<f64>() / samples.len() as f64;
    assert!((100.0..156.0).contains(&mean), "{mean}");
    assert!(samples.iter().any(|b| *b != samples[0]));
    drop(client);
    let code = command.join().unwrap();
    let (text, notes) = (out.text(), err.text());
    assert_eq!(code, 0, "{notes}");
    assert!(text.contains("share.h1 serves synth:fm"), "{text}");
    assert!(text.contains("share.client_joined"), "{text}");
    assert!(
        text.contains("1 clients") || text.contains("0 clients"),
        "{text}"
    );
}

#[test]
fn serve_refuses_a_control_that_does_not_exist() {
    let (code, _, err) = sdrkit(&["serve", "synth:fm", "--control", "everyone"]);
    assert_ne!(code, 0);
    assert!(
        err.contains("follow") && err.contains("remote") && err.contains("slice"),
        "{err}"
    );
}

#[test]
fn usb_record_writes_the_exchanges_of_a_scripted_dongle() {
    let file = std::env::temp_dir().join(format!(
        "sdrkit-usb-record-{}.usbscript",
        std::process::id()
    ));
    let path = file.to_string_lossy().into_owned();
    let (code, out, err) = sdrkit_on(
        &[
            "usb",
            "record",
            "usb:rtl:",
            "--out",
            &path,
            "--seconds",
            "0.5",
        ],
        bus(),
    );
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("samples from usb:rtl:"), "{out}");
    assert!(text.starts_with("sdrkit-usb 1"), "{text}");
    // The script holds the open of the dongle, its control transfers and its bulk data.
    assert!(text.lines().any(|l| l.starts_with("open ")), "{text}");
    assert!(text.lines().any(|l| l.starts_with("in ")), "{text}");
    assert!(text.lines().any(|l| l.starts_with("bulk ")), "{text}");
    // It reads back as a script.
    Script::parse(&text).unwrap();
}
