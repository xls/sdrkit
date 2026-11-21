// sdrkit: the typed interface of the stack family "wlan", version 1.1. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum WlanInterface {
    public static let major = 1
    public static let minor = 1
}

/// The result fields of `ble`.
public struct BleFields {
    /// Devices in the table.
    public var devices: Int64
    /// Good packets.
    public var framesOk: Int64
    /// Exact access-address matches whose CRC failed on every channel.
    public var framesBad: Int64
    /// Access-address candidates.
    public var aaHits: Int64
    /// The running mean of the eye SNR. No value before the first good packet.
    public var snrDb: Double?
    /// The carrier offset of the last good packet. No value before it.
    public var offsetHz: Double?
    /// The deviation of the last good packet. No value before it.
    public var deviationHz: Double?
    /// The level of the channel. No value before the first input.
    public var levelDbfs: Double?
    /// The tuned advertising channel. No value when it is not known, as for a recording.
    public var channel: Int64?
    /// The channel of the last good packet. No value before it.
    public var lastChannel: Int64?
    /// Good packets on channel 37.
    public var packets37: Int64
    /// Good packets on channel 38.
    public var packets38: Int64
    /// Good packets on channel 39.
    public var packets39: Int64
    /// The host steps through the advertising channels.
    public var hopping: Bool
    /// The privacy note.
    public var privacy: String

