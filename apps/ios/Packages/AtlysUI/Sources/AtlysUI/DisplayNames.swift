import Foundation

/// Human-readable names for record types and source identifiers.
public enum DisplayNames {
    static let typeNames: [String: String] = [
        "health.quantity": "Health measurements",
        "health.category": "Health events",
        "health.workout": "Workouts",
        "health.workout_route": "Workout routes",
        "health.electrocardiogram": "ECGs",
        "health.heartbeat_series": "Heartbeat series",
        "health.audiogram": "Hearing tests",
        "health.state_of_mind": "State of Mind",
        "health.clinical_record": "Health records",
        "health.characteristic": "Health profile",
        "health.sample": "Other health data",
        "location.point": "Locations",
        "location.visit": "Visits",
        "device.motion_activity": "Motion activity",
        "device.pedometer": "Pedometer",
        "device.battery": "Battery",
        "device.connectivity": "Network",
        "device.info": "Device info",
    ]

    static let prefixes = [
        "HKQuantityTypeIdentifier", "HKCategoryTypeIdentifier", "HKCharacteristicTypeIdentifier",
        "HKClinicalTypeIdentifier", "HKDataTypeIdentifier", "HKWorkoutTypeIdentifier",
    ]

    /// `health.workout` → "Workouts"; unknown types are shown as-is.
    public static func type(_ recordType: String) -> String {
        typeNames[recordType] ?? recordType
    }

    /// `HKQuantityTypeIdentifierStepCount` → "Step Count".
    public static func subtype(_ identifier: String) -> String {
        var name = identifier
        for prefix in prefixes where name.hasPrefix(prefix) {
            name.removeFirst(prefix.count)
            break
        }
        var out = ""
        let chars = Array(name)
        for (i, c) in chars.enumerated() {
            let prev = i > 0 ? chars[i - 1] : nil
            let next = i + 1 < chars.count ? chars[i + 1] : nil
            if let prev, c.isUppercase || (c.isNumber && !prev.isNumber),
               prev.isLowercase || (prev.isUppercase && (next?.isLowercase ?? false)) || (prev.isNumber && !c.isNumber) {
                out.append(" ")
            }
            out.append(c)
        }
        return out.isEmpty ? identifier : out
    }

    /// The family a record type belongs to: `health`, `location`, or `device`.
    public static func family(_ recordType: String) -> String {
        switch recordType.split(separator: ".").first {
        case "health": "Apple Health"
        case "location": "Location"
        case "device": "Device and motion"
        default: "Other"
        }
    }
}

extension Date {
    init(millis: Int64) {
        self.init(timeIntervalSince1970: Double(millis) / 1000)
    }
}
