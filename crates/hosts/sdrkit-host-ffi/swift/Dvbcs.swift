// sdrkit: the typed interface of the stack family "dvbcs", version 2.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum DvbcsInterface {
    public static let major = 2
    public static let minor = 0
}

/// The result fields of `satbeacon`: the carrier, the subcarrier, the bit clock and the frame learner. A measurement of the subcarrier has no value without a subcarrier.
public struct SatbeaconFields {
    public enum State: Int32 {
        case noCarrier = 0
        case carrierLocked = 1
        case clockLocked = 2
        case framesFound = 3
    }
    public enum Coding: Int32 {
        case nrz = 0
        case biphase = 1
    }
    public enum SyncWord: Int32 {
        case faf320 = 0
        case fe6b2840 = 1
        case eb90 = 2
        case n1acffc1d = 3
    }
    /// What the decoder does now.
    public var state: State?
    /// The signal time decoded.
    public var signalTime: Double
    /// The work rate of the carrier loop.
    public var workRate: Double
    /// The offset of the carrier from the centre.
    public var offsetHz: Double?
    /// The RF frequency of the carrier.
    public var rf: Double?
    /// The level of the input.
    public var levelDbfs: Double
    /// The carrier to noise density.
    public var cn0Dbhz: Double
    /// The drift of the carrier.
    public var driftHzMin: Double
    /// The noise density.
    public var noiseDensityDbfs: Double
    /// The carrier loop is locked.
    public var carrierLocked: Bool
    /// The rms of the phase modulation.
    public var pmRmsDeg: Double
    /// The subcarrier frequency.
    public var subcarrier: Double?
    /// The humps of the subcarrier spectrum.
    public var tones: Int64?
    /// Their spacing.
    public var toneSpacing: Double?
    /// The subcarrier over the noise of the phase modulation.
    public var subcarrierSnrDb: Double?
    /// The bit rate. No value before the bit clock is found.
    public var bitRateBps: Double?
    /// The line code.
    public var coding: Coding?
    /// The bit clock is locked.
    public var clockLocked: Bool
    /// The frame length. No value before the frames are found.
    public var frameBits: Int64?
    /// The agreement of the frames with the template.
    public var frameAgreement: Double
    /// Bits that do not change from frame to frame.
    public var fixedBits: Int64
    /// The sync word.
    public var syncWord: SyncWord?
    /// The byte of the frame counter.
    public var counterByte: Int64?
    /// The modulo of the frame counter.
    public var counterModulo: Int64?
    /// Bits received.
    public var bits: Int64
    /// Frames good.
    public var framesOk: Int64
    /// Frames bad.
    public var framesBad: Int64
    /// The template of a frame.
    public var template: [UInt8]
    /// The mask of the fixed bits, one bit for each bit of the frame.
    public var fixedMask: [UInt8]
    /// The spacing of a second carrier above the beacon.
    public var pilotSpacing: Double?
    /// The lines of the spectrum.
    public var lines: Int64
    /// The signal time when the first frames were found.
    public var firstFrames: Double?

