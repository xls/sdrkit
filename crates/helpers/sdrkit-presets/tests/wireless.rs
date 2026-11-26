//! The channels and the start of the wireless stacks in the data folder of the repository: Bluetooth, Wi-Fi, ESP-NOW,
//! IEEE 802.15.4, nRF24 and ANT. The plans are in `plans/bluetooth.json`, `plans/wifi.json`, `plans/ieee802154.json`,
//! `plans/nrf24.json` and `plans/ant.json`.

use sdrkit_presets::default_dials::dial_hz;
use sdrkit_presets::wlan::{read_wifi_domains, WIFI_DOMAINS_FILE};
use sdrkit_presets::{Data, Options, PlanChannel, Region};

/// The countries that each test covers.
const COUNTRIES: &[&str] = &["AU", "US", "DE", "GB", "JP"];

fn data() -> Data {
    Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).expect("the data folder")
}

fn region(code: &str) -> Region {
    Region::new(code).expect("a country code")
}

/// The channels of `stack` in `code`, in MHz, after a check that they are in ascending order without a repeat.
fn channels(d: &Data, stack: &str, code: &str) -> Vec<PlanChannel> {
    let out = d.stack_channels(stack, &region(code)).expect("channels");
    assert!(
        out.windows(2)
            .all(|w| w[0].frequency_hz < w[1].frequency_hz),
        "{stack} {code}: the channels are not in ascending order"
    );
    out
}

fn mhz(c: &[PlanChannel]) -> Vec<f64> {
    c.iter().map(|c| c.frequency_hz / 1e6).collect()
}

/// The start frequency and the plan of the preset of `stack` in `code`, MHz.
fn start(d: &Data, stack: &str, code: &str) -> (f64, String) {
    let p = d
        .preset(stack, &region(code), &Options::default())
        .expect("a preset");
    (p.frequency_hz / 1e6, p.plan)
}

#[test]
fn bluetooth_le_has_three_advertising_and_37_data_channels() {
    let d = data();
    for stack in ["ble", "bletracker"] {
        for &code in COUNTRIES {
            let c = channels(&d, stack, code);
            assert_eq!(c.len(), 40, "{stack} {code}");
            let expected: Vec<f64> = (0..40).map(|k| 2402.0 + 2.0 * f64::from(k)).collect();
            assert_eq!(mhz(&c), expected, "{stack} {code}");
            assert!(c.iter().all(|c| c.width_hz == Some(2e6)));
            let label = |hz: f64| {
                c.iter()
                    .find(|c| c.frequency_hz == hz * 1e6)
                    .unwrap()
                    .label
                    .as_str()
            };
            assert_eq!(label(2402.0), "Advertising 37 (2402 MHz)");
            assert_eq!(label(2426.0), "Advertising 38 (2426 MHz)");
            assert_eq!(label(2480.0), "Advertising 39 (2480 MHz)");
            assert_eq!(label(2404.0), "Data 0 (2404 MHz)");
            assert_eq!(label(2424.0), "Data 10 (2424 MHz)");
            assert_eq!(label(2428.0), "Data 11 (2428 MHz)");
            assert_eq!(label(2478.0), "Data 36 (2478 MHz)");
            assert_eq!(
                start(&d, stack, code),
                (2402.0, stack.to_owned()),
                "{stack} {code}"
            );
        }
    }
}

#[test]
fn bluetooth_classic_has_79_channels_and_starts_in_the_middle() {
    let d = data();
    for &code in COUNTRIES {
        let c = channels(&d, "btclassic", code);
        let expected: Vec<f64> = (0..79).map(|k| 2402.0 + f64::from(k)).collect();
        assert_eq!(mhz(&c), expected, "{code}");
        assert!(c.iter().all(|c| c.width_hz == Some(1e6)));
        assert_eq!(
            start(&d, "btclassic", code),
            (2441.0, "btclassic".to_owned())
        );
    }
}

#[test]
fn ieee_802_15_4_has_16_channels() {
    let d = data();
    let expected: Vec<f64> = (11..=26)
        .map(|k| 2405.0 + 5.0 * f64::from(k - 11))
        .collect();
    for &code in COUNTRIES {
        for (stack, first) in [("zigbee", 2405.0), ("thread", 2425.0)] {
            let c = channels(&d, stack, code);
            assert_eq!(mhz(&c), expected, "{stack} {code}");
            assert!(c.iter().all(|c| c.width_hz == Some(2e6)));
            assert_eq!(c[0].label, "Channel 11 (2405 MHz)");
            assert_eq!(
                start(&d, stack, code),
                (first, stack.to_owned()),
                "{stack} {code}"
            );
        }
    }
    // The dial of ism/default_dials.csv is the first channel of the plan.
    let dials = d.default_dials().expect("the dial files");
    assert_eq!(dial_hz(&dials, "zigbee", None), Some(2405e6));
}

