import Foundation
import SDRKitC

/// An open radio. Thread safety: as for `Object`.
public final class Radio: Object {
    /// Stops and closes the radio. Throws `SDRKIT_E_RADIO_IN_USE` while sessions read it.
    public func close() throws {
        try ok(sdrkit_radio_close(handle))
    }
}

/// A share of a radio: the samples of the radio served to network clients over rtl_tcp and SoapyRemote.
/// `Engine.openShare(radio:)` opens it. Its settings (`ShareSettings`), its state (`ShareFields`), its tables of
/// listeners and clients and its commands (`listen`, `unlisten`, `kick`, `grant`) are those of the generated file
/// `Share.swift`.
public final class Share: Object {
    /// Closes the share: its listeners stop and its clients are disconnected.
    public func close() throws {
        try ok(sdrkit_share_close(handle))
    }
}

/// The audio that `Session.readAudio(into:)` copied.
public struct AudioInfo: Equatable, Sendable {
    /// The meaning and the order of the channels.
    public enum Layout: Int32, Sendable {
        /// Channels in an order that the library does not know.
        case unknown = 0
        /// One channel.
        case mono = 1
        /// Left, right.
        case stereo = 2
        /// 5.1: left, centre, right, left surround, right surround, low-frequency effects.
        case surround51 = 3
    }

    /// The number of frames copied, or 0 if no audio was ready. The buffer holds `frames` times `channels` floats.
    public var frames: Int
    /// Frames per second.
    public var rateHz: Double
    /// Samples per frame.
    public var channels: Int
    /// The order of the channels.
    public var layout: Layout
}

/// The spectrum frame that `Session.readSpectrum(into:)` copied.
public struct SpectrumInfo: Equatable, Sendable {
    /// The width of the frame in Hz.
    public var spanHz: Double
    /// The frequency at the middle bin.
    public var centreHz: Double
    /// Frames per second.
    public var frameRate: Double
    /// Seconds of spectrum before this frame.
    public var time: Double
}

/// The complex samples that `Session.readTransmit(into:)` copied.
public struct IQInfo: Equatable, Sendable {
    /// The number of samples copied, or 0 if no samples were ready. The buffer holds two floats for each sample, I
    /// then Q.
    public var samples: Int
    /// Samples per second.
    public var rateHz: Double
    /// The frequency at 0 Hz of the samples.
    public var centreHz: Double
}

/// The part of a tapped stream that `Session.readTap(_:into:)` copied.
public struct TapInfo: Equatable, Sendable {
    /// The number of floats copied, or 0 if nothing was ready. For `.iq` the values are I then Q.
    public var values: Int
    /// The type of the stream.
    public var type: StreamType
    /// Items per second.
    public var rateHz: Double
    /// The frequency at 0 Hz of the stream.
    public var centreHz: Double
    /// The number of items that the tap dropped because the caller did not read in time.
    public var dropped: UInt64
}

/// A media packet or a picture of a session.
public struct MediaPacket: Equatable, Sendable {
    /// The decoder that a packet needs.
    public enum Kind: Int32, Sendable {
        case h264 = 0
        case hevc = 1
        case mpeg2Video = 2
        case aac = 3
        case heAAC = 4
        case xheAAC = 5
        case ac3 = 6
        case eac3 = 7
        case mp1 = 8
        case mp2 = 9
        case mp3 = 10
        case picture = 11
        case mpegTS = 12
        /// Decoded video: one frame of raw pixels in display order, in the layout of `fourCC`, `width`, `height`
        /// and `strides`.
        case rawVideo = 13
    }

    /// Whether more of a picture can come.
    public enum PictureState: Int32, Sendable {
        /// The picture can change: more rows, more parts, or better data. A later packet with the same `pictureID`
        /// replaces this one.
        case growing = 1
        /// The picture is complete or its signal ended. A later packet with the same `pictureID` is the same
        /// picture drawn again, after a change of a setting of its view.
        case finished = 2
    }