    init(_ c: sdrkit_ble_fields) {
        devices = c.devices
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        aaHits = c.aa_hits
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        deviationHz = c.has_deviation_hz ? c.deviation_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        channel = c.has_channel ? c.channel : nil
        lastChannel = c.has_last_channel ? c.last_channel : nil
        packets37 = c.packets_37
        packets38 = c.packets_38
        packets39 = c.packets_39
        hopping = c.hopping
        privacy = sdrkitText(c.privacy)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BleFields, SDRKitCode> {
        var c = sdrkit_ble_fields()
        let code = sdrkit_ble_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BleFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the devices of `ble`. The row key is the address and "r" (random) or "p" (public).
public struct BleDeviceRow {
    public enum AddressType: Int32 {
        case `public` = 0
        case randomStatic = 1
        case resolvablePrivate = 2
        case nonResolvablePrivate = 3
        case random = 4
    }
    /// The row key.
    public var key: String
    /// The address.
    public var address: String
    /// The kind of the address.
    public var addressType: AddressType?
    /// The name, if a packet gave one.
    public var name: String?
    /// The level of the last packet.
    public var levelDbfs: Double
    /// The UTC of the last packet.
    public var lastHeard: Double?
    /// Packets heard.
    public var heard: Int64?
    /// The vendor, if known.
    public var vendor: String?
    /// The beacon, the services and the TX power, if a packet gave them.
    public var beacon: String?
    /// The channels the device was heard on, as "37 38 39".
    public var channels: String
    /// The device takes connections.
    public var connectable: Bool
    /// Another device sent a scan request to it.
    public var scanned: Bool
    /// The first 32 bytes of manufacturer data after the company identifier, hex, if a packet gave them.
    public var manufacturerData: String?
    /// The UTC of the first packet.
    public var firstHeard: Double?

    init(_ c: sdrkit_ble_device_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        addressType = AddressType(rawValue: c.address_type)
        name = c.has_name ? sdrkitText(c.name) : nil
        levelDbfs = c.level_dbfs
        lastHeard = c.has_last_heard ? c.last_heard : nil
        heard = c.has_heard ? c.heard : nil
        vendor = c.has_vendor ? sdrkitText(c.vendor) : nil
        beacon = c.has_beacon ? sdrkitText(c.beacon) : nil
        channels = sdrkitText(c.channels)
        connectable = c.connectable
        scanned = c.scanned
        manufacturerData = c.has_manufacturer_data ? sdrkitText(c.manufacturer_data) : nil
        firstHeard = c.has_first_heard ? c.first_heard : nil
    }
}

/// A row of the packet log of `ble`.
public struct BlePacketRow {
    /// The row key.
    public var key: String
    /// The signal time of the packet.
    public var signalS: Double
    /// The advertising channel of the packet.
    public var channel: Int64
    /// The PDU type name.
    public var pdu: String
    /// The address of the sender, if the PDU has one.
    public var address: String?
    /// The level of the packet.
    public var levelDbfs: Double
    /// The payload length.
    public var length: Int64
    /// The one-line summary.
    public var details: String

    init(_ c: sdrkit_ble_packet_row) {
        key = sdrkitText(c.key)
        signalS = c.signal_s
        channel = c.channel
        pdu = sdrkitText(c.pdu)
        address = c.has_address ? sdrkitText(c.address) : nil
        levelDbfs = c.level_dbfs
        length = c.length
        details = sdrkitText(c.details)
    }
}

/// A row of the good packets of each PDU type of `ble`. The row key is the PDU type name.
public struct BlePduTypeRow {
    /// The row key.
    public var key: String
    /// The PDU type name.
    public var pdu: String
    /// Good packets of the type.
    public var packets: Int64

    init(_ c: sdrkit_ble_pdu_type_row) {
        key = sdrkitText(c.key)
        pdu = sdrkitText(c.pdu)
        packets = c.packets
    }
}

/// A row of the level history of the devices of `ble`. The row key is `<device row>/<slot>`.
public struct BleDeviceLevelRow {
    /// The row key.
    public var key: String
    /// The row of the device.
    public var device: String
    /// The number of the level in the history of its device, counted from the first one heard.
    public var sample: Int64
    /// The level.
    public var levelDbfs: Double

    init(_ c: sdrkit_ble_device_level_row) {
        key = sdrkitText(c.key)
        device = sdrkitText(c.device)
        sample = c.sample
        levelDbfs = c.level_dbfs
    }
}

/// The result fields of `bletracker`.
public struct BletrackerFields {
    /// Trackers that follow the radio now.
    public var followingNow: Int64
    /// Tracker advertisements taken since the last clear.
    public var trackerPackets: Int64
    /// Trackers heard in the last minute.
    public var trackersNow: Int64
    /// Trackers in the table.
    public var trackers: Int64
    /// How long a tracker must stay before the alert.
    public var alertAfter: Double
    /// The averaged level from which a tracker counts as near.
    public var nearDbfs: Double
    /// The signal time of the book.
    public var nowS: Double
    /// Good packets.
    public var framesOk: Int64
    /// Exact access-address matches whose CRC failed on every channel.
    public var framesBad: Int64
    /// The running mean of the eye SNR. No value before the first good packet.
    public var snrDb: Double?
    /// The carrier offset of the last good packet. No value before it.
    public var offsetHz: Double?
    /// The level of the channel. No value before the first input.
    public var levelDbfs: Double?
    /// The tuned advertising channel. No value when it is not known, as for a recording.
    public var channel: Int64?
    /// What the detector cannot tell.
    public var limits: String
    /// The privacy note.
    public var privacy: String

    init(_ c: sdrkit_bletracker_fields) {
        followingNow = c.following_now
        trackerPackets = c.tracker_packets
        trackersNow = c.trackers_now
        trackers = c.trackers
        alertAfter = c.alert_after
        nearDbfs = c.near_dbfs
        nowS = c.now_s
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        channel = c.has_channel ? c.channel : nil
        limits = sdrkitText(c.limits)
        privacy = sdrkitText(c.privacy)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BletrackerFields, SDRKitCode> {
        var c = sdrkit_bletracker_fields()
        let code = sdrkit_bletracker_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BletrackerFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the trackers of `bletracker`. The row key is the tracker id.
public struct BletrackerTrackerRow {
    public enum Kind: Int32 {
        case findMy = 0
        case tile = 1
        case chipolo = 2
        case smartTag = 3
        case google = 4
    }
    public enum State: Int32 {
        case mine = 0
        case following = 1
        case separated = 2
        case nearby = 3
    }
    public enum Trend: Int32 {
        case closer = 0
        case farther = 1
        case steady = 2
    }
    /// The row key.
    public var key: String
    /// The label of the tracker: its kind and the last two address bytes.
    public var tracker: String
    /// The kind, if recognised.
    public var kind: Kind?
    /// What the tracker does now.
    public var state: State?
    /// How long it stayed with the radio.
    public var stays: Double
    /// The share of the time it was near.
    public var nearPct: Double
    /// The averaged level.
    public var levelDbfs: Double
    /// The highest level.
    public var peakDbfs: Double
    /// Whether the level rises or falls.
    public var trend: Trend?
    /// The addresses it used.
    public var addresses: Int64
    /// The signal time it was last heard.
    public var lastS: Double
    /// Packets heard.
    public var packets: Int64
    /// It says it is away from its owner.
    public var separated: Bool
    /// The status byte, if the advertisement has one.
    public var status: Int64?
    /// It follows the radio.
    public var following: Bool
    /// It is marked as the user's own.
    public var mine: Bool
    /// The tracker id.
    public var id: Int64

    init(_ c: sdrkit_bletracker_tracker_row) {
        key = sdrkitText(c.key)
        tracker = sdrkitText(c.tracker)
        kind = c.has_kind ? Kind(rawValue: c.kind) : nil
        state = State(rawValue: c.state)
        stays = c.stays
        nearPct = c.near_pct
        levelDbfs = c.level_dbfs
        peakDbfs = c.peak_dbfs
        trend = Trend(rawValue: c.trend)
        addresses = c.addresses
        lastS = c.last_s
        packets = c.packets
        separated = c.separated
        status = c.has_status ? c.status : nil
        following = c.following
        mine = c.mine
        id = c.id
    }
}

/// A row of the alerts of `bletracker`.
public struct BletrackerAlertRow {
    /// The row key.
    public var key: String
    /// The UTC of the alert, for a live source.
    public var time: Double?
    /// The signal time of the alert.
    public var signalS: Double
    /// The level of the tracker.
    public var levelDbfs: Double
    /// The text of the alert.
    public var alert: String

    init(_ c: sdrkit_bletracker_alert_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        levelDbfs = c.level_dbfs
        alert = sdrkitText(c.alert)
    }
}

/// A row of the trackers heard of each kind of `bletracker`. The row key is the key of the kind.
public struct BletrackerKindRow {
    public enum Kind: Int32 {
        case findMy = 0
        case tile = 1
        case chipolo = 2
        case smartTag = 3
        case google = 4
    }
    /// The row key.
    public var key: String
    /// The kind.
    public var kind: Kind?
    /// Trackers of the kind heard in the last minute.
    public var lastMinute: Int64
    /// Trackers of the kind heard in the last 30 minutes.
    public var last30Min: Int64

    init(_ c: sdrkit_bletracker_kind_row) {
        key = sdrkitText(c.key)
        kind = Kind(rawValue: c.kind)
        lastMinute = c.last_minute
        last30Min = c.last_30_min
    }
}

/// The result fields of `wifi`.
public struct WifiFields {
    public enum Band: Int32 {
        case n24 = 0
        case n5Ghz = 1
        case n6Ghz = 2
    }
    public enum Phy: Int32 {
        case dsss = 0
        case ofdm = 1
    }
    /// Networks in the table.
    public var networks: Int64
    /// Frames with a good FCS.
    public var framesOk: Int64
    /// Frames with a bad FCS.
    public var framesBad: Int64
    /// DSSS SFDs found.
    public var dsssSfds: Int64
    /// DSSS PLCP headers with a good CRC.
    public var dsssHeadersOk: Int64
    /// DSSS PLCP headers with a bad CRC.
    public var dsssHeadersBad: Int64
    /// DSSS headers of a CCK rate, not decoded.
    public var dsssCckHeaders: Int64
    /// OFDM short preambles found.
    public var ofdmPreambles: Int64
    /// OFDM SIGNAL fields with a good parity.
    public var ofdmSignalOk: Int64
    /// OFDM SIGNAL fields with a bad parity or rate.
    public var ofdmSignalBad: Int64
    /// Management frames.
    public var mgmtFrames: Int64
    /// Control frames.
    public var controlFrames: Int64
    /// Data frames.
    public var dataFrames: Int64
    /// Beacons.
    public var beacons: Int64
    /// Probe responses.
    public var probeResponses: Int64
    /// FILS discovery frames.
    public var filsFrames: Int64
    /// Frames heard again on a second channel at once.
    public var duplicateFrames: Int64
    /// The band of the tuned frequency, if it is in one.
    public var band: Band?
    /// The channel of the tuned frequency, if it is one.
    public var channel: Int64?
    /// The 20 MHz channels that the stack reads at once.
    public var subChannels: Int64
    /// The PHY of the last good frame. No value before it.
    public var phy: Phy?
    /// The running mean of the SNR. No value before the first good frame.
    public var snrDb: Double?
    /// The carrier offset of the last good frame. No value before it.
    public var offsetHz: Double?
    /// The level of the input. No value before the first input.
    public var levelDbfs: Double?
    /// The host steps through the hop list.
    public var hopping: Bool
    /// The passive note.
    public var passive: String

    init(_ c: sdrkit_wifi_fields) {
        networks = c.networks
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        dsssSfds = c.dsss_sfds
        dsssHeadersOk = c.dsss_headers_ok
        dsssHeadersBad = c.dsss_headers_bad
        dsssCckHeaders = c.dsss_cck_headers
        ofdmPreambles = c.ofdm_preambles
        ofdmSignalOk = c.ofdm_signal_ok
        ofdmSignalBad = c.ofdm_signal_bad
        mgmtFrames = c.mgmt_frames
        controlFrames = c.control_frames
        dataFrames = c.data_frames
        beacons = c.beacons
        probeResponses = c.probe_responses
        filsFrames = c.fils_frames
        duplicateFrames = c.duplicate_frames
        band = c.has_band ? Band(rawValue: c.band) : nil
        channel = c.has_channel ? c.channel : nil
        subChannels = c.sub_channels
        phy = c.has_phy ? Phy(rawValue: c.phy) : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        hopping = c.hopping
        passive = sdrkitText(c.passive)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WifiFields, SDRKitCode> {
        var c = sdrkit_wifi_fields()
        let code = sdrkit_wifi_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WifiFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the networks of `wifi`. The row key is the BSSID.
public struct WifiNetworkRow {
    public enum Band: Int32 {
        case n24 = 0
        case n5Ghz = 1
        case n6Ghz = 2
    }
    /// The row key.
    public var key: String
    /// The SSID, or "(hidden)".
    public var ssid: String
    /// The BSSID.
    public var bssid: String
    /// The vendor of the BSSID, or "(local)" for a locally administered one.
    public var vendor: String
    /// The band.
    public var band: Band?
    /// The primary channel.
    public var channel: Int64
    /// The channel the frames were heard on.
    public var heardOn: Int64
    /// The channel width.
    public var widthHz: Double
    /// The newest standard ("ax").
    public var standard: String
    /// The security.
    public var security: String
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// Beacons heard.
    public var beacons: Int64
    /// Probe responses heard.
    public var probeResponses: Int64
    /// The beacon interval.
    public var interval: Double
    /// The pairwise ciphers.
    public var ciphers: String
    /// The key management.
    public var akm: String
    /// Protected management frames: "required", "capable" or "".
    public var pmf: String
    /// The capability bits as text.
    public var capability: String
    /// The other elements as text.
    public var details: String

    init(_ c: sdrkit_wifi_network_row) {
        key = sdrkitText(c.key)
        ssid = sdrkitText(c.ssid)
        bssid = sdrkitText(c.bssid)
        vendor = sdrkitText(c.vendor)
        band = c.has_band ? Band(rawValue: c.band) : nil
        channel = c.channel
        heardOn = c.heard_on
        widthHz = c.width_hz
        standard = sdrkitText(c.standard)
        security = sdrkitText(c.security)
        levelDbfs = c.level_dbfs
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        beacons = c.beacons
        probeResponses = c.probe_responses
        interval = c.interval
        ciphers = sdrkitText(c.ciphers)
        akm = sdrkitText(c.akm)
        pmf = sdrkitText(c.pmf)
        capability = sdrkitText(c.capability)
        details = sdrkitText(c.details)
    }
}

/// A row of the frame log of `wifi`.
public struct WifiBeaconRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var signalS: Double
    /// The channel the frame was heard on.
    public var channel: Int64
    /// The BSSID.
    public var bssid: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The length, FCS included.
    public var bytes: Int64
    /// The summary.
    public var details: String

    init(_ c: sdrkit_wifi_beacon_row) {
        key = sdrkitText(c.key)
        signalS = c.signal_s
        channel = c.channel
        bssid = sdrkitText(c.bssid)
        levelDbfs = c.level_dbfs
        bytes = c.bytes
        details = sdrkitText(c.details)
    }
}

/// A row of the good OFDM frames at each rate of `wifi`. The row key is the rate in Mbit/s.
public struct WifiRateRow {
    /// The row key.
    public var key: String
    /// The rate.
    public var rateMbps: Double
    /// Good frames at the rate.
    public var frames: Int64

    init(_ c: sdrkit_wifi_rate_row) {
        key = sdrkitText(c.key)
        rateMbps = c.rate_mbps
        frames = c.frames
    }
}

/// A row of the summary of each band of `wifi`. The row key is the key of the band.
public struct WifiBandRow {
    public enum Band: Int32 {
        case n24 = 0
        case n5Ghz = 1
        case n6Ghz = 2
    }
    /// The row key.
    public var key: String
    /// The band.
    public var band: Band?
    /// Networks in the band.
    public var networks: Int64
    /// Beacons heard in the band.
    public var beacons: Int64
    /// Networks of 802.11ax or 802.11be.
    public var axBe: Int64
    /// Networks with WPA3 or OWE.
    public var wpa3Owe: Int64
    /// Open networks.
    public var `open`: Int64
    /// The level of the strongest network.
    public var strongestDbfs: Double

    init(_ c: sdrkit_wifi_band_row) {
        key = sdrkitText(c.key)
        band = Band(rawValue: c.band)
        networks = c.networks
        beacons = c.beacons
        axBe = c.ax_be
        wpa3Owe = c.wpa3_owe
        `open` = c.open
        strongestDbfs = c.strongest_dbfs
    }
}

/// The result fields of `halow`.
public struct HalowFields {
    /// Networks in the table.
    public var networks: Int64
    /// Frames with a good FCS.
    public var framesOk: Int64
    /// Frames with a bad FCS.
    public var framesBad: Int64
    /// PPDUs read from their SIG field only.
    public var undecoded: Int64
    /// PPDUs with several spatial streams.
    public var mimoFrames: Int64
    /// PPDUs with STBC.
    public var stbcFrames: Int64
    /// PPDUs with LDPC coding.
    public var ldpcFrames: Int64
    /// A-MPDUs.
    public var ampduFrames: Int64
    /// Null data packets.
    public var ndpFrames: Int64
    /// Preambles detected.
    public var preambles: Int64
    /// SIG fields read.
    public var sigOk: Int64
    /// SIG fields not readable.
    public var sigBad: Int64
    /// Data fields cut short.
    public var dataBad: Int64
    /// Network frames (beacons and probe responses).
    public var beaconsSeen: Int64
    /// Frames in the last whole second.
    public var framesPerSecond: Int64
    /// The tuned frequency. No value when it is not known, as for a recording.
    public var tunedHz: Double?
    /// The nearest 1 MHz channel of the plan, if one is within 1 MHz of the tuned frequency.
    public var channel: Int64?
    /// The widest channel that the rate holds.
    public var maxWidthHz: Double
    /// The running mean of the SNR. No value before the first good frame.
    public var snrDb: Double?
    /// The carrier offset of the last good frame. No value before it.
    public var offsetHz: Double?
    /// The level of the input. No value before the first input.
    public var levelDbfs: Double?
    /// The host steps through the channels.
    public var scanning: Bool
    /// The receive and privacy note.
    public var receiveNote: String
    /// The note on what the data decoder reads.
    public var dataNote: String

    init(_ c: sdrkit_halow_fields) {
        networks = c.networks
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        undecoded = c.undecoded
        mimoFrames = c.mimo_frames
        stbcFrames = c.stbc_frames
        ldpcFrames = c.ldpc_frames
        ampduFrames = c.ampdu_frames
        ndpFrames = c.ndp_frames
        preambles = c.preambles
        sigOk = c.sig_ok
        sigBad = c.sig_bad
        dataBad = c.data_bad
        beaconsSeen = c.beacons_seen
        framesPerSecond = c.frames_per_second
        tunedHz = c.has_tuned_hz ? c.tuned_hz : nil
        channel = c.has_channel ? c.channel : nil
        maxWidthHz = c.max_width_hz
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        scanning = c.scanning
        receiveNote = sdrkitText(c.receive_note)
        dataNote = sdrkitText(c.data_note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HalowFields, SDRKitCode> {
        var c = sdrkit_halow_fields()
        let code = sdrkit_halow_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(HalowFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the networks of `halow`. The row key is the BSSID.
public struct HalowNetworkRow {
    /// The row key.
    public var key: String
    /// The SSID, "(hidden)", the compressed SSID or "-" when not known.
    public var ssid: String
    /// The BSSID.
    public var bssid: String
    /// The vendor.
    public var vendor: String
    /// The channel number.
    public var channel: Int64
    /// The frequency of the channel, if known.
    public var freqHz: Double?
    /// The width of the last PPDU.
    public var widthHz: Double
    /// The operating width, if a beacon gave it.
    public var opWidthHz: Double?
    /// The MCS of the last PPDU.
    public var mcs: Int64
    /// The spatial streams of the last PPDU.
    public var nss: Int64
    /// The SNR of the last PPDU.
    public var snrDb: Double
    /// The security.
    public var security: String
    /// The beacon interval.
    public var interval: Double
    /// The level of the last PPDU.
    public var levelDbfs: Double
    /// Beacons heard.
    public var beacons: Int64
    /// The country.
    public var country: String
    /// The UTC of the last frame.
    public var lastHeard: Double?

    init(_ c: sdrkit_halow_network_row) {
        key = sdrkitText(c.key)
        ssid = sdrkitText(c.ssid)
        bssid = sdrkitText(c.bssid)
        vendor = sdrkitText(c.vendor)
        channel = c.channel
        freqHz = c.has_freq_hz ? c.freq_hz : nil
        widthHz = c.width_hz
        opWidthHz = c.has_op_width_hz ? c.op_width_hz : nil
        mcs = c.mcs
        nss = c.nss
        snrDb = c.snr_db
        security = sdrkitText(c.security)
        interval = c.interval
        levelDbfs = c.level_dbfs
        beacons = c.beacons
        country = sdrkitText(c.country)
        lastHeard = c.has_last_heard ? c.last_heard : nil
    }
}

/// A row of the frame log of `halow`.
public struct HalowFrameRow {
    /// The row key.
    public var key: String
    /// The signal time of the frame.
    public var signalS: Double
    /// The width of the PPDU.
    public var widthHz: Double
    /// The MCS.
    public var mcs: Int64
    /// The spatial streams.
    public var nss: Int64
    /// The name of the preamble, if the SIG field gave it.
    public var preamble: String?
    /// The SNR of the PPDU.
    public var snrDb: Double
    /// The kind of the frame.
    public var type: String
    /// The BSSID.
    public var bssid: String
    /// The length.
    public var length: Int64
    /// The details of the frame.
    public var details: String

    init(_ c: sdrkit_halow_frame_row) {
        key = sdrkitText(c.key)
        signalS = c.signal_s
        widthHz = c.width_hz
        mcs = c.mcs
        nss = c.nss
        preamble = c.has_preamble ? sdrkitText(c.preamble) : nil
        snrDb = c.snr_db
        type = sdrkitText(c.type)
        bssid = sdrkitText(c.bssid)
        length = c.length
        details = sdrkitText(c.details)
    }
}

/// A row of the width view of `halow`. The row key is the width in MHz.
public struct HalowWidthRow {
    public enum State: Int32 {
        case listening = 0
        case notSelected = 1
        case rateTooLow = 2
    }
    /// The row key.
    public var key: String
    /// The width.
    public var widthHz: Double
    /// SIG fields read at the width.
    public var sigOk: Int64
    /// The lowest radio rate for the width.
    public var minRateHz: Double
    /// The radio rate recommended for the width.
    public var rateHz: Double
    /// Whether the stack reads the width.
    public var state: State?

    init(_ c: sdrkit_halow_width_row) {
        key = sdrkitText(c.key)
        widthHz = c.width_hz
        sigOk = c.sig_ok
        minRateHz = c.min_rate_hz
        rateHz = c.rate_hz
        state = State(rawValue: c.state)
    }
}

/// A row of the PPDUs of each MCS of `halow`. The row key is the MCS.
public struct HalowMcsRow {
    /// The row key.
    public var key: String
    /// The MCS.
    public var mcs: Int64
    /// PPDUs counted.
    public var ppdus: Int64

    init(_ c: sdrkit_halow_mcs_row) {
        key = sdrkitText(c.key)
        mcs = c.mcs
        ppdus = c.ppdus
    }
}

/// A row of the PPDUs of each preamble of `halow`. The row key is the key of the preamble.
public struct HalowPreambleRow {
    public enum Preamble: Int32 {
        case s1gShort = 0
        case s1gLong = 1
        case s1g1m = 2
    }
    /// The row key.
    public var key: String
    /// The preamble.
    public var preamble: Preamble?
    /// PPDUs counted.
    public var ppdus: Int64

    init(_ c: sdrkit_halow_preamble_row) {
        key = sdrkitText(c.key)
        preamble = Preamble(rawValue: c.preamble)
        ppdus = c.ppdus
    }
}

/// The result fields of `btclassic`.
public struct BtclassicFields {
    /// Good access codes.
    public var accessCodes: Int64
    /// Bursts analysed without an access code.
    public var burstsWithoutCode: Int64
    /// GIAC and LIAC ID packets.
    public var inquiryIds: Int64
    /// FHS packets read.
    public var inquiryResponses: Int64
    /// Bursts analysed.
    public var burstsAnalysed: Int64
    /// Wideband bursts skipped.
    public var burstsWideband: Int64
    /// The channels inside the capture.
    public var channelsCovered: Int64
    /// The channel numbers count from a capture centre taken as channel 39 (the radio centre is not known).
    public var channelsRelative: Bool
    /// The carrier offset of the last good packet. No value before it.
    public var cfoHz: Double?
    /// The SNR of the last good packet. No value before it.
    public var snrDb: Double?
    /// The input level. No value before the first input.
    public var inputLevelDbfs: Double?
    /// 1 when an access code was found in the last 5 s of signal.
    public var state: Int64
    /// Devices in the table.
    public var devices: Int64
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_btclassic_fields) {
        accessCodes = c.access_codes
        burstsWithoutCode = c.bursts_without_code
        inquiryIds = c.inquiry_ids
        inquiryResponses = c.inquiry_responses
        burstsAnalysed = c.bursts_analysed
        burstsWideband = c.bursts_wideband
        channelsCovered = c.channels_covered
        channelsRelative = c.channels_relative
        cfoHz = c.has_cfo_hz ? c.cfo_hz : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        inputLevelDbfs = c.has_input_level_dbfs ? c.input_level_dbfs : nil
        state = c.state
        devices = c.devices
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BtclassicFields, SDRKitCode> {
        var c = sdrkit_btclassic_fields()
        let code = sdrkit_btclassic_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(BtclassicFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
public struct WlanDecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    public var dropped: Int64?

    init(_ c: sdrkit_wlan_decoder_fields) {
        dropped = c.has_dropped ? c.dropped : nil
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WlanDecoderFields, SDRKitCode> {
        var c = sdrkit_wlan_decoder_fields()
        let code = sdrkit_wlan_decoder_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WlanDecoderFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the devices of `btclassic`. The row key is the LAP.
public struct BtclassicDeviceRow {
    /// The row key.
    public var key: String
    /// The LAP as hex text.
    public var lap: String
    /// What the device is.
    public var kind: String
    /// The address from an FHS packet.
    public var address: String?
    /// The class of device as text.
    public var `class`: String?
    /// The major class name.
    public var majorClass: String?
    /// The class of device value.
    public var classRaw: Int64?
    /// CLK27..CLK2 from the FHS packet.
    public var clock: Int64?
    /// The level of the last packet.
    public var levelDbfs: Double?
    /// The last levels.
    public var levelHistory: String
    /// The UTC of the first packet.
    public var firstHeard: Double?
    /// The UTC of the last packet.
    public var lastHeard: Double?
    /// Packets heard.
    public var packets: Int64
    /// The channel of the last packet.
    public var lastChannelMhz: Double
    /// The channels heard, as hex: bit k is channel k.
    public var channelsHeard: String

    init(_ c: sdrkit_btclassic_device_row) {
        key = sdrkitText(c.key)
        lap = sdrkitText(c.lap)
        kind = sdrkitText(c.kind)
        address = c.has_address ? sdrkitText(c.address) : nil
        `class` = c.has_class_ ? sdrkitText(c.class_) : nil
        majorClass = c.has_major_class ? sdrkitText(c.major_class) : nil
        classRaw = c.has_class_raw ? c.class_raw : nil
        clock = c.has_clock ? c.clock : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        levelHistory = sdrkitText(c.level_history)
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        packets = c.packets
        lastChannelMhz = c.last_channel_mhz
        channelsHeard = sdrkitText(c.channels_heard)
    }
}

/// A row of the packet log of `btclassic`.
public struct BtclassicPacketRow {
    /// The row key.
    public var key: String
    /// The UTC of the packet, for a live source.
    public var utc: Double?
    /// The signal time of the packet.
    public var timeS: Double
    /// The channel of the packet.
    public var channelMhz: Double
    /// The LAP as hex text.
    public var lap: String
    /// The level of the packet.
    public var levelDbfs: Double
    /// Sync-word bits that differed.
    public var syncErrors: Int64
    /// What the packet is.
    public var text: String

    init(_ c: sdrkit_btclassic_packet_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        channelMhz = c.channel_mhz
        lap = sdrkitText(c.lap)
        levelDbfs = c.level_dbfs
        syncErrors = c.sync_errors
        text = sdrkitText(c.text)
    }
}

/// The result fields of `wfb`.
public struct WfbFields {
    /// Frames of wfb-ng or OpenHD links.
    public var framesOk: Int64
    /// HT frames with a good FCS.
    public var htFrames: Int64
    /// HT-mixed PPDUs that the decoder does not take.
    public var htNotDecodable: Int64
    /// HT frames with a bad FCS.
    public var badFcs: Int64
    /// Links in the table.
    public var linksHeard: Int64
    /// Frames lost over every link.
    public var framesLost: Int64
    /// The SNR of the last frame. No value before the first frame.
    public var snrDb: Double?
    /// What no public document confirms.
    public var note: String

    init(_ c: sdrkit_wfb_fields) {
        framesOk = c.frames_ok
        htFrames = c.ht_frames
        htNotDecodable = c.ht_not_decodable
        badFcs = c.bad_fcs
        linksHeard = c.links_heard
        framesLost = c.frames_lost
        snrDb = c.has_snr_db ? c.snr_db : nil
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WfbFields, SDRKitCode> {
        var c = sdrkit_wfb_fields()
        let code = sdrkit_wfb_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(WfbFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the links of `wfb`. The row key is the system and the link id.
public struct WfbLinkRow {
    /// The row key.
    public var key: String
    /// "wfb-ng" or "OpenHD".
    public var link: String
    /// The link id.
    public var id: String
    /// Frames heard.
    public var frames: Int64
    /// Frames lost.
    public var framesLost: Int64
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The SNR of the last frame.
    public var snrDb: Double
    /// The type of the last frame.
    public var lastType: String
    /// The detail of the last frame.
    public var lastDetail: String

    init(_ c: sdrkit_wfb_link_row) {
        key = sdrkitText(c.key)
        link = sdrkitText(c.link)
        id = sdrkitText(c.id)
        frames = c.frames
        framesLost = c.frames_lost
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        snrDb = c.snr_db
        lastType = sdrkitText(c.last_type)
        lastDetail = sdrkitText(c.last_detail)
    }
}

/// A row of the frame log of `wfb`.
public struct WfbFrameRow {
    /// The row key.
    public var key: String
    /// The UTC of the frame, for a live source.
    public var utc: Double?
    /// The signal time of the frame.
    public var timeS: Double
    /// "wfb-ng" or "OpenHD".
    public var link: String
    /// The link id.
    public var id: String
    /// The type of the frame.
    public var type: String
    /// The detail of the frame.
    public var detail: String
    /// The first payload bytes, hex.
    public var hex: String
    /// The SNR of the frame.
    public var snrDb: Double

    init(_ c: sdrkit_wfb_frame_row) {
        key = sdrkitText(c.key)
        utc = c.has_utc ? c.utc : nil
        timeS = c.time_s
        link = sdrkitText(c.link)
        id = sdrkitText(c.id)
        type = sdrkitText(c.type)
        detail = sdrkitText(c.detail)
        hex = sdrkitText(c.hex)
        snrDb = c.snr_db
    }
}

/// The result fields of `espnow`.
public struct EspnowFields {
    /// Devices in the table.
    public var devices: Int64
    /// Wi-Fi frames with a good FCS.
    public var framesOk: Int64
    /// Wi-Fi frames with a bad FCS.
    public var framesBad: Int64
    /// Management action frames.
    public var actionFrames: Int64
    /// ESP-NOW frames.
    public var espnowFrames: Int64
    /// ESP-NOW frames that were protected.
    public var encrypted: Int64
    /// Protected frames that a key opened.
    public var decrypted: Int64
    /// ESP-NOW frames to the broadcast address.
    public var broadcast: Int64
    /// An ESP-NOW frame came in the last 5 s.
    public var heard: Bool
    /// The channel nearest the radio centre. No value when the centre is not known.
    public var channel: Int64?
    /// The centre frequency of the channel. No value when the centre is not known.
    public var channelHz: Double?
    /// The offset of the channel's centre from the radio centre. No value when the centre is not known.
    public var channelOffsetHz: Double?
    /// The level of the last good frame. No value before it.
    public var levelDbfs: Double?
    /// The SNR of the last good frame. No value before it.
    public var snrDb: Double?
    /// The carrier offset of the last good frame. No value before it.
    public var offsetHz: Double?
    /// The input rate.
    public var inputRateHz: Double
    /// DSSS SFDs found.
    public var dsssSfds: Int64
    /// DSSS PLCP headers with a good CRC.
    public var dsssHeadersOk: Int64
    /// DSSS PLCP headers with a bad CRC.
    public var dsssHeadersBad: Int64
    /// DSSS headers of a CCK rate, not decoded.
    public var dsssCckHeaders: Int64
    /// OFDM short preambles found.
    public var ofdmPreambles: Int64
    /// OFDM SIGNAL fields with a good parity.
    public var ofdmSignalOk: Int64
    /// OFDM SIGNAL fields with a bad parity or rate.
    public var ofdmSignalBad: Int64
    /// The host steps through the hop list.
    public var hopping: Bool
    /// What is decoded, what is only detected, and what no public document confirms.
    public var note: String

    init(_ c: sdrkit_espnow_fields) {
        devices = c.devices
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        actionFrames = c.action_frames
        espnowFrames = c.espnow_frames
        encrypted = c.encrypted
        decrypted = c.decrypted
        broadcast = c.broadcast
        heard = c.heard
        channel = c.has_channel ? c.channel : nil
        channelHz = c.has_channel_hz ? c.channel_hz : nil
        channelOffsetHz = c.has_channel_offset_hz ? c.channel_offset_hz : nil
        levelDbfs = c.has_level_dbfs ? c.level_dbfs : nil
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        inputRateHz = c.input_rate_hz
        dsssSfds = c.dsss_sfds
        dsssHeadersOk = c.dsss_headers_ok
        dsssHeadersBad = c.dsss_headers_bad
        dsssCckHeaders = c.dsss_cck_headers
        ofdmPreambles = c.ofdm_preambles
        ofdmSignalOk = c.ofdm_signal_ok
        ofdmSignalBad = c.ofdm_signal_bad
        hopping = c.hopping
        note = sdrkitText(c.note)
    }

    /// The result fields of the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EspnowFields, SDRKitCode> {
        var c = sdrkit_espnow_fields()
        let code = sdrkit_espnow_fields_read(handle, &c)
        return code == SDRKIT_OK ? .success(EspnowFields(c)) : .failure(SDRKitCode(code: code))
    }
}

/// A row of the devices of `espnow`. The row key is the sender address.
public struct EspnowDeviceRow {
    /// The row key.
    public var key: String
    /// The sender address.
    public var address: String
    /// The maker of the address, or "(local)" for a locally administered one.
    public var vendor: String
    /// The address is one of Espressif.
    public var espressif: Bool
    /// ESP-NOW frames heard.
    public var packets: Int64
    /// Protected frames heard.
    public var encrypted: Int64
    /// Frames to the broadcast address.
    public var broadcasts: Int64
    /// Packets per second over the present window of 1 to 10 s; 0 after 10 s unheard.
    public var rate: Double
    /// The level of the last frame.
    public var levelDbfs: Double
    /// The channel of the last frame (0: not known).
    public var channel: Int64
    /// The PHY rate of the last frame.
    public var phyRateKbps: Int64
    /// "broadcast" or the destination of the last frame.
    public var lastTo: String
    /// The payload of the last frame as hex; empty when it was encrypted.
    public var lastPayload: String
    /// The ESP-NOW version of the last readable frame. No value before one.
    public var version: Int64?
    /// The UTC of the first frame.
    public var firstHeard: Double?
    /// The UTC of the last frame.
    public var lastHeard: Double?
    /// The signal time of the first frame.
    public var firstS: Double
    /// The signal time of the last frame.
    public var lastS: Double

    init(_ c: sdrkit_espnow_device_row) {
        key = sdrkitText(c.key)
        address = sdrkitText(c.address)
        vendor = sdrkitText(c.vendor)
        espressif = c.espressif
        packets = c.packets
        encrypted = c.encrypted
        broadcasts = c.broadcasts
        rate = c.rate
        levelDbfs = c.level_dbfs
        channel = c.channel
        phyRateKbps = c.phy_rate_kbps
        lastTo = sdrkitText(c.last_to)
        lastPayload = sdrkitText(c.last_payload)
        version = c.has_version ? c.version : nil
        firstHeard = c.has_first_heard ? c.first_heard : nil
        lastHeard = c.has_last_heard ? c.last_heard : nil
        firstS = c.first_s
        lastS = c.last_s
    }
}

/// A row of the frame log of `espnow`.
public struct EspnowFrameRow {
    /// The row key.
    public var key: String
    /// The UTC of the frame. No value for a recording.
    public var time: Double?
    /// The signal time of the frame.
    public var signalS: Double
    /// The channel (0: not known).
    public var channel: Int64
    /// The PHY rate.
    public var phyRateKbps: Int64
    /// A DSSS frame with the short preamble.
    public var shortPreamble: Bool
    /// The sender address.
    public var from: String
    /// "broadcast" or the destination address.
    public var to: String
    /// The sequence number.
    public var seq: Int64
    /// The ESP-NOW version. No value for a frame that stayed encrypted.
    public var version: Int64?
    /// The frame was protected.
    public var encrypted: Bool
    /// A key opened the frame.
    public var decrypted: Bool
    /// The level of the frame.
    public var levelDbfs: Double
    /// The SNR of the frame.
    public var snrDb: Double?
    /// The carrier offset of the frame.
    public var offsetHz: Double?
    /// The payload as hex; empty when it stayed encrypted.
    public var payload: String
    /// The summary.
    public var details: String

    init(_ c: sdrkit_espnow_frame_row) {
        key = sdrkitText(c.key)
        time = c.has_time ? c.time : nil
        signalS = c.signal_s
        channel = c.channel
        phyRateKbps = c.phy_rate_kbps
        shortPreamble = c.short_preamble
        from = sdrkitText(c.from)
        to = sdrkitText(c.to)
        seq = c.seq
        version = c.has_version ? c.version : nil
        encrypted = c.encrypted
        decrypted = c.decrypted
        levelDbfs = c.level_dbfs
        snrDb = c.has_snr_db ? c.snr_db : nil
        offsetHz = c.has_offset_hz ? c.offset_hz : nil
        payload = sdrkitText(c.payload)
        details = sdrkitText(c.details)
    }
}

/// A row of the ESP-NOW frames at each PHY rate of `espnow`. The row key is the key of the rate.
public struct EspnowRateRow {
    /// The row key.
    public var key: String
    /// The PHY rate group.
    public var rate: String
    /// ESP-NOW frames at the rate.
    public var frames: Int64

    init(_ c: sdrkit_espnow_rate_row) {
        key = sdrkitText(c.key)
        rate = sdrkitText(c.rate)
        frames = c.frames
    }
}

/// A row of the ESP-NOW frames on each channel of `espnow`. The row key is the channel number.
public struct EspnowChannelRow {
    /// The row key.
    public var key: String
    /// The channel.
    public var channel: Int64
    /// ESP-NOW frames on the channel.
    public var frames: Int64

    init(_ c: sdrkit_espnow_channel_row) {
        key = sdrkitText(c.key)
        channel = c.channel
        frames = c.frames
    }
}

/// The fields of a good packet with an address.
public struct BleDeviceEvent {
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
    /// The address.
    public var source: String
    /// The name of the device, if known.
    public var text: String?
    /// The level of the packet.
    public var levelDbfs: Double
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_ble_device_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        text = c.has_text ? sdrkitText(c.text) : nil
        levelDbfs = c.level_dbfs
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BleDeviceEvent, SDRKitCode> {
        var c = sdrkit_ble_device_event()
        let code = sdrkit_ble_device_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BleDeviceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an unwanted tracking alert.
public struct BleTrackerEvent {
    public enum Kind: Int32 {
        case findMy = 0
        case tile = 1
        case chipolo = 2
        case smartTag = 3
        case google = 4
    }
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
    /// The label of the tracker.
    public var source: String
    /// The kind, if recognised.
    public var kind: Kind?
    /// The text of the alert.
    public var text: String
    /// How long it stayed with the radio.
    public var stays: Double
    /// The level of the tracker.
    public var levelDbfs: Double
    /// Urgent for a tracker away from its owner, else a notice.
    public var urgency: Urgency?
    /// The family of the source: an alert.
    public var family: Family?

    init(_ c: sdrkit_ble_tracker_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        kind = c.has_kind ? Kind(rawValue: c.kind) : nil
        text = sdrkitText(c.text)
        stays = c.stays
        levelDbfs = c.level_dbfs
        urgency = Urgency(rawValue: c.urgency)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BleTrackerEvent, SDRKitCode> {
        var c = sdrkit_ble_tracker_event()
        let code = sdrkit_ble_tracker_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BleTrackerEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a LAP heard for the first time.
public struct BtclassicDeviceNewEvent {
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
    /// The LAP as hex text.
    public var lap: String
    /// What the device is.
    public var kind: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_btclassic_device_new_event) {
        time = c.time
        freqHz = c.freq_hz
        lap = sdrkitText(c.lap)
        kind = sdrkitText(c.kind)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BtclassicDeviceNewEvent, SDRKitCode> {
        var c = sdrkit_btclassic_device_new_event()
        let code = sdrkit_btclassic_device_new_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BtclassicDeviceNewEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an inquiry response.
public struct BtclassicInquiryResponseEvent {
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
    /// The address.
    public var address: String
    /// The class of device as text.
    public var `class`: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_btclassic_inquiry_response_event) {
        time = c.time
        freqHz = c.freq_hz
        address = sdrkitText(c.address)
        `class` = sdrkitText(c.class_)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<BtclassicInquiryResponseEvent, SDRKitCode> {
        var c = sdrkit_btclassic_inquiry_response_event()
        let code = sdrkit_btclassic_inquiry_response_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(BtclassicInquiryResponseEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of an ESP-NOW frame.
public struct EspnowFrameEvent {
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
    /// The sender address.
    public var source: String
    /// "broadcast" or the destination address.
    public var to: String
    /// The summary.
    public var text: String
    /// The level of the frame.
    public var levelDbfs: Double
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_espnow_frame_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        to = sdrkitText(c.to)
        text = sdrkitText(c.text)
        levelDbfs = c.level_dbfs
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EspnowFrameEvent, SDRKitCode> {
        var c = sdrkit_espnow_frame_event()
        let code = sdrkit_espnow_frame_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EspnowFrameEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The fields of a device heard for the first time.
public struct EspnowDeviceEvent {
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
    /// The sender address.
    public var source: String
    /// The maker of the address.
    public var vendor: String
    /// The family of the source.
    public var family: Family?

    init(_ c: sdrkit_espnow_device_event) {
        time = c.time
        freqHz = c.freq_hz
        source = sdrkitText(c.source)
        vendor = sdrkitText(c.vendor)
        family = Family(rawValue: c.family)
    }

    /// Event `n` of the last take on `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<EspnowDeviceEvent, SDRKitCode> {
        var c = sdrkit_espnow_device_event()
        let code = sdrkit_espnow_device_event_read(handle, n, &c)
        return code == SDRKIT_OK ? .success(EspnowDeviceEvent(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "ble" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct BleDropped {
    /// The items that the component "adv" dropped.
    public var adv: Int64?

    init(_ c: sdrkit_ble_dropped) {
        adv = c.has_adv ? c.adv : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BleDropped, SDRKitCode> {
        var c = sdrkit_ble_dropped()
        let code = sdrkit_ble_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(BleDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "bletracker" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct BletrackerDropped {
    /// The items that the component "adv" dropped.
    public var adv: Int64?

    init(_ c: sdrkit_bletracker_dropped) {
        adv = c.has_adv ? c.adv : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<BletrackerDropped, SDRKitCode> {
        var c = sdrkit_bletracker_dropped()
        let code = sdrkit_bletracker_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(BletrackerDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "wifi" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct WifiDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_wifi_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<WifiDropped, SDRKitCode> {
        var c = sdrkit_wifi_dropped()
        let code = sdrkit_wifi_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(WifiDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "halow" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct HalowDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_halow_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<HalowDropped, SDRKitCode> {
        var c = sdrkit_halow_dropped()
        let code = sdrkit_halow_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(HalowDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The items that each decoder component of the stack "espnow" dropped because it was slow. A member holds a
/// value while its decoder runs on a helper thread.
public struct EspnowDropped {
    /// The items that the component "rx" dropped.
    public var rx: Int64?

    init(_ c: sdrkit_espnow_dropped) {
        rx = c.has_rx ? c.rx : nil
    }

    /// The counts in the snapshot of `handle`, or the code of the error.
    public static func read(_ handle: SDRKIT_HANDLE) -> Result<EspnowDropped, SDRKitCode> {
        var c = sdrkit_espnow_dropped()
        let code = sdrkit_espnow_dropped_read(handle, &c)
        return code == SDRKIT_OK ? .success(EspnowDropped(c)) : .failure(SDRKitCode(code: code))
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum WlanTables {
    /// The changed rows of the table "devices" of the stack "ble", newest first, at most `capacity` rows.
    public static func bleDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[BleDeviceRow], SDRKitCode> {
        var rows = [sdrkit_ble_device_row](repeating: sdrkit_ble_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ble_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BleDeviceRow.init))
    }
    /// The changed rows of the table "packets" of the stack "ble", newest first, at most `capacity` rows.
    public static func blePackets(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[BlePacketRow], SDRKitCode> {
        var rows = [sdrkit_ble_packet_row](repeating: sdrkit_ble_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ble_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BlePacketRow.init))
    }
    /// The changed rows of the table "pdu_types" of the stack "ble", newest first, at most `capacity` rows.
    public static func blePduTypes(_ handle: SDRKIT_HANDLE, capacity: Int = 16) -> Result<[BlePduTypeRow], SDRKitCode> {
        var rows = [sdrkit_ble_pdu_type_row](repeating: sdrkit_ble_pdu_type_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ble_pdu_types_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BlePduTypeRow.init))
    }
    /// The changed rows of the table "device_levels" of the stack "ble", newest first, at most `capacity` rows.
    public static func bleDeviceLevels(_ handle: SDRKIT_HANDLE, capacity: Int = 9600) -> Result<[BleDeviceLevelRow], SDRKitCode> {
        var rows = [sdrkit_ble_device_level_row](repeating: sdrkit_ble_device_level_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_ble_device_levels_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BleDeviceLevelRow.init))
    }
    /// The changed rows of the table "trackers" of the stack "bletracker", newest first, at most `capacity` rows.
    public static func bletrackerTrackers(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[BletrackerTrackerRow], SDRKitCode> {
        var rows = [sdrkit_bletracker_tracker_row](repeating: sdrkit_bletracker_tracker_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bletracker_trackers_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BletrackerTrackerRow.init))
    }
    /// The changed rows of the table "alerts" of the stack "bletracker", newest first, at most `capacity` rows.
    public static func bletrackerAlerts(_ handle: SDRKIT_HANDLE, capacity: Int = 50) -> Result<[BletrackerAlertRow], SDRKitCode> {
        var rows = [sdrkit_bletracker_alert_row](repeating: sdrkit_bletracker_alert_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bletracker_alerts_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BletrackerAlertRow.init))
    }
    /// The changed rows of the table "tracker_kinds" of the stack "bletracker", newest first, at most `capacity` rows.
    public static func bletrackerTrackerKinds(_ handle: SDRKIT_HANDLE, capacity: Int = 5) -> Result<[BletrackerKindRow], SDRKitCode> {
        var rows = [sdrkit_bletracker_kind_row](repeating: sdrkit_bletracker_kind_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_bletracker_tracker_kinds_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BletrackerKindRow.init))
    }
    /// The changed rows of the table "networks" of the stack "wifi", newest first, at most `capacity` rows.
    public static func wifiNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[WifiNetworkRow], SDRKitCode> {
        var rows = [sdrkit_wifi_network_row](repeating: sdrkit_wifi_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wifi_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WifiNetworkRow.init))
    }
    /// The changed rows of the table "beacons" of the stack "wifi", newest first, at most `capacity` rows.
    public static func wifiBeacons(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[WifiBeaconRow], SDRKitCode> {
        var rows = [sdrkit_wifi_beacon_row](repeating: sdrkit_wifi_beacon_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wifi_beacons_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WifiBeaconRow.init))
    }
    /// The changed rows of the table "ofdm_rates" of the stack "wifi", newest first, at most `capacity` rows.
    public static func wifiOfdmRates(_ handle: SDRKIT_HANDLE, capacity: Int = 8) -> Result<[WifiRateRow], SDRKitCode> {
        var rows = [sdrkit_wifi_rate_row](repeating: sdrkit_wifi_rate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wifi_ofdm_rates_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WifiRateRow.init))
    }
    /// The changed rows of the table "bands" of the stack "wifi", newest first, at most `capacity` rows.
    public static func wifiBands(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[WifiBandRow], SDRKitCode> {
        var rows = [sdrkit_wifi_band_row](repeating: sdrkit_wifi_band_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wifi_bands_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WifiBandRow.init))
    }
    /// The changed rows of the table "networks" of the stack "halow", newest first, at most `capacity` rows.
    public static func halowNetworks(_ handle: SDRKIT_HANDLE, capacity: Int = 200) -> Result<[HalowNetworkRow], SDRKitCode> {
        var rows = [sdrkit_halow_network_row](repeating: sdrkit_halow_network_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_halow_networks_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HalowNetworkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "halow", newest first, at most `capacity` rows.
    public static func halowFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 600) -> Result<[HalowFrameRow], SDRKitCode> {
        var rows = [sdrkit_halow_frame_row](repeating: sdrkit_halow_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_halow_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HalowFrameRow.init))
    }
    /// The changed rows of the table "widths" of the stack "halow", newest first, at most `capacity` rows.
    public static func halowWidths(_ handle: SDRKIT_HANDLE, capacity: Int = 5) -> Result<[HalowWidthRow], SDRKitCode> {
        var rows = [sdrkit_halow_width_row](repeating: sdrkit_halow_width_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_halow_widths_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HalowWidthRow.init))
    }
    /// The changed rows of the table "by_mcs" of the stack "halow", newest first, at most `capacity` rows.
    public static func halowByMcs(_ handle: SDRKIT_HANDLE, capacity: Int = 11) -> Result<[HalowMcsRow], SDRKitCode> {
        var rows = [sdrkit_halow_mcs_row](repeating: sdrkit_halow_mcs_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_halow_by_mcs_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HalowMcsRow.init))
    }
    /// The changed rows of the table "by_preamble" of the stack "halow", newest first, at most `capacity` rows.
    public static func halowByPreamble(_ handle: SDRKIT_HANDLE, capacity: Int = 3) -> Result<[HalowPreambleRow], SDRKitCode> {
        var rows = [sdrkit_halow_preamble_row](repeating: sdrkit_halow_preamble_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_halow_by_preamble_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(HalowPreambleRow.init))
    }
    /// The changed rows of the table "devices" of the stack "btclassic", newest first, at most `capacity` rows.
    public static func btclassicDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[BtclassicDeviceRow], SDRKitCode> {
        var rows = [sdrkit_btclassic_device_row](repeating: sdrkit_btclassic_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_btclassic_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BtclassicDeviceRow.init))
    }
    /// The changed rows of the table "packets" of the stack "btclassic", newest first, at most `capacity` rows.
    public static func btclassicPackets(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[BtclassicPacketRow], SDRKitCode> {
        var rows = [sdrkit_btclassic_packet_row](repeating: sdrkit_btclassic_packet_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_btclassic_packets_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(BtclassicPacketRow.init))
    }
    /// The changed rows of the table "links" of the stack "wfb", newest first, at most `capacity` rows.
    public static func wfbLinks(_ handle: SDRKIT_HANDLE, capacity: Int = 64) -> Result<[WfbLinkRow], SDRKitCode> {
        var rows = [sdrkit_wfb_link_row](repeating: sdrkit_wfb_link_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wfb_links_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WfbLinkRow.init))
    }
    /// The changed rows of the table "frames" of the stack "wfb", newest first, at most `capacity` rows.
    public static func wfbFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 400) -> Result<[WfbFrameRow], SDRKitCode> {
        var rows = [sdrkit_wfb_frame_row](repeating: sdrkit_wfb_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_wfb_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(WfbFrameRow.init))
    }
    /// The changed rows of the table "devices" of the stack "espnow", newest first, at most `capacity` rows.
    public static func espnowDevices(_ handle: SDRKIT_HANDLE, capacity: Int = 300) -> Result<[EspnowDeviceRow], SDRKitCode> {
        var rows = [sdrkit_espnow_device_row](repeating: sdrkit_espnow_device_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_espnow_devices_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EspnowDeviceRow.init))
    }
    /// The changed rows of the table "frames" of the stack "espnow", newest first, at most `capacity` rows.
    public static func espnowFrames(_ handle: SDRKIT_HANDLE, capacity: Int = 500) -> Result<[EspnowFrameRow], SDRKitCode> {
        var rows = [sdrkit_espnow_frame_row](repeating: sdrkit_espnow_frame_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_espnow_frames_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EspnowFrameRow.init))
    }
    /// The changed rows of the table "by_rate" of the stack "espnow", newest first, at most `capacity` rows.
    public static func espnowByRate(_ handle: SDRKIT_HANDLE, capacity: Int = 5) -> Result<[EspnowRateRow], SDRKitCode> {
        var rows = [sdrkit_espnow_rate_row](repeating: sdrkit_espnow_rate_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_espnow_by_rate_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EspnowRateRow.init))
    }
    /// The changed rows of the table "by_channel" of the stack "espnow", newest first, at most `capacity` rows.
    public static func espnowByChannel(_ handle: SDRKIT_HANDLE, capacity: Int = 14) -> Result<[EspnowChannelRow], SDRKitCode> {
        var rows = [sdrkit_espnow_channel_row](repeating: sdrkit_espnow_channel_row(), count: capacity)
        var info = sdrkit_rows_info.sized
        let code = sdrkit_espnow_by_channel_rows(handle, &rows, UInt32(capacity), &info)
        guard code == SDRKIT_OK else { return .failure(SDRKitCode(code: code)) }
        return .success(rows.prefix(min(Int(info.rows), capacity)).map(EspnowChannelRow.init))
    }
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum WlanCommands {
    /// Clear. Forgets everything decoded so far.
    public static func bleClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_ble_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func bleRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_ble_device_args()
        args.size = UInt32(MemoryLayout<sdrkit_ble_device_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_ble_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func bletrackerClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_bletracker_clear(handle)
        return sdrkitResult(code)
    }
    /// Mark as mine. A tracker marked as the user's own raises no alert.
    public static func bletrackerMine(_ handle: SDRKIT_HANDLE, row: String, mine: Bool? = nil) -> Result<Void, SDRKitCode> {
        var args = sdrkit_bletracker_mine_args()
        args.size = UInt32(MemoryLayout<sdrkit_bletracker_mine_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        args.has_mine = mine != nil
        if let given = mine { args.mine = given }
        return sdrkit_bletracker_mine(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func wifiClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wifi_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func wifiRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wlan_network_args()
        args.size = UInt32(MemoryLayout<sdrkit_wlan_network_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_wifi_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func halowClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_halow_clear(handle)
        return sdrkitResult(code)
    }
    /// Remove. Forgets one object, a favourite too.
    public static func halowRemove(_ handle: SDRKIT_HANDLE, row: String) -> Result<Void, SDRKitCode> {
        var args = sdrkit_wlan_network_args()
        args.size = UInt32(MemoryLayout<sdrkit_wlan_network_args>.size)
        let code: Int32 = sdrkitWithString(row) { row_ in { () -> Int32 in
        args.row = row_
        return sdrkit_halow_remove(handle, &args)
        }() }
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func btclassicClearDevices(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_btclassic_clear_devices(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func wfbClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_wfb_clear(handle)
        return sdrkitResult(code)
    }
    /// Clear. Forgets everything decoded so far.
    public static func espnowClear(_ handle: SDRKIT_HANDLE) -> Result<Void, SDRKitCode> {
        let code = sdrkit_espnow_clear(handle)
        return sdrkitResult(code)
    }
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum WlanSettings {
    /// The options of the setting "channel" of the stack "ble".
    public enum BleChannel: Int32 {
        case n372402Mhz = 0
        case n382426Mhz = 1
        case n392480Mhz = 2
    }
    /// Channel.
    public static func bleChannel(_ handle: SDRKIT_HANDLE) -> Result<BleChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(BleChannel(rawValue: v) ?? BleChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBleChannel(_ handle: SDRKIT_HANDLE, _ value: BleChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel", value.rawValue))
    }
    /// Hop.
    public static func bleHop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBleHop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop", value))
    }
    /// Dwell.
    public static func bleHopSeconds(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "hop_seconds", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBleHopSeconds(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "hop_seconds", value))
    }
    /// Filter.
    public static func bleFilterText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setBleFilterText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter_text", $0) })
    }
    /// Level above.
    public static func bleMinLevelDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBleMinLevelDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value))
    }
    /// Named only.
    public static func bleNamedOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "named_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBleNamedOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "named_only", value))
    }
    /// The options of the setting "channel" of the stack "bletracker".
    public enum BletrackerChannel: Int32 {
        case n372402Mhz = 0
        case n382426Mhz = 1
        case n392480Mhz = 2
    }
    /// Channel.
    public static func bletrackerChannel(_ handle: SDRKIT_HANDLE) -> Result<BletrackerChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(BletrackerChannel(rawValue: v) ?? BletrackerChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBletrackerChannel(_ handle: SDRKIT_HANDLE, _ value: BletrackerChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "channel", value.rawValue))
    }
    /// Alert after.
    public static func bletrackerAlertAfterMin(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "alert_after_min", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBletrackerAlertAfterMin(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "alert_after_min", value))
    }
    /// Near above.
    public static func bletrackerNearDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "near_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBletrackerNearDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "near_dbfs", value))
    }
    /// Alert after (seconds, for tests).
    public static func bletrackerAlertAfterS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "alert_after_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBletrackerAlertAfterS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "alert_after_s", value))
    }
    /// The options of the setting "band" of the stack "wifi".
    public enum WifiBand: Int32 {
        case n24 = 0
        case n5Ghz = 1
        case n6Ghz = 2
    }
    /// Band.
    public static func wifiBand(_ handle: SDRKIT_HANDLE) -> Result<WifiBand, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band", &v)
        return code == SDRKIT_OK ? .success(WifiBand(rawValue: v) ?? WifiBand(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiBand(_ handle: SDRKIT_HANDLE, _ value: WifiBand) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band", value.rawValue))
    }
    /// Channel.
    public static func wifiChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Hop.
    public static func wifiHop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiHop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop", value))
    }
    /// Dwell.
    public static func wifiDwellMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiDwellMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell_ms", value))
    }
    /// 6 GHz: preferred scanning channels only.
    public static func wifiPscOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "psc_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiPscOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "psc_only", value))
    }
    /// 2.4 GHz channels.
    public static func wifiChannels24(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_24", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiChannels24(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_24", $0) })
    }
    /// 5 GHz channels.
    public static func wifiChannels5(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_5", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiChannels5(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_5", $0) })
    }
    /// 6 GHz channels.
    public static func wifiChannels6(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_6", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiChannels6(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_6", $0) })
    }
    /// Filter.
    public static func wifiFilterText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiFilterText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter_text", $0) })
    }
    /// Level above.
    public static func wifiMinLevelDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiMinLevelDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value))
    }
    /// Open only.
    public static func wifiOpenOnly(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "open_only", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiOpenOnly(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "open_only", value))
    }
    /// The options of the setting "band_filter" of the stack "wifi".
    public enum WifiBandFilter: Int32 {
        case all = 0
        case n24 = 1
        case n5Ghz = 2
        case n6Ghz = 3
    }
    /// Band shown.
    public static func wifiBandFilter(_ handle: SDRKIT_HANDLE) -> Result<WifiBandFilter, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "band_filter", &v)
        return code == SDRKIT_OK ? .success(WifiBandFilter(rawValue: v) ?? WifiBandFilter(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiBandFilter(_ handle: SDRKIT_HANDLE, _ value: WifiBandFilter) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "band_filter", value.rawValue))
    }
    /// Channel starting frequency.
    public static func halowStartHz(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "start_hz", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowStartHz(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "start_hz", value))
    }
    /// 1 MHz channels.
    public static func halowChannels1(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_1", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannels1(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_1", $0) })
    }
    /// 2 MHz channels.
    public static func halowChannels2(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_2", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannels2(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_2", $0) })
    }
    /// 4 MHz channels.
    public static func halowChannels4(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_4", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannels4(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_4", $0) })
    }
    /// 8 MHz channels.
    public static func halowChannels8(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_8", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannels8(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_8", $0) })
    }
    /// 16 MHz channels.
    public static func halowChannels16(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_16", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannels16(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_16", $0) })
    }
    /// The options of the setting "list_width" of the stack "halow".
    public enum HalowListWidth: Int32 {
        case n1Mhz = 0
        case n2Mhz = 1
        case n4Mhz = 2
        case n8Mhz = 3
        case n16Mhz = 4
    }
    /// Channel list width.
    public static func halowListWidth(_ handle: SDRKIT_HANDLE) -> Result<HalowListWidth, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "list_width", &v)
        return code == SDRKIT_OK ? .success(HalowListWidth(rawValue: v) ?? HalowListWidth(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowListWidth(_ handle: SDRKIT_HANDLE, _ value: HalowListWidth) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "list_width", value.rawValue))
    }
    /// Channel.
    public static func halowChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Scan.
    public static func halowScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "scan", value))
    }
    /// Dwell.
    public static func halowDwellS(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell_s", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowDwellS(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell_s", value))
    }
    /// Filter.
    public static func halowFilterText(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "filter_text", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowFilterText(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "filter_text", $0) })
    }
    /// Level above.
    public static func halowMinLevelDbfs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "min_level_dbfs", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowMinLevelDbfs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "min_level_dbfs", value))
    }
    /// The options of the setting "receive" of the stack "halow".
    public enum HalowReceive: Int32 {
        case auto14 = 0
        case autoAll = 1
        case n1Mhz = 2
        case n2Mhz = 3
        case n4Mhz = 4
        case n8Mhz = 5
        case n16Mhz = 6
    }
    /// Receive.
    public static func halowReceive(_ handle: SDRKIT_HANDLE) -> Result<HalowReceive, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "receive", &v)
        return code == SDRKIT_OK ? .success(HalowReceive(rawValue: v) ?? HalowReceive(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowReceive(_ handle: SDRKIT_HANDLE, _ value: HalowReceive) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "receive", value.rawValue))
    }
    /// Sub-channel scan.
    public static func halowSubChannelScan(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "sub_channel_scan", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setHalowSubChannelScan(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "sub_channel_scan", value))
    }
    /// Sync word errors allowed.
    public static func btclassicSyncErrors(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "sync_errors", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBtclassicSyncErrors(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "sync_errors", value))
    }
    /// Burst threshold.
    public static func btclassicThresholdDb(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "threshold_db", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBtclassicThresholdDb(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "threshold_db", value))
    }
    /// Wideband from.
    public static func btclassicWideChannels(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "wide_channels", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBtclassicWideChannels(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "wide_channels", value))
    }
    /// The options of the setting "links" of the stack "wfb".
    public enum WfbLinks: Int32 {
        case wfbNg = 0
        case openhd = 1
        case both = 2
    }
    /// Links shown.
    public static func wfbLinks(_ handle: SDRKIT_HANDLE) -> Result<WfbLinks, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "links", &v)
        return code == SDRKIT_OK ? .success(WfbLinks(rawValue: v) ?? WfbLinks(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWfbLinks(_ handle: SDRKIT_HANDLE, _ value: WfbLinks) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "links", value.rawValue))
    }
    /// Channel.
    public static func espnowChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "channel", value))
    }
    /// Hop.
    public static func espnowHop(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "hop", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowHop(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "hop", value))
    }
    /// Dwell.
    public static func espnowDwellMs(_ handle: SDRKIT_HANDLE) -> Result<Double, SDRKitCode> {
        var v: Double = 0
        let code = sdrkit_typed_setting_get_f64(handle, "dwell_ms", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowDwellMs(_ handle: SDRKIT_HANDLE, _ value: Double) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_f64(handle, "dwell_ms", value))
    }
    /// 2.4 GHz channels.
    public static func espnowChannels24(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "channels_24", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowChannels24(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "channels_24", $0) })
    }
    /// Keys.
    public static func espnowKeys(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "keys", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowKeys(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "keys", $0) })
    }
}

