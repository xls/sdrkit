// sdrkit: the typed interface of the shared interface "dvbs2", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum Dvbs2Interface {
    public static let major = 1
    public static let minor = 0
}

/// The result fields of the DVB-S/S2 receiver: the carrier search, the frames, the code and the locks. A measurement of the carrier search has no value without a carrier.
public struct Dvbs2Fields {
    public enum Standard: Int32 {
        case dvbs = 0
        case dvbs2 = 1
    }
    public enum SignalNote: Int32 {
        case noCarrier = 0
        case notDecoded = 1
    }
    public enum RollOffSource: Int32 {
        case assumed = 0
        case measured = 1
        case signalled = 2
    }
    public enum Modulation: Int32 {
        case qPSK = 0
        case n8PSK = 1
        case n16APSK = 2
        case n32APSK = 3
    }
    public enum CodeRate: Int32 {
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
    public enum Frame: Int32 {
        case normal = 0
        case short = 1
    }
    /// The standard received.
    public var standard: Standard?
    /// A note on the signal for the user.
    public var signalNote: SignalNote?
    /// The symbol rate in use. No value before a rate is set or measured.
    public var symbolRateHz: Double?
    /// The symbol rate from the spectrum (width or cyclic line).
    public var spectrumRate: Double?
    /// The error of the symbol clock.
    public var clockPpm: Double
    /// The symbol rate was set by hand.
    public var manualRate: Bool
    /// The plateau of the carrier over the noise floor.
    public var spectrumSnrDb: Double?
    /// The misfit of the raised cosine fit.
    public var misfit: Double?
    /// The lower -3 dB edge of the carrier, from the centre.
    public var edgeLow: Double?
    /// The upper -3 dB edge of the carrier, from the centre.
    public var edgeHigh: Double?
    /// The carrier offset from the centre. No value without a carrier.
    public var offsetHz: Double?
    /// The spectrum is inverted.
    public var inverted: Bool
    /// The roll-off.
    public var rollOff: Double
    /// Where the roll-off came from.
    public var rollOffSource: RollOffSource?
    /// The modulation.
    public var modulation: Modulation?
    /// The code rate.
    public var codeRate: CodeRate?
    /// The MODCOD (DVB-S2).
    public var modcod: Int64?
    /// The FEC frame size (DVB-S2).
    public var frame: Frame?
    /// Pilots are on.
    public var pilots: Bool
    /// The MODCOD changes between frames.
    public var vcm: Bool
    /// The carrier has one input stream.
    public var singleStream: Bool
    /// The PL scrambling code in use.
    public var plScramblingCode: Int64
    /// PLFRAMEs seen.
    public var plframes: Int64
    /// Dummy PLFRAMEs seen.
    public var dummyFrames: Int64
    /// Frames followed but not decoded.
    public var unsupportedFrames: Int64
    /// Frames skipped by the MODCOD rule or the frame length rule.
    public var otherModcodFrames: Int64
    /// Frames whose pilots differ from the setting.
    public var pilotMismatch: Int64
    /// BBHEADERs refused (CRC-8 or length).
    public var bbheaderBad: Int64
    /// The net bit rate of the data fields.
    public var netBitRate: Double
    /// Es/N0.
    public var esn0Db: Double?
    /// The modulation error ratio.
    public var merDb: Double?
    /// Eb/N0.
    public var ebn0Db: Double?
    /// Es/N0 above the quasi-error-free level of EN 302 307-1 table 13.
    public var marginToQefDb: Double?
    /// LDPC iterations, recent mean.
    public var ldpcIterations: Double
    /// BCH words good.
    public var bchOk: Int64
    /// BCH words that failed.
    public var bchBad: Int64
    /// Reed-Solomon words with no error (DVB-S).
    public var rsClean: Int64
    /// Reed-Solomon words corrected (DVB-S).
    public var rsCorrected: Int64
    /// Reed-Solomon words that failed (DVB-S).
    public var rsFailed: Int64
    /// A carrier is found.
    public var carrierLock: Bool
    /// The symbol timing runs.
    public var timingLock: Bool
    /// The carrier phase and the framing hold.
    public var frameLock: Bool
    /// The FEC decodes words.
    public var fecLock: Bool
    /// The transport stream is valid.
    public var tsLock: Bool
    /// The level of the input.
    public var levelDbfs: Double
    /// The LNB supply selects the other polarisation than the transponder's.
    public var lnbMismatch: Bool

