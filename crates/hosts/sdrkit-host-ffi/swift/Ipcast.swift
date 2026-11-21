// sdrkit: the typed interface of the shared interface "ipcast", whose items the stacks of several families show, version 1.0. MIT licence.
// Written from the interface description of the family. Do not edit.

import SDRKitC

/// The version of the family that this file describes.
public enum IpcastInterface {
    public static let major = 1
    public static let minor = 0
}

/// The result fields of `ipcast.bbframe`: the counts of the mode adaptation.
public struct IpcastBbFrameFields {
    /// BBFRAMEs with a good header.
    public var framesOk: Int64
    /// BBFRAMEs with a bad header.
    public var framesBad: Int64
    /// Transport packets of the chosen stream.
    public var tsPackets: Int64
    /// User packets with a bad CRC-8.
    public var crcErrors: Int64
    /// GSE PDUs of every stream.
    public var pdus: Int64
    /// GSE PDUs that carry no IP.
    public var nonIp: Int64

    init(_ c: sdrkit_ipcast_bb_frame_fields) {
        framesOk = c.frames_ok
        framesBad = c.frames_bad
        tsPackets = c.ts_packets
        crcErrors = c.crc_errors
        pdus = c.pdus
        nonIp = c.non_ip
    }
}

/// The result fields of `ipcast.alp`: the counts of the link layer.
public struct IpcastAlpFields {
    /// ALP packets read.
    public var alpPackets: Int64
    /// ALP packets dropped.
    public var alpDropped: Int64
    /// ALP packets of IP with compressed headers.
    public var compressedIp: Int64

    init(_ c: sdrkit_ipcast_alp_fields) {
        alpPackets = c.alp_packets
        alpDropped = c.alp_dropped
        compressedIp = c.compressed_ip
    }
}

/// The result fields of `ipcast.ip`: the counts of the IP layer.
public struct IpcastIpFields {
    /// IP packets read.
    public var ipPackets: Int64
    /// IP packets refused.
    public var ipBad: Int64
    /// Datagrams put together from fragments.
    public var reassembled: Int64
    /// Flows seen.
    public var flows: Int64
    /// Flows whose payload is encrypted.
    public var encryptedFlows: Int64

    init(_ c: sdrkit_ipcast_ip_fields) {
        ipPackets = c.ip_packets
        ipBad = c.ip_bad
        reassembled = c.reassembled
        flows = c.flows
        encryptedFlows = c.encrypted_flows
    }
}

/// The result fields of `ipcast.route`: the counts of the object delivery.
public struct IpcastRouteFields {
    /// Objects received whole.
    public var objects: Int64
    /// Objects dropped.
    public var objectsDropped: Int64

    init(_ c: sdrkit_ipcast_route_fields) {
        objects = c.objects
        objectsDropped = c.objects_dropped
    }
}

/// The tables of the family, each read from the snapshot of the handle of a session.
public enum IpcastTables {
}

/// The commands of the family, each run on the handle of a session of its stack.
public enum IpcastCommands {
}

/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one
/// setting.
public enum IpcastSettings {
}
