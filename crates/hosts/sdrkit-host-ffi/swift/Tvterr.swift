// sdrkit: the typed interface of the stack family "tvterr", version 2.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum TvterrInterface {
    public static let major = 2
    public static let minor = 0
}

/// The result fields of `isdbt` and `isdbtsb`: the detection, the transmission parameters, the signal and the TMCC. A parameter of the mode has no value before the mode is found. A value of the TMCC has no value while no TMCC is decoded.
public struct IsdbtFields {
    public enum Guard: Int32 {
        case n14 = 0
        case n18 = 1
        case n116 = 2
        case n132 = 3
    }
    public enum Quality: Int32 {
        case excellent = 0
        case good = 1
        case marginal = 2
        case poor = 3
        case noLock = 4
    }
    public enum StepMode: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepTmcc: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepLayers: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepTs: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    /// Detection level: 0 nothing, 1 a periodic cyclic prefix, 2 TMCC decoded, 3 transport stream flowing.
    public var detection: Int64
    /// The channel width in use.
    public var width: Double
    /// The width setting is automatic.
    public var widthAuto: Bool
    /// The IFFT sampling rate of the width.
    public var ifftRate: Double
    /// Mode 1, 2 or 3.
    public var mode: Int64?
    /// The guard interval.
    public var `guard`: Guard?
    /// The FFT size.
    public var fftSize: Int64?
    /// The carriers in the band.
    public var carriers: Int64?
    /// The carrier spacing.
    public var carrierSpacing: Double?
    /// The frame length (204 symbols).
    public var frameLength: Double?
    /// The carrier offset.
    public var offsetHz: Double?
    /// The error of the sample clock.
    public var clockPpm: Double?
    /// The SNR of the equalised cells.
    public var snrDb: Double?
    /// Reed-Solomon words clean or corrected.
    public var framesOk: Int64
    /// Reed-Solomon words that failed.
    public var framesBad: Int64
    /// The integer carrier shift that the TMCC search found.
    public var carrierShift: Int64?
    /// The index of the symbol in the frame. No value while no TMCC is decoded.
    public var frameSymbol: Int64?
    /// The time since the last good TMCC.
    public var sinceTmcc: Double?
    /// I and Q are swapped.
    public var mirrored: Bool
    /// The system identification of the TMCC (0 ISDB-T, 1 ISDB-TSB).
    public var systemId: Int64?
    /// The parameter switching countdown of the TMCC (15: no switch announced).
    public var switchCountdown: Int64?
    /// The emergency alarm start flag of the TMCC.
    public var emergencyAlarm: Bool?
    /// The partial reception flag of the TMCC.
    public var partialReception: Bool?
    /// The share of the Reed-Solomon words that are clean or corrected.
    public var packetsOkPct: Double
    /// The net bit rate of every layer together.
    public var netRateBitS: Double
    /// The signal quality score.
    public var qualityPct: Double
    /// The signal quality class.
    public var quality: Quality?
    /// The mode step: the mode and the guard are found.
    public var stepMode: StepMode?
    /// The TMCC step.
    public var stepTmcc: StepTmcc?
    /// The layers step: a layer is synchronised.
    public var stepLayers: StepLayers?
    /// The transport stream step.
    public var stepTs: StepTs?

