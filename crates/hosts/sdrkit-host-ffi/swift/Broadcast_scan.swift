// sdrkit: the typed interface of the shared interface "broadcast_scan", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum BroadcastScanInterface {
    public static let major = 1
    public static let minor = 0
}

/// The progress of a scan. No frequency to go to before the scan is over.
public struct BroadcastScanProgressFields {
    /// The centres visited of all.
    public var progress: String
    /// The radio centre now.
    public var centre: Double
    /// The stations found so far.
    public var found: Int64
    /// Where the radio goes when the scan is over.
    public var tuneTo: Double?
    /// Whether the scan is over.
    public var done: Bool

    init(_ c: sdrkit_broadcast_scan_progress_fields) {
        progress = sdrkitText(c.progress)
        centre = c.centre
        found = c.found
        tuneTo = c.has_tune_to ? c.tune_to : nil
        done = c.done
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BroadcastScanProgressFields, SDRKitCode> {
        var c = sdrkit_broadcast_scan_progress_fields()
        let code = sdrkit_broadcast_scan_progress_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BroadcastScanProgressFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A station that the scan found. The row key is the frequency in whole hertz.
public struct BroadcastScanStationRow {
    public enum State: Int32 {
        case decoded = 0
        case found = 1
    }
    /// The row key.
    public var key: String
    /// The frequency of the station.
    public var freqHz: Double
    /// How far the station got.
    public var state: State?
    /// The name of the station.
    public var name: String
    /// The receiver's SNR.
    public var snrDb: Double?
    /// The receiver's level.
    public var levelDbfs: Double?
    /// How far the signal stood above the noise floor.
    public var overFloorDb: Double

    init(_ c: sdrkit_broadcast_scan_station_row) {
        key = sdrkitText(c.key)
        freqHz = c.freq_hz
        state = State(rawValue: c.state)
        name = sdrkitText(c.name)
        snrDb = c.has_snr_db ? c.snr_db : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        overFloorDb = c.over_floor_db
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum BroadcastScanTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum BroadcastScanCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum BroadcastScanSettings {
}