/// The settings of the test signal generators of the family, each read and written on the handle of a session of
/// its stack.
public enum WlanGeneratorSettings {
    /// The options of the setting "gen_channel" of the stack "ble".
    public enum BleChannel: Int32 {
        case round = 0
        case allOn37 = 1
        case allOn38 = 2
        case allOn39 = 3
    }
    /// Whitening.
    public static func bleChannel(_ handle: SDRKIT_HANDLE) -> Result<BleChannel, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_channel", &v)
        return code == SDRKIT_OK ? .success(BleChannel(rawValue: v) ?? BleChannel(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBleChannel(_ handle: SDRKIT_HANDLE, _ value: BleChannel) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_channel", value.rawValue))
    }
    /// The options of the setting "gen_schedule" of the stack "bletracker".
    public enum BletrackerSchedule: Int32 {
        case all = 0
        case findMy = 1
    }
    /// Schedule.
    public static func bletrackerSchedule(_ handle: SDRKIT_HANDLE) -> Result<BletrackerSchedule, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_schedule", &v)
        return code == SDRKIT_OK ? .success(BletrackerSchedule(rawValue: v) ?? BletrackerSchedule(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setBletrackerSchedule(_ handle: SDRKIT_HANDLE, _ value: BletrackerSchedule) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_schedule", value.rawValue))
    }
    /// The options of the setting "gen_scene" of the stack "wifi".
    public enum WifiScene: Int32 {
        case n24 = 0
        case n5Ghz = 1
        case n6Ghz = 2
    }
    /// Scene.
    public static func wifiScene(_ handle: SDRKIT_HANDLE) -> Result<WifiScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(WifiScene(rawValue: v) ?? WifiScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiScene(_ handle: SDRKIT_HANDLE, _ value: WifiScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// Primary channel.
    public static func wifiChannel(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_channel", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setWifiChannel(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_channel", value))
    }
    /// Test signal wideband bursts.
    public static func btclassicWideband(_ handle: SDRKIT_HANDLE) -> Result<Bool, SDRKitCode> {
        var v: Bool = false
        let code = sdrkit_typed_setting_get_bool(handle, "gen_wideband", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setBtclassicWideband(_ handle: SDRKIT_HANDLE, _ value: Bool) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_bool(handle, "gen_wideband", value))
    }
    /// The options of the setting "gen_scene" of the stack "espnow".
    public enum EspnowScene: Int32 {
        case mixed = 0
        case single = 1
    }
    /// Scene.
    public static func espnowScene(_ handle: SDRKIT_HANDLE) -> Result<EspnowScene, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &v)
        return code == SDRKIT_OK ? .success(EspnowScene(rawValue: v) ?? EspnowScene(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowScene(_ handle: SDRKIT_HANDLE, _ value: EspnowScene) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_scene", value.rawValue))
    }
    /// The options of the setting "gen_phy" of the stack "espnow".
    public enum EspnowPhy: Int32 {
        case dsss1 = 0
        case dsss2Long = 1
        case dsss2Short = 2
        case ofdm6 = 3
        case ofdm12 = 4
    }
    /// PHY.
    public static func espnowPhy(_ handle: SDRKIT_HANDLE) -> Result<EspnowPhy, SDRKitCode> {
        var v: Int32 = 0
        let code = sdrkit_typed_setting_get_enum(handle, "gen_phy", &v)
        return code == SDRKIT_OK ? .success(EspnowPhy(rawValue: v) ?? EspnowPhy(rawValue: 0)!) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowPhy(_ handle: SDRKIT_HANDLE, _ value: EspnowPhy) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_enum(handle, "gen_phy", value.rawValue))
    }
    /// Sender.
    public static func espnowSender(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_sender", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowSender(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_sender", $0) })
    }
    /// Peer.
    public static func espnowPeer(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_peer", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowPeer(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_peer", $0) })
    }
    /// Payload.
    public static func espnowPayload(_ handle: SDRKIT_HANDLE) -> Result<String, SDRKitCode> {
        var v: sdrkit_string = sdrkit_string()
        let code = sdrkit_typed_setting_get_string(handle, "gen_payload", &v)
        return code == SDRKIT_OK ? .success(sdrkitText(v)) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowPayload(_ handle: SDRKIT_HANDLE, _ value: String) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkitWithString(value) { sdrkit_typed_setting_set_string(handle, "gen_payload", $0) })
    }
    /// Version.
    public static func espnowVersion(_ handle: SDRKIT_HANDLE) -> Result<Int64, SDRKitCode> {
        var v: Int64 = 0
        let code = sdrkit_typed_setting_get_i64(handle, "gen_version", &v)
        return code == SDRKIT_OK ? .success(v) : .failure(SDRKitCode(code: code))
    }
    public static func setEspnowVersion(_ handle: SDRKIT_HANDLE, _ value: Int64) -> Result<Void, SDRKitCode> {
        sdrkitResult(sdrkit_typed_setting_set_i64(handle, "gen_version", value))
    }
}