#[test]
fn nrf24_and_ant_list_every_channel_of_the_chip() {
    let d = data();
    for &code in COUNTRIES {
        let c = channels(&d, "nrf24", code);
        let expected: Vec<f64> = (0..126).map(|k| 2400.0 + f64::from(k)).collect();
        assert_eq!(mhz(&c), expected, "{code}");
        assert_eq!(start(&d, "nrf24", code), (2476.0, "nrf24".to_owned()));
        let c = channels(&d, "ant", code);
        let expected: Vec<f64> = (0..125).map(|k| 2400.0 + f64::from(k)).collect();
        assert_eq!(mhz(&c), expected, "{code}");
        assert!(c.iter().any(|c| c.label == "ANT+ 2457 MHz"));
        assert_eq!(start(&d, "ant", code), (2457.0, "ant".to_owned()));
    }
}

/// The centre of Wi-Fi channel `n` of `band` ("2.4", "5", "6"), MHz.
fn wifi_centre(band: &str, n: u32) -> f64 {
    let n = f64::from(n);
    match band {
        "2.4" if n == 14.0 => 2484.0,
        "2.4" => 2407.0 + 5.0 * n,
        "5" => 5000.0 + 5.0 * n,
        _ if n == 2.0 => 5935.0,
        _ => 5950.0 + 5.0 * n,
    }
}

#[test]
fn wifi_and_espnow_channels_are_the_rows_of_the_domain_file() {
    let d = data();
    let rows =
        read_wifi_domains(&d.folder().join(WIFI_DOMAINS_FILE)).expect("the Wi-Fi domain file");
    let of = |domain: &str, wifi: bool| -> Vec<f64> {
        let mut out: Vec<f64> = rows
            .iter()
            .filter(|r| r.domain == domain && (wifi || r.band == "2.4"))
            .flat_map(|r| {
                let list = if wifi {
                    r.channels.clone()
                } else {
                    r.hop_list(false)
                };
                list.into_iter()
                    .map(|n| wifi_centre(&r.band, n))
                    .collect::<Vec<_>>()
            })
            .collect();
        out.sort_by(f64::total_cmp);
        out
    };
    for (code, domain, plan) in [
        ("AU", "au", "au"),
        ("US", "us", "us"),
        ("DE", "eu", "eu"),
        ("GB", "eu", "eu"),
        ("JP", "jp", "jp"),
        ("CN", "cn", "cn"),
    ] {
        let wifi = channels(&d, "wifi", code);
        assert_eq!(mhz(&wifi), of(domain, true), "wifi {code}");
        assert!(wifi.iter().all(|c| c.width_hz == Some(20e6)));
        assert_eq!(
            start(&d, "wifi", code),
            (2437.0, format!("wifi_{plan}")),
            "wifi {code}"
        );
        assert_eq!(
            mhz(&channels(&d, "espnow", code)),
            of(domain, false),
            "espnow {code}"
        );
        assert_eq!(
            start(&d, "espnow", code),
            (2412.0, format!("espnow_{plan}")),
            "espnow {code}"
        );
    }
    // Japan has channel 14; the 2.4 GHz hop list leaves it out.
    let jp = channels(&d, "wifi", "JP");
    assert!(jp
        .iter()
        .any(|c| c.frequency_hz == 2484e6 && c.label == "Channel 14 (2484 MHz)"));
    assert!(channels(&d, "espnow", "JP")
        .iter()
        .all(|c| c.frequency_hz < 2480e6));
    // Australia: 5600 to 5650 MHz (channels 120 to 128) is closed, 6 GHz is open up to 6585 MHz.
    let au = mhz(&channels(&d, "wifi", "AU"));
    assert!(
        !au.contains(&5600.0)
            && !au.contains(&5640.0)
            && au.contains(&5580.0)
            && au.contains(&5660.0)
    );
    assert_eq!(au.last(), Some(&6575.0));
    // A country without a domain gets the channels that every domain permits, without 6 GHz.
    let nz = mhz(&channels(&d, "wifi", "NZ"));
    assert_eq!(nz.len(), 19);
    assert_eq!(
        (nz[0], nz[10], nz[11], nz[18]),
        (2412.0, 2462.0, 5180.0, 5320.0)
    );
    assert_eq!(start(&d, "wifi", "NZ"), (2437.0, "wifi_common".to_owned()));
    assert_eq!(
        start(&d, "espnow", "NZ"),
        (2412.0, "espnow_common".to_owned())
    );
}
