import Foundation
import SDRKitC

/// One option of a value of type `.option`, or one flag of a value of type `.flags`.
public struct Option: Equatable, Sendable {
    /// The key that data files use.
    public var key: String
    /// The name for people.
    public var label: String
    /// What a flag says, in one line. Empty for an option.
    public var help: String = ""
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
    /// What the special number (`specialValue`, 0 unless declared) means when it is not a quantity ("Automatic"). With a
    /// meaning, the special number is valid outside the range.
    public var zeroMeans: String
    /// The number that `zeroMeans` names: 0, or another (-1 for a gain stage that follows the gain).
    public var specialValue: Double
    /// The options of a value of type `.option`, by index, or the flags of a value of type `.flags`, by bit.
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
    /// What the setting chooses, if it states it. A front end finds the setting that chooses the view of the picture
    /// by this member and not by the key. Settings only.
    public var role: SettingRole? = nil
    /// With `refersTable`: the key of the column of that table that holds the value to pass, or empty. Empty: the
    /// value is a row key of the table. With a column, a front end offers the rows of the table and passes the cell
    /// of this column of the chosen row; other values stay valid (the addressee of a send command). Arguments and
    /// settings.
    public var refersColumn: String = ""
    /// The place of the field among the key readings of its object, from 1: what a person looks at first. `nil` for
    /// a field that is no key reading. Result fields only.
    public var reading: Int? = nil

    /// What a setting chooses.
    public enum SettingRole: Int32, Sendable {
        /// The setting chooses whose picture the object shows: an instrument, a product.
        case pictureSource = 1
        /// The setting chooses the view of the picture shown: a channel, a composite.
        case pictureView = 2
    }

    /// Returns the index of the option with the key `key`.
    public func optionIndex(key: String) -> UInt32? {
        options.firstIndex { $0.key == key }.map(UInt32.init)
    }

    /// Returns the option that `value` selects, if `value` is an option index of this declaration.
    public func option(of value: Value) -> Option? {
        guard case .option(let i) = value, Int(i) < options.count else { return nil }
        return options[Int(i)]
    }

    /// Returns the flags that are set in `value`, in bit order, if `value` is a flag set of this declaration.
    public func flags(of value: Value) -> [Option]? {
        guard case .flags(let mask) = value else { return nil }
        return options.enumerated().filter { $0.offset < 64 && mask >> UInt64($0.offset) & 1 == 1 }.map(\.element)
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
        specialValue = d.special_value
        options = []
        refersObject = copy(d.refers_object)
        refersTable = copy(d.refers_table)
        content = Content(rawValue: d.content) ?? .none
        fourCC = fourCCString(d.fourcc)
        role = SettingRole(rawValue: d.role)
        refersColumn = copy(d.refers_column)
        reading = d.reading == 0 ? nil : Int(d.reading)
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
    /// The id of the column that names a row for people (a call sign, a station name), if the table states one.
    public var labelColumn: UInt32? = nil
    /// The default order of the rows, if the table states one. Without it, the newest row is first. It is a hint for
    /// a front end: the rows of a snapshot are always newest first.
    public var sort: TableSort? = nil
    /// The columns that hold the place of a row on the map.
    public var place: TablePlace = TablePlace()
    /// What the rows are, if the table states it. A front end finds the message log, the call log, the nodes and the
    /// groups of an object by this member and not by the key of the table.
    public var role: TableRole? = nil
    /// The columns of a message or a call.
    public var message: TableMessage = TableMessage()
}

/// What the rows of a table are.
public enum TableRole: Int32, Sendable {
    /// Each row is a message: the main log of messages of the object.
    case messages = 1
    /// Each row is a call or a transmission: who, to whom, when.
    case calls = 2
    /// Each row is a station, a node or a unit that was heard.
    case nodes = 3
    /// Each row is a group that stations talk in: a talkgroup, a channel, a net.
    case groups = 4
    /// Each row is a frame, a packet or a report that arrived: a running log. An object can have several.
    case log = 5
}

/// The columns of a table that hold the parts of a message or a call, each by its id. A column that the table does
/// not have is `nil`.
public struct TableMessage: Equatable, Sendable {
    /// The column of the sender: a call sign, an address, a node, a unit.
    public var sender: UInt32? = nil
    /// The column of the addressee: a call sign, an address, a group, a pager.
    public var addressee: UInt32? = nil
    /// The column of the text for people.
    public var text: UInt32? = nil
    /// The column that holds the time of the row: a column of the time kind, or a number of seconds of signal with
    /// the unit `s`.
    public var time: UInt32? = nil
    /// The bool column that is true for a message that this station sent. Without it, in a table with a `delivery`
    /// column, a row with a delivery state is an own message.
    public var own: UInt32? = nil
    /// The column of the delivery state of an own message: an enum that has no value in the row of a received
    /// message.
    public var delivery: UInt32? = nil
    /// The column of the reason of a failed delivery, as text for people.
    public var failure: UInt32? = nil

    /// The columns by their ids; a column left out is `nil`. `TableMessage()` is a table with no message columns.
    public init(sender: UInt32? = nil, addressee: UInt32? = nil, text: UInt32? = nil, time: UInt32? = nil,
                own: UInt32? = nil, delivery: UInt32? = nil, failure: UInt32? = nil) {
        self.sender = sender
        self.addressee = addressee
        self.text = text
        self.time = time
        self.own = own
        self.delivery = delivery
        self.failure = failure
    }
}

/// The default order of the rows of a table.
public struct TableSort: Equatable, Sendable {
    /// The id of the column that orders the rows.
    public var column: UInt32
    /// True for the highest value first, false for the lowest value first.
    public var descending: Bool
}

/// The columns of a table that hold the place of a row on the map, each by its id. A column that the table does not
/// have is `nil`.
public struct TablePlace: Equatable, Sendable {
    /// The column of the position kind.
    public var position: UInt32? = nil
    /// The column of the latitude in degrees north, in a table without a position column.
    public var latitude: UInt32? = nil
    /// The column of the longitude in degrees east, in a table without a position column.
    public var longitude: UInt32? = nil
    /// The column of the height. The declaration of the column has the unit.
    public var altitude: UInt32? = nil
    /// The column of the direction of travel in degrees from north.
    public var course: UInt32? = nil
    /// The column of the speed. The declaration of the column has the unit.
    public var speed: UInt32? = nil

    /// True if the table has a position, or a latitude and a longitude.
    public var onMap: Bool { position != nil || (latitude != nil && longitude != nil) }
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
    case share = 11
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
