import Foundation
import SDRKitC

/// One option of a value of type `.option`.
public struct Option: Equatable, Sendable {
    /// The key that data files use.
    public var key: String
    /// The name for people.
    public var label: String
}

/// The declaration of one value: a setting, an argument, a reply field, a result field, a table column or an event
/// field. A member that does not apply is empty, nil or zero.
public struct Declaration: Equatable, Sendable {
    /// The bits of `flags`.
    public struct Flags: OptionSet, Sendable {
        public let rawValue: UInt32
        public init(rawValue: UInt32) { self.rawValue = rawValue }

        /// The declaration has a range.
        public static let range = Flags(rawValue: UInt32(SDRKIT_DECL_RANGE))
        /// The argument has no default: the caller must pass it.
        public static let required = Flags(rawValue: UInt32(SDRKIT_DECL_REQUIRED))
        /// A change takes effect after a restart of the object.
        public static let restart = Flags(rawValue: UInt32(SDRKIT_DECL_RESTART))
        /// The value cannot change.
        public static let fixed = Flags(rawValue: UInt32(SDRKIT_DECL_FIXED))
        /// The setting is advanced: a program shows the other settings first.
        public static let advanced = Flags(rawValue: UInt32(SDRKIT_DECL_ADVANCED))
    }

    /// What the bytes of a value hold.
    public enum Content: Int32, Sendable {
        case none = 0
        case media = 1
        case plain = 2
        case csv = 3
        case json = 4
        case xml = 5
        case binary = 6
    }

    /// The id of the value in its list.
    public var id: UInt32
    /// The key.
    public var key: String
    /// The name for people.
    public var label: String
    /// One or two sentences for people. Settings and arguments only.
    public var help: String
    /// The section that the setting shows in.
    public var section: String
    /// The unit.
    public var unit: String
    /// The type of the value.
    public var type: ValueType
    /// The flags.
    public var flags: Flags
    /// The lowest and the highest number, if `flags` has `.range`.
    public var range: ClosedRange<Double>?
    /// The step of the range, or 0 for any number.
    public var step: Double
    /// The default value. Settings and arguments only.
    public var defaultValue: Value
    /// What the number 0 means when it is not a quantity ("Automatic"). With a meaning, 0 is valid outside the range.
    public var zeroMeans: String
    /// The options of a value of type `.option`, by index.
    public var options: [Option]
    /// The values of a number setting that people normally use, for the present option of the setting that they depend
    /// on (the filter widths of a demodulator). Empty for none. Settings only.
    public var commonValues: [Double] = []
    /// For a column that holds row keys of another table: the key of the object of that table, or empty for the same
    /// object.
    public var refersObject: String
    /// The key of that table.
    public var refersTable: String
    /// What the bytes of the value hold.
    public var content: Content
    /// For media content: the four-character code of the codec.
    public var fourCC: String

    /// Returns the index of the option with the key `key`.
    public func optionIndex(key: String) -> UInt32? {
        options.firstIndex { $0.key == key }.map(UInt32.init)
    }

    /// Returns the option that `value` selects, if `value` is an option index of this declaration.
    public func option(of value: Value) -> Option? {
        guard case .option(let i) = value, Int(i) < options.count else { return nil }
        return options[Int(i)]
    }

    /// Copies the declaration `d` without its options. The views of `d` must still be valid.
    init(id: UInt32, _ d: sdrkit_decl) {
        self.id = id
        key = copy(d.key)
        label = copy(d.label)
        help = copy(d.help)
        section = copy(d.section)
        unit = copy(d.unit)
        type = ValueType(raw: d.type)
        flags = Flags(rawValue: d.flags)
        range = flags.contains(.range) && d.min <= d.max ? d.min...d.max : nil
        step = d.step
        defaultValue = Value(d.default_value)
        zeroMeans = copy(d.zero_means)
        options = []
        refersObject = copy(d.refers_object)
        refersTable = copy(d.refers_table)
        content = Content(rawValue: d.content) ?? .none
        fourCC = fourCCString(d.fourcc)
    }
}


/// The declaration of one command.
public struct Command: Equatable, Sendable {
    /// The id of the command.
    public var id: UInt32
    /// The key.
    public var key: String
    /// The name for people.
    public var label: String
    /// One or two sentences for people.
    public var help: String
    /// The arguments, by id.
    public var arguments: [Declaration]
    /// The reply fields, by id. Most commands have none.
    public var replyFields: [Declaration]
    /// What the command sends on the air, one of `SDRKIT_TX_CONTENT_*` (`SDRKIT_TX_CONTENT_CARRIER` for an unmodulated
    /// carrier), or `nil` if it does not transmit. The engine refuses a command that transmits with
    /// `SDRKIT_E_TRANSMIT_OFF` while the transmitter of the session is off.
    public var transmits: Int32? = nil
    /// The key of the command that stops the transmission that this command starts, or `nil` for a transmission that
    /// ends by itself and for a command that does not transmit.
    public var stoppedBy: String? = nil
}

/// The declaration of one table.
public struct TableDeclaration: Equatable, Sendable {
    /// The id of the table.
    public var id: UInt32
    /// The key.
    public var key: String
    /// The name for people.
    public var label: String
    /// The columns, by id.
    public var columns: [Declaration]
    /// The most rows that the table keeps. When the table is full, a new row removes the oldest row.
    public var cap: Int
}

/// The declaration of one series (a small plot) or raster (an image of values, such as a waterfall).
public struct PlotDeclaration: Equatable, Sendable {
    /// The id of the series or raster.
    public var id: UInt32
    /// The key.
    public var key: String
    /// The name for people.
    public var label: String
    /// The unit of the x axis.
    public var xUnit: String
    /// The unit of the y axis.
    public var yUnit: String
    /// The unit of the values of a raster. Empty for a series.
    public var unit: String
}

/// The declaration of the results of an object.
public struct ResultsDeclaration: Equatable, Sendable {
    /// The result fields, by id.
    public var fields: [Declaration] = []
    /// The tables, by id.
    public var tables: [TableDeclaration] = []
    /// The series, by id.
    public var series: [PlotDeclaration] = []
    /// The rasters, by id.
    public var rasters: [PlotDeclaration] = []
    /// The label of the map points. Empty if the object has none.
    public var pointsLabel: String = ""
}

/// The declaration of one event type of the catalog of the engine.
public struct EventType: Equatable, Sendable {
    /// The id of the event type.
    public var id: UInt32
    /// The key, such as `mesh.text`.
    public var key: String
    /// The name for people.
    public var label: String
    /// The fields, by id.
    public var fields: [Declaration]
}

/// The type of an object.
public enum ObjectKind: Int32, Sendable {
    case engine = 0
    case radio = 1
    case session = 2
    case stack = 3
    case component = 4
    case stackType = 5
    case driverType = 6
    case componentType = 7
    case service = 8
}

/// The type of a stream.
public enum StreamType: Int32, Sendable {
    case none = -1
    case iq = 0
    case real = 1
    case audio = 2
    case symbols = 3
    case hardBits = 4
    case softBits = 5
    case bytes = 6
    case messages = 7
    case spectrum = 8
    case item = 9
}
