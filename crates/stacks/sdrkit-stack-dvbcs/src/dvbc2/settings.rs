//! The settings of the DVB-C2 stack: the PLP choice, the centre-known switch, the raster, the receiver thresholds that no
//! public document gives (each with its default), and the test signal.

use super::signal::{Guard, Raster};
use super::tables::{CodeRate, ModCod, Qam, W_RM_DEFAULT};
use sdrkit_proto_dvbs2::transmit::{GEN_PAYLOAD, TEST_SIGNAL};
use sdrkit_core::publish::field;
use sdrkit_core::Setting;
use sdrkit_proto_mpegts::layer::DECODE_VIDEO;

/// The section of the receiver thresholds.
pub const THRESHOLDS: &str = "Decoder thresholds";

/// The PLP decoded: its PLP_ID, or -1 for the first data PLP of the first slice in the window.
pub const PLP: Setting<field::I64> = Setting::new("plp", "PLP", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help("PLP_ID of the PLP to decode, as the table of PLPs lists it; -1: the first data PLP of the first data slice inside the receive window. The common PLP of its group is decoded with it.");
/// The data slice of the PLP: its DSLICE_ID, or -1 for the first slice in the window that carries it.
pub const SLICE: Setting<field::I64> = Setting::new("slice", "Data slice", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help("DSLICE_ID of the data slice; -1: the first slice inside the receive window that carries the PLP.");
/// The tuned frequency is the RF centre of the samples.
pub const CENTRE_KNOWN: Setting<field::Bool> = Setting::new("centre_known", "Centre frequency known", "", true)
    .help("Off: the absolute carrier index is found by a full search over every carrier position (about a second once).");
/// The channel raster.
pub const RASTER: Setting<field::Choice<Raster>> = Setting::choice("raster", "Channel raster", Raster::R8)
    .help("8 MHz: 64/7 Msps, carriers 2232 Hz apart; 6 MHz: 48/7 Msps, 1674 Hz. Transmitter and receiver must agree.");
/// The most LDPC iterations per FEC block.
pub const LDPC_ITERATIONS: Setting<field::I64> = Setting::new("ldpc_iterations", "LDPC iterations", "", 50)
    .range(1.0, 200.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// The guard correlation metric that accepts a symbol timing.
pub const TIMING_METRIC: Setting<field::F64> = Setting::new("timing_metric", "Timing metric", "", 0.22)
    .range(0.01, 1.0, 0.0)
    .section(THRESHOLDS)
    .advanced()
    .help("The mean of |C| / E of the guard correlation folded over 8 symbol periods.");
/// The scattered pilot energy ratio below which a symbol is taken for a preamble.
pub const PREAMBLE_RATIO: Setting<field::F64> = Setting::new("preamble_ratio", "Preamble ratio", "", 3.0)
    .range(1.0, 10.0, 0.0)
    .section(THRESHOLDS)
    .advanced()
    .help("The energy of the strongest scattered pilot phase against the median: about 5 on a data symbol.");
/// The preamble pilot correlation that accepts an absolute carrier index.
pub const INDEX_METRIC: Setting<field::F64> = Setting::new("index_metric", "Carrier index metric", "", 0.35)
    .range(0.05, 1.0, 0.0)
    .section(THRESHOLDS)
    .advanced();
/// Carriers either side of the nominal index that the search tries when the centre is known.
pub const INDEX_SPAN: Setting<field::I64> = Setting::new("index_span", "Carrier index span", "", 64)
    .range(0.0, 1000.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// The FECFrame header candidate measure.
pub const HEADER_CANDIDATE: Setting<field::F64> = Setting::new("header_candidate", "Header candidate", "", 0.45)
    .range(0.05, 1.0, 0.0)
    .section(THRESHOLDS)
    .advanced();
/// Code bits of 32 that must agree with a decoded header.
pub const HEADER_AGREE: Setting<field::I64> = Setting::new("header_agree", "Header agreement", "", 22)
    .range(16.0, 32.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// Failed headers in a row that lose the header lock.
pub const HEADER_MISSES: Setting<field::I64> = Setting::new("header_misses", "Header misses", "", 2)
    .range(1.0, 100.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// Failed FEC blocks in a row that drop the alignment of a type 1 slice.
pub const BLOCK_MISSES: Setting<field::I64> = Setting::new("block_misses", "Block misses", "", 8)
    .range(1.0, 1000.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// Frames without a preamble before the timing search starts again.
pub const PREAMBLE_MISSES: Setting<field::I64> = Setting::new("preamble_misses", "Preamble misses", "", 3)
    .range(1.0, 100.0, 1.0)
    .section(THRESHOLDS)
    .advanced();
/// BBFRAMEs of a bundled PLP held to restore their order.
pub const BUNDLE_HOLD: Setting<field::I64> = Setting::new("bundle_hold", "Bundled PLP hold", "frames", 16)
    .range(1.0, 1000.0, 1.0)
    .section(THRESHOLDS)
    .advanced()
    .help("The BBFRAMEs of a PLP bundled over several slices wait here until they can go out in the order of their ISCR.");
/// The 32-bit word w^RM that scrambles the lower branch of the Reed-Muller headers, w^RM_0 the most significant bit.
pub const RM_SCRAMBLING: Setting<field::I64> =
    Setting::new("rm_scrambling", "Header scrambling word", "", W_RM_DEFAULT as i64)
        .range(0.0, 4_294_967_295.0, 1.0)
        .section(THRESHOLDS)
        .advanced()
        .help("w^RM of clause 7.2.2.3 as a number, w^RM_0 the most significant bit; default 0x1D8D3BF3. Transmitter and receiver must agree.");
/// The time interleaver of a data slice has one column per carrier of the slice.
pub const TI_FULL_WIDTH: Setting<field::Bool> = Setting::new("ti_full_width", "Time interleaver over every carrier", "", true)
    .section(THRESHOLDS)
    .advanced()
    .help("On: N_DSLICE_WIDTH = K_DS,max - K_DS,min + 1, one column per carrier from K_DS,min to K_DS,max (clause 9.4.1 makes K_DS,max the last carrier of the slice). Off: K_DS,max - K_DS,min as clause 9.4.4 prints it, and the last carrier of the slice carries no data.");

sdrkit_core::choice_enum! {
    /// The coding of the test signal.
    pub enum GenModCod {
        /// 256-QAM 5/6.
        Q256R56 = ("256_5_6", "256-QAM 5/6"),
        /// 16-QAM 4/5.
        Q16R45 = ("16_4_5", "16-QAM 4/5"),
        /// 64-QAM 2/3.
        Q64R23 = ("64_2_3", "64-QAM 2/3"),
        /// 256-QAM 3/4.
        Q256R34 = ("256_3_4", "256-QAM 3/4"),
        /// 64-QAM 9/10.
        Q64R910 = ("64_9_10", "64-QAM 9/10"),
        /// 256-QAM 9/10.
        Q256R910 = ("256_9_10", "256-QAM 9/10"),
        /// 1024-QAM 3/4.
        Q1024R34 = ("1024_3_4", "1024-QAM 3/4"),
        /// 4096-QAM 5/6.
        Q4096R56 = ("4096_5_6", "4096-QAM 5/6"),
    }
}

impl GenModCod {
    /// The coding with 64K (`long`) or 16K frames.
    pub fn modcod(self, long: bool) -> ModCod {
        let (qam, rate) = match self {
            GenModCod::Q256R56 => (Qam::Q256, CodeRate::R5_6),
            GenModCod::Q16R45 => (Qam::Q16, CodeRate::R4_5),
            GenModCod::Q64R23 => (Qam::Q64, CodeRate::R2_3),
            GenModCod::Q256R34 => (Qam::Q256, CodeRate::R3_4),
            GenModCod::Q64R910 => (Qam::Q64, CodeRate::R9_10),
            GenModCod::Q256R910 => (Qam::Q256, CodeRate::R9_10),
            GenModCod::Q1024R34 => (Qam::Q1024, CodeRate::R3_4),
            GenModCod::Q4096R56 => (Qam::Q4096, CodeRate::R5_6),
        };
        ModCod { long, qam, rate }
    }
}

sdrkit_core::choice_enum! {
    /// The data slice type of the test signal.
    pub enum GenSliceType {
        /// Type 1: one PLP, its FEC blocks from PLP_START.
        Type1 = ("type1", "Type 1"),
        /// Type 2 with the robust (QPSK) FECFrame header.
        Type2Robust = ("type2_robust", "Type 2, robust header"),
        /// Type 2 with the high efficiency (16-QAM) FECFrame header.
        Type2Efficient = ("type2_efficient", "Type 2, high efficiency header"),
    }
}

sdrkit_core::choice_enum! {
    /// The time interleaving depth of the test signal's slice.
    pub enum GenTiDepth {
        /// None.
        None = ("none", "None"),
        /// 4 symbols.
        D4 = ("4", "4 symbols"),
        /// 8 symbols.
        D8 = ("8", "8 symbols"),
        /// 16 symbols.
        D16 = ("16", "16 symbols"),
    }
}

sdrkit_core::choice_enum! {
    /// L1_TI_MODE of the test signal.
    pub enum GenL1Ti {
        /// 00: none.
        None = ("none", "None"),
        /// 01: best fit.
        BestFit = ("best_fit", "Best fit"),
        /// 10: 4 symbols.
        D4 = ("4", "4 symbols"),
        /// 11: 8 symbols.
        D8 = ("8", "8 symbols"),
    }
}

/// The RF centre of the test signal's samples, Hz: the frequency the receiver is tuned to.
pub const GEN_FREQUENCY: Setting<field::Hz> = Setting::new("gen_frequency", "Centre frequency", "Hz", 346e6)
    .range(47e6, 1002e6, 0.0)
    .section(TEST_SIGNAL)
    .advanced()
    .help("The system is centred on the carrier below this frequency that is a multiple of D_X.");
/// The coding of the test signal.
pub const GEN_MODCOD: Setting<field::Choice<GenModCod>> = Setting::choice("gen_modcod", "Coding", GenModCod::Q256R56)
    .section(TEST_SIGNAL)
    .advanced();
/// 16K FEC frames.
pub const GEN_SHORT_FRAMES: Setting<field::Bool> = Setting::new("gen_short_frames", "16K FEC frames", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The guard interval of the test signal.
pub const GEN_GUARD: Setting<field::Choice<Guard>> = Setting::choice("gen_guard", "Guard interval", Guard::G1_64)
    .section(TEST_SIGNAL)
    .advanced();
/// The slice type of the test signal.
pub const GEN_SLICE_TYPE: Setting<field::Choice<GenSliceType>> =
    Setting::choice("gen_slice_type", "Data slice type", GenSliceType::Type1)
        .section(TEST_SIGNAL)
        .advanced();
/// The time interleaving depth of the test signal's slice.
pub const GEN_TI_DEPTH: Setting<field::Choice<GenTiDepth>> =
    Setting::choice("gen_ti_depth", "Time interleaving", GenTiDepth::None)
        .section(TEST_SIGNAL)
        .advanced();
/// L1_TI_MODE of the test signal.
pub const GEN_L1_TI: Setting<field::Choice<GenL1Ti>> = Setting::choice("gen_l1_ti", "L1 time interleaving", GenL1Ti::None)
    .section(TEST_SIGNAL)
    .advanced();
/// A narrowband notch at half the tuning position.
pub const GEN_NOTCH: Setting<field::Bool> = Setting::new("gen_notch", "Narrowband notch", "", false)
    .section(TEST_SIGNAL)
    .advanced()
    .help("35 carriers at guard 1/64, 23 at 1/128.");
/// Reserved tones on.
pub const GEN_RESERVED_TONES: Setting<field::Bool> = Setting::new("gen_reserved_tones", "Reserved tones", "", false)
    .section(TEST_SIGNAL)
    .advanced();
sdrkit_core::choice_enum! {
    /// The data slices and PLPs of the test signal.
    pub enum GenSystem {
        /// One slice over one L1 block, PLP 1.
        Single = ("single", "One slice, one PLP"),
        /// One type 2 slice whose packets of PLP 1 and PLP 2 alternate.
        TwoPlps = ("two_plps", "One slice, two PLPs"),
        /// One type 2 slice: the grouped data PLP 1 and the common PLP 9 of its group.
        Common = ("common", "Data PLP and common PLP"),
        /// Two type 2 slices in one L1 block that both carry the bundled PLP 1, in high efficiency mode with ISSY.
        Bundled = ("bundled", "PLP bundled over two slices"),
        /// A system two L1 blocks wide with two slices: PLP 1 in the lower, PLP 2 in the upper.
        TwoSlices = ("two_slices", "Two slices, two L1 blocks wide"),
        /// A type 1 slice with PLP 1 and a type 2 slice with PLP 2, half an L1 block each.
        MixedSlices = ("mixed_slices", "A type 1 and a type 2 slice"),
        /// A type 2 slice with the grouped data PLPs 1 and 2 and a type 2 slice with the common PLP 9 of their group.
        Group = ("group", "Two data PLPs and their common PLP"),
        /// The group over two L1 blocks: PLP 1 over the lower block, on which the samples are centred; the common PLP 9 in
        /// the first 432 carriers of the upper block and PLP 2 in the rest of it.
        FarSlice = ("far_slice", "Group over two L1 blocks"),
    }
}

sdrkit_core::choice_enum! {
    /// What the data PLPs of the test signal carry.
    pub enum GenStream {
        /// The test transport stream.
        Ts = ("ts", "Transport stream"),
        /// GSE with UDP packets of 40 to 3000 bytes.
        Gse = ("gse", "GSE"),
        /// A generic fixed-length packetised stream of 96-byte packets.
        Gfps = ("gfps", "GFPS"),
        /// A generic continuous stream of 32-bit counters.
        Gcs = ("gcs", "GCS"),
    }
}

/// The data slices and PLPs of the test signal.
pub const GEN_SYSTEM: Setting<field::Choice<GenSystem>> = Setting::choice("gen_system", "Slices and PLPs", GenSystem::Single)
    .section(TEST_SIGNAL)
    .advanced()
    .help("two_plps and common use a type 2 slice; bundled, group and far_slice use type 2 slices only; mixed_slices has one slice of each type. group and far_slice carry numbered packets. two_slices and far_slice are 15.2 MHz wide (8 MHz raster): two_slices is centred on gen_frequency and needs 20 Msps to be seen whole; far_slice has the centre of its lower L1 block there, and the window at the tuning position of the slice of PLP 2 holds that slice and the common PLP.");
/// High efficiency mode BBFRAMEs.
pub const GEN_HEM: Setting<field::Bool> = Setting::new("gen_hem", "High efficiency mode", "", false)
    .section(TEST_SIGNAL)
    .advanced()
    .help("Transport packets without sync byte and CRC-8, or GSE, in BBFRAMEs of MODE 1.");
/// What the data PLPs carry.
pub const GEN_STREAM: Setting<field::Choice<GenStream>> = Setting::choice("gen_stream", "Stream", GenStream::Ts)
    .section(TEST_SIGNAL)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the DVB-C2 stack.
    pub enum Dvbc2Setting {
        /// The PLP.
        Plp = PLP,
        /// The data slice.
        Slice = SLICE,
        /// The tuned frequency is the RF centre.
        CentreKnown = CENTRE_KNOWN,
        /// The channel raster.
        Raster = RASTER,
        /// The most LDPC iterations.
        LdpcIterations = LDPC_ITERATIONS,
        /// The timing metric.
        TimingMetric = TIMING_METRIC,
        /// The preamble ratio.
        PreambleRatio = PREAMBLE_RATIO,
        /// The carrier index metric.
        IndexMetric = INDEX_METRIC,
        /// The carrier index span.
        IndexSpan = INDEX_SPAN,
        /// The header candidate measure.
        HeaderCandidate = HEADER_CANDIDATE,
        /// The header agreement.
        HeaderAgree = HEADER_AGREE,
        /// Failed headers that lose the lock.
        HeaderMisses = HEADER_MISSES,
        /// Failed blocks that drop the alignment.
        BlockMisses = BLOCK_MISSES,
        /// Frames without a preamble before a new timing search.
        PreambleMisses = PREAMBLE_MISSES,
        /// The width of the time interleaver.
        TiFullWidth = TI_FULL_WIDTH,
        /// The header scrambling word.
        RmScrambling = RM_SCRAMBLING,
        /// The bundled PLP hold.
        BundleHold = BUNDLE_HOLD,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The test signal's centre frequency.
        GenFrequency = GEN_FREQUENCY,
        /// The test signal's coding.
        GenModcod = GEN_MODCOD,
        /// 16K FEC frames.
        GenShortFrames = GEN_SHORT_FRAMES,
        /// The test signal's guard interval.
        GenGuard = GEN_GUARD,
        /// The test signal's slice type.
        GenSliceType = GEN_SLICE_TYPE,
        /// The test signal's time interleaving.
        GenTiDepth = GEN_TI_DEPTH,
        /// The test signal's L1 time interleaving.
        GenL1Ti = GEN_L1_TI,
        /// A notch.
        GenNotch = GEN_NOTCH,
        /// Reserved tones.
        GenReservedTones = GEN_RESERVED_TONES,
        /// The slices and PLPs.
        GenSystem = GEN_SYSTEM,
        /// The stream of the data PLPs.
        GenStream = GEN_STREAM,
        /// High efficiency mode.
        GenHem = GEN_HEM,
        /// The payload.
        GenPayload = GEN_PAYLOAD,
    }
}

sdrkit_core::settings! {
    /// The settings of the receiver component `dvbc2.receiver`.
    pub enum RxSetting {
        /// The PLP.
        Plp = PLP,
        /// The data slice.
        Slice = SLICE,
        /// The tuned frequency is the RF centre.
        CentreKnown = CENTRE_KNOWN,
        /// The channel raster.
        Raster = RASTER,
        /// The most LDPC iterations.
        LdpcIterations = LDPC_ITERATIONS,
        /// The timing metric.
        TimingMetric = TIMING_METRIC,
        /// The preamble ratio.
        PreambleRatio = PREAMBLE_RATIO,
        /// The carrier index metric.
        IndexMetric = INDEX_METRIC,
        /// The carrier index span.
        IndexSpan = INDEX_SPAN,
        /// The header candidate measure.
        HeaderCandidate = HEADER_CANDIDATE,
        /// The header agreement.
        HeaderAgree = HEADER_AGREE,
        /// Failed headers that lose the lock.
        HeaderMisses = HEADER_MISSES,
        /// Failed blocks that drop the alignment.
        BlockMisses = BLOCK_MISSES,
        /// Frames without a preamble before a new timing search.
        PreambleMisses = PREAMBLE_MISSES,
        /// The width of the time interleaver.
        TiFullWidth = TI_FULL_WIDTH,
        /// The header scrambling word.
        RmScrambling = RM_SCRAMBLING,
        /// The bundled PLP hold.
        BundleHold = BUNDLE_HOLD,
    }
}
