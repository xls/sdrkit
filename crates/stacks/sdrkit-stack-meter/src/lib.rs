//! Measurement stacks: instruments that read a signal or a span and decode nothing. Each stack is a graph of components with typed
//! ports, declares what it publishes, and has a test signal.
//!
//! | Id | Stack | Module |
//! |---|---|---|
//! | `fm_monitor` | FM modulation monitor | [`fm_monitor`] |
//! | `am_monitor` | AM modulation monitor | [`am_monitor`] |
//! | `occupancy` | Spectrum occupancy monitor | [`occupancy`] |
//! | `noise_floor` | Noise floor logger | [`noise_floor`] |
//! | `burst_finder` | Burst finder | [`burst_finder`] |
//! | `hopper` | Frequency-hopper analyser | [`hopper`] |
//! | `birdie` | Persistent carrier finder | [`birdie`] |
//! | `obw` | Occupied bandwidth meter | [`obw`] |
//! | `ptt_log` | PTT and duty-cycle logger | [`ptt_log`] |
//! | `noise_locator` | Power-line and impulse noise locator | [`noise_locator`] |
//! | `freq_accuracy` | Carrier frequency accuracy meter | [`freq_accuracy`] |
//! | `sinad` | FM SINAD meter | [`sinad`] |
//! | `nfm_meter` | Narrowband FM deviation and CTCSS meter | [`nfm_meter`] |
//! | `tx_stability` | Transmitter frequency stability meter | [`tx_stability`] |
//! | `ssb_test` | SSB transmitter test meter | [`ssb_test`] |
//! | `afsk_tuner` | AFSK tone tuner | [`afsk_tuner`] |
//! | `phase_noise` | Phase noise meter | [`phase_noise`] |
//! | `cw_keying` | CW keying quality meter | [`cw_keying`] |
//! | `fsk_analyser` | FSK parameter analyser | [`fsk_analyser`] |
//! | `mer` | Modulation quality meter | [`mer`] |
//! | `papr` | Crest factor meter | [`papr`] |
//! | `input_health` | SDR input health meter | [`input_health`] |
//! | `noise_figure` | Noise figure (Y-factor) meter | [`noise_figure`] |
//! | `kurtosis` | Spectral kurtosis RFI finder | [`kurtosis`] |
//! | `glitch` | SDR stream glitch detector | [`glitch`] |
//! | `intermod` | Intermodulation finder | [`intermod`] |
//! | `spur` | Spur and image finder | [`spur`] |
//! | `fading` | Fading analyser | [`fading`] |
//! | `beacon_monitor` | Beacon propagation monitor | [`beacon_monitor`] |

#![warn(missing_docs)]

pub mod afsk_tuner;
pub mod am_monitor;
pub mod beacon_monitor;
pub mod bench;
pub mod birdie;
pub mod burst_finder;
pub mod cw_keying;
pub mod fading;
pub mod fm_monitor;
pub mod freq_accuracy;
pub mod fsk_analyser;
pub mod glitch;
pub mod hopper;
pub mod input_health;
pub mod instrument;
pub mod intermod;
pub mod kurtosis;
pub mod mer;
pub mod nfm_meter;
pub mod noise_figure;
pub mod noise_floor;
pub mod noise_locator;
pub mod obw;
pub mod occupancy;
pub mod papr;
pub mod phase_noise;
pub mod ptt_log;
pub mod scene;
pub mod sinad;
pub mod spur;
pub mod ssb_test;
pub mod tx_stability;
pub mod tracked;
pub mod txtest;
pub mod typed;

use sdrkit_core::StackDescriptor;

/// The stacks of the crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &fm_monitor::DESCRIPTOR,
        &am_monitor::DESCRIPTOR,
        &occupancy::DESCRIPTOR,
        &noise_floor::DESCRIPTOR,
        &burst_finder::DESCRIPTOR,
        &hopper::DESCRIPTOR,
        &birdie::DESCRIPTOR,
        &obw::DESCRIPTOR,
        &ptt_log::DESCRIPTOR,
        &noise_locator::DESCRIPTOR,
        &freq_accuracy::DESCRIPTOR,
        &sinad::DESCRIPTOR,
        &nfm_meter::DESCRIPTOR,
        &tx_stability::DESCRIPTOR,
        &ssb_test::DESCRIPTOR,
        &afsk_tuner::DESCRIPTOR,
        &phase_noise::DESCRIPTOR,
        &cw_keying::DESCRIPTOR,
        &fsk_analyser::DESCRIPTOR,
        &mer::DESCRIPTOR,
        &papr::DESCRIPTOR,
        &input_health::DESCRIPTOR,
        &noise_figure::DESCRIPTOR,
        &kurtosis::DESCRIPTOR,
        &glitch::DESCRIPTOR,
        &intermod::DESCRIPTOR,
        &spur::DESCRIPTOR,
        &fading::DESCRIPTOR,
        &beacon_monitor::DESCRIPTOR,
    ]
}

/// Adds every component of the crate to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    fm_monitor::register(r);
    am_monitor::register(r);
    occupancy::register(r);
    noise_floor::register(r);
    burst_finder::register(r);
    hopper::register(r);
    birdie::register(r);
    obw::register(r);
    ptt_log::register(r);
    noise_locator::register(r);
    freq_accuracy::register(r);
    sinad::register(r);
    nfm_meter::register(r);
    tx_stability::register(r);
    ssb_test::register(r);
    afsk_tuner::register(r);
    phase_noise::register(r);
    cw_keying::register(r);
    fsk_analyser::register(r);
    mer::register(r);
    papr::register(r);
    input_health::register(r);
    noise_figure::register(r);
    kurtosis::register(r);
    glitch::register(r);
    intermod::register(r);
    spur::register(r);
    fading::register(r);
    beacon_monitor::register(r);
}