    /// The colour matrix of raw pixels.
    public enum ColourMatrix: Int32, Sendable {
        /// Not known, or no pixels.
        case unknown = 0
        /// ITU-R BT.601 (standard definition).
        case bt601 = 1
        /// ITU-R BT.709 (high definition).
        case bt709 = 2
        /// ITU-R BT.2020.
        case bt2020 = 3
        /// RGB or grey: no matrix.
        case identity = 4
    }

    /// How packets are framed.
    public enum Packaging: Int32, Sendable {
        /// Bare access units.
        case raw = 0
        /// H.264 or HEVC with start codes.
        case annexB = 1
        /// H.264 or HEVC with a big-endian length of `lengthSize` bytes before each unit.
        case lengthPrefixed = 2
        /// AAC with ADTS headers.
        case adts = 3
        /// AAC in LATM / LOAS.
        case latm = 4
    }

    /// The counter of the packet. Pass it as `after` to the next read.
    public var counter: UInt64
    /// The number of packets that the session dropped before this packet and after the packet of `after`.
    public var missed: UInt64
    /// The decoder that the packet needs, or nil for a kind that this package does not know.
    public var kind: Kind?
    /// The four-character code of the codec or of the pixel format.
    public var fourCC: String
    /// How packets are framed.
    public var packaging: Packaging
    /// For `.lengthPrefixed`: the number of bytes of each length.
    public var lengthSize: Int
    /// The decoder configuration.
    public var config: Data
    /// The unit of `pts` and `dts` in seconds, as a numerator and a denominator.
    public var timebase: (numerator: UInt32, denominator: UInt32) { (timebaseNumerator, timebaseDenominator) }
    /// The numerator of the unit of `pts` and `dts`.
    public var timebaseNumerator: UInt32
    /// The denominator of that unit.
    public var timebaseDenominator: UInt32
    /// The presentation time, if the packet has one.
    public var pts: Int64?
    /// The decoding time, if the packet has one.
    public var dts: Int64?
    /// True if the packet starts a group that decodes alone.
    public var keyframe: Bool
    /// True if the packet is a keyframe after which nothing refers to data before it: an H.264 or HEVC IDR picture,
    /// an HEVC BLA picture, a decoded frame, an audio frame. A decoder that starts at a keyframe that is not a clean
    /// start (a recovery point, a CRA picture, an MPEG-2 I picture) can show damage until the next clean start. Some
    /// streams have no clean start at all (MPEG-2 video, H.264 with recovery points only): a host that waits for one
    /// in vain starts at a keyframe and drops the pictures with an earlier `pts` until the next keyframe.
    public var cleanStart: Bool
    /// The time of the signal at which the session made the packet, in seconds.
    public var time: Double
    /// The bytes of the packet.
    public var bytes: Data
    /// For raw pixels: the width in pixels, else 0.
    public var width: Int
    /// For raw pixels: the height in pixels, else 0.
    public var height: Int
    /// For raw pixels: the bytes per row of each of the four planes, 0 for a plane that does not exist.
    public var strides: [Int]
    /// For raw pixels: the colour matrix.
    public var colourMatrix: ColourMatrix
    /// For raw pixels: true for full range (0 to 255), false for video range (16 to 235).
    public var fullRange: Bool
    /// For video: the aspect ratio of the displayed picture, width to height, or nil if it is not known.
    public var aspect: (width: UInt32, height: UInt32)? {
        aspectWidth == 0 || aspectHeight == 0 ? nil : (aspectWidth, aspectHeight)
    }
    /// The width part of the aspect ratio, 0 if it is not known.
    public var aspectWidth: UInt32
    /// The height part of the aspect ratio, 0 if it is not known.
    public var aspectHeight: UInt32
    /// For video: the frames per second, or nil if the rate is not known.
    public var frameRate: Double? {
        frameRateDenominator == 0 ? nil : Double(frameRateNumerator) / Double(frameRateDenominator)
    }
    /// The numerator of the frame rate, 0 if it is not known.
    public var frameRateNumerator: UInt32
    /// The denominator of the frame rate, 0 if it is not known.
    public var frameRateDenominator: UInt32
    /// For decoded video: the two fields of the frame are from different times.
    public var interlaced: Bool
    /// For decoded video: the top field is the first in time.
    public var topFieldFirst: Bool
    /// For decoded video: the stream lost or damaged pictures and the frame is the last good one again.
    public var damaged: Bool
    /// For a picture: the number of the picture in the session, from 1. Packets with the same number are states of
    /// one picture, and each replaces the one before. A new picture has a new number, also when it has the size and
    /// the name of the one before. 0 for media.
    public var pictureID: UInt64
    /// For a picture: whether more of it can come. nil for media.
    public var pictureState: PictureState?
    /// For a picture: its name (a product, a channel, a picture mode, a file name). Empty if it has none.
    public var pictureName: String