    init(_ c: sdrkit_isdbt_fields) {
        detection = c.detection
        width = c.width
        widthAuto = c.width_auto
        ifftRate = c.ifft_rate
        mode = c.has_mode ? c.mode : nil
        `guard` = c.has_guard ? Guard(rawValue: c.guard) : nil
        fftSize = c.has_fft_size ? c.fft_size : nil
        carriers = c.has_carriers ? c.carriers : nil
        carrierSpacing = c.has_carrier_spacing ? c.carrier_spacing : nil
        frameLength = c.has_frame_length ? c.frame_length : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        carrierShift = c.has_carrier_shift ? c.carrier_shift : nil
        frameSymbol = c.has_frame_symbol ? c.frame_symbol : nil
        sinceTmcc = c.has_since_tmcc ? c.since_tmcc : nil
        mirrored = c.mirrored
        systemId = c.has_system_id ? c.system_id : nil
        switchCountdown = c.has_switch_countdown ? c.switch_countdown : nil
        emergencyAlarm = c.has_emergency_alarm ? c.emergency_alarm : nil
        partialReception = c.has_partial_reception ? c.partial_reception : nil
        packetsOkPct = c.packets_ok_pct
        netRateBitS = c.net_rate_bit_s
        qualityPct = c.quality_pct
        quality = Quality(rawValue: c.quality)
        stepMode = StepMode(rawValue: c.step_mode)
        stepTmcc = StepTmcc(rawValue: c.step_tmcc)
        stepLayers = StepLayers(rawValue: c.step_layers)
        stepTs = StepTs(rawValue: c.step_ts)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsdbtFields, SDRKitCode> {
        var c = sdrkit_isdbt_fields()
        let code = sdrkit_isdbt_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsdbtFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the layer table of `isdbt` and `isdbtsb`, one for each layer in use. The row key is `A`, `B` or `C`.
public struct IsdbtLayerRow {
    public enum Layer: Int32 {
        case a = 0
        case b = 1
        case c = 2
    }
    public enum Modulation: Int32 {
        case dqpsk = 0
        case qpsk = 1
        case n16qam = 2
        case n64qam = 3
    }
    public enum CodeRate: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// The row key.
    public var key: String
    /// The layer.
    public var layer: Layer?
    /// The layer is the partial-reception segment (one-seg).
    public var partial: Bool
    /// The segments of the layer.
    public var segments: Int64
    /// The carrier modulation. No value for a code that the standard reserves.
    public var modulation: Modulation?
    /// The inner code rate. No value for a code that the standard reserves.
    public var codeRate: CodeRate?
    /// The time interleaving length I.
    public var interleaveLength: Int64
    /// The net bit rate of the layer.
    public var rateBitS: Double
    /// The packets delivered.
    public var packets: Int64
    /// Reed-Solomon words clean.
    public var rsClean: Int64
    /// Reed-Solomon words corrected.
    public var rsCorrected: Int64
    /// Reed-Solomon words that failed.
    public var rsFailed: Int64
    /// Reed-Solomon has synchronised the layer.
    public var rsLocked: Bool
    /// The Viterbi margin (1: no noise).
    public var viterbiMargin: Double

    init(_ c: sdrkit_isdbt_layer_row) {
        key = sdrkitText(c.key)
        layer = Layer(rawValue: c.layer)
        partial = c.partial
        segments = c.segments
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        interleaveLength = c.interleave_length
        rateBitS = c.rate_bit_s
        packets = c.packets
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        rsLocked = c.rs_locked
        viterbiMargin = c.viterbi_margin
    }
}

/// The result fields that a narrow-band receiver (`isdbtsb`) adds.
public struct IsdbtNarrowFields {
    /// The segment count that the receiver uses (1 or 3).
    public var bandSegments: Int64
    /// What the receiver decodes and which of its values no public document confirms.
    public var note: String

    init(_ c: sdrkit_isdbt_narrow_fields) {
        bandSegments = c.band_segments
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<IsdbtNarrowFields, SDRKitCode> {
        var c = sdrkit_isdbt_narrow_fields()
        let code = sdrkit_isdbt_narrow_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IsdbtNarrowFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `atsc`: the detection, the locks, the signal and the counts of the decoder. A measurement of a stage has no value before the stage locks.
public struct AtscFields {
    public enum Quality: Int32 {
        case excellent = 0
        case good = 1
        case marginal = 2
        case poor = 3
        case noLock = 4
    }
    public enum StepPilot: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepSegment: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepField: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepEqualiser: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepTrellis: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepRs: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepTs: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    /// Detection level: 0 nothing, 1 pilot, 2 segment sync, 3 field sync, 4 transport stream flowing.
    public var detection: Int64
    /// The pilot loop is locked.
    public var pilotLocked: Bool
    /// The segment sync and the symbol clock are locked.
    public var segmentSync: Bool
    /// Fields are framed.
    public var fieldSync: Bool
    /// The equaliser is trained (SNR on the known symbols at least 9 dB).
    public var equaliserTrained: Bool
    /// The transport stream flows.
    public var tsFlowing: Bool
    /// The carrier offset. No value before the pilot locks.
    public var offsetHz: Double?
    /// The error of the symbol clock. No value before the segment sync.
    public var clockPpm: Double?
    /// The SNR on the known symbols. No value before the field sync.
    public var snrDb: Double?
    /// Reed-Solomon words good.
    public var framesOk: Int64
    /// Reed-Solomon words that failed.
    public var framesBad: Int64
    /// The quality of the segment sync correlation, 0 to 1.
    public var segmentCorrelation: Double
    /// The field parity, 1 or 2. No value without the field sync.
    public var fieldParity: Int64?
    /// The SNR of the data symbols against the decoded trellis path. No value before a field decodes.
    public var dataSnrDb: Double?
    /// The pilot level over the data level. No value before the pilot locks.
    public var pilotDb: Double?
    /// Fields decoded.
    public var fields: Int64
    /// Reed-Solomon words clean.
    public var rsClean: Int64
    /// Reed-Solomon words corrected.
    public var rsCorrected: Int64
    /// Reed-Solomon words that failed.
    public var rsFailed: Int64
    /// Packets delivered.
    public var packets: Int64
    /// Field syncs expected but not found.
    public var fieldSyncMisses: Int64
    /// Times the field sync was lost.
    public var lockLosses: Int64
    /// The signal quality score.
    public var qualityPct: Double
    /// The signal quality class.
    public var quality: Quality?
    /// The pilot step.
    public var stepPilot: StepPilot?
    /// The segment sync step.
    public var stepSegment: StepSegment?
    /// The field sync step.
    public var stepField: StepField?
    /// The equaliser step.
    public var stepEqualiser: StepEqualiser?
    /// The trellis step.
    public var stepTrellis: StepTrellis?
    /// The Reed-Solomon step.
    public var stepRs: StepRs?
    /// The transport stream step.
    public var stepTs: StepTs?

    init(_ c: sdrkit_atsc_fields) {
        detection = c.detection
        pilotLocked = c.pilot_locked
        segmentSync = c.segment_sync
        fieldSync = c.field_sync
        equaliserTrained = c.equaliser_trained
        tsFlowing = c.ts_flowing
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        segmentCorrelation = c.segment_correlation
        fieldParity = c.has_field_parity ? c.field_parity : nil
        dataSnrDb = c.has_data_snr_db ? c.data_snr_db : nil
        pilotDb = c.has_pilot_db ? c.pilot_db : nil
        fields = c.fields
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        packets = c.packets
        fieldSyncMisses = c.field_sync_misses
        lockLosses = c.lock_losses
        qualityPct = c.quality_pct
        quality = Quality(rawValue: c.quality)
        stepPilot = StepPilot(rawValue: c.step_pilot)
        stepSegment = StepSegment(rawValue: c.step_segment)
        stepField = StepField(rawValue: c.step_field)
        stepEqualiser = StepEqualiser(rawValue: c.step_equaliser)
        stepTrellis = StepTrellis(rawValue: c.step_trellis)
        stepRs = StepRs(rawValue: c.step_rs)
        stepTs = StepTs(rawValue: c.step_ts)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<AtscFields, SDRKitCode> {
        var c = sdrkit_atsc_fields()
        let code = sdrkit_atsc_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(AtscFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `atsc3`: the bootstrap, the frames, the service list and the media. A value of the bootstrap has no value before a bootstrap is found.
public struct Atsc3Fields {
    public enum SystemBandwidth: Int32 {
        case n6Mhz = 0
        case n7Mhz = 1
        case n8Mhz = 2
        case more = 3
    }
    public enum StepBootstrap: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepL1: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum StepService: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    /// A bootstrap was seen recently.
    public var bootstrapSeen: Bool
    /// Bootstraps found.
    public var bootstraps: Int64
    /// Frames taken.
    public var framesTaken: Int64
    /// Frames decoded.
    public var framesOk: Int64
    /// The carrier offset of the last bootstrap.
    public var offsetHz: Double?
    /// The minor version of the bootstrap.
    public var minorVersion: Int64?
    /// The system bandwidth that the bootstrap signals.
    public var systemBandwidth: SystemBandwidth?
    /// The preamble structure, 0 to 159.
    public var preambleStructure: Int64?
    /// The minimum time to the next frame.
    public var minTimeNext: Double?
    /// The sample rate after the bootstrap.
    public var postRate: Double?
    /// The emergency alert wake-up.
    public var eaWakeUp: Bool?
    /// The time between the last two bootstraps.
    public var frameLength: Double?
    /// Service list tables read.
    public var llsTables: Int64
    /// Signed multi-tables or signed signalling packages arrived in the last 10 s.
    public var signedSignalling: Bool
    /// TAI minus UTC from the system time table.
    public var leapSeconds: Int64?
    /// The service chosen with `service.select`. No value for the automatic choice.
    public var service: Int64?
    /// The service received: the chosen one, or the first visible linear TV service delivered by ROUTE.
    public var serviceReceived: Int64?
    /// ROUTE objects taken.
    public var routeObjects: Int64
    /// Access units given out as media packets, video and audio.
    public var mediaSamples: Int64
    /// The bootstrap step.
    public var stepBootstrap: StepBootstrap?
    /// The L1 step.
    public var stepL1: StepL1?
    /// The service step: green while a component of the received service is given.
    public var stepService: StepService?
    /// Video access units given out as media packets.
    public var videoPackets: Int64
    /// Audio access units given out as media packets.
    public var audioPackets: Int64

    init(_ c: sdrkit_atsc3_fields) {
        bootstrapSeen = c.bootstrap_seen
        bootstraps = c.bootstraps
        framesTaken = c.frames_taken
        framesOk = c.frames_ok
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        minorVersion = c.has_minor_version ? c.minor_version : nil
        systemBandwidth = c.has_system_bandwidth ? SystemBandwidth(rawValue: c.system_bandwidth) : nil
        preambleStructure = c.has_preamble_structure ? c.preamble_structure : nil
        minTimeNext = c.has_min_time_next ? c.min_time_next : nil
        postRate = c.has_post_rate ? c.post_rate : nil
        eaWakeUp = c.has_ea_wake_up ? c.ea_wake_up : nil
        frameLength = c.has_frame_length ? c.frame_length : nil
        llsTables = c.lls_tables
        signedSignalling = c.signed_signalling
        leapSeconds = c.has_leap_seconds ? c.leap_seconds : nil
        service = c.has_service ? c.service : nil
        serviceReceived = c.has_service_received ? c.service_received : nil
        routeObjects = c.route_objects
        mediaSamples = c.media_samples
        stepBootstrap = StepBootstrap(rawValue: c.step_bootstrap)
        stepL1 = StepL1(rawValue: c.step_l1)
        stepService = StepService(rawValue: c.step_service)
        videoPackets = c.video_packets
        audioPackets = c.audio_packets
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Atsc3Fields, SDRKitCode> {
        var c = sdrkit_atsc3_fields()
        let code = sdrkit_atsc3_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Atsc3Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `atsc3` about the frames: what the L1 signalling says, the PLP that is decoded and its FEC blocks. A value of the signalling has no value before L1-Basic is read, a value of the PLP none before a FEC block is decoded.
public struct Atsc3FrameFields {
    public enum NotDecoded: Int32 {
        case structure = 0
        case sampleRate = 1
        case l1Basic = 2
        case l1Detail = 3
        case additionalParity = 4
        case signalling = 5
        case mimo = 6
        case miso = 7
        case toneReservation = 8
        case plp = 9
        case samples = 10
        case ldm = 11
        case length = 12
        case fec = 13
    }
    public enum StepPlp: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    /// Frames that were not decoded.
    public var framesFailed: Int64
    /// Why the last frame was not decoded. No value after a decoded frame.
    public var notDecoded: NotDecoded?
    /// The FFT size of the Preamble and the first subframe, points.
    public var fftSize: Int64?
    /// The guard interval of the Preamble and the first subframe.
    public var guardSamples: Int64?
    /// The L1-Basic mode that the bootstrap signals, 1 to 5.
    public var l1BasicMode: Int64?
    /// The L1-Detail mode, 1 to 7.
    public var l1DetailMode: Int64?
    /// The Preamble symbols of the frame.
    public var preambleSymbols: Int64?
    /// The subframes of the frame. The first one is decoded.
    public var subframes: Int64?
    /// The data and subframe boundary symbols of the first subframe.
    public var subframeSymbols: Int64?
    /// The spacing of the carriers that bear scattered pilots in the first subframe.
    public var pilotDx: Int64?
    /// The symbols of one scattered pilot sequence in the first subframe.
    public var pilotDy: Int64?
    /// The PLPs of the frame: the different PLP ids in all its subframes.
    public var plps: Int64?
    /// The id of the PLP that is decoded.
    public var plpId: Int64?
    /// The constellation points of the PLP that is decoded: 4, 16, 64 or 256.
    public var plpPoints: Int64?
    /// The code rate of the PLP that is decoded, in fifteenths.
    public var plpCodeRate: Int64?
    /// The LDPC code word of the PLP that is decoded: 16200 or 64800 bits.
    public var plpLdpcBits: Int64?
    /// FEC blocks of the PLP that ended in the last frame.
    public var fecBlocks: Int64
    /// Those of them that decoded.
    public var fecBlocksGood: Int64
    /// Baseband packets given out.
    public var basebandPackets: Int64
    /// FEC blocks whose baseband packet was not good.
    public var basebandPacketsBad: Int64
    /// Bits that the outer BCH code corrected after the LDPC decoder.
    public var outerCorrected: Int64
    /// The mean LDPC iterations of the FEC blocks of the last frame.
    public var ldpcIterations: Double?
    /// The MER of the cells of the decoded PLP against the nearest constellation points.
    public var merDb: Double?
    /// The signal to noise ratio that the pilots of the first subframe give.
    public var pilotSnrDb: Double?
    /// The PLP step: green when more than 99 % of the baseband packets are good, amber above 50 %, red below.
    public var stepPlp: StepPlp?

    init(_ c: sdrkit_atsc3_frame_fields) {
        framesFailed = c.frames_failed
        notDecoded = c.has_not_decoded ? NotDecoded(rawValue: c.not_decoded) : nil
        fftSize = c.has_fft_size ? c.fft_size : nil
        guardSamples = c.has_guard_samples ? c.guard_samples : nil
        l1BasicMode = c.has_l1_basic_mode ? c.l1_basic_mode : nil
        l1DetailMode = c.has_l1_detail_mode ? c.l1_detail_mode : nil
        preambleSymbols = c.has_preamble_symbols ? c.preamble_symbols : nil
        subframes = c.has_subframes ? c.subframes : nil
        subframeSymbols = c.has_subframe_symbols ? c.subframe_symbols : nil
        pilotDx = c.has_pilot_dx ? c.pilot_dx : nil
        pilotDy = c.has_pilot_dy ? c.pilot_dy : nil
        plps = c.has_plps ? c.plps : nil
        plpId = c.has_plp_id ? c.plp_id : nil
        plpPoints = c.has_plp_points ? c.plp_points : nil
        plpCodeRate = c.has_plp_code_rate ? c.plp_code_rate : nil
        plpLdpcBits = c.has_plp_ldpc_bits ? c.plp_ldpc_bits : nil
        fecBlocks = c.fec_blocks
        fecBlocksGood = c.fec_blocks_good
        basebandPackets = c.baseband_packets
        basebandPacketsBad = c.baseband_packets_bad
        outerCorrected = c.outer_corrected
        ldpcIterations = c.has_ldpc_iterations ? c.ldpc_iterations : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        pilotSnrDb = c.has_pilot_snr_db ? c.pilot_snr_db : nil
        stepPlp = StepPlp(rawValue: c.step_plp)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Atsc3FrameFields, SDRKitCode> {
        var c = sdrkit_atsc3_frame_fields()
        let code = sdrkit_atsc3_frame_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Atsc3FrameFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the services of the service list. The row key is the service id; `service.select` takes it.
public struct Atsc3ServiceRow {
    public enum Category: Int32 {
        case tv = 0
        case radio = 1
        case app = 2
        case guide = 3
        case alerts = 4
        case drm = 5
        case other = 6
    }
    public enum Delivery: Int32 {
        case route = 0
        case mmtp = 1
        case other = 2
    }
    public enum Action: Int32 {
        case receive = 0
        case waiting = 1
        case receiving = 2
        case unsupported = 3
        case encrypted = 4
    }
    /// The row key.
    public var key: String
    /// The major channel number.
    public var major: Int64
    /// The minor channel number.
    public var minor: Int64
    /// The short name.
    public var name: String
    /// The service is hidden.
    public var hidden: Bool
    /// The service id.
    public var id: Int64
    /// The category of the service.
    public var category: Category?
    /// How the signalling is delivered.
    public var delivery: Delivery?
    /// What the receiver does with the service.
    public var action: Action?
    /// Service layer signalling objects received on TSI 0 of the ROUTE session of the service.
    public var slsObjects: Int64
    /// Signalling documents (USBD, S-TSID, MPD and others) read from those objects.
    public var slsDocuments: Int64
    /// Those objects that were signed packages. The signature is not verified.
    public var slsSignedPackages: Int64
    /// The service list says that a component needed to present the service is protected, for example encrypted.
    public var protected: Bool

    init(_ c: sdrkit_atsc3_service_row) {
        key = sdrkitText(c.key)
        major = c.major
        minor = c.minor
        name = sdrkitText(c.name)
        hidden = c.hidden
        id = c.id
        category = Category(rawValue: c.category)
        delivery = Delivery(rawValue: c.delivery)
        action = Action(rawValue: c.action)
        slsObjects = c.sls_objects
        slsDocuments = c.sls_documents
        slsSignedPackages = c.sls_signed_packages
        protected = c.protected_
    }
}

/// A row of the media components seen, in the order they appeared. The row key is the destination address and port of the ROUTE session and the transport session id of the component: `239.255.10.2:5002/1`.
public struct Atsc3ComponentRow {
    public enum Kind: Int32 {
        case video = 0
        case audio = 1
        case subtitles = 2
        case other = 3
    }
    public enum State: Int32 {
        case given = 0
        case otherService = 1
        case otherTrack = 2
        case encrypted = 3
        case noFormat = 4
        case unbound = 5
    }
    /// The row key.
    public var key: String
    /// The transport session id of the component in its ROUTE session.
    public var tsi: Int64
    /// The sample entry type of the component: the four-character code of its codec.
    public var fourcc: String
    /// The destination address and port of the ROUTE session.
    public var destination: String
    /// The service whose session description names the component. No value when none that was read names it.
    public var service: Int64?
    /// What the component carries.
    public var kind: Kind?
    /// What the stack does with the component.
    public var state: State?

    init(_ c: sdrkit_atsc3_component_row) {
        key = sdrkitText(c.key)
        tsi = c.tsi
        fourcc = sdrkitText(c.fourcc)
        destination = sdrkitText(c.destination)
        service = c.has_service ? c.service : nil
        kind = Kind(rawValue: c.kind)
        state = State(rawValue: c.state)
    }
}

/// A row of the PLPs of the last frame: one row for each PLP of each subframe. The row key is the subframe number, a point and the PLP id.
public struct Atsc3PlpRow {
    /// The row key.
    public var key: String
    /// The number of the subframe in the frame, from 0.
    public var subframe: Int64
    /// The PLP id.
    public var id: Int64
    /// The layer: 0 core, above 0 enhanced.
    public var layer: Int64
    /// The constellation points, also of a coding that the stack does not decode. No value for a reserved code.
    public var points: Int64?
    /// The code rate in fifteenths.
    public var codeRate: Int64?
    /// The LDPC code word, bits.
    public var ldpcBits: Int64?
    /// The data cells of the PLP in the subframe.
    public var cells: Int64
    /// The time interleaver: 0 none, 1 convolutional, 2 hybrid.
    public var timeInterleaver: Int64
    /// The PLP is the one that the stack decodes.
    public var decoded: Bool
    /// The channel bonding format (table 9.25): 0 plain, 1 SNR averaging, 2 and 3 reserved. No value when the PLP is not bonded with another RF channel.
    public var bonding: Int64?
    /// The BSID of the RF channel that the PLP is bonded with, as L1-Detail lists it.
    public var bondedBsid: Int64?

    init(_ c: sdrkit_atsc3_plp_row) {
        key = sdrkitText(c.key)
        subframe = c.subframe
        id = c.id
        layer = c.layer
        points = c.has_points ? c.points : nil
        codeRate = c.has_code_rate ? c.code_rate : nil
        ldpcBits = c.has_ldpc_bits ? c.ldpc_bits : nil
        cells = c.cells
        timeInterleaver = c.time_interleaver
        decoded = c.decoded
        bonding = c.has_bonding ? c.bonding : nil
        bondedBsid = c.has_bonded_bsid ? c.bonded_bsid : nil
    }
}

/// The result fields of `dtmb`: the frame headers, the system information, the decoder and the signal. A value of the header has no value while no header is tracked, and a value of the profile none before the system information is decoded.
public struct DtmbFields {
    public enum Header: Int32 {
        case pn420 = 0
        case pn595 = 1
        case pn945 = 2
    }
    public enum StepPnSync: Int32 {
        case off = 0
        case amber = 1
        case green = 2
        case red = 3
    }
    public enum Mapping: Int32 {
        case n4qamNr = 0
        case n4qam = 1
        case n16qam = 2
        case n32qam = 3
        case n64qam = 4
    }
    public enum CodeRate: Int32 {
        case n04 = 0
        case n06 = 1
        case n08 = 2
    }
    public enum Interleaver: Int32 {
        case mode1 = 0
        case mode2 = 1
    }
    public enum Carriers: Int32 {
        case n3780 = 0
        case n1 = 1
    }
    /// Detection level: 0 nothing, 1 frame headers tracked, 2 system information decoded, 3 transport stream locked.
    public var detection: Int64
    /// The header grid holds.
    public var tracking: Bool
    /// The frame header.
    public var header: Header?
    /// The phase of the header changes from frame to frame.
    public var pnPhaseRotates: Bool?
    /// The channel width.
    public var width: Double
    /// The length of a signal frame (header and body).
    public var frameLength: Double?
    /// C/N from the fit of the header.
    public var cnDb: Double?
    /// The time between the first and the last significant path.
    public var echoSpan: Double?
    /// Frames with a usable header.
    public var frames: Int64
    /// Frames erased: their header window was exact silence (a zero-filled gap of the source).
    public var framesErased: Int64
    /// The share of recent frames without a usable header.
    public var noHeaderPct: Double
    /// The input rate is enough for the width.
    public var rateOk: Bool
    /// The header sync step.
    public var stepPnSync: StepPnSync?
    /// The system information index, 3 to 24.
    public var siIndex: Int64?
    /// The normalised score of the leading system information word (1 is perfect). No value while no header is tracked.
    public var siCorr: Double?
    /// The mapping.
    public var mapping: Mapping?
    /// The LDPC rate.
    public var codeRate: CodeRate?
    /// The time interleaver.
    public var interleaver: Interleaver?
    /// The carrier form.
    public var carriers: Carriers?
    /// The MER of the equalised data symbols.
    public var merDb: Double?
    /// LDPC code words that decoded.
    public var ldpcOk: Int64
    /// LDPC code words that did not.
    public var ldpcBad: Int64
    /// The mean LDPC iterations of recent code words.
    public var ldpcIterations: Double
    /// BCH blocks with one bit corrected.
    public var bchCorrected: Int64
    /// Code words dropped for lack of time.
    public var codewordsDropped: Int64
    /// Code words not tried during a long run of failures.
    public var codewordsSkipped: Int64
    /// Transport packets delivered.
    public var packets: Int64
    /// A good packet came out within the last 0.7 s.
    public var tsLock: Bool
    /// The net bit rate of the signalled profile. No value before the profile and the header are known.
    public var payloadBps: Double?
    /// What the stack decodes and which of its tables no public document confirms.
    public var note: String
    /// The carrier offset. No value while no header is tracked.
    public var offsetHz: Double?
    /// The error of the symbol clock. No value while no header is tracked.
    public var clockPpm: Double?
    /// The level of the input.
    public var levelDbfs: Double
    /// The SNR: the MER once the profile is known and it lies between 8 dB and the C/N, else the C/N.
    public var snrDb: Double?

    init(_ c: sdrkit_dtmb_fields) {
        detection = c.detection
        tracking = c.tracking
        header = c.has_header ? Header(rawValue: c.header) : nil
        pnPhaseRotates = c.has_pn_phase_rotates ? c.pn_phase_rotates : nil
        width = c.width
        frameLength = c.has_frame_length ? c.frame_length : nil
        cnDb = c.has_cn_db ? c.cn_db : nil
        echoSpan = c.has_echo_span ? c.echo_span : nil
        frames = c.frames
        framesErased = c.frames_erased
        noHeaderPct = c.no_header_pct
        rateOk = c.rate_ok
        stepPnSync = StepPnSync(rawValue: c.step_pn_sync)
        siIndex = c.has_si_index ? c.si_index : nil
        siCorr = c.has_si_corr ? c.si_corr : nil
        mapping = c.has_mapping ? Mapping(rawValue: c.mapping) : nil
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        interleaver = c.has_interleaver ? Interleaver(rawValue: c.interleaver) : nil
        carriers = c.has_carriers ? Carriers(rawValue: c.carriers) : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        ldpcOk = c.ldpc_ok
        ldpcBad = c.ldpc_bad
        ldpcIterations = c.ldpc_iterations
        bchCorrected = c.bch_corrected
        codewordsDropped = c.codewords_dropped
        codewordsSkipped = c.codewords_skipped
        packets = c.packets
        tsLock = c.ts_lock
        payloadBps = c.has_payload_bps ? c.payload_bps : nil
        note = sdrkitText(c.note)
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        clockPpm = c.has_clock_ppm ? c.clock_ppm : nil
        levelDbfs = c.level_dbfs
        snrDb = c.has_snr_db ? c.snr_db : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<DtmbFields, SDRKitCode> {
        var c = sdrkit_dtmb_fields()
        let code = sdrkit_dtmb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(DtmbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result fields of `tv-auto`: the line shown, the capture and the last detection. A value of the detection has no value before the first detection.
public struct TvAutoFields {
    public enum Standard: Int32 {
        case none = 0
        case unknown = 1
        case dvbt = 2
        case dvbt2 = 3
        case isdbt = 4
        case atsc = 5
        case atsc3 = 6
        case dtmb = 7
        case dvbc = 8
        case dvbs = 9
        case analog = 10
    }
    public enum Verdict: Int32 {
        case noSignal = 0
        case unsure = 1
        case likely = 2
        case sure = 3
    }
    /// The line shown ("Listening to the channel...").
    public var line: String
    /// The progress of the capture.
    public var progressPct: Double
    /// Captures looked at.
    public var tries: Int64
    /// The id of the stack requested, when one is found.
    public var switchTo: String?
    /// The standard of the last detection.
    public var standard: Standard?
    /// The headline of the last detection.
    public var headline: String?
    /// The verdict of the last detection.
    public var verdict: Verdict?
    /// The confidence of the last detection.
    public var confidencePct: Double?
    /// The channel width found.
    public var width: Double?
    /// The centre of the channel from the centre of the capture.
    public var centreOffset: Double?
    /// The detector stopped: the command `try_again` starts it again.
    public var tryAgainOffered: Bool

    init(_ c: sdrkit_tv_auto_fields) {
        line = sdrkitText(c.line)
        progressPct = c.progress_pct
        tries = c.tries
        switchTo = c.has_switch_to ? sdrkitText(c.switch_to) : nil
        standard = c.has_standard ? Standard(rawValue: c.standard) : nil
        headline = c.has_headline ? sdrkitText(c.headline) : nil
        verdict = c.has_verdict ? Verdict(rawValue: c.verdict) : nil
        confidencePct = c.has_confidence_pct ? c.confidence_pct : nil
        width = c.has_width ? c.width : nil
        centreOffset = c.has_centre_offset ? c.centre_offset : nil
        tryAgainOffered = c.try_again_offered
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TvAutoFields, SDRKitCode> {
        var c = sdrkit_tv_auto_fields()
        let code = sdrkit_tv_auto_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TvAutoFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct TvAutoDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_tv_auto_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<TvAutoDecoderFields, SDRKitCode> {
        var c = sdrkit_tv_auto_decoder_fields()
        let code = sdrkit_tv_auto_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(TvAutoDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of the event `isdbt.tmcc`.
public struct IsdbtTmccEvent {
    public enum Urgency: Int32 {
        case none = 0
        case notice = 1
        case urgent = 2
        case emergency = 3
    }
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The system identification (0 ISDB-T, 1 ISDB-TSB).
    public var systemId: Int64
    /// The parameter switching countdown (15: no switch announced).
    public var switchCountdown: Int64
    /// The emergency alarm start flag.
    public var emergencyAlarm: Bool
    /// The partial reception flag.
    public var partialReception: Bool
    /// The layers in use ("A 1 QPSK 2/3, B 12 64QAM 3/4").
    public var text: String
    /// An emergency when the emergency alarm start flag is set.
    public var urgency: Urgency?
    /// The family of the source: a broadcast.
    public var family: Family?

    init(_ c: sdrkit_isdbt_tmcc_event) {
        time = c.time
        freqHz = c.freq_hz
        systemId = c.system_id
        switchCountdown = c.switch_countdown
        emergencyAlarm = c.emergency_alarm
        partialReception = c.partial_reception
        text = sdrkitText(c.text)
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<IsdbtTmccEvent, SDRKitCode> {
        var c = sdrkit_isdbt_tmcc_event()
        let code = sdrkit_isdbt_tmcc_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(IsdbtTmccEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a standard found. The text is the headline.
public struct TvAutoFoundEvent {
    public enum Standard: Int32 {
        case none = 0
        case unknown = 1
        case dvbt = 2
        case dvbt2 = 3
        case isdbt = 4
        case atsc = 5
        case atsc3 = 6
        case dtmb = 7
        case dvbc = 8
        case dvbs = 9
        case analog = 10
    }
    public enum Family: Int32 {
        case aircraft = 0
        case vessel = 1
        case aprs = 2
        case pager = 3
        case sonde = 4
        case broadcast = 5
        case voice = 6
        case message = 7
        case sensor = 8
        case satellite = 9
        case receiver = 10
        case alert = 11
        case other = 12
    }
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var freqHz: Double
    /// The id of the stack that receives the standard found.
    public var stack: String
    /// The standard found.
    public var standard: Standard?
    /// The headline.
    public var text: String
    /// The channel width found.
    public var width: Double
    /// The detector is sure.
    public var sure: Bool
    /// The family of the source: a receiver.
    public var family: Family?

    init(_ c: sdrkit_tv_auto_found_event) {
        time = c.time
        freqHz = c.freq_hz
        stack = sdrkitText(c.stack)
        standard = Standard(rawValue: c.standard)
        text = sdrkitText(c.text)
        width = c.width
        sure = c.sure
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<TvAutoFoundEvent, SDRKitCode> {
        var c = sdrkit_tv_auto_found_event()
        let code = sdrkit_tv_auto_found_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(TvAutoFoundEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of
/// a session.
public enum TvterrComponents {
    /// The result fields of the component "dc" of the stack "isdbt" in the snapshot of `handle`.
    public static func isdbtDcRemovalFields(_ handle: SDRKIT_HANDLE) -> Result<GraphDcRemovalFields, SDRKitCode> {
        var c = sdrkit_graph_dc_removal_fields()
        let code = sdrkit_isdbt_dc_removal_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GraphDcRemovalFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "isdbt" in the snapshot of `handle`.
    public static func isdbtTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_isdbt_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "isdbt" in the snapshot of `handle`.
    public static func isdbtPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_isdbt_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "dc" of the stack "isdbtsb" in the snapshot of `handle`.
    public static func isdbtsbDcRemovalFields(_ handle: SDRKIT_HANDLE) -> Result<GraphDcRemovalFields, SDRKitCode> {
        var c = sdrkit_graph_dc_removal_fields()
        let code = sdrkit_isdbtsb_dc_removal_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(GraphDcRemovalFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "isdbtsb" in the snapshot of `handle`.
    public static func isdbtsbTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_isdbtsb_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "isdbtsb" in the snapshot of `handle`.
    public static func isdbtsbPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_isdbtsb_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "atsc" in the snapshot of `handle`.
    public static func atscTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_atsc_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "atsc" in the snapshot of `handle`.
    public static func atscPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_atsc_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "alp" of the stack "atsc3" in the snapshot of `handle`.
    public static func atsc3AlpFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastAlpFields, SDRKitCode> {
        var c = sdrkit_ipcast_alp_fields()
        let code = sdrkit_atsc3_alp_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastAlpFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ip" of the stack "atsc3" in the snapshot of `handle`.
    public static func atsc3IpFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastIpFields, SDRKitCode> {
        var c = sdrkit_ipcast_ip_fields()
        let code = sdrkit_atsc3_ip_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastIpFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "route" of the stack "atsc3" in the snapshot of `handle`.
    public static func atsc3RouteFields(_ handle: SDRKIT_HANDLE) -> Result<IpcastRouteFields, SDRKitCode> {
        var c = sdrkit_ipcast_route_fields()
        let code = sdrkit_atsc3_route_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(IpcastRouteFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "ts_demux" of the stack "dtmb" in the snapshot of `handle`.
    public static func dtmbTsDemuxFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsTsDemuxFields, SDRKitCode> {
        var c = sdrkit_mpegts_ts_demux_fields()
        let code = sdrkit_dtmb_ts_demux_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsTsDemuxFields(c)) : .failure(SDRKitCode(code: code))
    }
    /// The result fields of the component "psi_si" of the stack "dtmb" in the snapshot of `handle`.
    public static func dtmbPsiSiFields(_ handle: SDRKIT_HANDLE) -> Result<MpegtsPsiSiFields, SDRKitCode> {
        var c = sdrkit_mpegts_psi_si_fields()
        let code = sdrkit_dtmb_psi_si_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(MpegtsPsiSiFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum TvterrTables {
    /// The changed rows of the table "layers" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtLayers(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[IsdbtLayerRow], SDRKitCode> {
        var rows = [sdrkit_isdbt_layer_row](repeating: sdrkit_isdbt_layer_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_layers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsdbtLayerRow.init))
    }
    /// The changed rows of the table "services" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "isdbt", newest first, at most `capacity` rows.
    public static func isdbtPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbt_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "layers" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbLayers(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[IsdbtLayerRow], SDRKitCode> {
        var rows = [sdrkit_isdbt_layer_row](repeating: sdrkit_isdbt_layer_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_layers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(IsdbtLayerRow.init))
    }
    /// The changed rows of the table "services" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "isdbtsb", newest first, at most `capacity` rows.
    public static func isdbtsbPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_isdbtsb_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "services" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "atsc", newest first, at most `capacity` rows.
    public static func atscPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
    /// The changed rows of the table "services" of the stack "atsc3", newest first, at most `capacity` rows.
    /// The column `name` names a row.
    public static func atsc3Services(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Atsc3ServiceRow], SDRKitCode> {
        var rows = [sdrkit_atsc3_service_row](repeating: sdrkit_atsc3_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc3_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Atsc3ServiceRow.init))
    }
    /// The changed rows of the table "components" of the stack "atsc3", newest first, at most `capacity` rows.
    public static func atsc3Components(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Atsc3ComponentRow], SDRKitCode> {
        var rows = [sdrkit_atsc3_component_row](repeating: sdrkit_atsc3_component_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc3_components_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Atsc3ComponentRow.init))
    }
    /// The changed rows of the table "plps" of the stack "atsc3", newest first, at most `capacity` rows.
    public static func atsc3Plps(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[Atsc3PlpRow], SDRKitCode> {
        var rows = [sdrkit_atsc3_plp_row](repeating: sdrkit_atsc3_plp_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_atsc3_plps_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(Atsc3PlpRow.init))
    }
    /// The changed rows of the table "services" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbServices(_ handle: SDRKIT_HANDLE, capacity: Int = 1000) -> Result<[MpegtsTsServiceRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_service_row](repeating: sdrkit_mpegts_ts_service_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_services_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsServiceRow.init))
    }
    /// The changed rows of the table "tracks" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbTracks(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsTrackRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_track_row](repeating: sdrkit_mpegts_ts_track_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_tracks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsTrackRow.init))
    }
    /// The changed rows of the table "guide" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbGuide(_ handle: SDRKIT_HANDLE, capacity: Int = 20000) -> Result<[MpegtsTsGuideRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_row](repeating: sdrkit_mpegts_ts_guide_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_guide_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRow.init))
    }
    /// The changed rows of the table "guide_ratings" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbGuideRatings(_ handle: SDRKIT_HANDLE, capacity: Int = 40000) -> Result<[MpegtsTsGuideRatingRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_guide_rating_row](repeating: sdrkit_mpegts_ts_guide_rating_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_guide_ratings_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsGuideRatingRow.init))
    }
    /// The changed rows of the table "ts_demux.pids" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbTsDemuxPids(_ handle: SDRKIT_HANDLE, capacity: Int = 8192) -> Result<[MpegtsTsPidRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_pid_row](repeating: sdrkit_mpegts_ts_pid_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_ts_demux_pids_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsPidRow.init))
    }
    /// The changed rows of the table "psi_si.sections" of the stack "dtmb", newest first, at most `capacity` rows.
    public static func dtmbPsiSiSections(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[MpegtsTsSectionRow], SDRKitCode> {
        var rows = [sdrkit_mpegts_ts_section_row](repeating: sdrkit_mpegts_ts_section_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_dtmb_psi_si_sections_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(MpegtsTsSectionRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum TvterrCommands {
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func isdbtServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_isdbt_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func isdbtTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_isdbt_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func isdbtsbServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_isdbtsb_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func isdbtsbTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_isdbtsb_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func atscServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atsc_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func atscTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atsc_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func atsc3ServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_atsc3_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a service. Plays a service of the multiplex. An empty row plays the first service that can play.
    public static func dtmbServiceSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_service_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_service_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dtmb_service_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Select a track. Plays an audio track or shows a subtitle track.
    public static func dtmbTrackSelect(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_tvterr_track_select_args()
        args.size = UInt32(MemoryLayout<sdrkit_tvterr_track_select_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_dtmb_track_select(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func tvAutoTryAgain(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_tv_auto_try_again(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum TvterrSettings {
    /// The options of the setting "width" of the stack "isdbt".
    public enum IsdbtWidth: Int32 {
        case n6Mhz = 0
        case n7Mhz = 1
        case n8Mhz = 2
        case auto = 3
    }
    /// Channel width.
    public static func isdbtWidth(_ handle: SDRKIT_HANDLE) -> Result<IsdbtWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "width", &v)
        return code == SDRKIT_OK ? .success(IsdbtWidth(rawValue: v) ?? IsdbtWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtWidth(_ handle: SDRKIT_HANDLE, _ value: IsdbtWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "width", value.rawValue))
    }
    /// Decode video.
    public static func isdbtDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// The options of the setting "width" of the stack "isdbtsb".
    public enum IsdbtsbWidth: Int32 {
        case n6Mhz = 0
        case n7Mhz = 1
        case n8Mhz = 2
        case auto = 3
    }
    /// Channel width.
    public static func isdbtsbWidth(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "width", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbWidth(rawValue: v) ?? IsdbtsbWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbWidth(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "width", value.rawValue))
    }
    /// The options of the setting "segments" of the stack "isdbtsb".
    public enum IsdbtsbSegments: Int32 {
        case auto = 0
        case n1 = 1
        case n3 = 2
    }
    /// Segments.
    public static func isdbtsbSegments(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbSegments, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "segments", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbSegments(rawValue: v) ?? IsdbtsbSegments(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbSegments(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbSegments) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "segments", value.rawValue))
    }
    /// The options of the setting "edge_pilot" of the stack "isdbtsb".
    public enum IsdbtsbEdgePilot: Int32 {
        case sequence = 0
        case plus = 1
        case minus = 2
    }
    /// Upper-edge pilot.
    public static func isdbtsbEdgePilot(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbEdgePilot, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "edge_pilot", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbEdgePilot(rawValue: v) ?? IsdbtsbEdgePilot(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbEdgePilot(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbEdgePilot) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "edge_pilot", value.rawValue))
    }
    /// Decode video.
    public static func isdbtsbDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Decode video.
    public static func atscDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtscDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// PLP.
    public static func atsc3Plp(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "plp", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setAtsc3Plp(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "plp", value))
    }
    /// The options of the setting "width" of the stack "dtmb".
    public enum DtmbWidth: Int32 {
        case n8Mhz = 0
        case n6Mhz = 1
    }
    /// Channel.
    public static func dtmbWidth(_ handle: SDRKIT_HANDLE) -> Result<DtmbWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "width", &v)
        return code == SDRKIT_OK ? .success(DtmbWidth(rawValue: v) ?? DtmbWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setDtmbWidth(_ handle: SDRKIT_HANDLE, _ value: DtmbWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "width", value.rawValue))
    }
    /// Decode video.
    public static func dtmbDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setDtmbDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
    /// Satellite input.
    public static func tvAutoSatellite(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "satellite", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTvAutoSatellite(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "satellite", value))
    }
    /// Decode video.
    public static func tvAutoDecodeVideo(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "decode_video", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setTvAutoDecodeVideo(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "decode_video", value))
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum TvterrGeneratorSettings {
    /// The options of the setting "gen_mode" of the stack "isdbtsb".
    public enum IsdbtsbMode: Int32 {
        case mode1 = 0
        case mode2 = 1
        case mode3 = 2
    }
    /// Test signal mode.
    public static func isdbtsbMode(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbMode, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbMode(rawValue: v) ?? IsdbtsbMode(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbMode(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbMode) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_mode", value.rawValue))
    }
    /// The options of the setting "gen_guard" of the stack "isdbtsb".
    public enum IsdbtsbGuard: Int32 {
        case n14 = 0
        case n18 = 1
        case n116 = 2
        case n132 = 3
    }
    /// Test signal guard.
    public static func isdbtsbGuard(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbGuard, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_guard", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbGuard(rawValue: v) ?? IsdbtsbGuard(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbGuard(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbGuard) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_guard", value.rawValue))
    }
    /// The options of the setting "gen_modulation" of the stack "isdbtsb".
    public enum IsdbtsbModulation: Int32 {
        case qpsk = 0
        case n16qam = 1
        case n64qam = 2
    }
    /// Test signal layer B modulation.
    public static func isdbtsbModulation(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbModulation, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_modulation", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbModulation(rawValue: v) ?? IsdbtsbModulation(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbModulation(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbModulation) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_modulation", value.rawValue))
    }
    /// The options of the setting "gen_code_rate" of the stack "isdbtsb".
    public enum IsdbtsbCodeRate: Int32 {
        case n12 = 0
        case n23 = 1
        case n34 = 2
        case n56 = 3
        case n78 = 4
    }
    /// Test signal layer B code rate.
    public static func isdbtsbCodeRate(_ handle: SDRKIT_HANDLE) -> Result<IsdbtsbCodeRate, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_code_rate", &v)
        return code == SDRKIT_OK ? .success(IsdbtsbCodeRate(rawValue: v) ?? IsdbtsbCodeRate(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setIsdbtsbCodeRate(_ handle: SDRKIT_HANDLE, _ value: IsdbtsbCodeRate) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_code_rate", value.rawValue))
    }
}
