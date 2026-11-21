// sdrkit: the typed interface of the stack family "dvbt", version 2.5. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum DvbtInterface {
    public static let major = 2
    public static let minor = 5
}

/// The result fields of `dvbt`, which `report` writes: the input, the OFDM parameters, the TPS, the error correction and the quality. A value of the TPS has no value while the TPS is not locked; a value of the FFT mode has no value before the mode is found.
public struct DvbtFields {
    public enum BandwidthFrom: Int32 {
        case setting = 0
        case guess = 1
        case measured = 2
        case tried = 3
    }
    public enum Fft: Int32 {
        case n2K = 0
        case n8K = 1
    }
    public enum Guard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
    }
    public enum Constellation: Int32 {
        case qPSK = 0
        case n16QAM = 1
        case n64QAM = 2
    }
    public enum Hierarchy: Int32 {
        case none = 0
        case n1 = 1
        case n2 = 2
        case n4 = 3
    }
    public enum CodeRateHp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    public enum CodeRateLp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    public enum Stream: Int32 {
        case hp = 0
        case lp = 1
    }
    public enum QualityLabel: Int32 {
        case excellent = 0
        case good = 1
        case marginal = 2
        case poor = 3
        case noLock = 4
    }
    /// The detection level, 0 to 3.
    public var detectLevel: Int64
    /// The channel width in use.
    public var channelWidth: Double
    /// The `bandwidth` setting is Auto.
    public var bandwidthAuto: Bool
    /// Where the width in use came from.
    public var bandwidthFrom: BandwidthFrom?
    /// The measured occupied width.
    public var measuredWidth: Double?
    /// The level of the channel above the floor outside it.
    public var occupancyDb: Double?
    /// The input rate.
    public var inputRate: Double
    /// The natural rate of the width in use.
    public var naturalRate: Double
    /// The input is resampled.
    public var resampling: Bool
    /// The input rate is enough for the width.
    public var rateOk: Bool
    /// The FFT mode.
    public var fft: Fft?
    /// The FFT size.
    public var fftSize: Int64?
    /// The guard interval.
    public var `guard`: Guard?
    /// The guard samples.
    public var guardSamples: Int64?
    /// The carriers.
    public var carriers: Int64?
    /// The constellation.
    public var constellation: Constellation?
    /// The hierarchy.
    public var hierarchy: Hierarchy?
    /// The HP code rate.
    public var codeRateHp: CodeRateHp?
    /// The LP code rate.
    public var codeRateLp: CodeRateLp?
    /// The stream decoded.
    public var stream: Stream?
    /// The cell id. No value until two frames agree.
    public var cellId: Int64?
    /// The TPS length indicator.
    public var tpsLength: Int64?
    /// The DVB-H signalling bits.
    public var dvbhFlags: Int64?
    /// The frame number, 0 to 3.
    public var frame: Int64?
    /// The symbol in the frame, 0 to 67.
    public var symbol: Int64?
    /// The duration of a frame.
    public var frameDuration: Double?
    /// The net bit rate of the stream decoded.
    public var netBitrateBps: Double?
    /// The carrier offset from the centre.
    public var offsetHz: Double?
    /// The integer carrier offset.
    public var integerOffset: Int64?
    /// The shift of the channel locator.
    public var locatorHz: Double
    /// The error of the sample clock.
    public var clockPpm: Double
    /// The spectrum is mirrored.
    public var spectrumInverted: Bool
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled.
    public var snrDb: Double?
    /// The modulation error ratio. No value before the channel estimate is filled.
    public var merDb: Double?
    /// The BER before Viterbi over the last second: the received hard bits that differ from the decoded bits, coded again.
    public var berPreViterbi: Double
    /// The BER after Viterbi over the last second: the bits that Reed-Solomon corrected over the bits of the blocks that it decoded. A block that failed is not in it and is counted in `rs_failed`. 0 while no block decoded.
    public var berPostViterbi: Double
    /// The Viterbi margin.
    public var viterbiMargin: Double
    /// The puncturing phase.
    public var puncturePhase: Int64?
    /// RS blocks handled: those without error, those corrected and those that failed.
    public var rsBlocks: Int64
    /// RS blocks without error.
    public var rsClean: Int64
    /// RS blocks with 1 to 8 bytes wrong, which were corrected. Their corrected bits are the BER after Viterbi.
    public var rsCorrected: Int64
    /// RS blocks that could not be corrected (more than 8 bytes wrong). Each gives a packet with the transport_error_indicator set. They do not enter the BER after Viterbi.
    public var rsFailed: Int64
    /// Bytes that were corrected in the RS blocks corrected.
    public var rsBytesCorrected: Int64
    /// Transport packets delivered, with those of the blocks that failed.
    public var packets: Int64
    /// The packet alignment holds.
    public var fecSync: Bool
    /// Resynchronisations.
    public var resyncs: Int64
    /// The signal time since the last good TPS frame. No value before the first good frame.
    public var sinceTps: Double?
    /// The quality score, 0 to 100.
    public var qualityPct: Double
    /// The quality class.
    public var qualityLabel: QualityLabel?
    /// The C/N that the constellation and code rate need.
    public var requiredCnDb: Double?
    /// The SNR above the required C/N.
    public var marginDb: Double?
    /// The features that TPS signals and the receiver does not decode.
    public var unsupported: String?
    /// The channel estimate is filled.
    public var dataValid: Bool
    /// The signal time of the first good packet.
    public var firstPacket: Double?
    /// The level of the input. No value before it is measured.
    public var levelDbfs: Double?

    init(_ c: sdrkit_dvbt_fields) {
        detectLevel = c.detect_level
        channelWidth = c.channel_width
        bandwidthAuto = c.bandwidth_auto
        bandwidthFrom = c.has_bandwidth_from ? BandwidthFrom(rawValue: c.bandwidth_from) : nil
        measuredWidth = c.has_measured_width ? c.measured_width : nil
        occupancyDb = c.has_occupancy_db ? c.occupancy_db : nil
        inputRate = c.input_rate
        naturalRate = c.natural_rate
        resampling = c.resampling
        rateOk = c.rate_ok
        fft = c.has_fft ? Fft(rawValue: c.fft) : nil
        fftSize = c.has_fft_size ? c.fft_size : nil
        `guard` = c.has_guard ? Guard(rawValue: c.guard) : nil
        guardSamples = c.has_guard_samples ? c.guard_samples : nil
        carriers = c.has_carriers ? c.carriers : nil
        constellation = c.has_constellation ? Constellation(rawValue: c.constellation) : nil
        hierarchy = c.has_hierarchy ? Hierarchy(rawValue: c.hierarchy) : nil
        codeRateHp = c.has_code_rate_hp ? CodeRateHp(rawValue: c.code_rate_hp) : nil
        codeRateLp = c.has_code_rate_lp ? CodeRateLp(rawValue: c.code_rate_lp) : nil
        stream = Stream(rawValue: c.stream)
        cellId = c.has_cell_id ? c.cell_id : nil
        tpsLength = c.has_tps_length ? c.tps_length : nil
        dvbhFlags = c.has_dvbh_flags ? c.dvbh_flags : nil
        frame = c.has_frame ? c.frame : nil
        symbol = c.has_symbol ? c.symbol : nil
        frameDuration = c.has_frame_duration ? c.frame_duration : nil
        netBitrateBps = c.has_net_bitrate_bps ? c.net_bitrate_bps : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        integerOffset = c.has_integer_offset ? c.integer_offset : nil
        locatorHz = c.locator_hz
        clockPpm = c.clock_ppm
        spectrumInverted = c.spectrum_inverted
        snrDb = c.has_snr_db ? c.snr_db : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        berPreViterbi = c.ber_pre_viterbi
        berPostViterbi = c.ber_post_viterbi
        viterbiMargin = c.viterbi_margin
        puncturePhase = c.has_puncture_phase ? c.puncture_phase : nil
        rsBlocks = c.rs_blocks
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        rsBytesCorrected = c.rs_bytes_corrected
        packets = c.packets
        fecSync = c.fec_sync
        resyncs = c.resyncs
        sinceTps = c.has_since_tps ? c.since_tps : nil
        qualityPct = c.quality_pct
        qualityLabel = QualityLabel(rawValue: c.quality_label)
        requiredCnDb = c.has_required_cn_db ? c.required_cn_db : nil
        marginDb = c.has_margin_db ? c.margin_db : nil
        unsupported = c.has_unsupported ? sdrkitText(c.unsupported) : nil
        dataValid = c.data_valid
        firstPacket = c.has_first_packet ? c.first_packet : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbtFields, SDRKitCode> {
        var c = sdrkit_dvbt_fields()
        let code = sdrkit_dvbt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the transport stream layer that `dvbt` shows under its own keys: the network and the time of the stream (`psi_si`), and the service that plays (`ts_demux`).
public struct DvbtMultiplexFields {
    public enum PlayerStatus: Int32 {
        case starting = 0
        case playing = 1
        case scrambled = 2
        case noService = 3
    }
    /// The network name.
    public var networkName: String
    /// network_id.
    public var networkId: Int64
    /// The number of services.
    public var serviceCount: Int64
    /// The UTC of the stream. No value before the stream gave the time.
    public var utc: Double?
    /// The service that plays. No value while none plays.
    public var selectedService: Int64?
    /// What the player does.
    public var playerStatus: PlayerStatus?
    /// The bit rate of the multiplex.
    public var muxKbps: Double

    init(_ c: sdrkit_dvbt_multiplex_fields) {
        networkName = sdrkitText(c.network_name)
        networkId = c.network_id
        serviceCount = c.service_count
        utc = c.has_utc ? c.utc : nil
        selectedService = c.has_selected_service ? c.selected_service : nil
        playerStatus = PlayerStatus(rawValue: c.player_status)
        muxKbps = c.mux_kbps
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbtMultiplexFields, SDRKitCode> {
        var c = sdrkit_dvbt_multiplex_fields()
        let code = sdrkit_dvbt_multiplex_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtMultiplexFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the sender `transmit` of `dvbt`.
public struct DvbtTxFields {
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the centre of the channel sent. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// The bit rate of the transport stream that the settings carry (of the high-priority stream in a hierarchical multiplex): the rate that the host feeds at most.
    public var txBitrateBps: Double
    /// The bit rate of the low-priority stream. No value in a multiplex that is not hierarchical.
    public var txBitrateLpBps: Double?
    /// The packets of the host sent in this transmission, or in the last one.
    public var txPackets: Int64
    /// The null packets put in slots that found no packet of the host.
    public var txNulls: Int64
    /// The packets that wait to be sent.
    public var txQueue: Int64
    /// The packets of the low-priority stream that wait to be sent.
    public var txQueueLp: Int64
    /// The time that the waiting packets take at the useful bit rate.
    public var txQueueS: Double
    /// The PCRs rewritten to the time of their slot.
    public var txPcrRestamped: Int64

    init(_ c: sdrkit_dvbt_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txBitrateBps = c.tx_bitrate_bps
        txBitrateLpBps = c.has_tx_bitrate_lp_bps ? c.tx_bitrate_lp_bps : nil
        txPackets = c.tx_packets
        txNulls = c.tx_nulls
        txQueue = c.tx_queue
        txQueueLp = c.tx_queue_lp
        txQueueS = c.tx_queue_s
        txPcrRestamped = c.tx_pcr_restamped
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxFields, SDRKitCode> {
        var c = sdrkit_dvbt_tx_fields()
        let code = sdrkit_dvbt_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the synchroniser `sync`. No value before the FFT mode and the guard interval are found.
public struct DvbtSyncFields {
    /// The timing error of the last symbol.
    public var timing: Double?
    /// The correlation coefficient of the last symbol.
    public var rho: Double?
    /// The fractional carrier offset in use.
    public var frac: Double?

    init(_ c: sdrkit_dvbt_sync_fields) {
        timing = c.has_timing ? c.timing : nil
        rho = c.has_rho ? c.rho : nil
        frac = c.has_frac ? c.frac : nil
    }
}

/// The result fields of the FFT stage `fft`.
public struct DvbtFftFields {
    /// The integer carrier offset. No value before it is found.
    public var integerOffset: Int64?
    /// The spectrum is mirrored.
    public var spectrumInverted: Bool

    init(_ c: sdrkit_dvbt_fft_fields) {
        integerOffset = c.has_integer_offset ? c.integer_offset : nil
        spectrumInverted = c.spectrum_inverted
    }
}

/// The result fields of the TPS decoder `tps`.
public struct DvbtTpsFields {
    /// TPS frames decoded.
    public var tpsFramesOk: Int64
    /// TPS frames that failed.
    public var tpsFramesFailed: Int64

    init(_ c: sdrkit_dvbt_tps_fields) {
        tpsFramesOk = c.tps_frames_ok
        tpsFramesFailed = c.tps_frames_failed
    }
}

/// The result fields of the equaliser `eq`.
public struct DvbtEqFields {
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled.
    public var snrDb: Double?
    /// The modulation error ratio. No value before the channel estimate is filled.
    public var merDb: Double?
    /// The time-averaging gain of the channel estimate.
    public var alpha: Double
    /// The centre of the delay window.
    public var delayWindowCentre: Double
    /// The half width of the delay window.
    public var delayWindowHalf: Double
    /// The measured delay spread.
    public var delaySpread: Double

    init(_ c: sdrkit_dvbt_eq_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        alpha = c.alpha
        delayWindowCentre = c.delay_window_centre
        delayWindowHalf = c.delay_window_half
        delaySpread = c.delay_spread
    }
}

/// The result fields of the error correction `fec`.
public struct DvbtFecFields {
    /// Transport packets delivered, with those of the blocks that failed.
    public var framesOk: Int64
    /// Reed-Solomon blocks that could not be corrected. Each gives a packet with the transport_error_indicator set.
    public var framesBad: Int64

    init(_ c: sdrkit_dvbt_fec_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
    }
}

/// The result set that the receiver of `dvb_terrestrial` adds to the results of the DVB-T receiver.
public struct DvbTerrestrialFields {
    public enum Standard: Int32 {
        case dvbt2 = 0
        case dvbt = 1
    }
    /// The standard that decodes now.
    public var standard: Standard?

    init(_ c: sdrkit_dvb_terrestrial_fields) {
        standard = Standard(rawValue: c.standard)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialFields, SDRKitCode> {
        var c = sdrkit_dvb_terrestrial_fields()
        let code = sdrkit_dvb_terrestrial_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbTerrestrialFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `dvbsh`, which `data` writes: the OFDM parameters, the SH frame and the turbo code. A value of the TPS has no value while the TPS is not locked.
public struct DvbshFields {
    public enum Waveform: Int32 {
        case ofdm = 0
        case tdm = 1
    }
    /// OFDM or TDM.
    public var waveform: Waveform?
    /// The channel width in use.
    public var widthMhz: Double
    /// 0 searching, 1 OFDM symbols found, 2 TPS, 3 parameters, 4 transport stream flowing.
    public var lock: Int64
    /// The carrier offset. No value before the FFT mode is found.
    public var cfoHz: Double?
    /// The FFT size.
    public var fftSize: Int64?
    /// The guard samples.
    public var guardSamples: Int64?
    /// The OFDM mode from TPS.
    public var mode: String?
    /// The guard interval from TPS.
    public var `guard`: String?
    /// The constellation from TPS.
    public var constellation: String?
    /// The hierarchy from TPS.
    public var hierarchy: String?
    /// The code rate of the high-priority stream ("1/3", "1/3c" for complementary).
    public var rateHp: String?
    /// The code rate of the low-priority stream of a hierarchical multiplex.
    public var rateLp: String?
    /// The stream decoded, HP or LP.
    public var stream: String
    /// The interleaver profile, "cm / lt / ns / sd / nli".
    public var interleaver: String?
    /// The length of the time interleaver.
    public var interleaverS: Double?
    /// The memory that the time de-interleaver holds.
    public var interleaverMb: Double?
    /// Why the data path does not run (an interleaver too long for this receiver, a reserved pattern). No value while it runs.
    public var refused: String?
    /// The 16-bit cell id from TPS.
    public var cellId: Int64?
    /// The super-frame number in the SH frame.
    public var superframe: Int64?
    /// Code words decoded.
    public var codewordsOk: Int64
    /// Code words not decoded.
    public var codewordsBad: Int64
    /// Packets with a good CRC.
    public var packetsOk: Int64
    /// Packets passed with the transport_error_indicator.
    public var packetsBad: Int64
    /// The mean turbo iterations of the decoded code words. No value before a code word decodes.
    public var turboIterations: Double?
    /// The signal time of the first good packet.
    public var firstPacketS: Double?
    /// What the stack decodes and which of its values no public document confirms.
    public var note: String
    /// The error of the sample clock.
    public var clockPpm: Double
    /// The level of the input. No value before it is measured.
    public var levelDbfs: Double?
    /// The modulation of the TDM form, which the SOF shows.
    public var tdmModulation: String?
    /// The symbol rate of the TDM form that is tried or received.
    public var tdmSymbolRate: Double?
    /// The roll-off of the TDM form, from the row of the symbol rate. No value before slots are found.
    public var tdmRolloff: Double?
    /// The width of the TDM SH frame from its signalling field.
    public var tdmFrameWidthCu: Int64?
    /// The power of a pilot symbol of the TDM form over the noise in the symbol rate.
    public var tdmMerDb: Double?
    /// Slots of the TDM form erased for want of pilots.
    public var tdmSlotsErased: Int64
    /// Signalling fields of the TDM form that did not decode.
    public var tdmSignallingFailed: Int64

    init(_ c: sdrkit_dvbsh_fields) {
        waveform = Waveform(rawValue: c.waveform)
        widthMhz = c.width_mhz
        lock = c.lock
        cfoHz = c.has_cfo_hz ? c.cfo_hz : nil
        fftSize = c.has_fft_size ? c.fft_size : nil
        guardSamples = c.has_guard_samples ? c.guard_samples : nil
        mode = c.has_mode ? sdrkitText(c.mode) : nil
        `guard` = c.has_guard ? sdrkitText(c.guard) : nil
        constellation = c.has_constellation ? sdrkitText(c.constellation) : nil
        hierarchy = c.has_hierarchy ? sdrkitText(c.hierarchy) : nil
        rateHp = c.has_rate_hp ? sdrkitText(c.rate_hp) : nil
        rateLp = c.has_rate_lp ? sdrkitText(c.rate_lp) : nil
        stream = sdrkitText(c.stream)
        interleaver = c.has_interleaver ? sdrkitText(c.interleaver) : nil
        interleaverS = c.has_interleaver_s ? c.interleaver_s : nil
        interleaverMb = c.has_interleaver_mb ? c.interleaver_mb : nil
        refused = c.has_refused ? sdrkitText(c.refused) : nil
        cellId = c.has_cell_id ? c.cell_id : nil
        superframe = c.has_superframe ? c.superframe : nil
        codewordsOk = c.codewords_ok
        codewordsBad = c.codewords_bad
        packetsOk = c.packets_ok
        packetsBad = c.packets_bad
        turboIterations = c.has_turbo_iterations ? c.turbo_iterations : nil
        firstPacketS = c.has_first_packet_s ? c.first_packet_s : nil
        note = sdrkitText(c.note)
        clockPpm = c.clock_ppm
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        tdmModulation = c.has_tdm_modulation ? sdrkitText(c.tdm_modulation) : nil
        tdmSymbolRate = c.has_tdm_symbol_rate ? c.tdm_symbol_rate : nil
        tdmRolloff = c.has_tdm_rolloff ? c.tdm_rolloff : nil
        tdmFrameWidthCu = c.has_tdm_frame_width_cu ? c.tdm_frame_width_cu : nil
        tdmMerDb = c.has_tdm_mer_db ? c.tdm_mer_db : nil
        tdmSlotsErased = c.tdm_slots_erased
        tdmSignallingFailed = c.tdm_signalling_failed
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbshFields, SDRKitCode> {
        var c = sdrkit_dvbsh_fields()
        let code = sdrkit_dvbsh_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbshFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the OFDM chain that `dvbsh` shows under its own keys: the SNR of the equaliser and the counts of the TPS decoder.
public struct DvbshChainFields {
    /// The power of a data cell over the noise of a carrier, which the continual pilots give. No value before the channel estimate is filled.
    public var snrDb: Double?
    /// TPS frames decoded.
    public var tpsFramesOk: Int64
    /// TPS frames that failed.
    public var tpsFramesFailed: Int64

    init(_ c: sdrkit_dvbsh_chain_fields) {
        snrDb = c.has_snr_db ? c.snr_db : nil
        tpsFramesOk = c.tps_frames_ok
        tpsFramesFailed = c.tps_frames_failed
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbshChainFields, SDRKitCode> {
        var c = sdrkit_dvbsh_chain_fields()
        let code = sdrkit_dvbsh_chain_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbshChainFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum DvbtComponents {
    /// The result fields of the component "sync" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtSyncFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtSyncFields, SDRKitCode> {
        var c = sdrkit_dvbt_sync_fields()
        let code = sdrkit_dvbt_sync_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtSyncFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "fft" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtFftFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtFftFields, SDRKitCode> {
        var c = sdrkit_dvbt_fft_fields()
        let code = sdrkit_dvbt_fft_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFftFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "tps" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtTpsFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtTpsFields, SDRKitCode> {
        var c = sdrkit_dvbt_tps_fields()
        let code = sdrkit_dvbt_tps_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtTpsFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "eq" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtEqFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtEqFields, SDRKitCode> {
        var c = sdrkit_dvbt_eq_fields()
        let code = sdrkit_dvbt_eq_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtEqFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "fec" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtFecFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtFecFields, SDRKitCode> {
        var c = sdrkit_dvbt_fec_fields()
        let code = sdrkit_dvbt_fec_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFecFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbt_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbt_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtTeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvbt_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvbt" in the snapshot of `handle`.
    public static func dvbtDvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvbt_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "sync" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialSyncFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtSyncFields, SDRKitCode> {
        var c = sdrkit_dvbt_sync_fields()
        let code = sdrkit_dvb_terrestrial_sync_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtSyncFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "fft" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialFftFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtFftFields, SDRKitCode> {
        var c = sdrkit_dvbt_fft_fields()
        let code = sdrkit_dvb_terrestrial_fft_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFftFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "tps" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialTpsFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtTpsFields, SDRKitCode> {
        var c = sdrkit_dvbt_tps_fields()
        let code = sdrkit_dvb_terrestrial_tps_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtTpsFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "eq" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialEqFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtEqFields, SDRKitCode> {
        var c = sdrkit_dvbt_eq_fields()
        let code = sdrkit_dvb_terrestrial_eq_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtEqFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "fec" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialFecFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtFecFields, SDRKitCode> {
        var c = sdrkit_dvbt_fec_fields()
        let code = sdrkit_dvb_terrestrial_fec_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFecFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvb_terrestrial_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvb_terrestrial_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialTeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvb_terrestrial_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvb_terrestrial" in the snapshot of `handle`.
    public static func dvbTerrestrialDvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvb_terrestrial_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "sync" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshSyncFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtSyncFields, SDRKitCode> {
        var c = sdrkit_dvbt_sync_fields()
        let code = sdrkit_dvbsh_sync_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtSyncFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "fft" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshFftFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtFftFields, SDRKitCode> {
        var c = sdrkit_dvbt_fft_fields()
        let code = sdrkit_dvbsh_fft_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtFftFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "tps" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshTpsFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtTpsFields, SDRKitCode> {
        var c = sdrkit_dvbt_tps_fields()
        let code = sdrkit_dvbsh_tps_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtTpsFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "eq" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshEqFields(_ handle: SDRKIT_HANDLE) -> Result<DvbtEqFields, SDRKitCode> {
        var c = sdrkit_dvbt_eq_fields()
        let code = sdrkit_dvbsh_eq_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbtEqFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbsh_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbsh" in the snapshot of `handle`.
    public static func dvbshPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbsh_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum DvbtTables {
    /// The changed rows of the table "services" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbt", newest first, at most `capacity` rows.
    public static func dvbtPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbt_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvb_terrestrial", newest first, at most `capacity` rows.
    public static func dvbTerrestrialPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvb_terrestrial_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbsh", newest first, at most `capacity` rows.
    public static func dvbshPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbsh_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum DvbtCommands {
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dvbtServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a teletext page. Shows a teletext page.
    public static func dvbtTeletextSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_teletext_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_teletext_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt_teletext_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func dvbtTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbt_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Start sending. Sends a DVB-T signal with the tx_* settings from the start of a super-frame, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func dvbtStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbt_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission after the frame on air with a fall of 5 ms. The packets that wait are dropped.
    public static func dvbtStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbt_stop(handle)
        return sdrkitResult(code)
    }
    /// Add packets. Adds transport packets to the queue of the stream, or of the high-priority stream of a hierarchical multiplex. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
    public static func dvbtAppend(_ handle: SDRKIT_HANDLE, packets: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_tx_append_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_tx_append_args>.size)
        let code: Int32 = packets.withUnsafeBufferPointer { packets_ in { () -> Int32 in
        args.packets = sdrkit_bytes(ptr: packets_.baseAddress, len: packets_.count)
        return sdrkit_dvbt_append(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Add packets (LP). Adds transport packets to the queue of the low-priority stream of a hierarchical multiplex. All or none.
    public static func dvbtAppendLp(_ handle: SDRKIT_HANDLE, packets: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_tx_append_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_tx_append_args>.size)
        let code: Int32 = packets.withUnsafeBufferPointer { packets_ in { () -> Int32 in
        args.packets = sdrkit_bytes(ptr: packets_.baseAddress, len: packets_.count)
        return sdrkit_dvbt_append_lp(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dvbTerrestrialServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvb_terrestrial_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a teletext page. Shows a teletext page.
    public static func dvbTerrestrialTeletextSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_teletext_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_teletext_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvb_terrestrial_teletext_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func dvbTerrestrialTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvb_terrestrial_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dvbshServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbsh_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func dvbshTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbt_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbt_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dvbsh_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum DvbtSettings {
    /// The options of the setting "bandwidth" of the stack "dvbt".
    public enum DvbtBandwidth: Int32 {
        case auto = 0
        case n8Mhz = 1
        case n7Mhz = 2
        case n6Mhz = 3
        case n5Mhz = 4
    }
    /// Channel bandwidth.
    public static func dvbtBandwidth(_ handle: SDRKIT_HANDLE) -> Result<DvbtBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(DvbtBandwidth(rawValue: v) ?? DvbtBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtBandwidth(_ handle: SDRKIT_HANDLE, _ value: DvbtBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// The options of the setting "stream" of the stack "dvbt".
    public enum DvbtStream: Int32 {
        case hp = 0
        case lp = 1
    }
    /// Hierarchical stream.
    public static func dvbtStream(_ handle: SDRKIT_HANDLE) -> Result<DvbtStream, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stream", &v)
        return code == SDRKIT_OK ? .success(DvbtStream(rawValue: v) ?? DvbtStream(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtStream(_ handle: SDRKIT_HANDLE, _ value: DvbtStream) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stream", value.rawValue))
    }
    /// The options of the setting "viterbi" of the stack "dvbt".
    public enum DvbtViterbi: Int32 {
        case auto = 0
        case exact = 1
        case fast = 2
    }
    /// Viterbi decoder.
    public static func dvbtViterbi(_ handle: SDRKIT_HANDLE) -> Result<DvbtViterbi, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "viterbi", &v)
        return code == SDRKIT_OK ? .success(DvbtViterbi(rawValue: v) ?? DvbtViterbi(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtViterbi(_ handle: SDRKIT_HANDLE, _ value: DvbtViterbi) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "viterbi", value.rawValue))
    }
    /// Time interpolation of pilots.
    public static func dvbtTimeInterp(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "time_interp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTimeInterp(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "time_interp", value))
    }
    /// Narrowband interferer weighting.
    public static func dvbtInterfererErase(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "interferer_erase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtInterfererErase(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "interferer_erase", value))
    }
    /// The options of the setting "standard" of the stack "dvbt".
    public enum DvbtStandard: Int32 {
        case auto = 0
        case dvbt = 1
    }
    /// Standard.
    public static func dvbtStandard(_ handle: SDRKIT_HANDLE) -> Result<DvbtStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "standard", &v)
        return code == SDRKIT_OK ? .success(DvbtStandard(rawValue: v) ?? DvbtStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtStandard(_ handle: SDRKIT_HANDLE, _ value: DvbtStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "standard", value.rawValue))
    }
    /// Channels.
    public static func dvbtChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Audio language.
    public static func dvbtAudioLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "audio_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtAudioLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "audio_lang", $0) })
    }
    /// Audio description.
    public static func dvbtAudioDescription(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_description", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtAudioDescription(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_description", value))
    }
    /// Stereo downmix.
    public static func dvbtDownmix(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "downmix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtDownmix(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "downmix", value))
    }
    /// Decode video.
    public static func dvbtDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Subtitle language.
    public static func dvbtSubtitleLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "subtitle_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtSubtitleLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "subtitle_lang", $0) })
    }
    /// Subtitles for the hard of hearing.
    public static func dvbtHardOfHearing(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hard_of_hearing", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtHardOfHearing(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hard_of_hearing", value))
    }
    /// Country.
    public static func dvbtCountry(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "country", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtCountry(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "country", $0) })
    }
    /// Guide language.
    public static func dvbtGuideLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "guide_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtGuideLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "guide_lang", $0) })
    }
    /// Flat noise weighting.
    public static func dvbtTestFlatCsi(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_flat_csi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTestFlatCsi(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_flat_csi", value))
    }
    /// Full delay window.
    public static func dvbtTestFullWindow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_full_window", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTestFullWindow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_full_window", value))
    }
    /// Window from the correlation peak.
    public static func dvbtTestPeakOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_peak_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTestPeakOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_peak_only", value))
    }
    /// The options of the setting "tx_bandwidth" of the stack "dvbt".
    public enum DvbtTxBandwidth: Int32 {
        case n5Mhz = 0
        case n6Mhz = 1
        case n7Mhz = 2
        case n8Mhz = 3
    }
    /// Channel width.
    public static func dvbtTxBandwidth(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_bandwidth", &v)
        return code == SDRKIT_OK ? .success(DvbtTxBandwidth(rawValue: v) ?? DvbtTxBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxBandwidth(_ handle: SDRKIT_HANDLE, _ value: DvbtTxBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_bandwidth", value.rawValue))
    }
    /// The options of the setting "tx_mode" of the stack "dvbt".
    public enum DvbtTxMode: Int32 {
        case n2K = 0
        case n8K = 1
    }
    /// FFT mode.
    public static func dvbtTxMode(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_mode", &v)
        return code == SDRKIT_OK ? .success(DvbtTxMode(rawValue: v) ?? DvbtTxMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxMode(_ handle: SDRKIT_HANDLE, _ value: DvbtTxMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_mode", value.rawValue))
    }
    /// The options of the setting "tx_guard" of the stack "dvbt".
    public enum DvbtTxGuard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
    }
    /// Guard interval.
    public static func dvbtTxGuard(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_guard", &v)
        return code == SDRKIT_OK ? .success(DvbtTxGuard(rawValue: v) ?? DvbtTxGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxGuard(_ handle: SDRKIT_HANDLE, _ value: DvbtTxGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_guard", value.rawValue))
    }
    /// The options of the setting "tx_constellation" of the stack "dvbt".
    public enum DvbtTxConstellation: Int32 {
        case qPSK = 0
        case n16QAM = 1
        case n64QAM = 2
    }
    /// Constellation.
    public static func dvbtTxConstellation(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxConstellation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_constellation", &v)
        return code == SDRKIT_OK ? .success(DvbtTxConstellation(rawValue: v) ?? DvbtTxConstellation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxConstellation(_ handle: SDRKIT_HANDLE, _ value: DvbtTxConstellation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_constellation", value.rawValue))
    }
    /// The options of the setting "tx_hierarchy" of the stack "dvbt".
    public enum DvbtTxHierarchy: Int32 {
        case none = 0
        case n1 = 1
        case n2 = 2
        case n4 = 3
    }
    /// Hierarchy.
    public static func dvbtTxHierarchy(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxHierarchy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_hierarchy", &v)
        return code == SDRKIT_OK ? .success(DvbtTxHierarchy(rawValue: v) ?? DvbtTxHierarchy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxHierarchy(_ handle: SDRKIT_HANDLE, _ value: DvbtTxHierarchy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_hierarchy", value.rawValue))
    }
    /// The options of the setting "tx_rate_hp" of the stack "dvbt".
    public enum DvbtTxRateHp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// Code rate.
    public static func dvbtTxRateHp(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxRateHp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rate_hp", &v)
        return code == SDRKIT_OK ? .success(DvbtTxRateHp(rawValue: v) ?? DvbtTxRateHp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxRateHp(_ handle: SDRKIT_HANDLE, _ value: DvbtTxRateHp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rate_hp", value.rawValue))
    }
    /// The options of the setting "tx_rate_lp" of the stack "dvbt".
    public enum DvbtTxRateLp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// Code rate (LP).
    public static func dvbtTxRateLp(_ handle: SDRKIT_HANDLE) -> Result<DvbtTxRateLp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rate_lp", &v)
        return code == SDRKIT_OK ? .success(DvbtTxRateLp(rawValue: v) ?? DvbtTxRateLp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxRateLp(_ handle: SDRKIT_HANDLE, _ value: DvbtTxRateLp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rate_lp", value.rawValue))
    }
    /// Cell id.
    public static func dvbtTxCellId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_cell_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtTxCellId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_cell_id", value))
    }
    /// The options of the setting "bandwidth" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialBandwidth: Int32 {
        case auto = 0
        case n8Mhz = 1
        case n7Mhz = 2
        case n6Mhz = 3
        case n5Mhz = 4
    }
    /// Channel bandwidth.
    public static func dvbTerrestrialBandwidth(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "bandwidth", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialBandwidth(rawValue: v) ?? DvbTerrestrialBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialBandwidth(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "bandwidth", value.rawValue))
    }
    /// The options of the setting "stream" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialStream: Int32 {
        case hp = 0
        case lp = 1
    }
    /// Hierarchical stream.
    public static func dvbTerrestrialStream(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialStream, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "stream", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialStream(rawValue: v) ?? DvbTerrestrialStream(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialStream(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialStream) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "stream", value.rawValue))
    }
    /// The options of the setting "viterbi" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialViterbi: Int32 {
        case auto = 0
        case exact = 1
        case fast = 2
    }
    /// Viterbi decoder.
    public static func dvbTerrestrialViterbi(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialViterbi, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "viterbi", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialViterbi(rawValue: v) ?? DvbTerrestrialViterbi(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialViterbi(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialViterbi) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "viterbi", value.rawValue))
    }
    /// Time interpolation of pilots.
    public static func dvbTerrestrialTimeInterp(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "time_interp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialTimeInterp(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "time_interp", value))
    }
    /// Narrowband interferer weighting.
    public static func dvbTerrestrialInterfererErase(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "interferer_erase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialInterfererErase(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "interferer_erase", value))
    }
    /// The options of the setting "standard" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialStandard: Int32 {
        case auto = 0
        case dvbt = 1
    }
    /// Standard.
    public static func dvbTerrestrialStandard(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "standard", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialStandard(rawValue: v) ?? DvbTerrestrialStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialStandard(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "standard", value.rawValue))
    }
    /// Channels.
    public static func dvbTerrestrialChannels(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialChannels(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels", $0) })
    }
    /// Audio language.
    public static func dvbTerrestrialAudioLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "audio_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialAudioLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "audio_lang", $0) })
    }
    /// Audio description.
    public static func dvbTerrestrialAudioDescription(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "audio_description", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialAudioDescription(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "audio_description", value))
    }
    /// Stereo downmix.
    public static func dvbTerrestrialDownmix(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "downmix", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialDownmix(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "downmix", value))
    }
    /// Decode video.
    public static func dvbTerrestrialDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Subtitle language.
    public static func dvbTerrestrialSubtitleLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "subtitle_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialSubtitleLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "subtitle_lang", $0) })
    }
    /// Subtitles for the hard of hearing.
    public static func dvbTerrestrialHardOfHearing(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hard_of_hearing", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialHardOfHearing(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hard_of_hearing", value))
    }
    /// Country.
    public static func dvbTerrestrialCountry(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "country", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialCountry(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "country", $0) })
    }
    /// Guide language.
    public static func dvbTerrestrialGuideLang(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "guide_lang", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialGuideLang(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "guide_lang", $0) })
    }
    /// Flat noise weighting.
    public static func dvbTerrestrialTestFlatCsi(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_flat_csi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialTestFlatCsi(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_flat_csi", value))
    }
    /// Full delay window.
    public static func dvbTerrestrialTestFullWindow(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_full_window", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialTestFullWindow(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_full_window", value))
    }
    /// Window from the correlation peak.
    public static func dvbTerrestrialTestPeakOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "test_peak_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialTestPeakOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "test_peak_only", value))
    }
    /// The options of the setting "width" of the stack "dvbsh".
    public enum DvbshWidth: Int32 {
        case auto = 0
        case n5Mhz = 1
        case n8Mhz = 2
        case n7Mhz = 3
        case n6Mhz = 4
        case n17 = 5
    }
    /// Channel width.
    public static func dvbshWidth(_ handle: SDRKIT_HANDLE) -> Result<DvbshWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "width", &v)
        return code == SDRKIT_OK ? .success(DvbshWidth(rawValue: v) ?? DvbshWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshWidth(_ handle: SDRKIT_HANDLE, _ value: DvbshWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "width", value.rawValue))
    }
    /// Low-priority stream.
    public static func dvbshLowPriority(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "low_priority", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshLowPriority(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "low_priority", value))
    }
    /// Time de-interleaver memory limit.
    public static func dvbshMaxInterleaverMb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "max_interleaver_mb", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshMaxInterleaverMb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "max_interleaver_mb", value))
    }
    /// Turbo iterations.
    public static func dvbshTurboIterations(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "turbo_iterations", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTurboIterations(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "turbo_iterations", value))
    }
    /// Decode video.
    public static func dvbshDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// The options of the setting "waveform" of the stack "dvbsh".
    public enum DvbshWaveform: Int32 {
        case auto = 0
        case ofdm = 1
        case tdm = 2
    }
    /// Waveform.
    public static func dvbshWaveform(_ handle: SDRKIT_HANDLE) -> Result<DvbshWaveform, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "waveform", &v)
        return code == SDRKIT_OK ? .success(DvbshWaveform(rawValue: v) ?? DvbshWaveform(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshWaveform(_ handle: SDRKIT_HANDLE, _ value: DvbshWaveform) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "waveform", value.rawValue))
    }
    /// TDM scrambling code number.
    public static func dvbshTdmGold(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tdm_gold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTdmGold(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tdm_gold", value))
    }
    /// TDM symbol rate.
    public static func dvbshTdmSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tdm_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTdmSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tdm_symbol_rate", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum DvbtGeneratorSettings {
    /// The options of the setting "gen_mode" of the stack "dvbt".
    public enum DvbtMode: Int32 {
        case n2K = 0
        case n8K = 1
    }
    /// Mode.
    public static func dvbtMode(_ handle: SDRKIT_HANDLE) -> Result<DvbtMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(DvbtMode(rawValue: v) ?? DvbtMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtMode(_ handle: SDRKIT_HANDLE, _ value: DvbtMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_guard" of the stack "dvbt".
    public enum DvbtGuard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
    }
    /// Guard interval.
    public static func dvbtGuard(_ handle: SDRKIT_HANDLE) -> Result<DvbtGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(DvbtGuard(rawValue: v) ?? DvbtGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtGuard(_ handle: SDRKIT_HANDLE, _ value: DvbtGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_constellation" of the stack "dvbt".
    public enum DvbtConstellation: Int32 {
        case qPSK = 0
        case n16QAM = 1
        case n64QAM = 2
    }
    /// Constellation.
    public static func dvbtConstellation(_ handle: SDRKIT_HANDLE) -> Result<DvbtConstellation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &v)
        return code == SDRKIT_OK ? .success(DvbtConstellation(rawValue: v) ?? DvbtConstellation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtConstellation(_ handle: SDRKIT_HANDLE, _ value: DvbtConstellation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_constellation", value.rawValue))
    }
    /// The options of the setting "gen_hierarchy" of the stack "dvbt".
    public enum DvbtHierarchy: Int32 {
        case none = 0
        case n1 = 1
        case n2 = 2
        case n4 = 3
    }
    /// Hierarchy.
    public static func dvbtHierarchy(_ handle: SDRKIT_HANDLE) -> Result<DvbtHierarchy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &v)
        return code == SDRKIT_OK ? .success(DvbtHierarchy(rawValue: v) ?? DvbtHierarchy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtHierarchy(_ handle: SDRKIT_HANDLE, _ value: DvbtHierarchy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", value.rawValue))
    }
    /// The options of the setting "gen_rate_hp" of the stack "dvbt".
    public enum DvbtRateHp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// HP code rate.
    public static func dvbtRateHp(_ handle: SDRKIT_HANDLE) -> Result<DvbtRateHp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate_hp", &v)
        return code == SDRKIT_OK ? .success(DvbtRateHp(rawValue: v) ?? DvbtRateHp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtRateHp(_ handle: SDRKIT_HANDLE, _ value: DvbtRateHp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate_hp", value.rawValue))
    }
    /// The options of the setting "gen_rate_lp" of the stack "dvbt".
    public enum DvbtRateLp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// LP code rate.
    public static func dvbtRateLp(_ handle: SDRKIT_HANDLE) -> Result<DvbtRateLp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &v)
        return code == SDRKIT_OK ? .success(DvbtRateLp(rawValue: v) ?? DvbtRateLp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtRateLp(_ handle: SDRKIT_HANDLE, _ value: DvbtRateLp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", value.rawValue))
    }
    /// The options of the setting "gen_bandwidth" of the stack "dvbt".
    public enum DvbtBandwidth: Int32 {
        case auto = 0
        case n8Mhz = 1
        case n7Mhz = 2
        case n6Mhz = 3
        case n5Mhz = 4
    }
    /// Bandwidth.
    public static func dvbtBandwidth(_ handle: SDRKIT_HANDLE) -> Result<DvbtBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bandwidth", &v)
        return code == SDRKIT_OK ? .success(DvbtBandwidth(rawValue: v) ?? DvbtBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtBandwidth(_ handle: SDRKIT_HANDLE, _ value: DvbtBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bandwidth", value.rawValue))
    }
    /// Cell id.
    public static func dvbtCellId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_cell_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtCellId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_cell_id", value))
    }
    /// The options of the setting "gen_length" of the stack "dvbt".
    public enum DvbtLength: Int32 {
        case n23 = 0
        case n31 = 1
    }
    /// TPS length indicator.
    public static func dvbtLength(_ handle: SDRKIT_HANDLE) -> Result<DvbtLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_length", &v)
        return code == SDRKIT_OK ? .success(DvbtLength(rawValue: v) ?? DvbtLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtLength(_ handle: SDRKIT_HANDLE, _ value: DvbtLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_length", value.rawValue))
    }
    /// Puncturing start phase.
    public static func dvbtPhase(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_phase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtPhase(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_phase", value))
    }
    /// Service id.
    public static func dvbtService(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_service", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtService(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_service", value))
    }
    /// Service name.
    public static func dvbtName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_name", $0) })
    }
    /// The options of the setting "gen_source" of the stack "dvbt".
    public enum DvbtSource: Int32 {
        case builtin = 0
        case pattern = 1
        case null = 2
    }
    /// Packets.
    public static func dvbtSource(_ handle: SDRKIT_HANDLE) -> Result<DvbtSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_source", &v)
        return code == SDRKIT_OK ? .success(DvbtSource(rawValue: v) ?? DvbtSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbtSource(_ handle: SDRKIT_HANDLE, _ value: DvbtSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_source", value.rawValue))
    }
    /// The options of the setting "gen_mode" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialMode: Int32 {
        case n2K = 0
        case n8K = 1
    }
    /// Mode.
    public static func dvbTerrestrialMode(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialMode(rawValue: v) ?? DvbTerrestrialMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialMode(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_guard" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialGuard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
    }
    /// Guard interval.
    public static func dvbTerrestrialGuard(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialGuard(rawValue: v) ?? DvbTerrestrialGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialGuard(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_constellation" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialConstellation: Int32 {
        case qPSK = 0
        case n16QAM = 1
        case n64QAM = 2
    }
    /// Constellation.
    public static func dvbTerrestrialConstellation(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialConstellation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialConstellation(rawValue: v) ?? DvbTerrestrialConstellation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialConstellation(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialConstellation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_constellation", value.rawValue))
    }
    /// The options of the setting "gen_hierarchy" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialHierarchy: Int32 {
        case none = 0
        case n1 = 1
        case n2 = 2
        case n4 = 3
    }
    /// Hierarchy.
    public static func dvbTerrestrialHierarchy(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialHierarchy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialHierarchy(rawValue: v) ?? DvbTerrestrialHierarchy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialHierarchy(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialHierarchy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", value.rawValue))
    }
    /// The options of the setting "gen_rate_hp" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialRateHp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// HP code rate.
    public static func dvbTerrestrialRateHp(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialRateHp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate_hp", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialRateHp(rawValue: v) ?? DvbTerrestrialRateHp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialRateHp(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialRateHp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate_hp", value.rawValue))
    }
    /// The options of the setting "gen_rate_lp" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialRateLp: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// LP code rate.
    public static func dvbTerrestrialRateLp(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialRateLp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialRateLp(rawValue: v) ?? DvbTerrestrialRateLp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialRateLp(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialRateLp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", value.rawValue))
    }
    /// The options of the setting "gen_bandwidth" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialBandwidth: Int32 {
        case auto = 0
        case n8Mhz = 1
        case n7Mhz = 2
        case n6Mhz = 3
        case n5Mhz = 4
    }
    /// Bandwidth.
    public static func dvbTerrestrialBandwidth(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialBandwidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_bandwidth", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialBandwidth(rawValue: v) ?? DvbTerrestrialBandwidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialBandwidth(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialBandwidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_bandwidth", value.rawValue))
    }
    /// Cell id.
    public static func dvbTerrestrialCellId(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_cell_id", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialCellId(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_cell_id", value))
    }
    /// The options of the setting "gen_length" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialLength: Int32 {
        case n23 = 0
        case n31 = 1
    }
    /// TPS length indicator.
    public static func dvbTerrestrialLength(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_length", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialLength(rawValue: v) ?? DvbTerrestrialLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialLength(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_length", value.rawValue))
    }
    /// Puncturing start phase.
    public static func dvbTerrestrialPhase(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_phase", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialPhase(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_phase", value))
    }
    /// Service id.
    public static func dvbTerrestrialService(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_service", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialService(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_service", value))
    }
    /// Service name.
    public static func dvbTerrestrialName(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_name", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialName(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_name", $0) })
    }
    /// The options of the setting "gen_source" of the stack "dvb_terrestrial".
    public enum DvbTerrestrialSource: Int32 {
        case builtin = 0
        case pattern = 1
        case null = 2
    }
    /// Packets.
    public static func dvbTerrestrialSource(_ handle: SDRKIT_HANDLE) -> Result<DvbTerrestrialSource, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_source", &v)
        return code == SDRKIT_OK ? .success(DvbTerrestrialSource(rawValue: v) ?? DvbTerrestrialSource(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbTerrestrialSource(_ handle: SDRKIT_HANDLE, _ value: DvbTerrestrialSource) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_source", value.rawValue))
    }
    /// The options of the setting "gen_width" of the stack "dvbsh".
    public enum DvbshWidth: Int32 {
        case auto = 0
        case n5Mhz = 1
        case n8Mhz = 2
        case n7Mhz = 3
        case n6Mhz = 4
        case n17 = 5
    }
    /// Test signal width.
    public static func dvbshWidth(_ handle: SDRKIT_HANDLE) -> Result<DvbshWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_width", &v)
        return code == SDRKIT_OK ? .success(DvbshWidth(rawValue: v) ?? DvbshWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshWidth(_ handle: SDRKIT_HANDLE, _ value: DvbshWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_width", value.rawValue))
    }
    /// The options of the setting "gen_mode" of the stack "dvbsh".
    public enum DvbshMode: Int32 {
        case n1k = 0
        case n2k = 1
        case n4k = 2
        case n8k = 3
    }
    /// Test signal FFT size.
    public static func dvbshMode(_ handle: SDRKIT_HANDLE) -> Result<DvbshMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(DvbshMode(rawValue: v) ?? DvbshMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshMode(_ handle: SDRKIT_HANDLE, _ value: DvbshMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_constellation" of the stack "dvbsh".
    public enum DvbshConstellation: Int32 {
        case qpsk = 0
        case n16qam = 1
    }
    /// Test signal constellation.
    public static func dvbshConstellation(_ handle: SDRKIT_HANDLE) -> Result<DvbshConstellation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_constellation", &v)
        return code == SDRKIT_OK ? .success(DvbshConstellation(rawValue: v) ?? DvbshConstellation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshConstellation(_ handle: SDRKIT_HANDLE, _ value: DvbshConstellation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_constellation", value.rawValue))
    }
    /// The options of the setting "gen_hierarchy" of the stack "dvbsh".
    public enum DvbshHierarchy: Int32 {
        case none = 0
        case n1 = 1
        case n2 = 2
        case n4 = 3
    }
    /// Test signal hierarchy.
    public static func dvbshHierarchy(_ handle: SDRKIT_HANDLE) -> Result<DvbshHierarchy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_hierarchy", &v)
        return code == SDRKIT_OK ? .success(DvbshHierarchy(rawValue: v) ?? DvbshHierarchy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshHierarchy(_ handle: SDRKIT_HANDLE, _ value: DvbshHierarchy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_hierarchy", value.rawValue))
    }
    /// The options of the setting "gen_guard" of the stack "dvbsh".
    public enum DvbshGuard: Int32 {
        case n132 = 0
        case n116 = 1
        case n18 = 2
        case n14 = 3
    }
    /// Test signal guard interval.
    public static func dvbshGuard(_ handle: SDRKIT_HANDLE) -> Result<DvbshGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(DvbshGuard(rawValue: v) ?? DvbshGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshGuard(_ handle: SDRKIT_HANDLE, _ value: DvbshGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_rate" of the stack "dvbsh".
    public enum DvbshRate: Int32 {
        case n15 = 0
        case n29 = 1
        case n14 = 2
        case n27 = 3
        case n13 = 4
        case n13c = 5
        case n25 = 6
        case n25c = 7
        case n12 = 8
        case n12c = 9
        case n23 = 10
        case n23c = 11
    }
    /// Test signal code rate.
    public static func dvbshRate(_ handle: SDRKIT_HANDLE) -> Result<DvbshRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(DvbshRate(rawValue: v) ?? DvbshRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshRate(_ handle: SDRKIT_HANDLE, _ value: DvbshRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// The options of the setting "gen_rate_lp" of the stack "dvbsh".
    public enum DvbshRateLp: Int32 {
        case n15 = 0
        case n29 = 1
        case n14 = 2
        case n27 = 3
        case n13 = 4
        case n13c = 5
        case n25 = 6
        case n25c = 7
        case n12 = 8
        case n12c = 9
        case n23 = 10
        case n23c = 11
    }
    /// Test signal LP code rate.
    public static func dvbshRateLp(_ handle: SDRKIT_HANDLE) -> Result<DvbshRateLp, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate_lp", &v)
        return code == SDRKIT_OK ? .success(DvbshRateLp(rawValue: v) ?? DvbshRateLp(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshRateLp(_ handle: SDRKIT_HANDLE, _ value: DvbshRateLp) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate_lp", value.rawValue))
    }
    /// Test signal common_multiplier.
    public static func dvbshTiCm(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ti_cm", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTiCm(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ti_cm", value))
    }
    /// Test signal nof_late_taps.
    public static func dvbshTiLt(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ti_lt", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTiLt(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ti_lt", value))
    }
    /// Test signal nof_slices.
    public static func dvbshTiNs(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ti_ns", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTiNs(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ti_ns", value))
    }
    /// Test signal slice_distance.
    public static func dvbshTiSd(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ti_sd", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTiSd(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ti_sd", value))
    }
    /// Test signal non_late_increment.
    public static func dvbshTiNli(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_ti_nli", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshTiNli(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_ti_nli", value))
    }
    /// The options of the setting "gen_waveform" of the stack "dvbsh".
    public enum DvbshWaveform: Int32 {
        case ofdm = 0
        case tdmQpsk = 1
        case tdm8psk = 2
        case tdm16apsk = 3
    }
    /// Test signal waveform.
    public static func dvbshWaveform(_ handle: SDRKIT_HANDLE) -> Result<DvbshWaveform, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_waveform", &v)
        return code == SDRKIT_OK ? .success(DvbshWaveform(rawValue: v) ?? DvbshWaveform(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshWaveform(_ handle: SDRKIT_HANDLE, _ value: DvbshWaveform) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_waveform", value.rawValue))
    }
    /// The options of the setting "gen_rolloff" of the stack "dvbsh".
    public enum DvbshRolloff: Int32 {
        case n015 = 0
        case n025 = 1
        case n035 = 2
    }
    /// Test signal TDM roll-off.
    public static func dvbshRolloff(_ handle: SDRKIT_HANDLE) -> Result<DvbshRolloff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(DvbshRolloff(rawValue: v) ?? DvbshRolloff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbshRolloff(_ handle: SDRKIT_HANDLE, _ value: DvbshRolloff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rolloff", value.rawValue))
    }
}