    /// Copies the packet `i`. The views of `i` must still be valid.
    init(_ i: sdrkit_media_info) {
        counter = i.counter
        missed = i.missed
        kind = Kind(rawValue: i.type)
        fourCC = fourCCString(i.fourcc)
        packaging = Packaging(rawValue: i.packaging) ?? .raw
        lengthSize = Int(i.length_size)
        config = copy(i.config)
        timebaseNumerator = i.timebase_num
        timebaseDenominator = i.timebase_den
        pts = i.has_pts ? i.pts : nil
        dts = i.has_dts ? i.dts : nil
        keyframe = i.keyframe
        cleanStart = i.clean_start
        time = i.time
        bytes = copy(i.bytes)
        width = Int(i.width)
        height = Int(i.height)
        strides = [i.strides.0, i.strides.1, i.strides.2, i.strides.3]
        colourMatrix = ColourMatrix(rawValue: i.colour_matrix) ?? .unknown
        fullRange = i.full_range
        aspectWidth = i.aspect_num
        aspectHeight = i.aspect_den
        frameRateNumerator = i.frame_rate_num
        frameRateDenominator = i.frame_rate_den
        interlaced = i.interlaced
        topFieldFirst = i.top_field_first
        damaged = i.damaged
        pictureID = i.picture_id
        pictureState = PictureState(rawValue: i.picture_state)
        pictureName = copy(i.picture_name)
    }
}

/// A session: one stack or one graph that reads a radio or a host feed.
///
/// Thread safety: as for `Object`. A thread that reads audio and a thread that reads the results each need their
/// own handle of the session: open the second one with `Engine.find(_:)` and the key of the session.
public final class Session: Object {
    /// Stops the session. Its results stay until `close()`.
    public func stop() throws {
        try ok(sdrkit_session_stop(handle))
    }

    /// Closes the stopped session. Throws `SDRKIT_E_STILL_RUNNING` if the session was not stopped.
    public func close() throws {
        try ok(sdrkit_session_close(handle))
    }

    /// What is wrong with the session now, as sentences: the problem of its receiver, the last refused request, and
    /// what stops it from transmitting. Empty if nothing is wrong.
    public var problem: String {
        get throws {
            var s = sdrkit_string()
            try ok(sdrkit_problem(handle, &s))
            return copy(s)
        }
    }

    /// Returns a new handle of the stack of the session, for the settings, commands and results of the stack.
    public func stack() throws -> Object {
        var o: UInt64 = 0
        try ok(sdrkit_object_stack(handle, &o))
        return Object(handle: o, owner: owner)
    }

    /// Returns a new handle of the component named `name` of the graph of the session. Throws
    /// `SDRKIT_E_NO_SUCH_KEY` if the graph has no component of that name.
    public func component(_ name: String) throws -> Object {
        var o: UInt64 = 0
        try ok(sdrkit_object_component(handle, try checked(name), &o))
        return Object(handle: o, owner: owner)
    }

    /// Copies the waiting audio into `buffer`, at most `buffer.count` floats, and returns what it copied. The call
    /// copies whole interleaved frames of one rate and one channel count, and does not wait: `frames` is 0 if no
    /// audio was ready. Audio that the caller does not read in time is lost, and the result field
    /// `session.audio_lost` counts it.
    public func readAudio(into buffer: inout [Float]) throws -> AudioInfo {
        var info = sdrkit_audio_info.sized
        let code = buffer.withUnsafeMutableBufferPointer { b in
            sdrkit_audio_read(handle, b.baseAddress, b.count, &info)
        }
        try ok(code)
        return AudioInfo(
            frames: info.frames, rateHz: info.rate_hz, channels: Int(info.channels),
            layout: AudioInfo.Layout(rawValue: info.layout) ?? .unknown)
    }

