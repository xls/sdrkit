//! The values that the results of the DVB-SH stack show. The shapes of the results are in [`crate::typed`].

sdrkit_core::choice_enum! {
    /// The waveform received.
    pub enum Waveform {
        /// OFDM, SH-A.
        Ofdm = ("ofdm", "OFDM"),
        /// TDM, SH-B.
        Tdm = ("tdm", "TDM"),
    }
}
