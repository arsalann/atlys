import Foundation

/// A JSON value for record payloads. Building records from this type (rather
/// than `JSONSerialization`) keeps them `Sendable` and turns non-finite
/// numbers into absent fields instead of exceptions.
public enum JSONValue: Sendable, Equatable, Encodable {
    case null
    case bool(Bool)
    case int(Int64)
    case double(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    /// A number, or `nil` if it is NaN or infinite.
    public static func number(_ value: Double) -> JSONValue? {
        value.isFinite ? .double(value) : nil
    }

    public static func int(_ value: Int) -> JSONValue { .int(Int64(value)) }

    /// An object from optional fields; `nil` fields are left out.
    public static func object(_ fields: [String: JSONValue?]) -> JSONValue {
        .object(fields.compactMapValues { $0 })
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case .bool(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .string(let v): try container.encode(v)
        case .array(let v): try container.encode(v)
        case .object(let v): try container.encode(v)
        }
    }

    /// Compact JSON text with sorted keys.
    public func encoded() -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        // Every case encodes; non-finite doubles are excluded by construction.
        guard let data = try? encoder.encode(self) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    /// Converts a property-list-style value (HealthKit metadata, for example)
    /// into JSON. Dates become epoch milliseconds; unknown values become strings.
    public static func from(any value: Any) -> JSONValue? {
        switch value {
        case let v as Bool: return .bool(v)
        case let v as Int: return .int(v)
        case let v as Int64: return .int(v)
        case let v as Double: return .number(v)
        case let v as Float: return .number(Double(v))
        case let v as String: return .string(v)
        case let v as Date: return .int(v.millis)
        case let v as [Any]: return .array(v.compactMap { from(any: $0) })
        case let v as [String: Any]: return .object(v.compactMapValues { from(any: $0) })
        case let v as NSNumber: return .number(v.doubleValue)
        default: return .string(String(describing: value))
        }
    }
}

extension Date {
    /// Unix epoch milliseconds.
    public var millis: Int64 { Int64((timeIntervalSince1970 * 1000).rounded()) }

    public init(millis: Int64) {
        self.init(timeIntervalSince1970: Double(millis) / 1000)
    }
}
