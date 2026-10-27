//! The v0 record types (RFC-0001). This table is the single source of truth:
//! the JSON Schemas in `spec/adm/v0/types/` are generated from it and a test
//! keeps them in sync.

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
    /// `true` or `false`.
    Bool,
    /// One of the listed strings.
    Enum(&'static [&'static str]),
    /// A non-empty string of at most `max_len` Unicode scalar values.
    Text { max_len: usize },
    /// A lowercase identifier (`[a-z][a-z0-9_]*`) of at most `max_len` characters.
    Ident { max_len: usize },
}

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

const fn positive(max: f64) -> FieldKind {
    FieldKind::Num {
        min: 0.0,
        max,
        min_exclusive: true,
    }
}

const TEXT: FieldKind = FieldKind::Text { max_len: 128 };
const BPM: Field = req("bpm", positive(300.0), "Beats per minute.");
const LATITUDE: Field = req("latitude", num(-90.0, 90.0), "WGS84 latitude in degrees.");
const LONGITUDE: Field = req(
    "longitude",
    num(-180.0, 180.0),
    "WGS84 longitude in degrees.",
);
const HORIZONTAL_ACCURACY: Field = req(
    "horizontal_accuracy_m",
    num(0.0, 1.0e6),
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
                FieldKind::Int {
                    min: 0,
                    max: i64::MAX,
                },
                "Free storage available to the app, in bytes.",
            ),
            opt("locale", TEXT, "Current locale identifier."),
            opt("time_zone", TEXT, "Current IANA time zone."),
        ],
    },
    RecordType {
        name: "device.motion_activity",
        description: "A segment of one detected motion activity.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::Medium,
        fields: &[
            req(
                "activity",
                FieldKind::Enum(&[
                    "stationary",
                    "walking",
                    "running",
                    "cycling",
                    "automotive",
                    "unknown",
                ]),
                "Detected activity.",
            ),
            req(
                "confidence",
                FieldKind::Enum(&["low", "medium", "high"]),
                "Detection confidence.",
            ),
        ],
    },
    RecordType {
        name: "health.body_mass",
        description: "A body mass measurement.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::High,
        fields: &[req("kg", positive(1000.0), "Body mass in kilograms.")],
    },
    RecordType {
        name: "health.distance",
        description: "Distance covered during the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req("meters", num(0.0, 1.0e7), "Distance in meters."),
            req(
                "activity",
                FieldKind::Enum(&[
                    "walking_running",
                    "cycling",
                    "swimming",
                    "wheelchair",
                    "downhill_snow_sports",
                ]),
                "How the distance was covered.",
            ),
        ],
    },
    RecordType {
        name: "health.energy",
        description: "Energy burned during the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req("kcal", num(0.0, 1.0e5), "Energy in kilocalories."),
            req(
                "kind",
                FieldKind::Enum(&["active", "basal"]),
                "Active or basal (resting) energy.",
            ),
        ],
    },
    RecordType {
        name: "health.exercise_time",
        description: "Exercise time during the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[req("seconds", num(0.0, 1.0e7), "Exercise time in seconds.")],
    },
    RecordType {
        name: "health.floors",
        description: "Floors climbed during the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[req(
            "count",
            FieldKind::Int {
                min: 0,
                max: 100_000,
            },
            "Floors climbed.",
        )],
    },
    RecordType {
        name: "health.heart_rate",
        description: "A heart rate sample.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[BPM],
    },
    RecordType {
        name: "health.height",
        description: "A height measurement.",
        time: TimeShape::Instant,
        sensitivity: Sensitivity::High,
        fields: &[req("m", positive(3.0), "Height in meters.")],
    },
    RecordType {
        name: "health.hrv",
        description: "Heart rate variability (SDNN).",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[req(
            "sdnn_ms",
            num(0.0, 5000.0),
            "Standard deviation of NN intervals in milliseconds.",
        )],
    },
    RecordType {
        name: "health.mindful_session",
        description: "A mindfulness session.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[],
    },
    RecordType {
        name: "health.resting_heart_rate",
        description: "Resting heart rate, usually one value per day.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[BPM],
    },
    RecordType {
        name: "health.sleep_stage",
        description: "A segment of sleep analysis.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[req(
            "stage",
            FieldKind::Enum(&[
                "in_bed",
                "asleep_unspecified",
                "asleep_core",
                "asleep_deep",
                "asleep_rem",
                "awake",
            ]),
            "Sleep stage.",
        )],
    },
    RecordType {
        name: "health.stand_hour",
        description: "Whether the user stood during an hour.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[req(
            "stood",
            FieldKind::Bool,
            "Stood for at least a minute during the hour.",
        )],
    },
    RecordType {
        name: "health.steps",
        description: "Steps taken during the interval.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[req(
            "count",
            FieldKind::Int {
                min: 0,
                max: 1_000_000,
            },
            "Number of steps.",
        )],
    },
    RecordType {
        name: "health.walking_heart_rate",
        description: "Walking heart rate average, usually one value per day.",
        time: TimeShape::Either,
        sensitivity: Sensitivity::High,
        fields: &[BPM],
    },
    RecordType {
        name: "health.workout",
        description: "A workout.",
        time: TimeShape::Interval,
        sensitivity: Sensitivity::High,
        fields: &[
            req(
                "activity",
                FieldKind::Ident { max_len: 64 },
                "Activity type in snake_case, for example running or traditional_strength_training.",
            ),
            req(
                "duration_s",
                num(0.0, 1.0e7),
                "Active duration in seconds, excluding pauses.",
            ),
            opt("kcal", num(0.0, 1.0e5), "Active energy in kilocalories."),
            opt("meters", num(0.0, 1.0e7), "Distance in meters."),
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
                num(-1.0e4, 1.0e5),
                "Altitude above sea level in meters.",
            ),
            opt(
                "vertical_accuracy_m",
                num(0.0, 1.0e6),
                "Altitude uncertainty in meters.",
            ),
            opt("speed_mps", num(0.0, 1000.0), "Speed in meters per second."),
            opt(
                "course_deg",
                num(0.0, 360.0),
                "Direction of travel in degrees from true north.",
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