    /// Gives the session audio at `rateHz`, such as a microphone, for the host audio input of its graph. `samples`
    /// holds interleaved frames of `channels` floats: 1 for mono, or 2 for stereo as left then right. A part of a
    /// frame at the end is left out. The engine converts the audio to the rate and the channel count of that input:
    /// mono audio goes to both channels of a stereo input, and stereo audio becomes the mean of left and right for a
    /// mono input. The channel count and the rate can change from call to call. The session keeps 32768 floats
    /// (0.68 s of mono or 0.34 s of stereo at 48 kHz): audio that the graph does not take in time is lost, oldest
    /// first, and counted in the result field `session.audio_in_lost`. Open the microphone only while the result
    /// field `session.audio_in_wanted` is true: a transmission that sends the audio runs or push-to-talk is pressed.
    ///
    /// The call never waits and allocates nothing when it succeeds, so the render callback of an audio unit can make
    /// it, also while another thread uses this session. One thread at a time writes the audio of a session. Throws
    /// `SDRKIT_E_UNSUPPORTED` for a channel count that is not 1 or 2, and `SDRKIT_E_CONFIG` for a rate that is not
    /// above 0 or not finite.
    public func writeAudio(_ samples: UnsafeBufferPointer<Float>, channels: Int, rateHz: Double) throws {
        let count = UInt16(clamping: channels)
        let frames = count == 0 ? 0 : samples.count / Int(count)
        try ok(sdrkit_session_write_audio(handle, samples.baseAddress, UInt32(frames), count, rateHz))
    }

    /// `writeAudio(_:channels:rateHz:)` for an array.
    public func writeAudio(_ samples: [Float], channels: Int, rateHz: Double) throws {
        try samples.withUnsafeBufferPointer { try writeAudio($0, channels: channels, rateHz: rateHz) }
    }

    /// Replaces `bins` with the newest spectrum frame, lowest frequency first, if a frame came after the last call.
    /// Returns nil and leaves `bins` as it is if no frame came. The setting `spectrum` of the session turns the
    /// spectrum on.
    public func readSpectrum(into bins: inout [Float]) throws -> SpectrumInfo? {
        var frame = sdrkit_spectrum_frame.sized
        var copy = [Float](repeating: 0, count: max(bins.capacity, 1024))
        try ok(sdrkit_spectrum_read(handle, &copy, UInt32(copy.count), &frame))
        guard frame.found else { return nil }
        if frame.total > frame.bins {
            // The frame has more bins than the buffer: the next frame is read with a buffer of its size.
            bins.reserveCapacity(Int(frame.total))
            return nil
        }
        bins.removeAll(keepingCapacity: true)
        bins.append(contentsOf: copy.prefix(Int(frame.bins)))
        return SpectrumInfo(
            spanHz: frame.span_hz, centreHz: frame.centre_hz, frameRate: frame.frame_rate, time: frame.time)
    }

    /// Returns the first media packet after the counter `after`, or nil if there is none. Pass 0 for the oldest
    /// packet. The session keeps the coded packets of the last ten seconds, at most 32 MiB, and decoded video frames
    /// of the last half second, at most 16 MiB. Packets that leave before they are read are counted in the result
    /// field `session.media_lost`. Pictures are not media packets.
    public func readMedia(after: UInt64) throws -> MediaPacket? {
        var info = sdrkit_media_info.sized
        try ok(sdrkit_media_read(handle, after, &info))
        return info.found ? MediaPacket(info) : nil
    }

    /// Returns the first picture after the counter `after`, or nil if there is none. Pass 0 for the oldest picture.
    /// A picture is compressed bytes, or raw pixels in the layout of `width`, `height` and `strides`. `pictureID`
    /// says which picture the packet holds, `pictureState` whether more of it can come, and `pictureName` its name.
    /// A picture that grows comes again with the same `pictureID` and all of its bytes. The session keeps the newest
    /// state of each of its last 16 pictures.
    public func readPicture(after: UInt64) throws -> MediaPacket? {
        var info = sdrkit_media_info.sized
        try ok(sdrkit_picture_read(handle, after, &info))
        return info.found ? MediaPacket(info) : nil
    }

