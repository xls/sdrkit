//! The acceleration backend of each kernel family as an engine setting.

use sdrkit_core::backend::{Backend, Registry, UnknownBackend};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::ChoiceDecl;

/// The key of the choice that selects the platform's default backend of a family.
const AUTO: &str = "auto";

/// The calls of a backend registry that the engine uses, for every family alike.
trait Family: Sync {
    fn names(&self) -> Vec<&'static str>;
    fn select(&self, name: &str) -> Result<(), UnknownBackend>;
    fn active(&self) -> &'static str;
}

impl<B: ?Sized + Backend> Family for Registry<B> {
    fn names(&self) -> Vec<&'static str> {
        Registry::names(self).collect()
    }
    fn select(&self, name: &str) -> Result<(), UnknownBackend> {
        Registry::select(self, name)
    }
    fn active(&self) -> &'static str {
        Registry::active(self)
    }
}

/// The choices of a backend setting: the platform's default, then every backend of `f`.
fn choices(f: &dyn Family) -> Vec<ChoiceDecl> {
    let names = f.names();
    let mut out = vec![ChoiceDecl {
        key: AUTO.into(),
        label: format!("Default ({})", names.first().copied().unwrap_or("")).into(),
    }];
    out.extend(names.into_iter().map(|n| ChoiceDecl {
        key: n.into(),
        label: n.into(),
    }));
    out
}

macro_rules! backend_setting {
    ($(#[$doc:meta])* $name:ident, $key:literal, $label:literal, $registry:path) => {
        $(#[$doc])*
        pub const $name: CatalogueSetting = CatalogueSetting {
            key: $key,
            label: $label,
            help: "The implementation that new objects of the family use. Objects that run keep theirs.",
            choices: || choices(&$registry),
            default: AUTO,
            advanced: false,
        };
    };
}

backend_setting!(
    /// The FFT backend.
    BACKEND_FFT, "backend_fft", "FFT backend", sdrkit_algo_dsp::fft::BACKENDS
);
backend_setting!(
    /// The FIR filter backend.
    BACKEND_FIR, "backend_fir", "FIR backend", sdrkit_algo_dsp::fir::BACKENDS
);
backend_setting!(
    /// The resampler backend.
    BACKEND_RESAMPLE, "backend_resample", "Resampler backend", sdrkit_algo_dsp::resample::BACKENDS
);
backend_setting!(
    /// The Viterbi decoder backend.
    BACKEND_VITERBI, "backend_viterbi", "Viterbi backend", sdrkit_algo_fec::conv::BACKENDS
);
backend_setting!(
    /// The Reed-Solomon decoder backend.
    BACKEND_RS, "backend_rs", "Reed-Solomon backend", sdrkit_algo_fec::rs::BACKENDS
);
backend_setting!(
    /// The LDPC decoder backend.
    BACKEND_LDPC, "backend_ldpc", "LDPC backend", sdrkit_algo_fec::ldpc::BACKENDS
);

/// The families, with their settings and their short names for the status line.
static FAMILIES: [(&CatalogueSetting, &str, &dyn Family); 6] = [
    (&BACKEND_FFT, "fft", &sdrkit_algo_dsp::fft::BACKENDS),
    (&BACKEND_FIR, "fir", &sdrkit_algo_dsp::fir::BACKENDS),
    (
        &BACKEND_RESAMPLE,
        "resample",
        &sdrkit_algo_dsp::resample::BACKENDS,
    ),
    (
        &BACKEND_VITERBI,
        "viterbi",
        &sdrkit_algo_fec::conv::BACKENDS,
    ),
    (&BACKEND_RS, "rs", &sdrkit_algo_fec::rs::BACKENDS),
    (&BACKEND_LDPC, "ldpc", &sdrkit_algo_fec::ldpc::BACKENDS),
];

/// Selects the backend of choice `choice` for the family of the setting with key `setting`: [`AUTO`] selects the
/// platform's default.
///
/// # Errors
///
/// [`UnknownBackend`] if `setting` is not a backend setting, or `choice` is not a backend of its family.
pub(crate) fn select(setting: &str, choice: &str) -> Result<(), UnknownBackend> {
    let (_, _, f) = FAMILIES
        .iter()
        .find(|(s, _, _)| s.key == setting)
        .ok_or(UnknownBackend)?;
    match choice {
        AUTO => f.select(f.names().first().copied().ok_or(UnknownBackend)?),
        name => f.select(name),
    }
}

/// The backend that runs in each family, as a status line: `fft accelerate, fir plain, ...`.
pub(crate) fn status() -> String {
    FAMILIES
        .iter()
        .map(|(_, short, f)| format!("{short} {}", f.active()))
        .collect::<Vec<_>>()
        .join(", ")
}