    init(_ c: sdrkit_dvbs2_fields) {
        standard = c.has_standard ? Standard(rawValue: c.standard) : nil
        signalNote = c.has_signal_note ? SignalNote(rawValue: c.signal_note) : nil
        symbolRateHz = c.has_symbol_rate_hz ? c.symbol_rate_hz : nil
        spectrumRate = c.has_spectrum_rate ? c.spectrum_rate : nil
        clockPpm = c.clock_ppm
        manualRate = c.manual_rate
        spectrumSnrDb = c.has_spectrum_snr_db ? c.spectrum_snr_db : nil
        misfit = c.has_misfit ? c.misfit : nil
        edgeLow = c.has_edge_low ? c.edge_low : nil
        edgeHigh = c.has_edge_high ? c.edge_high : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        inverted = c.inverted
        rollOff = c.roll_off
        rollOffSource = RollOffSource(rawValue: c.roll_off_source)
        modulation = c.has_modulation ? Modulation(rawValue: c.modulation) : nil
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        modcod = c.has_modcod ? c.modcod : nil
        frame = c.has_frame ? Frame(rawValue: c.frame) : nil
        pilots = c.pilots
        vcm = c.vcm
        singleStream = c.single_stream
        plScramblingCode = c.pl_scrambling_code
        plframes = c.plframes
        dummyFrames = c.dummy_frames
        unsupportedFrames = c.unsupported_frames
        otherModcodFrames = c.other_modcod_frames
        pilotMismatch = c.pilot_mismatch
        bbheaderBad = c.bbheader_bad
        netBitRate = c.net_bit_rate
        esn0Db = c.has_esn0_db ? c.esn0_db : nil
        merDb = c.has_mer_db ? c.mer_db : nil
        ebn0Db = c.has_ebn0_db ? c.ebn0_db : nil
        marginToQefDb = c.has_margin_to_qef_db ? c.margin_to_qef_db : nil
        ldpcIterations = c.ldpc_iterations
        bchOk = c.bch_ok
        bchBad = c.bch_bad
        rsClean = c.rs_clean
        rsCorrected = c.rs_corrected
        rsFailed = c.rs_failed
        carrierLock = c.carrier_lock
        timingLock = c.timing_lock
        frameLock = c.frame_lock
        fecLock = c.fec_lock
        tsLock = c.ts_lock
        levelDbfs = c.level_dbfs
        lnbMismatch = c.lnb_mismatch
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<Dvbs2Fields, SDRKitCode> {
        var c = sdrkit_dvbs2_fields()
        let code = sdrkit_dvbs2_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(Dvbs2Fields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the carriers of the survey. The row key is the offset in kHz.
public struct Dvbs2CarrierRow {
    /// The row key.
    public var key: String
    /// The offset of the carrier from the centre.
    public var offset: Double
    /// Its symbol rate.
    public var symbolRate: Double
    /// Its roll-off.
    public var rollOff: Double
    /// Its plateau over the floor.
    public var snrDb: Double
    /// Its level against the strongest.
    public var levelDb: Double
    /// It is the carrier decoded.
    public var decoded: Bool

    init(_ c: sdrkit_dvbs2_carrier_row) {
        key = sdrkitText(c.key)
        offset = c.offset
        symbolRate = c.symbol_rate
        rollOff = c.roll_off
        snrDb = c.snr_db
        levelDb = c.level_db
        decoded = c.decoded
    }
}

/// A row of the input streams. The row key is the ISI.
public struct Dvbs2StreamRow {
    public enum Carries: Int32 {
        case ts = 0
        case gse = 1
        case gseHem = 2
        case tsGeneric = 3
        case packets = 4
    }
    /// The row key.
    public var key: String
    /// The input stream identifier.
    public var isi: Int64
    /// What the stream carries.
    public var carries: Carries?
    /// The user packet length. No value for a stream without user packets.
    public var packetBytes: Int64?
    /// The sync byte of the user packets.
    public var syncByte: Int64?
    /// Constant coding and modulation.
    public var ccm: Bool
    /// Frames of the stream.
    public var frames: Int64
    /// Data field bits of the stream.
    public var bits: Int64

    init(_ c: sdrkit_dvbs2_stream_row) {
        key = sdrkitText(c.key)
        isi = c.isi
        carries = Carries(rawValue: c.carries)
        packetBytes = c.has_packet_bytes ? c.packet_bytes : nil
        syncByte = c.has_sync_byte ? c.sync_byte : nil
        ccm = c.ccm
        frames = c.frames
        bits = c.bits
    }
}

/// A row of the MODCODs of the input streams. The row key is `<isi>/<modcod>`.
public struct Dvbs2StreamModcodRow {
    public enum Modulation: Int32 {
        case qPSK = 0
        case n8PSK = 1
        case n16APSK = 2
        case n32APSK = 3
    }
    public enum CodeRate: Int32 {
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
    /// The row key.
    public var key: String
    /// The input stream, a row key of the stream table.
    public var stream: String
    /// The MODCOD.
    public var modcod: Int64
    /// The modulation of the MODCOD.
    public var modulation: Modulation?
    /// The code rate of the MODCOD.
    public var codeRate: CodeRate?
    /// The frames of the stream with this MODCOD.
    public var frames: Int64
    /// The share of the stream's frames with this MODCOD.
    public var sharePct: Double

    init(_ c: sdrkit_dvbs2_stream_modcod_row) {
        key = sdrkitText(c.key)
        stream = sdrkitText(c.stream)
        modcod = c.modcod
        modulation = Modulation(rawValue: c.modulation)
        codeRate = c.has_code_rate ? CodeRate(rawValue: c.code_rate) : nil
        frames = c.frames
        sharePct = c.share_pct
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum Dvbs2Tables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum Dvbs2Commands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum Dvbs2Settings {
}