    /// Copies the samples that the component `tx.sink` of the graph of the session wrote into `buffer`, at most
    /// `buffer.count / 2` samples of two floats (I then Q). Samples that the caller does not read in time are lost,
    /// and the result field `session.transmit_lost` counts them. Throws `SDRKIT_E_TO_RADIO` if the engine sends the
    /// samples to the radio.
    public func readTransmit(into buffer: inout [Float]) throws -> IQInfo {
        var info = sdrkit_iq_info.sized
        let code = buffer.withUnsafeMutableBufferPointer { b in
            sdrkit_transmit_read(handle, b.baseAddress, b.count / 2, &info)
        }
        try ok(code)
        return IQInfo(samples: info.samples, rateHz: info.rate_hz, centreHz: info.centre_hz)
    }

    /// Opens a tap on the output `port` (`<component>.<port>`, or `input`) of the graph of the session and returns
    /// its id. A tap copies the stream at that point into at most `chunks` buffers until the caller reads them.
    /// Throws `SDRKIT_E_NO_GRAPH` or `SDRKIT_E_NO_SUCH_KEY`.
    public func openTap(_ port: String, chunks: Int) throws -> UInt32 {
        var tap: UInt32 = 0
        try ok(sdrkit_tap_open(handle, try checked(port), chunks, &tap))
        return tap
    }

    /// Copies the waiting values of the tap `tap` into `buffer`, at most `buffer.count` floats. Throws
    /// `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
    public func readTap(_ tap: UInt32, into buffer: inout [Float]) throws -> TapInfo {
        var info = sdrkit_tap_info.sized
        let code = buffer.withUnsafeMutableBufferPointer { b in
            sdrkit_tap_read(handle, tap, b.baseAddress, b.count, &info)
        }
        try ok(code)
        return TapInfo(
            values: info.values, type: StreamType(rawValue: info.type) ?? .none, rateHz: info.rate_hz,
            centreHz: info.centre_hz, dropped: info.dropped)
    }

    /// Closes the tap `tap`. Throws `SDRKIT_E_NO_SUCH_ID` if the tap does not exist.
    public func closeTap(_ tap: UInt32) throws {
        try ok(sdrkit_tap_close(handle, tap))
    }
}

/// A host feed: complex samples that the caller pushes, for every session that reads the feed. `deinit` closes the
/// feed.
///
/// Thread safety: as for `Object`. Push from one thread or one serial queue.
public final class Feed {
    /// The format of pushed samples.
    public enum SampleFormat: Int32, Sendable {
        /// 32-bit floats, I then Q.
        case cf32 = 0
        /// Signed 16-bit integers, I then Q, little-endian.
        case cs16 = 1
        /// Signed 32-bit integers, I then Q, little-endian.
        case cs32 = 2
        /// Signed 8-bit integers, I then Q.
        case cs8 = 3
        /// Unsigned 8-bit integers with zero at 127.5, I then Q.
        case cu8 = 4
    }

    let handle: UInt64
    let owner: EngineHandle

    init(handle: UInt64, owner: EngineHandle) {
        self.handle = handle
        self.owner = owner
    }

    deinit {
        sdrkit_free(handle)
    }

    /// Writes the bytes of `samples`, whole samples of the format `format`, to the feed. The call does not wait.
    /// Throws `SDRKIT_E_PART_SAMPLE` if the bytes end in a part of a sample, or `SDRKIT_E_BEHIND` if a session of
    /// the feed has not used the previous samples: the call then writes no samples, and the caller pushes them
    /// again later (`SDRKitError.isBehind`).
    public func push(_ samples: UnsafeRawBufferPointer, format: SampleFormat) throws {
        try check(sdrkit_samples_push(handle, format.rawValue, samples.baseAddress, samples.count), handle: handle)
    }

    /// Writes `samples`, 32-bit floats I then Q, to the feed. See `push(_:format:)`.
    public func push(_ samples: [Float]) throws {
        try samples.withUnsafeBytes { try push($0, format: .cf32) }
    }
}
