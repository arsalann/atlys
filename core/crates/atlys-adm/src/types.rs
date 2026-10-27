//! The v0 record types (RFC-0001). This table is the single source of truth:
//! the JSON Schemas in `spec/adm/v0/types/` are generated from it and a test
//! keeps them in sync.
//!
//! Records keep source data raw: values stay in the unit the source reported
//! (the unit is stored next to the value where the source has one), and
//! source identifiers are kept as-is. Apple Health samples use generic
//! families (`health.quantity`, `health.category`, ...) keyed by their
//! HealthKit identifier, so every readable type is covered, including types
//! added in future iOS releases.

/// Whether a record type is a point in time, an interval, or either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeShape {
    /// `end_ms` is not allowed.
    Instant,
    /// `end_ms` is required.
    Interval,
    /// `end_ms` is optional.
    Either,
}

/// Default sensitivity tier of a record type (see plan/05-data-model.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sensitivity {
    /// Low: device telemetry.
    Low,
    /// Medium: activity patterns.
    Medium,
    /// High: health and precise location.
    High,
}

impl Sensitivity {
    /// The name used in record JSON.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// The JSON value a `data` field holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldKind {
    /// A JSON integer (no fraction or exponent) in `min..=max`.
    Int { min: i64, max: i64 },
    /// A JSON number in `min..=max`, or `min < x <= max` if `min_exclusive`.
    Num {
        min: f64,
        max: f64,
        min_exclusive: bool,
    },
    /// Any JSON number, as the source reported it.
    AnyNum,
    /// `true` or `false`.
    Bool,
    /// One of the listed strings.
    Enum(&'static [&'static str]),
    /// A non-empty string of at most `max_len` Unicode scalar values.
    Text { max_len: usize },
    /// A source identifier such as `HKQuantityTypeIdentifierStepCount`:
    /// `[A-Za-z][A-Za-z0-9_.]*`, at most 128 characters.
    Name,
    /// A JSON object with source-defined keys (for example HealthKit metadata).
    Object,
    /// Any JSON value, kept as the source structured it.
    Any,
}

/// Pattern of [`FieldKind::Name`] values.
pub const NAME_PATTERN: &str = "^[A-Za-z][A-Za-z0-9_.]*$";
/// Longest [`FieldKind::Name`] value.
pub const NAME_MAX: usize = 128;

/// One field of a record type's `data` object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    pub name: &'static str,
    pub kind: FieldKind,
    pub required: bool,
    pub description: &'static str,
}

/// A record type: its name, time shape, sensitivity, and `data` fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecordType {
    pub name: &'static str,
    pub description: &'static str,
    pub time: TimeShape,
    pub sensitivity: Sensitivity,
    pub fields: &'static [Field],
}

const fn req(name: &'static str, kind: FieldKind, description: &'static str) -> Field {
    Field {
        name,
        kind,
        required: true,
        description,
    }
}

const fn opt(name: &'static str, kind: FieldKind, description: &'static str) -> Field {
    Field {
        name,
        kind,
        required: false,
        description,
    }
}

const fn num(min: f64, max: f64) -> FieldKind {
    FieldKind::Num {
        min,
        max,
        min_exclusive: false,
    }
}

const TEXT: FieldKind = FieldKind::Text { max_len: 256 };
const NON_NEGATIVE: FieldKind = num(0.0, f64::INFINITY);
const COUNT: FieldKind = FieldKind::Int {
    min: 0,
    max: i64::MAX,
};

const IDENTIFIER: Field = req(
    "identifier",
    FieldKind::Name,
    "HealthKit type identifier, as-is.",
);
const METADATA: Field = opt(
    "metadata",
    FieldKind::Object,
    "HealthKit metadata, as-is. Dates become epoch milliseconds and quantities `{value, unit}`.",
);
const LATITUDE: Field = req("latitude", num(-90.0, 90.0), "WGS84 latitude in degrees.");
const LONGITUDE: Field = req(
    "longitude",
    num(-180.0, 180.0),
    "WGS84 longitude in degrees.",
);
const HORIZONTAL_ACCURACY: Field = req(
    "horizontal_accuracy_m",
    NON_NEGATIVE,
    "Radius of uncertainty in meters.",
);

