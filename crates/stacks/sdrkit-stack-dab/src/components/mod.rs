//! The DAB components and the typed items between them.
//!
//! | Type name | Component | Ports |
//! |---|---|---|
//! | `dab.centring` | [`front::Centring`] | iq, sync (feedback) -> iq |
//! | `dab.guard` | [`front::SampleGuard`] | iq -> iq, status |
//! | `dab.ofdm` | [`ofdm::OfdmComponent`] | iq -> fic, cif, cells, null, status |
//! | `dab.fic` | [`fic::FicComponent`] | fic -> fib |
//! | `dab.figs` | [`fic::FigParser`] | fib -> fig |
//! | `dab.ensemble` | [`fic::EnsembleComponent`] | fig -> ensemble |
//! | `dab.msc` | [`msc::MscComponent`] | cif, ensemble -> frame, status |
//! | `dab.superframe` | [`audio::SuperframeComponent`] | frame -> aac, pad, status |
//! | `dab.mp2_framer` | [`audio::Mp2FramerComponent`] | frame -> mp2, pad, status |
//! | `dab.mp2_decoder` | [`audio::Mp2DecoderComponent`] | mp2 -> pcm (for `bcdata.playout`) |
//! | `dab.pad` | [`data::PadComponent`] | pad (aac), pad (mp2), ensemble -> label, datagroup |
//! | `dab.mot` | [`data::MotComponent`] | datagroup -> object |
//! | `dab.packet` | [`services::PacketComponent`] | frame, ensemble -> datagroup, status |
//! | `dab.journaline` | [`services::JournalineComponent`] | datagroup (X-PAD), datagroup (packet), ensemble -> journaline |
//! | `dab.spi` | [`services::SpiComponent`] | object (X-PAD), object (packet) -> spi |
//! | `dab.tii` | [`services::TiiComponent`] | null, ensemble -> tii |
//! | `dab.dmb` | [`services::DmbComponent`] | frame -> media, status |
//! | `dab.report` | [`report::ReportComponent`] | every status -> events, picture (and the results) |

pub mod audio;
pub mod data;
pub mod fic;
pub mod front;
pub mod items;
pub mod msc;
pub mod ofdm;
pub mod report;
pub mod services;

/// Adds every DAB component to `r`, under the type names in the table above, so a description (JSON) can use them.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("dab.centring", || Box::new(front::Centring::default()));
    r.add("dab.guard", || Box::new(front::SampleGuard::default()));
    r.add("dab.ofdm", || Box::new(ofdm::OfdmComponent::default()));
    r.add("dab.fic", || Box::new(fic::FicComponent::default()));
    r.add("dab.figs", || Box::new(fic::FigParser::default()));
    r.add("dab.ensemble", || {
        Box::new(fic::EnsembleComponent::default())
    });
    r.add("dab.msc", || Box::new(msc::MscComponent::default()));
    r.add("dab.superframe", || {
        Box::new(audio::SuperframeComponent::default())
    });
    r.add("dab.mp2_framer", || {
        Box::new(audio::Mp2FramerComponent::default())
    });
    r.add("dab.mp2_decoder", || {
        Box::new(audio::Mp2DecoderComponent::default())
    });
    r.add("dab.pad", || Box::new(data::PadComponent::default()));
    r.add("dab.mot", || Box::new(data::MotComponent::default()));
    r.add("dab.packet", || {
        Box::new(services::PacketComponent::default())
    });
    r.add("dab.journaline", || {
        Box::new(services::JournalineComponent::default())
    });
    r.add("dab.spi", || Box::new(services::SpiComponent::default()));
    r.add("dab.tii", || Box::new(services::TiiComponent::default()));
    r.add("dab.dmb", || Box::new(services::DmbComponent::default()));
    r.add(
        "dab.report",
        || Box::new(report::ReportComponent::default()),
    );
}
