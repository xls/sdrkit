import Foundation
import SDRKitC

/// What an object does with its signal.
public enum State: Int32, Sendable {
    /// The object does not run.
    case off = 0
    /// The object looks for a signal.
    case searching = 1
    /// The object has a signal and does not decode it yet.
    case tracking = 2
    /// The object decodes the signal.
    case locked = 3
}

/// The status of an object.
public struct Status: Equatable, Sendable {
    /// The state.
    public var state: State = .off
    /// The signal to noise ratio in dB, if known.
    public var snrDb: Float?
    /// The level in dB full scale, if known.
    public var levelDbfs: Float?
    /// The offset from the tuned frequency in Hz, if known.
    public var offsetHz: Double?
    /// One line for people.
    public var summary: String = ""
}

/// One value with its declaration: a result field, a reply field or an event field.
public struct Field: Equatable, Sendable {
    /// The declaration.
    public var declaration: Declaration
    /// The value.
    public var value: Value

    /// The key of the declaration.
    public var key: String { declaration.key }
}

/// One row of a table.
public struct Row: Equatable, Sendable {
    /// The row key.
    public var key: String
    /// The cells, by column id.
    public var cells: [Value]
}

/// One table with its rows.
public struct Table: Equatable, Sendable {
    /// The declaration.
    public var declaration: TableDeclaration
    /// The rows, newest first.
    public var rows: [Row] = []

    /// The key of the declaration.
    public var key: String { declaration.key }

    /// Returns the row with the key `key`.
    public func row(_ key: String) -> Row? {
        rows.first { $0.key == key }
    }

    /// Returns the id of the column with the key `key`.
    public func column(_ key: String) -> Int? {
        declaration.columns.firstIndex { $0.key == key }
    }

    /// Returns the cell of the column with the key `column` in `row`.
    public func value(_ row: Row, _ column: String) -> Value? {
        guard let c = self.column(column), c < row.cells.count else { return nil }
        return row.cells[c]
    }
}

/// A point on a map.
public struct MapPoint: Equatable, Sendable {
    /// The id of the point.
    public var id: String
    /// Degrees north.
    public var latitude: Double
    /// Degrees east.
    public var longitude: Double
    /// The name for people.
    public var label: String
    /// Degrees from north for a moving target, if known.
    public var heading: Float?
}

/// The results of an object at one moment.
public struct Snapshot: Equatable, Sendable {
    /// The change counter of the results.
    public var counter: UInt64 = 0
    /// The status.
    public var status = Status()
    /// The result fields, by id.
    public var fields: [Field] = []
    /// The tables, by id.
    public var tables: [Table] = []
    /// The log, oldest line first.
    public var log: [String] = []
    /// The map points.
    public var points: [MapPoint] = []

    /// Returns the value of the result field with the key `key`.
    public func value(of key: String) -> Value? {
        fields.first { $0.key == key }?.value
    }

    /// Returns the table with the key `key`.
    public func table(_ key: String) -> Table? {
        tables.first { $0.key == key }
    }
}

/// The points of a series.
public struct Series: Equatable, Sendable {
    /// The change counter of the series.
    public var counter: UInt64
    /// The points, each x then y.
    public var points: [Float]
    /// True if the points make a line, false if they are separate points.
    public var line: Bool
}

/// The values of a raster.
public struct Raster: Equatable, Sendable {
    /// The change counter of the raster.
    public var counter: UInt64
    /// `width` times `height` values, row by row.
    public var values: [Float]
    /// The number of cells in a row.
    public var width: Int
    /// The number of rows.
    public var height: Int
    /// The x value at the centre of the first cell of a row.
    public var xStart: Double
    /// The change of x from one cell to the next.
    public var xStep: Double
    /// The y value at the centre of the first row.
    public var yStart: Double
    /// The change of y from one row to the next.
    public var yStep: Double
}

/// One event of an object.
public struct Event: Equatable, Sendable {
    /// The key of the event type, such as `mesh.text`.
    public var type: String
    /// The name of the event type for people.
    public var label: String
    /// Seconds since the object started, in the time of the signal.
    public var time: Double
    /// The frequency of the signal in Hz.
    public var frequencyHz: Double
    /// The fields, by id.
    public var fields: [Field]

    /// Returns the value of the field with the key `key`.
    public func value(of key: String) -> Value? {
        fields.first { $0.key == key }?.value
    }
}