/// Every v0 record type, sorted by name.
pub static TYPES: &[RecordType] = &[
    RecordType {
        name: "device.battery",
        description: "A battery snapshot.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::Low,
        fields: &[
            opt(
                "level",
                num(0.0, 1.0),
                "Charge level from 0 to 1. Omitted when unknown.",
            ),
            req(
                "state",
                FieldKind::Enum(&["unknown", "unplugged", "charging", "full"]),
                "Charging state.",
            ),
            req(
                "low_power_mode",
                FieldKind::Bool,
                "Whether Low Power Mode is on.",
            ),
        ],
    },
    RecordType {
        name: "device.connectivity",
        description: "A network path snapshot. No network requests are made to take it.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::Low,
        fields: &[
            req(
                "status",
                FieldKind::Enum(&["satisfied", "unsatisfied", "requires_connection"]),
                "Whether the path can carry traffic.",
            ),
            req(
                "interface",
                FieldKind::Enum(&["wifi", "cellular", "wired", "loopback", "other", "none"]),
                "Interface type of the path.",
            ),
            req(
                "expensive",
                FieldKind::Bool,
                "The path is expensive (for example cellular or a hotspot).",
            ),
            req(
                "constrained",
                FieldKind::Bool,
                "The path is constrained (Low Data Mode).",
            ),
            opt("supports_ipv4", FieldKind::Bool, "The path can route IPv4."),
            opt("supports_ipv6", FieldKind::Bool, "The path can route IPv6."),
            opt(
                "supports_dns",
                FieldKind::Bool,
                "The path has a DNS server.",
            ),
        ],
    },
    RecordType {
        name: "device.info",
        description: "A device and app snapshot.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::Low,
        fields: &[
            req("os_name", TEXT, "Operating system name, for example iOS."),
            req("os_version", TEXT, "Operating system version."),
            req("model", TEXT, "Hardware model identifier."),
            req("app_version", TEXT, "Atlys app version and build."),
            opt(
                "free_storage_bytes",
                COUNT,
                "Free storage available to the app, in bytes.",
            ),
            opt(
                "total_storage_bytes",
                COUNT,
                "Total storage capacity, in bytes.",
            ),
            opt("locale", TEXT, "Current locale identifier."),
            opt("time_zone", TEXT, "Current IANA time zone."),
            opt(
                "thermal_state",
                FieldKind::Enum(&["nominal", "fair", "serious", "critical"]),
                "System thermal state.",
            ),
            opt("uptime_s", NON_NEGATIVE, "Seconds since the device booted."),
            opt(
                "physical_memory_bytes",
                COUNT,
                "Installed memory, in bytes.",
            ),
        ],
    },
    RecordType {
        name: "device.motion_activity",
        description: "A motion activity update from Core Motion. It holds until the next update. Several flags can be true at once.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::Medium,
        fields: &[
            req("stationary", FieldKind::Bool, "The device is not moving."),
            req("walking", FieldKind::Bool, "The user is walking."),
            req("running", FieldKind::Bool, "The user is running."),
            req("cycling", FieldKind::Bool, "The user is cycling."),
            req("automotive", FieldKind::Bool, "The device is in a vehicle."),
            req("unknown", FieldKind::Bool, "The activity is unknown."),
            req(
                "confidence",
                FieldKind::Int { min: 0, max: 2 },
                "Core Motion confidence: 0 low, 1 medium, 2 high.",
            ),
        ],
    },
    RecordType {
        name: "device.pedometer",
        description: "Pedometer totals from Core Motion for the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::Medium,
        fields: &[
            req("steps", COUNT, "Steps taken."),
            opt("distance_m", NON_NEGATIVE, "Estimated distance in meters."),
            opt("floors_ascended", COUNT, "Floors ascended."),
            opt("floors_descended", COUNT, "Floors descended."),
            opt(
                "average_active_pace_s_per_m",
                NON_NEGATIVE,
                "Average pace while moving, in seconds per meter.",
            ),
        ],
    },
    RecordType {
        name: "health.audiogram",
        description: "A hearing test result.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "sensitivity_points",
                FieldKind::Any,
                "Test points as HealthKit reports them.",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.category",
        description: "A HealthKit category sample (for example sleep analysis, stand hour, or a symptom).",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            IDENTIFIER,
            req(
                "value",
                FieldKind::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "Raw HealthKit category value.",
            ),
            opt(
                "value_name",
                TEXT,
                "Name of the value, when Atlys knows it (for example asleepCore).",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.characteristic",
        description: "A HealthKit characteristic (for example date of birth or blood type), as read at the record time.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::High,
        fields: &[
            IDENTIFIER,
            req(
                "value",
                FieldKind::Any,
                "Raw value: an integer code, or a date as {year, month, day}.",
            ),
            opt(
                "value_name",
                TEXT,
                "Name of the value, when Atlys knows it.",
            ),
        ],
    },
    RecordType {
        name: "health.clinical_record",
        description: "A health record from a connected provider, with its FHIR resource.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            IDENTIFIER,
            req("display_name", TEXT, "Name shown in Apple Health."),
            opt("fhir_version", TEXT, "FHIR release of the resource."),
            opt(
                "fhir_resource_type",
                TEXT,
                "FHIR resource type, for example Observation.",
            ),
            opt(
                "fhir_resource",
                FieldKind::Any,
                "The FHIR resource JSON, as-is.",
            ),
        ],
    },
    RecordType {
        name: "health.electrocardiogram",
        description: "An ECG recording with its voltage samples.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "classification",
                FieldKind::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "Raw HealthKit classification.",
            ),
            opt("classification_name", TEXT, "Name of the classification."),
            opt(
                "symptoms_status",
                FieldKind::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "Raw HealthKit symptoms status.",
            ),
            opt(
                "average_heart_rate_bpm",
                FieldKind::AnyNum,
                "Average heart rate in count/min.",
            ),
            opt(
                "sampling_frequency_hz",
                FieldKind::AnyNum,
                "Sampling frequency in hertz.",
            ),
            opt(
                "voltages",
                FieldKind::Any,
                "Samples as [seconds since start, microvolts] for lead I.",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.heartbeat_series",
        description: "Beat-to-beat timings recorded by Apple Watch.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "beats",
                FieldKind::Any,
                "Beats as [seconds since start, preceded by a gap].",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.quantity",
        description: "A HealthKit quantity sample (for example step count or heart rate), in the unit Apple Health uses for it.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            IDENTIFIER,
            req("value", FieldKind::AnyNum, "The value in `unit`."),
            req(
                "unit",
                FieldKind::Text { max_len: 64 },
                "HealthKit unit string, for example count, count/min, or km.",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.sample",
        description: "Any other HealthKit sample type, with the details Atlys can read.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            IDENTIFIER,
            opt("details", FieldKind::Any, "Type-specific details."),
            METADATA,
        ],
    },
    RecordType {
        name: "health.state_of_mind",
        description: "A logged emotion or mood.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "kind",
                FieldKind::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "Raw kind: momentary emotion or daily mood.",
            ),
            req(
                "valence",
                num(-1.0, 1.0),
                "From -1 (very unpleasant) to 1 (very pleasant).",
            ),
            opt("labels", FieldKind::Any, "Raw label codes."),
            opt("associations", FieldKind::Any, "Raw association codes."),
            METADATA,
        ],
    },
    RecordType {
        name: "health.workout",
        description: "A workout.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "activity_type",
                FieldKind::Int {
                    min: 0,
                    max: i64::MAX,
                },
                "Raw HKWorkoutActivityType value.",
            ),
            opt(
                "activity_name",
                TEXT,
                "Name of the activity type, when Atlys knows it.",
            ),
            req("duration_s", NON_NEGATIVE, "Duration in seconds."),
            opt(
                "statistics",
                FieldKind::Object,
                "Per quantity type: sum, average, minimum, maximum, and unit.",
            ),
            opt(
                "events",
                FieldKind::Any,
                "Workout events such as pauses, laps, and segments.",
            ),
            opt(
                "activities",
                FieldKind::Any,
                "Sub-activities of multisport workouts.",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "health.workout_route",
        description: "GPS points recorded during a workout.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "workout_id",
                TEXT,
                "Source ID of the workout this route belongs to.",
            ),
            req(
                "points",
                FieldKind::Any,
                "Locations, each with the location.point fields and t_ms.",
            ),
            METADATA,
        ],
    },
    RecordType {
        name: "location.point",
        description: "A location fix.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::High,
        fields: &[
            LATITUDE,
            LONGITUDE,
            HORIZONTAL_ACCURACY,
            opt(
                "altitude_m",
                FieldKind::AnyNum,
                "Altitude above sea level in meters.",
            ),
            opt(
                "ellipsoidal_altitude_m",
                FieldKind::AnyNum,
                "Altitude above the WGS84 ellipsoid in meters.",
            ),
            opt(
                "vertical_accuracy_m",
                NON_NEGATIVE,
                "Altitude uncertainty in meters.",
            ),
            opt("speed_mps", NON_NEGATIVE, "Speed in meters per second."),
            opt(
                "speed_accuracy_mps",
                NON_NEGATIVE,
                "Speed uncertainty in meters per second.",
            ),
            opt(
                "course_deg",
                num(0.0, 360.0),
                "Direction of travel in degrees from true north.",
            ),
            opt(
                "course_accuracy_deg",
                NON_NEGATIVE,
                "Course uncertainty in degrees.",
            ),
            opt(
                "floor",
                FieldKind::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "Building floor, when known.",
            ),
            opt(
                "simulated",
                FieldKind::Bool,
                "The location was simulated by software.",
            ),
            opt(
                "from_accessory",
                FieldKind::Bool,
                "The location came from an external accessory.",
            ),
            opt(
                "trigger",
                FieldKind::Enum(&["significant_change", "foreground", "detailed", "visit"]),
                "What delivered the fix.",
            ),
        ],
    },
    RecordType {
        name: "location.visit",
        description: "A stay at one place. `end_ms` is absent until the departure is known.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[LATITUDE, LONGITUDE, HORIZONTAL_ACCURACY],
    },
];

/// Looks up a record type by name.
pub fn lookup(name: &str) -> Option<&'static RecordType> {
    TYPES
        .binary_search_by(|t| t.name.cmp(name))
        .ok()
        .map(|i| &TYPES[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_unique_and_well_formed() {
        for pair in TYPES.windows(2) {
            assert!(
                pair[0].name < pair[1].name,
                "{} before {}",
                pair[0].name,
                pair[1].name
            );
        }
        for t in TYPES {
            assert_eq!(lookup(t.name), Some(t));
            let mut names: Vec<_> = t.fields.iter().map(|f| f.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), t.fields.len(), "{}", t.name);
        }
        assert_eq!(lookup("health.nope"), None);
    }
}