    init(_ c: sdrkit_satbeacon_fields) {
        state = State(rawValue: c.state)
        signalTime = c.signal_time
        workRate = c.work_rate
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        rf = c.has_rf ? c.rf : nil
        levelDbfs = c.level_dbfs
        cn0Dbhz = c.cn0_dbhz
        driftHzMin = c.drift_hz_min
        noiseDensityDbfs = c.noise_density_dbfs
        carrierLocked = c.carrier_locked
        pmRmsDeg = c.pm_rms_deg
        subcarrier = c.has_subcarrier ? c.subcarrier : nil
        tones = c.has_tones ? c.tones : nil
        toneSpacing = c.has_tone_spacing ? c.tone_spacing : nil
        subcarrierSnrDb = c.has_subcarrier_snr_db ? c.subcarrier_snr_db : nil
        bitRateBps = c.has_bit_rate_bps ? c.bit_rate_bps : nil
        coding = c.has_coding ? Coding(rawValue: c.coding) : nil
        clockLocked = c.clock_locked
        frameBits = c.has_frame_bits ? c.frame_bits : nil
        frameAgreement = c.frame_agreement
        fixedBits = c.fixed_bits
        syncWord = c.has_sync_word ? SyncWord(rawValue: c.sync_word) : nil
        counterByte = c.has_counter_byte ? c.counter_byte : nil
        counterModulo = c.has_counter_modulo ? c.counter_modulo : nil
        bits = c.bits
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        template = [UInt8](UnsafeBufferPointer(start: c.template_.ptr, count: c.template_.len))
        fixedMask = [UInt8](UnsafeBufferPointer(start: c.fixed_mask.ptr, count: c.fixed_mask.len))
        pilotSpacing = c.has_pilot_spacing ? c.pilot_spacing : nil
        lines = c.lines
        firstFrames = c.has_first_frames ? c.first_frames : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<SatbeaconFields, SDRKitCode> {
        var c = sdrkit_satbeacon_fields()
        let code = sdrkit_satbeacon_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(SatbeaconFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the narrow lines of the spectrum, strongest first. The row key is the place.
public struct SatbeaconLineRow {
    /// The row key.
    public var key: String
    /// The offset of the line from the centre.
    public var offset: Double
    /// Its level.
    public var levelDbfs: Double
    /// Its height over the noise.
    public var overNoiseDb: Double

    init(_ c: sdrkit_satbeacon_line_row) {
        key = sdrkitText(c.key)
        offset = c.offset
        levelDbfs = c.level_dbfs
        overNoiseDb = c.over_noise_db
    }
}

/// A row of the frames received. The row key is the frame number.
public struct SatbeaconFrameRow {
    /// The row key.
    public var key: String
    /// The frame number.
    public var frame: Int64
    /// The frame counter.
    public var counter: Int64?
    /// The errors against the template.
    public var errors: Int64
    /// The frame is good.
    public var good: Bool
    /// The bytes of the frame.
    public var bytes: [UInt8]
    /// The signal time when the frame was found.
    public var timeS: Double

    init(_ c: sdrkit_satbeacon_frame_row) {
        key = sdrkitText(c.key)
        frame = c.frame
        counter = c.has_counter ? c.counter : nil
        errors = c.errors
        good = c.good
        bytes = [UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len))
        timeS = c.time_s
    }
}

/// A row of the decoded view. The row key is the name and the bytes.
public struct SatbeaconDecodedRow {
    public enum Kind: Int32 {
        case field = 0
        case summary = 1
    }
    public enum Confidence: Int32 {
        case decoded = 0
        case estimated = 1
        case unknown = 2
    }
    /// The row key.
    public var key: String
    /// The name of the field.
    public var name: String
    /// What the line is.
    public var kind: Kind?
    /// Its value, for people.
    public var value: String
    /// Its bytes in the frame, for people.
    public var bytes: String
    /// How sure the reading is.
    public var confidence: Confidence?
    /// Why, for people.
    public var reason: String

    init(_ c: sdrkit_satbeacon_decoded_row) {
        key = sdrkitText(c.key)
        name = sdrkitText(c.name)
        kind = Kind(rawValue: c.kind)
        value = sdrkitText(c.value)
        bytes = sdrkitText(c.bytes)
        confidence = Confidence(rawValue: c.confidence)
        reason = sdrkitText(c.reason)
    }
}

/// The result fields of `dvbc`: the carrier, the demodulator, the outer decoder and the locks. A measurement of the carrier has no value before a carrier is found.
public struct DvbcFields {
    public enum Stage: Int32 {
        case spectrum = 0
        case cma = 1
        case freq = 2
        case phase = 3
        case dd = 4
        case trial = 5
        case locked = 6
    }
    public enum Note: Int32 {
        case noCarrier = 0
        case rateNotUsable = 1
        case manualRateDiffers = 2
        case manualRateNoLock = 3
    }
    public enum Qam: Int32 {
        case n16Qam = 0
        case n32Qam = 1
        case n64Qam = 2
        case n128Qam = 3
        case n256Qam = 4
    }
    public enum Annex: Int32 {
        case a = 0
        case c = 1
    }
    /// What the demodulator is doing.
    public var stage: Stage?
    /// The reason the demodulator does not lock, when known.
    public var note: Note?
    /// The QAM order in use.
    public var qam: Qam?
    /// The symbol rate in use.
    public var symbolRateHz: Double?
    /// The symbol rate measured from the spectrum.
    public var measuredRate: Double?
    /// The manual symbol rate is in use.
    public var manualRate: Bool
    /// The -3 dB width of the carrier.
    public var width: Double?
    /// The annex of J.83.
    public var annex: Annex?
    /// The roll-off of the annex.
    public var rollOff: Double
    /// The spectrum is inverted.
    public var inverted: Bool
    /// The error of the symbol clock.
    public var clockPpm: Double
    /// The carrier offset from the centre.
    public var offsetHz: Double?
    /// The modulation error ratio.
    public var merDb: Double?
    /// The level of the input.
    public var levelDbfs: Double
    /// The fraction of bytes Reed-Solomon changed.
    public var preRs: Double
    /// Reed-Solomon words with no error.
    public var rsClean: Int64
    /// Reed-Solomon words corrected.
    public var rsCorrected: Int64
    /// Reed-Solomon words that failed.
    public var rsFailed: Int64
    /// Byte re-alignments without a loss of lock.
    public var realigned: Int64
    /// The net bit rate. No value while the receiver is not locked.
    public var netBitrateBps: Double?
    /// The carrier is found in the spectrum.
    public var spectrumLock: Bool
    /// The symbol timing holds.
    public var timingLock: Bool
    /// The equaliser has converged.
    public var equaliserLock: Bool
    /// The carrier loop holds.
    public var carrierLock: Bool
    /// The sync bytes are found.
    public var syncLock: Bool
    /// The transport stream is valid.
    public var tsLock: Bool
    /// Acquisitions started again.
    public var searches: Int64
    /// Frames good.
    public var framesOk: Int64
    /// Frames bad.
    public var framesBad: Int64

    init(_ c: sdrkit_dvbc_fields) {
        stage = Stage(rawValue: c.stage)
        note = c.has_note ? Note(rawValue: c.note) : nil
        qam = c.has_qam ? Qam(rawValue: c.qam) : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        measuredRate = c.has_measured_rate ? c.measured_rate : nil
        manualRate = c.manual_rate
        width = c.has_width ? c.width : nil
        annex = Annex(rawValue: c.annex)
        rollOff = c.roll_off
        inverted = c.inverted
        clockPpm = c.clock_ppm
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        levelDbfs = c.level_dbfs
        preRs = c.pre_rs
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        realigned = c.realigned
        netBitrateBps = c.has_net_bitrate_bps ? c.net_bitrate_bps : nil
        spectrumLock = c.spectrum_lock
        timingLock = c.timing_lock
        equaliserLock = c.equaliser_lock
        carrierLock = c.carrier_lock
        syncLock = c.sync_lock
        tsLock = c.ts_lock
        searches = c.searches
        framesOk = c.frames_ok
        framesBad = c.frames_bad
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbcFields, SDRKitCode> {
        var c = sdrkit_dvbc_fields()
        let code = sdrkit_dvbc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbcFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of the receiver of `dvbc2`: the signal, the L1 signalling and the FEC. A value of the L1 has no value before the L1 is read.
public struct Dvbc2Fields {
    public enum State: Int32 {
        case searching = 0
        case timing = 1
        case l1 = 2
        case data = 3
    }
    public enum Guard: Int32 {
        case n164 = 0
        case n1128 = 1
    }
    /// What the receiver does now.
    public var state: State?
    /// The guard interval.
    public var `guard`: Guard?
    /// The carrier offset from the centre. No value while the receiver searches.
    public var offsetHz: Double?
    /// The error of the sample clock.
    public var clockPpm: Double
    /// The carrier at the centre of the receive window.
    public var windowCarrier: Int64?
    /// The frequency of the centre of the receive window.
    public var windowHz: Double?
    /// Es/N0.
    public var esn0Db: Double?
    /// The modulation error ratio.
    public var merDb: Double?
    /// The network id of the L1.
    public var networkId: Int64?
    /// The C2 system id of the L1.
    public var systemId: Int64?
    /// The start frequency of the C2 system.
    public var startFrequencyHz: Double?
    /// The width of the C2 system.
    public var bandwidthHz: Double?
    /// The change counter of the L1.
    public var changeCounter: Int64?
    /// The early warning flag of the L1.
    public var ews: Bool?
    /// The C2 version of the L1.
    public var c2Version: Int64?
    /// Reserved tones are in use.
    public var reservedTones: Bool?
    /// FEC blocks good.
    public var blocksOk: Int64
    /// FEC blocks that failed.
    public var blocksBad: Int64
    /// L1 blocks that failed.
    public var l1Bad: Int64
    /// Losses of the frame sync.
    public var syncLosses: Int64
    /// The bit error ratio before the LDPC decoder.
    public var preBer: Double?
    /// LDPC iterations, recent mean.
    public var meanIterations: Double?
    /// C2 frames received.
    public var c2Frames: Int64
    /// The data slice decoded.
    public var chosenSlice: Int64?
    /// The PLP decoded.
    public var chosenPlp: Int64?
    /// The common PLP of the PLP decoded.
    public var commonPlp: Int64?
    /// The data slices that carry the PLP, when more than one.
    public var bundledSlices: Int64?
    /// The frequency to tune to for the chosen data slice, when it is outside the window.
    public var retuneHz: Double?
    /// The data slice that the decoding needs and that reaches out of the receive window.
    public var outsideSlice: Int64?
    /// How far that data slice reaches beyond the edge of the receive window.
    public var outsideHz: Double?
    /// Packets of a generic stream.
    public var genericPackets: Int64
    /// Bytes of a generic stream.
    public var genericBytes: Int64
    /// A note on what the receiver decodes.
    public var note: String

    init(_ c: sdrkit_dvbc2_fields) {
        state = State(rawValue: c.state)
        `guard` = c.has_guard ? Guard(rawValue: c.guard) : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.clock_ppm
        windowCarrier = c.has_window_carrier ? c.window_carrier : nil
        windowHz = c.has_window_hz ? c.window_hz : nil
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        networkId = c.has_network_id ? c.network_id : nil
        systemId = c.has_system_id ? c.system_id : nil
        startFrequencyHz = c.has_start_frequency_hz ? c.start_frequency_hz : nil
        bandwidthHz = c.has_bandwidth_hz ? c.bandwidth_hz : nil
        changeCounter = c.has_change_counter ? c.change_counter : nil
        ews = c.has_ews ? c.ews : nil
        c2Version = c.has_c2_version ? c.c2_version : nil
        reservedTones = c.has_reserved_tones ? c.reserved_tones : nil
        blocksOk = c.blocks_ok
        blocksBad = c.blocks_bad
        l1Bad = c.l1_bad
        syncLosses = c.sync_losses
        preBer = c.has_pre_ber ? c.pre_ber : nil
        meanIterations = c.has_mean_iterations ? c.mean_iterations : nil
        c2Frames = c.c2_frames
        chosenSlice = c.has_chosen_slice ? c.chosen_slice : nil
        chosenPlp = c.has_chosen_plp ? c.chosen_plp : nil
        commonPlp = c.has_common_plp ? c.common_plp : nil
        bundledSlices = c.has_bundled_slices ? c.bundled_slices : nil
        retuneHz = c.has_retune_hz ? c.retune_hz : nil
        outsideSlice = c.has_outside_slice ? c.outside_slice : nil
        outsideHz = c.has_outside_hz ? c.outside_hz : nil
        genericPackets = c.generic_packets
        genericBytes = c.generic_bytes
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Fields, SDRKitCode> {
        var c = sdrkit_dvbc2_fields()
        let code = sdrkit_dvbc2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbc2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The counts of the stream adaptation and the IP layer that `dvbc2` shows under its own keys.
public struct Dvbc2StreamFields {
    /// Transport packets of the chosen PLP.
    public var tsPackets: Int64
    /// GSE PDUs.
    public var pdus: Int64
    /// IP packets.
    public var ipPackets: Int64

    init(_ c: sdrkit_dvbc2_stream_fields) {
        tsPackets = c.ts_packets
        pdus = c.pdus
        ipPackets = c.ip_packets
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2StreamFields, SDRKitCode> {
        var c = sdrkit_dvbc2_stream_fields()
        let code = sdrkit_dvbc2_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbc2StreamFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the data slices of the L1. The row key is DSLICE_ID.
public struct Dvbc2SliceRow {
    public enum SliceType: Int32 {
        case type1 = 0
        case type2 = 1
    }
    public enum HeaderType: Int32 {
        case none = 0
        case robust = 1
        case efficient = 2
    }
    /// The row key.
    public var key: String
    /// DSLICE_ID.
    public var sliceId: Int64
    /// DSLICE_TYPE.
    public var sliceType: SliceType?
    /// The FECFrame header of a type 2 slice.
    public var headerType: HeaderType?
    /// The tuning position of the slice.
    public var tuneHz: Double
    /// The first carrier of the slice.
    public var startHz: Double?
    /// The last carrier of the slice.
    public var endHz: Double?
    /// The depth of the time interleaver.
    public var tiDepth: Int64
    /// The PLPs of the slice.
    public var plpCount: Int64
    /// The slice is inside the receive window.
    public var inWindow: Bool
    /// The width of the slice.
    public var widthHz: Double?

    init(_ c: sdrkit_dvbc2_slice_row) {
        key = sdrkitText(c.key)
        sliceId = c.slice_id
        sliceType = SliceType(rawValue: c.slice_type)
        headerType = HeaderType(rawValue: c.header_type)
        tuneHz = c.tune_hz
        startHz = c.has_start_hz ? c.start_hz : nil
        endHz = c.has_end_hz ? c.end_hz : nil
        tiDepth = c.ti_depth
        plpCount = c.plp_count
        inWindow = c.in_window
        widthHz = c.has_width_hz ? c.width_hz : nil
    }
}

/// A row of the PLPs of the L1. The row key is `<slice>/<PLP_ID>`.
public struct Dvbc2PlpRow {
    public enum PlpType: Int32 {
        case common = 0
        case grouped = 1
        case normal = 2
        case reserved = 3
    }
    public enum Payload: Int32 {
        case gfps = 0
        case gcs = 1
        case gse = 2
        case ts = 3
        case reserved = 4
    }
    public enum Modulation: Int32 {
        case n16Qam = 0
        case n64Qam = 1
        case n256Qam = 2
        case n1024Qam = 3
        case n4096Qam = 4
    }
    public enum CodeRate: Int32 {
        case n23 = 0
        case n34 = 1
        case n45 = 2
        case n56 = 3
        case n910 = 4
    }
    /// The row key.
    public var key: String
    /// The data slice of the PLP.
    public var plpSlice: Int64
    /// PLP_ID.
    public var plpId: Int64
    /// PLP_TYPE.
    public var plpType: PlpType?
    /// PLP_PAYLOAD_TYPE.
    public var payload: Payload?
    /// The length of the FEC frame.
    public var fecFrameBits: Int64?
    /// The constellation.
    public var modulation: Modulation?
    /// The code rate.
    public var codeRate: CodeRate?
    /// The transport stream id. No value for a PLP with PSI reprocessing.
    public var tsId: Int64?
    /// The original network id. No value for a PLP with PSI reprocessing.
    public var onId: Int64?
    /// It is the PLP decoded.
    public var chosen: Bool
    /// PLP_GROUP_ID. No value for a normal data PLP.
    public var group: Int64?
    /// PLP_BUNDLED: the PLP is spread over several data slices.
    public var bundled: Bool

    init(_ c: sdrkit_dvbc2_plp_row) {
        key = sdrkitText(c.key)
        plpSlice = c.plp_slice
        plpId = c.plp_id
        plpType = PlpType(rawValue: c.plp_type)
        payload = Payload(rawValue: c.payload)
        fecFrameBits = c.has_fec_frame_bits ? c.fec_frame_bits : nil
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        tsId = c.has_ts_id ? c.ts_id : nil
        onId = c.has_on_id ? c.on_id : nil
        chosen = c.chosen
        group = c.has_group ? c.group : nil
        bundled = c.bundled
    }
}

/// A row of the notches of the L1. The row key is the number of the notch.
public struct Dvbc2NotchRow {
    /// The row key.
    public var key: String
    /// The first carrier of the notch.
    public var notchStartHz: Double?
    /// The last carrier of the notch.
    public var notchEndHz: Double?

    init(_ c: sdrkit_dvbc2_notch_row) {
        key = sdrkitText(c.key)
        notchStartHz = c.has_notch_start_hz ? c.notch_start_hz : nil
        notchEndHz = c.has_notch_end_hz ? c.notch_end_hz : nil
    }
}

/// The result fields of the sender `transmit` of `dvbs`.
public struct DvbsTxFields {
    /// Whether a transmission is under way.
    public var transmitting: Bool
    /// The transmit frequency: the centre of the carrier sent. No value while the stack does not transmit.
    public var txFreqHz: Double?
    /// The bit rate of the transport stream that the settings carry: the rate that the host feeds at most.
    public var txBitrateBps: Double
    /// The packets of the host sent in this transmission, or in the last one.
    public var txPackets: Int64
    /// The null packets put in slots that found no packet of the host.
    public var txNulls: Int64
    /// The packets that wait to be sent.
    public var txQueue: Int64
    /// The time that the waiting packets take at the useful bit rate.
    public var txQueueS: Double
    /// The PCRs rewritten to the time of their slot.
    public var txPcrRestamped: Int64

    init(_ c: sdrkit_dvbs_tx_fields) {
        transmitting = c.transmitting
        txFreqHz = c.has_tx_freq_hz ? c.tx_freq_hz : nil
        txBitrateBps = c.tx_bitrate_bps
        txPackets = c.tx_packets
        txNulls = c.tx_nulls
        txQueue = c.tx_queue
        txQueueS = c.tx_queue_s
        txPcrRestamped = c.tx_pcr_restamped
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxFields, SDRKitCode> {
        var c = sdrkit_dvbs_tx_fields()
        let code = sdrkit_dvbs_tx_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DvbsTxFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `isdbs`: the demodulator, the TMCC and the outer decoder. The offset has no value before the frame sync is found.
public struct IsdbsFields {
    public enum State: Int32 {
        case searching = 0
        case frameSync = 1
        case tmcc = 2
        case ts = 3
    }
    /// What the receiver does now.
    public var state: State?
    /// The frame sync words are found.
    public var frameSync: Bool
    /// The carrier offset from the centre.
    public var offsetHz: Double?
    /// Es/N0.
    public var esn0Db: Double?
    /// Frames received.
    public var frames: Int64
    /// Super-frames received.
    public var superframes: Int64
    /// Losses of the frame sync.
    public var syncLosses: Int64
    /// TMCC words good.
    public var tmccGood: Int64
    /// TMCC words that failed.
    public var tmccBad: Int64
    /// The change counter of the TMCC.
    public var changeCounter: Int64?
    /// Reed-Solomon words with no error.
    public var rsClean: Int64
    /// Reed-Solomon words corrected.
    public var rsCorrected: Int64
    /// Reed-Solomon words that failed.
    public var rsFailed: Int64
    /// The stream decoded.
    public var stream: Int64?
    /// The emergency warning flag of the TMCC.
    public var ews: Bool
    /// The transport stream is valid.
    public var tsLock: Bool

    init(_ c: sdrkit_isdbs_fields) {
        state = State(rawValue: c.state)
        frameSync = c.frame_sync
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        frames = c.frames
        superframes = c.superframes
        syncLosses = c.sync_losses
        tmccGood = c.tmcc_good
        tmccBad = c.tmcc_bad
        changeCounter = c.has_change_counter ? c.change_counter : nil
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        stream = c.has_stream ? c.stream : nil
        ews = c.ews
        tsLock = c.ts_lock
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsdbsFields, SDRKitCode> {
        var c = sdrkit_isdbs_fields()
        let code = sdrkit_isdbs_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsdbsFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the streams of the transponder. The row key is the stream number.
public struct IsdbsStreamRow {
    public enum Modulation: Int32 {
        case bpsk12 = 0
        case qpsk12 = 1
        case qpsk23 = 2
        case qpsk34 = 3
        case qpsk56 = 4
        case qpsk78 = 5
        case tc8psk23 = 6
    }
    /// The row key.
    public var key: String
    /// The stream number in the TMCC.
    public var stream: Int64
    /// The transport stream id.
    public var tsId: Int64
    /// The slots of the stream.
    public var slots: Int64
    /// The modulation and code. No value for a stream with mixed modulation.
    public var modulation: Modulation?
    /// The slots of the stream differ in modulation.
    public var mixed: Bool
    /// The bit rate of the stream.
    public var bitrateBps: Double
    /// It is the stream decoded.
    public var decoded: Bool

    init(_ c: sdrkit_isdbs_stream_row) {
        key = sdrkitText(c.key)
        stream = c.stream
        tsId = c.ts_id
        slots = c.slots
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        mixed = c.mixed
        bitrateBps = c.bitrate_bps
        decoded = c.decoded
    }
}

/// The fields of a telemetry frame, when it is found.
public struct SatbeaconBeaconFrameEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The frame number.
    public var frame: Int64
    /// The frame counter.
    public var counter: Int64?
    /// The errors against the template.
    public var errors: Int64
    /// The frame is good.
    public var good: Bool
    /// The bytes of the frame.
    public var bytes: [UInt8]

    init(_ c: sdrkit_satbeacon_beacon_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        frame = c.frame
        counter = c.has_counter ? c.counter : nil
        errors = c.errors
        good = c.good
        bytes = [UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len))
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatbeaconBeaconFrameEvent, SDRKitCode> {
        var c = sdrkit_satbeacon_beacon_frame_event()
        let code = sdrkit_satbeacon_beacon_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatbeaconBeaconFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the carrier measurements of one second of signal.
public struct SatbeaconBeaconCarrierEvent {
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The signal time.
    public var signalTime: Double
    /// The offset of the carrier from the centre.
    public var offsetHz: Double?
    /// The level of the input.
    public var levelDbfs: Double
    /// The carrier to noise density.
    public var cn0Dbhz: Double
    /// The drift of the carrier.
    public var driftHzMin: Double
    /// The bit rate. No value before the bit clock is found.
    public var bitRateBps: Double?
    /// The frame length. No value before the frames are found.
    public var frameBits: Int64?

    init(_ c: sdrkit_satbeacon_beacon_carrier_event) {
        time = c.time
        freqHz = c.freq_hz
        signalTime = c.signal_time
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.level_dbfs
        cn0Dbhz = c.cn0_dbhz
        driftHzMin = c.drift_hz_min
        bitRateBps = c.has_bit_rate_bps ? c.bit_rate_bps : nil
        frameBits = c.has_frame_bits ? c.frame_bits : nil
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<SatbeaconBeaconCarrierEvent, SDRKitCode> {
        var c = sdrkit_satbeacon_beacon_carrier_event()
        let code = sdrkit_satbeacon_beacon_carrier_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(SatbeaconBeaconCarrierEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum DvbcsComponents {
    /// The result fields of the component "monitor" of the stack "dvbc" in the snapshot of `handle`.
    public static func dvbcMonitorTsMonitorFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsMonitorFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_monitor_fields()
        let code = sdrkit_dvbc_monitor_ts_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbc" in the snapshot of `handle`.
    public static func dvbcTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbc_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbc" in the snapshot of `handle`.
    public static func dvbcPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbc_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvbc" in the snapshot of `handle`.
    public static func dvbcTeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvbc_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvbc" in the snapshot of `handle`.
    public static func dvbcDvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvbc_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "monitor" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2MonitorTsMonitorFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsMonitorFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_monitor_fields()
        let code = sdrkit_dvbc2_monitor_ts_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "bbframe" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2BbframeBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_dvbc2_bbframe_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ip" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2IpFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastIpFields, SDRKitCode> {
        var c = sdrkit_ipcast_ip_fields()
        let code = sdrkit_dvbc2_ip_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastIpFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2TsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbc2_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2PsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbc2_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2TeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvbc2_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvbc2" in the snapshot of `handle`.
    public static func dvbc2DvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvbc2_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "monitor" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsMonitorTsMonitorFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsMonitorFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_monitor_fields()
        let code = sdrkit_dvbs_monitor_ts_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "bbframe" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsBbframeBbFrameFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastBbFrameFields, SDRKitCode> {
        var c = sdrkit_ipcast_bb_frame_fields()
        let code = sdrkit_dvbs_bbframe_bb_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastBbFrameFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ip" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsIpFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastIpFields, SDRKitCode> {
        var c = sdrkit_ipcast_ip_fields()
        let code = sdrkit_dvbs_ip_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastIpFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dvbs_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dvbs_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsTeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_dvbs_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "dvbs" in the snapshot of `handle`.
    public static func dvbsDvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_dvbs_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "monitor" of the stack "isdbs" in the snapshot of `handle`.
    public static func isdbsMonitorTsMonitorFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsMonitorFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_monitor_fields()
        let code = sdrkit_isdbs_monitor_ts_monitor_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsMonitorFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "isdbs" in the snapshot of `handle`.
    public static func isdbsTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_isdbs_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "isdbs" in the snapshot of `handle`.
    public static func isdbsPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_isdbs_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "teletext" of the stack "isdbs" in the snapshot of `handle`.
    public static func isdbsTeletextStreamFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTeletextStreamFields, SDRKitCode> {
        var c = sdrkit_mpegts_teletext_stream_fields()
        let code = sdrkit_isdbs_teletext_stream_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTeletextStreamFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dvb_subtitle" of the stack "isdbs" in the snapshot of `handle`.
    public static func isdbsDvbSubtitleFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsDvbSubtitleFields, SDRKitCode> {
        var c = sdrkit_mpegts_dvb_subtitle_fields()
        let code = sdrkit_isdbs_dvb_subtitle_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsDvbSubtitleFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum DvbcsTables {
    /// The changed rows of the table "lines" of the stack "satbeacon", newest first, at most `capacity` rows.
    /// Default order: the column `level_dbfs`, highest first.
    public static func satbeaconLines(_ handle: SDRKIT_HANDLE, capacity: Int = 6) -> Result<[SatbeaconLineRow], SDRKitCode> {
        var rows = [sdrkit_satbeacon_line_row](repeating: sdrkit_satbeacon_line_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_satbeacon_lines_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatbeaconLineRow.init))
    }
    /// The changed rows of the table "frames" of the stack "satbeacon", newest first, at most `capacity` rows.
    /// The rows are a log: they arrive over time.
    /// Time: `time_s`.
    public static func satbeaconFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 48) -> Result<[SatbeaconFrameRow], SDRKitCode> {
        var rows = [sdrkit_satbeacon_frame_row](repeating: sdrkit_satbeacon_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_satbeacon_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatbeaconFrameRow.init))
    }
    /// The changed rows of the table "decoded" of the stack "satbeacon", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func satbeaconDecoded(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[SatbeaconDecodedRow], SDRKitCode> {
        var rows = [sdrkit_satbeacon_decoded_row](repeating: sdrkit_satbeacon_decoded_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_satbeacon_decoded_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(SatbeaconDecodedRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbc", newest first, at most `capacity` rows.
    public static func dvbcPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "slices" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Slices(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbc2SliceRow], SDRKitCode> {
        var rows = [sdrkit_dvbc2_slice_row](repeating: sdrkit_dvbc2_slice_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_slices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbc2SliceRow.init))
    }
    /// The changed rows of the table "plps" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Plps(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbc2PlpRow], SDRKitCode> {
        var rows = [sdrkit_dvbc2_plp_row](repeating: sdrkit_dvbc2_plp_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_plps_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbc2PlpRow.init))
    }
    /// The changed rows of the table "notches" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Notches(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[Dvbc2NotchRow], SDRKitCode> {
        var rows = [sdrkit_dvbc2_notch_row](repeating: sdrkit_dvbc2_notch_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_notches_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbc2NotchRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Services(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Tracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2Guide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2GuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2TeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2TeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2TsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbc2", newest first, at most `capacity` rows.
    public static func dvbc2PsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbc2_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "carriers" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsCarriers(_ handle: SDRKIT_HANDLE, capacity: Int = 32) -> Result<[Dvbs2CarrierRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_carrier_row](repeating: sdrkit_dvbs2_carrier_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_carriers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2CarrierRow.init))
    }
    /// The changed rows of the table "streams" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 256) -> Result<[Dvbs2StreamRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_row](repeating: sdrkit_dvbs2_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamRow.init))
    }
    /// The changed rows of the table "stream_modcods" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsStreamModcods(_ handle: SDRKIT_HANDLE, capacity: Int = 1024) -> Result<[Dvbs2StreamModcodRow], SDRKitCode> {
        var rows = [sdrkit_dvbs2_stream_modcod_row](repeating: sdrkit_dvbs2_stream_modcod_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_stream_modcods_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Dvbs2StreamModcodRow.init))
    }
    /// The changed rows of the table "services" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dvbs", newest first, at most `capacity` rows.
    public static func dvbsPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dvbs_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "streams" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsStreams(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[IsdbsStreamRow], SDRKitCode> {
        var rows = [sdrkit_isdbs_stream_row](repeating: sdrkit_isdbs_stream_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_streams_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsdbsStreamRow.init))
    }
    /// The changed rows of the table "services" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "teletext_pages" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsTeletextPages(_ handle: SDRKIT_HANDLE, capacity: Int = 800) -> Result<[MpegtsTeletextPageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_page_row](repeating: sdrkit_mpegts_teletext_page_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_teletext_pages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextPageRow.init))
    }
    /// The changed rows of the table "teletext_subpages" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsTeletextSubpages(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTeletextSubpageRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_teletext_subpage_row](repeating: sdrkit_mpegts_teletext_subpage_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_teletext_subpages_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTeletextSubpageRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "isdbs", newest first, at most `capacity` rows.
    public static func isdbsPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbs_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum DvbcsCommands {
    /// Clear. Forgets the frames learned.
    public static func satbeaconClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_satbeacon_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets every count.
    public static func dvbcClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbc_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets every count.
    public static func dvbc2Clear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbc2_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets every count.
    public static func dvbsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbs_clear(handle)
        return sdrkitResult(code)
    }
    /// Start sending. Sends a DVB-S signal (EN 300 421) or a DVB-S2 signal (EN 302 307-1), as tx_standard says, with the tx_* settings, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit must be on. It transmits (file). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing. The transmission lasts until the command "stop" stops it.
    public static func dvbsStart(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbs_start(handle)
        return sdrkitResult(code)
    }
    /// Stop sending. Ends the transmission after the group of eight packets on air (DVB-S) or the PLFRAME on air (DVB-S2) with a fall of 5 ms. The packets that wait are dropped.
    public static func dvbsStop(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_dvbs_stop(handle)
        return sdrkitResult(code)
    }
    /// Add packets. Adds transport packets to the queue of the stream sent. All or none: refused when the queue has no room, so the host feeds at the useful bit rate.
    public static func dvbsAppend(_ handle: SDRKIT_HANDLE, packets: [UInt8]) -> Result<Void, SDRKitCode> {
        var args = sdrkit_dvbs_tx_append_args()
        args.size = UInt32(MemoryLayout<sdrkit_dvbs_tx_append_args>.size)
        let code: Int32 = packets.withUnsafeBufferPointer { packets_ in { () -> Int32 in
        args.packets = sdrkit_bytes(ptr: packets_.baseAddress, len: packets_.count)
        return sdrkit_dvbs_append(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets every count.
    public static func isdbsClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_isdbs_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum DvbcsSettings {
    /// Symbol rate.
    public static func dvbcSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "qam" of the stack "dvbc".
    public enum DvbcQam: Int32 {
        case auto = 0
        case n16Qam = 1
        case n32Qam = 2
        case n64Qam = 3
        case n128Qam = 4
        case n256Qam = 5
    }
    /// QAM.
    public static func dvbcQam(_ handle: SDRKIT_HANDLE) -> Result<DvbcQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "qam", &v)
        return code == SDRKIT_OK ? .success(DvbcQam(rawValue: v) ?? DvbcQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcQam(_ handle: SDRKIT_HANDLE, _ value: DvbcQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "qam", value.rawValue))
    }
    /// Annex C.
    public static func dvbcAnnexC(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "annex_c", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcAnnexC(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "annex_c", value))
    }
    /// Decode video.
    public static func dvbcDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// PLP.
    public static func dvbc2Plp(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "plp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Plp(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "plp", value))
    }
    /// Data slice.
    public static func dvbc2Slice(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "slice", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Slice(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "slice", value))
    }
    /// Centre frequency known.
    public static func dvbc2CentreKnown(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "centre_known", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2CentreKnown(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "centre_known", value))
    }
    /// The options of the setting "raster" of the stack "dvbc2".
    public enum Dvbc2Raster: Int32 {
        case n8Mhz = 0
        case n6Mhz = 1
    }
    /// Channel raster.
    public static func dvbc2Raster(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Raster, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "raster", &v)
        return code == SDRKIT_OK ? .success(Dvbc2Raster(rawValue: v) ?? Dvbc2Raster(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Raster(_ handle: SDRKIT_HANDLE, _ value: Dvbc2Raster) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "raster", value.rawValue))
    }
    /// LDPC iterations.
    public static func dvbc2LdpcIterations(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "ldpc_iterations", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2LdpcIterations(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "ldpc_iterations", value))
    }
    /// Timing metric.
    public static func dvbc2TimingMetric(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "timing_metric", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2TimingMetric(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "timing_metric", value))
    }
    /// Preamble ratio.
    public static func dvbc2PreambleRatio(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "preamble_ratio", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2PreambleRatio(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "preamble_ratio", value))
    }
    /// Carrier index metric.
    public static func dvbc2IndexMetric(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "index_metric", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2IndexMetric(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "index_metric", value))
    }
    /// Carrier index span.
    public static func dvbc2IndexSpan(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "index_span", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2IndexSpan(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "index_span", value))
    }
    /// Header candidate.
    public static func dvbc2HeaderCandidate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "header_candidate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2HeaderCandidate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "header_candidate", value))
    }
    /// Header agreement.
    public static func dvbc2HeaderAgree(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "header_agree", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2HeaderAgree(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "header_agree", value))
    }
    /// Header misses.
    public static func dvbc2HeaderMisses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "header_misses", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2HeaderMisses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "header_misses", value))
    }
    /// Block misses.
    public static func dvbc2BlockMisses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "block_misses", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2BlockMisses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "block_misses", value))
    }
    /// Preamble misses.
    public static func dvbc2PreambleMisses(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "preamble_misses", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2PreambleMisses(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "preamble_misses", value))
    }
    /// Time interleaver over every carrier.
    public static func dvbc2TiFullWidth(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ti_full_width", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2TiFullWidth(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ti_full_width", value))
    }
    /// Header scrambling word.
    public static func dvbc2RmScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "rm_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2RmScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "rm_scrambling", value))
    }
    /// Bundled PLP hold.
    public static func dvbc2BundleHold(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "bundle_hold", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2BundleHold(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "bundle_hold", value))
    }
    /// Decode video.
    public static func dvbc2DecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2DecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Symbol rate.
    public static func dvbsSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "symbol_rate", value))
    }
    /// The options of the setting "standard" of the stack "dvbs".
    public enum DvbsStandard: Int32 {
        case auto = 0
        case dvbs = 1
        case dvbs2 = 2
    }
    /// Standard.
    public static func dvbsStandard(_ handle: SDRKIT_HANDLE) -> Result<DvbsStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "standard", &v)
        return code == SDRKIT_OK ? .success(DvbsStandard(rawValue: v) ?? DvbsStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsStandard(_ handle: SDRKIT_HANDLE, _ value: DvbsStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "standard", value.rawValue))
    }
    /// The options of the setting "roll_off" of the stack "dvbs".
    public enum DvbsRollOff: Int32 {
        case auto = 0
        case n035 = 1
        case n025 = 2
        case n020 = 3
        case n015 = 4
        case n010 = 5
        case n005 = 6
    }
    /// Roll-off.
    public static func dvbsRollOff(_ handle: SDRKIT_HANDLE) -> Result<DvbsRollOff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "roll_off", &v)
        return code == SDRKIT_OK ? .success(DvbsRollOff(rawValue: v) ?? DvbsRollOff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsRollOff(_ handle: SDRKIT_HANDLE, _ value: DvbsRollOff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "roll_off", value.rawValue))
    }
    /// Input stream.
    public static func dvbsIsi(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "isi", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsIsi(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "isi", value))
    }
    /// PL scrambling code.
    public static func dvbsPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "pl_scrambling", value))
    }
    /// Carrier.
    public static func dvbsCarrierPick(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "carrier_pick", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsCarrierPick(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "carrier_pick", $0) })
    }
    /// The options of the setting "modcod_rule" of the stack "dvbs".
    public enum DvbsModcodRule: Int32 {
        case any = 0
        case fixed = 1
        case set = 2
    }
    /// MODCODs taken.
    public static func dvbsModcodRule(_ handle: SDRKIT_HANDLE) -> Result<DvbsModcodRule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "modcod_rule", &v)
        return code == SDRKIT_OK ? .success(DvbsModcodRule(rawValue: v) ?? DvbsModcodRule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsModcodRule(_ handle: SDRKIT_HANDLE, _ value: DvbsModcodRule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "modcod_rule", value.rawValue))
    }
    /// MODCOD.
    public static func dvbsModcod(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "modcod", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsModcod(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "modcod", value))
    }
    /// MODCOD set.
    public static func dvbsModcods(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "modcods", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsModcods(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "modcods", $0) })
    }
    /// The options of the setting "frame_length" of the stack "dvbs".
    public enum DvbsFrameLength: Int32 {
        case any = 0
        case normal = 1
        case short = 2
    }
    /// Frame length.
    public static func dvbsFrameLength(_ handle: SDRKIT_HANDLE) -> Result<DvbsFrameLength, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "frame_length", &v)
        return code == SDRKIT_OK ? .success(DvbsFrameLength(rawValue: v) ?? DvbsFrameLength(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsFrameLength(_ handle: SDRKIT_HANDLE, _ value: DvbsFrameLength) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "frame_length", value.rawValue))
    }
    /// The options of the setting "pilots" of the stack "dvbs".
    public enum DvbsPilots: Int32 {
        case any = 0
        case on = 1
        case off = 2
    }
    /// Pilots.
    public static func dvbsPilots(_ handle: SDRKIT_HANDLE) -> Result<DvbsPilots, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "pilots", &v)
        return code == SDRKIT_OK ? .success(DvbsPilots(rawValue: v) ?? DvbsPilots(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsPilots(_ handle: SDRKIT_HANDLE, _ value: DvbsPilots) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "pilots", value.rawValue))
    }
    /// PCAP output.
    public static func dvbsIpPcap(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "ip_pcap", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsIpPcap(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "ip_pcap", value))
    }
    /// The options of the setting "tx_polarisation" of the stack "dvbs".
    public enum DvbsTxPolarisation: Int32 {
        case unset = 0
        case v = 1
        case h = 2
    }
    /// Transponder.
    public static func dvbsTxPolarisation(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxPolarisation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_polarisation", &v)
        return code == SDRKIT_OK ? .success(DvbsTxPolarisation(rawValue: v) ?? DvbsTxPolarisation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxPolarisation(_ handle: SDRKIT_HANDLE, _ value: DvbsTxPolarisation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_polarisation", value.rawValue))
    }
    /// The options of the setting "lnb_supply" of the stack "dvbs".
    public enum DvbsLnbSupply: Int32 {
        case unset = 0
        case v = 1
        case h = 2
    }
    /// LNB supply.
    public static func dvbsLnbSupply(_ handle: SDRKIT_HANDLE) -> Result<DvbsLnbSupply, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "lnb_supply", &v)
        return code == SDRKIT_OK ? .success(DvbsLnbSupply(rawValue: v) ?? DvbsLnbSupply(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsLnbSupply(_ handle: SDRKIT_HANDLE, _ value: DvbsLnbSupply) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "lnb_supply", value.rawValue))
    }
    /// Decode video.
    public static func dvbsDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Symbol rate.
    public static func dvbsTxSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "tx_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "tx_symbol_rate", value))
    }
    /// The options of the setting "tx_rate" of the stack "dvbs".
    public enum DvbsTxRate: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// Code rate.
    public static func dvbsTxRate(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rate", &v)
        return code == SDRKIT_OK ? .success(DvbsTxRate(rawValue: v) ?? DvbsTxRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxRate(_ handle: SDRKIT_HANDLE, _ value: DvbsTxRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rate", value.rawValue))
    }
    /// Spectral inversion.
    public static func dvbsTxInversion(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_inversion", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxInversion(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_inversion", value))
    }
    /// The options of the setting "tx_standard" of the stack "dvbs".
    public enum DvbsTxStandard: Int32 {
        case dvbs2 = 0
        case dvbs = 1
    }
    /// Standard.
    public static func dvbsTxStandard(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_standard", &v)
        return code == SDRKIT_OK ? .success(DvbsTxStandard(rawValue: v) ?? DvbsTxStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxStandard(_ handle: SDRKIT_HANDLE, _ value: DvbsTxStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_standard", value.rawValue))
    }
    /// The options of the setting "tx_modcod" of the stack "dvbs".
    public enum DvbsTxModcod: Int32 {
        case qPSK14 = 0
        case qPSK13 = 1
        case qPSK25 = 2
        case qPSK12 = 3
        case qPSK35 = 4
        case qPSK23 = 5
        case qPSK34 = 6
        case qPSK45 = 7
        case qPSK56 = 8
        case qPSK89 = 9
        case qPSK910 = 10
        case n8PSK35 = 11
        case n8PSK23 = 12
        case n8PSK34 = 13
        case n8PSK56 = 14
        case n8PSK89 = 15
        case n8PSK910 = 16
        case n16APSK23 = 17
        case n16APSK34 = 18
        case n16APSK45 = 19
        case n16APSK56 = 20
        case n16APSK89 = 21
        case n16APSK910 = 22
        case n32APSK34 = 23
        case n32APSK45 = 24
        case n32APSK56 = 25
        case n32APSK89 = 26
        case n32APSK910 = 27
    }
    /// MODCOD.
    public static func dvbsTxModcod(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxModcod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_modcod", &v)
        return code == SDRKIT_OK ? .success(DvbsTxModcod(rawValue: v) ?? DvbsTxModcod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxModcod(_ handle: SDRKIT_HANDLE, _ value: DvbsTxModcod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_modcod", value.rawValue))
    }
    /// The options of the setting "tx_frame" of the stack "dvbs".
    public enum DvbsTxFrame: Int32 {
        case normal = 0
        case short = 1
    }
    /// FEC frame.
    public static func dvbsTxFrame(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxFrame, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_frame", &v)
        return code == SDRKIT_OK ? .success(DvbsTxFrame(rawValue: v) ?? DvbsTxFrame(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxFrame(_ handle: SDRKIT_HANDLE, _ value: DvbsTxFrame) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_frame", value.rawValue))
    }
    /// Pilots.
    public static func dvbsTxPilots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "tx_pilots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxPilots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "tx_pilots", value))
    }
    /// The options of the setting "tx_rolloff" of the stack "dvbs".
    public enum DvbsTxRolloff: Int32 {
        case n035 = 0
        case n025 = 1
        case n020 = 2
    }
    /// Roll-off.
    public static func dvbsTxRolloff(_ handle: SDRKIT_HANDLE) -> Result<DvbsTxRolloff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "tx_rolloff", &v)
        return code == SDRKIT_OK ? .success(DvbsTxRolloff(rawValue: v) ?? DvbsTxRolloff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxRolloff(_ handle: SDRKIT_HANDLE, _ value: DvbsTxRolloff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "tx_rolloff", value.rawValue))
    }
    /// PL scrambling code.
    public static func dvbsTxPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "tx_pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsTxPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "tx_pl_scrambling", value))
    }
    /// Stream.
    public static func isdbsStream(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "stream", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbsStream(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "stream", value))
    }
    /// Decode video.
    public static func isdbsDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbsDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum DvbcsGeneratorSettings {
    /// Bit rate.
    public static func satbeaconBitrate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_bitrate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatbeaconBitrate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_bitrate", value))
    }
    /// Subcarrier.
    public static func satbeaconSubcarrierKhz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_subcarrier_khz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatbeaconSubcarrierKhz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_subcarrier_khz", value))
    }
    /// NRZ instead of bi-phase.
    public static func satbeaconNrz(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_nrz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatbeaconNrz(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_nrz", value))
    }
    /// Carrier only.
    public static func satbeaconCarrierOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_carrier_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatbeaconCarrierOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_carrier_only", value))
    }
    /// Pilot carrier.
    public static func satbeaconPilot(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_pilot", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setSatbeaconPilot(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_pilot", value))
    }
    /// The options of the setting "gen_qam" of the stack "dvbc".
    public enum DvbcQam: Int32 {
        case n16Qam = 0
        case n32Qam = 1
        case n64Qam = 2
        case n128Qam = 3
        case n256Qam = 4
    }
    /// QAM.
    public static func dvbcQam(_ handle: SDRKIT_HANDLE) -> Result<DvbcQam, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_qam", &v)
        return code == SDRKIT_OK ? .success(DvbcQam(rawValue: v) ?? DvbcQam(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcQam(_ handle: SDRKIT_HANDLE, _ value: DvbcQam) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_qam", value.rawValue))
    }
    /// Annex C.
    public static func dvbcAnnexC(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_annex_c", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcAnnexC(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_annex_c", value))
    }
    /// Symbol rate.
    public static func dvbcSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// The options of the setting "gen_payload" of the stack "dvbc".
    public enum DvbcPayload: Int32 {
        case programme = 0
        case numbered = 1
    }
    /// Payload.
    public static func dvbcPayload(_ handle: SDRKIT_HANDLE) -> Result<DvbcPayload, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &v)
        return code == SDRKIT_OK ? .success(DvbcPayload(rawValue: v) ?? DvbcPayload(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbcPayload(_ handle: SDRKIT_HANDLE, _ value: DvbcPayload) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_payload", value.rawValue))
    }
    /// Centre frequency.
    public static func dvbc2Frequency(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_frequency", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Frequency(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_frequency", value))
    }
    /// The options of the setting "gen_modcod" of the stack "dvbc2".
    public enum Dvbc2Modcod: Int32 {
        case n25656 = 0
        case n1645 = 1
        case n6423 = 2
        case n25634 = 3
        case n64910 = 4
        case n256910 = 5
        case n102434 = 6
        case n409656 = 7
    }
    /// Coding.
    public static func dvbc2Modcod(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Modcod, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_modcod", &v)
        return code == SDRKIT_OK ? .success(Dvbc2Modcod(rawValue: v) ?? Dvbc2Modcod(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Modcod(_ handle: SDRKIT_HANDLE, _ value: Dvbc2Modcod) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_modcod", value.rawValue))
    }
    /// 16K FEC frames.
    public static func dvbc2ShortFrames(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_short_frames", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2ShortFrames(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_short_frames", value))
    }
    /// The options of the setting "gen_guard" of the stack "dvbc2".
    public enum Dvbc2Guard: Int32 {
        case n164 = 0
        case n1128 = 1
    }
    /// Guard interval.
    public static func dvbc2Guard(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Guard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(Dvbc2Guard(rawValue: v) ?? Dvbc2Guard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Guard(_ handle: SDRKIT_HANDLE, _ value: Dvbc2Guard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_slice_type" of the stack "dvbc2".
    public enum Dvbc2SliceType: Int32 {
        case type1 = 0
        case type2Robust = 1
        case type2Efficient = 2
    }
    /// Data slice type.
    public static func dvbc2SliceType(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2SliceType, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_slice_type", &v)
        return code == SDRKIT_OK ? .success(Dvbc2SliceType(rawValue: v) ?? Dvbc2SliceType(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2SliceType(_ handle: SDRKIT_HANDLE, _ value: Dvbc2SliceType) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_slice_type", value.rawValue))
    }
    /// The options of the setting "gen_ti_depth" of the stack "dvbc2".
    public enum Dvbc2TiDepth: Int32 {
        case none = 0
        case n4Symbols = 1
        case n8Symbols = 2
        case n16Symbols = 3
    }
    /// Time interleaving.
    public static func dvbc2TiDepth(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2TiDepth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_ti_depth", &v)
        return code == SDRKIT_OK ? .success(Dvbc2TiDepth(rawValue: v) ?? Dvbc2TiDepth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2TiDepth(_ handle: SDRKIT_HANDLE, _ value: Dvbc2TiDepth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_ti_depth", value.rawValue))
    }
    /// The options of the setting "gen_l1_ti" of the stack "dvbc2".
    public enum Dvbc2L1Ti: Int32 {
        case none = 0
        case bestFit = 1
        case n4Symbols = 2
        case n8Symbols = 3
    }
    /// L1 time interleaving.
    public static func dvbc2L1Ti(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2L1Ti, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_l1_ti", &v)
        return code == SDRKIT_OK ? .success(Dvbc2L1Ti(rawValue: v) ?? Dvbc2L1Ti(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2L1Ti(_ handle: SDRKIT_HANDLE, _ value: Dvbc2L1Ti) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_l1_ti", value.rawValue))
    }
    /// Narrowband notch.
    public static func dvbc2Notch(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_notch", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Notch(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_notch", value))
    }
    /// Reserved tones.
    public static func dvbc2ReservedTones(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_reserved_tones", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2ReservedTones(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_reserved_tones", value))
    }
    /// The options of the setting "gen_system" of the stack "dvbc2".
    public enum Dvbc2System: Int32 {
        case single = 0
        case twoPlps = 1
        case common = 2
        case bundled = 3
        case twoSlices = 4
        case mixedSlices = 5
        case group = 6
        case farSlice = 7
    }
    /// Slices and PLPs.
    public static func dvbc2System(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2System, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_system", &v)
        return code == SDRKIT_OK ? .success(Dvbc2System(rawValue: v) ?? Dvbc2System(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2System(_ handle: SDRKIT_HANDLE, _ value: Dvbc2System) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_system", value.rawValue))
    }
    /// The options of the setting "gen_stream" of the stack "dvbc2".
    public enum Dvbc2Stream: Int32 {
        case ts = 0
        case gse = 1
        case gfps = 2
        case gcs = 3
    }
    /// Stream.
    public static func dvbc2Stream(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Stream, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_stream", &v)
        return code == SDRKIT_OK ? .success(Dvbc2Stream(rawValue: v) ?? Dvbc2Stream(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Stream(_ handle: SDRKIT_HANDLE, _ value: Dvbc2Stream) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_stream", value.rawValue))
    }
    /// High efficiency mode.
    public static func dvbc2Hem(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_hem", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Hem(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_hem", value))
    }
    /// The options of the setting "gen_payload" of the stack "dvbc2".
    public enum Dvbc2Payload: Int32 {
        case programme = 0
        case numbered = 1
    }
    /// Payload.
    public static func dvbc2Payload(_ handle: SDRKIT_HANDLE) -> Result<Dvbc2Payload, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &v)
        return code == SDRKIT_OK ? .success(Dvbc2Payload(rawValue: v) ?? Dvbc2Payload(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbc2Payload(_ handle: SDRKIT_HANDLE, _ value: Dvbc2Payload) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_payload", value.rawValue))
    }
    /// The options of the setting "gen_standard" of the stack "dvbs".
    public enum DvbsStandard: Int32 {
        case dvbs2 = 0
        case dvbs = 1
    }
    /// Standard.
    public static func dvbsStandard(_ handle: SDRKIT_HANDLE) -> Result<DvbsStandard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_standard", &v)
        return code == SDRKIT_OK ? .success(DvbsStandard(rawValue: v) ?? DvbsStandard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsStandard(_ handle: SDRKIT_HANDLE, _ value: DvbsStandard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_standard", value.rawValue))
    }
    /// The options of the setting "gen_modulation" of the stack "dvbs".
    public enum DvbsModulation: Int32 {
        case qPSK = 0
        case n8PSK = 1
        case n16APSK = 2
        case n32APSK = 3
    }
    /// Modulation.
    public static func dvbsModulation(_ handle: SDRKIT_HANDLE) -> Result<DvbsModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &v)
        return code == SDRKIT_OK ? .success(DvbsModulation(rawValue: v) ?? DvbsModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsModulation(_ handle: SDRKIT_HANDLE, _ value: DvbsModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_modulation", value.rawValue))
    }
    /// The options of the setting "gen_rate" of the stack "dvbs".
    public enum DvbsRate: Int32 {
        case n14 = 0
        case n13 = 1
        case n25 = 2
        case n12 = 3
        case n35 = 4
        case n23 = 5
        case n34 = 6
        case n45 = 7
        case n56 = 8
        case n78 = 9
        case n89 = 10
        case n910 = 11
    }
    /// Code rate.
    public static func dvbsRate(_ handle: SDRKIT_HANDLE) -> Result<DvbsRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &v)
        return code == SDRKIT_OK ? .success(DvbsRate(rawValue: v) ?? DvbsRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsRate(_ handle: SDRKIT_HANDLE, _ value: DvbsRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rate", value.rawValue))
    }
    /// Short frames.
    public static func dvbsShort(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_short", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsShort(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_short", value))
    }
    /// Pilots.
    public static func dvbsPilots(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_pilots", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsPilots(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_pilots", value))
    }
    /// The options of the setting "gen_rolloff" of the stack "dvbs".
    public enum DvbsRolloff: Int32 {
        case n035 = 0
        case n025 = 1
        case n020 = 2
    }
    /// Roll-off.
    public static func dvbsRolloff(_ handle: SDRKIT_HANDLE) -> Result<DvbsRolloff, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_rolloff", &v)
        return code == SDRKIT_OK ? .success(DvbsRolloff(rawValue: v) ?? DvbsRolloff(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsRolloff(_ handle: SDRKIT_HANDLE, _ value: DvbsRolloff) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_rolloff", value.rawValue))
    }
    /// Symbol rate.
    public static func dvbsSymbolRate(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "gen_symbol_rate", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsSymbolRate(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "gen_symbol_rate", value))
    }
    /// PL scrambling code.
    public static func dvbsPlScrambling(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_pl_scrambling", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsPlScrambling(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_pl_scrambling", value))
    }
    /// A dummy frame every.
    public static func dvbsDummyEvery(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_dummy_every", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsDummyEvery(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_dummy_every", value))
    }
    /// GSE (IP) instead of a transport stream.
    public static func dvbsGse(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_gse", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsGse(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_gse", value))
    }
    /// Two input streams (TS and GSE).
    public static func dvbsMultistream(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_multistream", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsMultistream(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_multistream", value))
    }
    /// The options of the setting "gen_payload" of the stack "dvbs".
    public enum DvbsPayload: Int32 {
        case programme = 0
        case numbered = 1
    }
    /// Payload.
    public static func dvbsPayload(_ handle: SDRKIT_HANDLE) -> Result<DvbsPayload, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_payload", &v)
        return code == SDRKIT_OK ? .success(DvbsPayload(rawValue: v) ?? DvbsPayload(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDvbsPayload(_ handle: SDRKIT_HANDLE, _ value: DvbsPayload) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_payload", value.rawValue))
    }
    /// The options of the setting "gen_layout" of the stack "isdbs".
    public enum IsdbsLayout: Int32 {
        case tc8psk36Qpsk348Bpsk4 = 0
        case tc8psk48 = 1
        case qpsk1248 = 2
    }
    /// Layout.
    public static func isdbsLayout(_ handle: SDRKIT_HANDLE) -> Result<IsdbsLayout, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_layout", &v)
        return code == SDRKIT_OK ? .success(IsdbsLayout(rawValue: v) ?? IsdbsLayout(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbsLayout(_ handle: SDRKIT_HANDLE, _ value: IsdbsLayout) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_layout", value.rawValue))
    }
}
