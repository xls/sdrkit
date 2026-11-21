// sdrkit: the typed interface of the shared interface "graph", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum GraphInterface {
    public static let major = 1
    public static let minor = 0
}

/// The result set of [`DcRemoval`].
public struct GraphDcRemovalFields {
    /// The level that the component removes. No value before the first measurement.
    public var dcDbfs: Double?

    init(_ c: sdrkit_graph_dc_removal_fields) {
        dcDbfs = c.has_dc_dbfs ? c.dc_dbfs : nil
    }
}

/// The result set of [`IqCorrection`]. No field has a value before the first measurement.
public struct GraphIqCorrectionFields {
    /// The gain error between I and Q.
    public var gainErrorDb: Double?
    /// The phase error between I and Q.
    public var phaseErrorDeg: Double?

    init(_ c: sdrkit_graph_iq_correction_fields) {
        gainErrorDb = c.has_gain_error_db ? c.gain_error_db : nil
        phaseErrorDeg = c.has_phase_error_deg ? c.phase_error_deg : nil
    }
}

/// The result set of [`NoiseBlanker`]. No field has a value in the first second.
public struct GraphNoiseBlankerFields {
    /// The share of the last second that was blanked.
    public var blankedPct: Double?
    /// The blanks since the start.
    public var blankEvents: Int64?

    init(_ c: sdrkit_graph_noise_blanker_fields) {
        blankedPct = c.has_blanked_pct ? c.blanked_pct : nil
        blankEvents = c.has_blank_events ? c.blank_events : nil
    }
}

/// The result set of [`PowerMeterSink`]. No field has a value before the first interval ends.
public struct GraphPowerMeterFields {
    /// The mean level.
    public var levelDbfs: Double?
    /// The peak level of the interval.
    public var peakDbfs: Double?

    init(_ c: sdrkit_graph_power_meter_fields) {
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        peakDbfs = c.has_peak_dbfs ? c.peak_dbfs : nil
    }
}

/// The result set of [`Squelch`]. No field has a value before the first interval ends.
public struct GraphSquelchFields {
    /// The squelch passes the signal.
    public var `open`: Bool?
    /// The level that the squelch compares with its threshold.
    public var levelDb: Double?

    init(_ c: sdrkit_graph_squelch_fields) {
        `open` = c.has_open ? c.open : nil
        levelDb = c.has_level_db ? c.level_db : nil
    }
}

/// The result set of [`AmDemod`].
public struct GraphAmDemodFields {
    /// The offset of the carrier that the synchronous detector tracks. No value with the envelope detector and before the first measurement.
    public var carrierOffsetHz: Double?

    init(_ c: sdrkit_graph_am_demod_fields) {
        carrierOffsetHz = c.has_carrier_offset_hz ? c.carrier_offset_hz : nil
    }
}

/// The result set of [`StackGenerator`].
public struct GraphStackGeneratorFields {
    /// The finite test signal of the stack has ended. No value before the component is configured.
    public var ended: Bool?

    init(_ c: sdrkit_graph_stack_generator_fields) {
        ended = c.has_ended ? c.ended : nil
    }
}

/// The result set of the network sink.
public struct GraphNetSinkFields {
    /// The bytes sent. No value before the first send that succeeded.
    public var sentBytes: Int64?
    /// The sends that failed. The samples of a failed send are lost. No value before the first failure.
    public var failed: Int64?

    init(_ c: sdrkit_graph_net_sink_fields) {
        sentBytes = c.has_sent_bytes ? c.sent_bytes : nil
        failed = c.has_failed ? c.failed : nil
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum GraphTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum GraphCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum GraphSettings {
}
