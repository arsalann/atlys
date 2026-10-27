//! Record drafts (what collectors seal into the inbox), deletions, and stored
//! records (what the vault holds and shows).

use serde_json::{Map, Value, json};

use crate::ADM_VERSION;
use crate::error::{AdmError, ErrorKind};
use crate::time::{MAX_MS, rfc3339};
use crate::types::{self, FieldKind, RecordType, Sensitivity, TimeShape};

const SOURCE_ID_MAX: usize = 256;
const TZ_MAX: usize = 64;
const SOURCE_TEXT_MAX: usize = 256;

/// Where a record came from inside the device, for example the app and watch
/// that wrote a HealthKit sample. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    /// Display name of the app or service that produced the data.
    pub name: Option<String>,
    /// Bundle identifier of that app.
    pub bundle_id: Option<String>,
    /// Device that produced the data, for example `Apple Watch`.
    pub device: Option<String>,
}

impl Source {
    fn is_empty(&self) -> bool {
        self.name.is_none() && self.bundle_id.is_none() && self.device.is_none()
    }

    fn to_value(&self) -> Value {
        let mut out = Map::new();
        for (key, value) in self.entries() {
            if let Some(v) = value {
                out.insert(key.to_owned(), Value::String(v.clone()));
            }
        }
        Value::Object(out)
    }

    fn entries(&self) -> [(&'static str, &Option<String>); 3] {
        [
            ("name", &self.name),
            ("bundle_id", &self.bundle_id),
            ("device", &self.device),
        ]
    }

    fn from_value(value: &Value) -> Result<Self, AdmError> {
        let obj = object(value, "")?;
        let mut source = Self::default();
        for (key, v) in obj {
            let slot = match key.as_str() {
                "name" => &mut source.name,
                "bundle_id" => &mut source.bundle_id,
                "device" => &mut source.device,
                _ => return Err(AdmError::new(ErrorKind::UnknownField, key.as_str())),
            };
            *slot = Some(string(v, key)?.to_owned());
        }
        Ok(source)
    }

    fn validate(&self) -> Result<(), AdmError> {
        for (key, value) in self.entries() {
            if let Some(v) = value {
                check_text(v, SOURCE_TEXT_MAX).map_err(|kind| AdmError::new(kind, key))?;
            }
        }
        Ok(())
    }
}

/// A record produced by a collector, before it is merged into the vault.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDraft {
    /// Record type, for example `health.steps`.
    pub record_type: String,
    /// Stable identifier from the source (for example a HealthKit UUID).
    /// `(record_type, source_id)` identifies the record for dedupe.
    pub source_id: String,
    /// Start time, Unix epoch milliseconds.
    pub start_ms: i64,
    /// End time for interval records, Unix epoch milliseconds.
    pub end_ms: Option<i64>,
    /// IANA time zone where the record happened, if known.
    pub tz: Option<String>,
    /// Origin inside the device, if known.
    pub source: Option<Source>,
    /// Type-specific payload.
    pub data: Map<String, Value>,
}

impl RecordDraft {
    /// Parses and validates a draft from its JSON value.
    pub fn from_value(value: &Value) -> Result<Self, AdmError> {
        let obj = object(value, "")?;
        for key in obj.keys() {
            if !matches!(
                key.as_str(),
                "type" | "source_id" | "start_ms" | "end_ms" | "tz" | "source" | "data"
            ) {
                return Err(AdmError::new(ErrorKind::UnknownField, key.as_str()));
            }
        }
        let field = |name: &'static str| {
            obj.get(name)
                .ok_or_else(|| AdmError::new(ErrorKind::MissingField, name))
        };
        let draft = Self {
            record_type: string(field("type")?, "type")?.to_owned(),
            source_id: string(field("source_id")?, "source_id")?.to_owned(),
            start_ms: integer(field("start_ms")?, "start_ms")?,
            end_ms: obj
                .get("end_ms")
                .map(|v| integer(v, "end_ms"))
                .transpose()?,
            tz: obj
                .get("tz")
                .map(|v| string(v, "tz").map(str::to_owned))
                .transpose()?,
            source: obj
                .get("source")
                .map(|v| Source::from_value(v).map_err(|e| e.within("source")))
                .transpose()?,
            data: object(field("data")?, "data")?.clone(),
        };
        draft.validate()?;
        Ok(draft)
    }

    /// Validates the draft against RFC-0001 and its record type.
    pub fn validate(&self) -> Result<&'static RecordType, AdmError> {
        let spec = types::lookup(&self.record_type).ok_or_else(|| {
            AdmError::new(ErrorKind::UnknownType(self.record_type.clone()), "type")
        })?;
        check_source_id(&self.source_id).map_err(|kind| AdmError::new(kind, "source_id"))?;
        check_time(self.start_ms, "start_ms")?;
        match (spec.time, self.end_ms) {
            (TimeShape::Interval, None) => {
                return Err(AdmError::new(ErrorKind::MissingField, "end_ms"));
            }
            (TimeShape::Instant, Some(_)) => {
                return Err(AdmError::new(ErrorKind::UnknownField, "end_ms"));
            }
            (_, Some(end)) => {
                check_time(end, "end_ms")?;
                if end < self.start_ms {
                    return Err(AdmError::new(ErrorKind::InvalidTime, "end_ms"));
                }
            }
            (_, None) => {}
        }
        if let Some(tz) = &self.tz
            && !is_tz(tz)
        {
            return Err(AdmError::new(ErrorKind::InvalidValue, "tz"));
        }
        if let Some(source) = &self.source {
            source.validate().map_err(|e| e.within("source"))?;
        }
        validate_data(spec, &self.data).map_err(|e| e.within("data"))?;
        Ok(spec)
    }

    /// The draft's JSON value, as sealed into a batch.
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("type".into(), self.record_type.clone().into());
        out.insert("source_id".into(), self.source_id.clone().into());
        out.insert("start_ms".into(), self.start_ms.into());
        if let Some(end) = self.end_ms {
            out.insert("end_ms".into(), end.into());
        }
        if let Some(tz) = &self.tz {
            out.insert("tz".into(), tz.clone().into());
        }
        if let Some(source) = self.source.as_ref().filter(|s| !s.is_empty()) {
            out.insert("source".into(), source.to_value());
        }
        out.insert("data".into(), Value::Object(self.data.clone()));
        Value::Object(out)
    }
}

/// A request to delete a record, for example after a sample was deleted in
/// Apple Health. Applies to the record with the same type and source ID,
/// and keeps it from coming back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deletion {
    pub record_type: String,
    pub source_id: String,
}

impl Deletion {
    /// Parses and validates a deletion from its JSON value.
    pub fn from_value(value: &Value) -> Result<Self, AdmError> {
        let obj = object(value, "")?;
        for key in obj.keys() {
            if !matches!(key.as_str(), "type" | "source_id") {
                return Err(AdmError::new(ErrorKind::UnknownField, key.as_str()));
            }
        }
        let field = |name: &'static str| {
            obj.get(name)
                .ok_or_else(|| AdmError::new(ErrorKind::MissingField, name))
                .and_then(|v| string(v, name))
        };
        let deletion = Self {
            record_type: field("type")?.to_owned(),
            source_id: field("source_id")?.to_owned(),
        };
        deletion.validate()?;
        Ok(deletion)
    }

    /// Checks that the type is known and the source ID is well formed.
    pub fn validate(&self) -> Result<(), AdmError> {
        if types::lookup(&self.record_type).is_none() {
            return Err(AdmError::new(
                ErrorKind::UnknownType(self.record_type.clone()),
                "type",
            ));
        }
        check_source_id(&self.source_id).map_err(|kind| AdmError::new(kind, "source_id"))
    }

    /// The deletion's JSON value.
    pub fn to_value(&self) -> Value {
        json!({ "type": self.record_type, "source_id": self.source_id })
    }
}

/// A record as stored in the vault: a validated draft plus the identity and
/// provenance the vault assigned when it merged the draft.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Vault-local ID (a ULID), stable for the life of the record.
    pub id: String,
    /// Collector that sealed the batch, for example `health`.
    pub collector: String,
    /// When the vault merged the record, Unix epoch milliseconds.
    pub ingested_ms: i64,
    /// When the vault learned that the source deleted the item, Unix epoch
    /// milliseconds. The record is kept; see RFC-0001 "Deletions".
    pub deleted_at_source_ms: Option<i64>,
    /// The validated draft.
    pub draft: RecordDraft,
}

impl RecordDraft {
    /// The source's own type name inside a generic record type, for example
    /// `HKQuantityTypeIdentifierStepCount` in a `health.quantity` record.
    /// Used to group records for browsing.
    pub fn subtype(&self) -> Option<&str> {
        self.data.get("identifier").and_then(Value::as_str)
    }
}

impl Record {
    /// Default sensitivity tier of the record's type.
    pub fn sensitivity(&self) -> Sensitivity {
        types::lookup(&self.draft.record_type).map_or(Sensitivity::High, |t| t.sensitivity)
    }

    /// The record's JSON (RFC-0001 "Record JSON"), as shown in the app and
    /// written by exports.
    pub fn to_json(&self) -> Value {
        let d = &self.draft;
        let mut time = Map::new();
        time.insert("start".into(), rfc3339(d.start_ms).into());
        if let Some(end) = d.end_ms {
            time.insert("end".into(), rfc3339(end).into());
        }
        if let Some(tz) = &d.tz {
            time.insert("tz".into(), tz.clone().into());
        }
        let mut provenance = Map::new();
        provenance.insert("collector".into(), self.collector.clone().into());
        provenance.insert("source_id".into(), d.source_id.clone().into());
        if let Some(source) = d.source.as_ref().filter(|s| !s.is_empty()) {
            provenance.insert("source".into(), source.to_value());
        }
        provenance.insert("ingested_at".into(), rfc3339(self.ingested_ms).into());
        if let Some(deleted) = self.deleted_at_source_ms {
            provenance.insert("deleted_at_source".into(), rfc3339(deleted).into());
        }
        json!({
            "id": self.id,
            "type": d.record_type,
            "schema_version": ADM_VERSION,
            "time": time,
            "data": d.data,
            "provenance": provenance,
            "sensitivity": self.sensitivity().as_str(),
        })
    }
}

fn validate_data(spec: &RecordType, data: &Map<String, Value>) -> Result<(), AdmError> {
    for key in data.keys() {
        if !spec.fields.iter().any(|f| f.name == key) {
            return Err(AdmError::new(ErrorKind::UnknownField, key.as_str()));
        }
    }
    for field in spec.fields {
        match data.get(field.name) {
            None if field.required => {
                return Err(AdmError::new(ErrorKind::MissingField, field.name));
            }
            None => {}
            Some(value) => {
                check_kind(field.kind, value).map_err(|kind| AdmError::new(kind, field.name))?
            }
        }
    }
    Ok(())
}

fn check_kind(kind: FieldKind, value: &Value) -> Result<(), ErrorKind> {
    match kind {
        FieldKind::Int { min, max } => {
            if !(value.is_i64() || value.is_u64()) {
                return Err(ErrorKind::WrongType("integer"));
            }
            match value.as_i64() {
                Some(v) if (min..=max).contains(&v) => Ok(()),
                _ => Err(ErrorKind::OutOfRange),
            }
        }
        FieldKind::Num {
            min,
            max,
            min_exclusive,
        } => {
            let v = value.as_f64().ok_or(ErrorKind::WrongType("number"))?;
            let above_min = if min_exclusive { v > min } else { v >= min };
            if above_min && v <= max {
                Ok(())
            } else {
                Err(ErrorKind::OutOfRange)
            }
        }
        FieldKind::Bool => value
            .is_boolean()
            .then_some(())
            .ok_or(ErrorKind::WrongType("boolean")),
        FieldKind::Enum(allowed) => {
            let v = value.as_str().ok_or(ErrorKind::WrongType("string"))?;
            allowed
                .contains(&v)
                .then_some(())
                .ok_or(ErrorKind::InvalidValue)
        }
        FieldKind::Text { max_len } => check_text(
            value.as_str().ok_or(ErrorKind::WrongType("string"))?,
            max_len,
        ),
        FieldKind::AnyNum => value
            .is_number()
            .then_some(())
            .ok_or(ErrorKind::WrongType("number")),
        FieldKind::Name => {
            let v = value.as_str().ok_or(ErrorKind::WrongType("string"))?;
            is_name(v).then_some(()).ok_or(ErrorKind::InvalidValue)
        }
        FieldKind::Object => value
            .is_object()
            .then_some(())
            .ok_or(ErrorKind::WrongType("object")),
        FieldKind::Any => Ok(()),
    }
}

/// `[A-Za-z][A-Za-z0-9_.]*`, at most [`types::NAME_MAX`] characters.
fn is_name(v: &str) -> bool {
    let mut bytes = v.bytes();
    v.len() <= types::NAME_MAX
        && bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

fn check_text(v: &str, max_len: usize) -> Result<(), ErrorKind> {
    let len = v.chars().count();
    if (1..=max_len).contains(&len) {
        Ok(())
    } else {
        Err(ErrorKind::InvalidValue)
    }
}

/// 1–256 characters, no C0 control characters or DEL.
fn check_source_id(v: &str) -> Result<(), ErrorKind> {
    check_text(v, SOURCE_ID_MAX)?;
    if v.chars().any(|c| c < ' ' || c == '\u{7f}') {
        return Err(ErrorKind::InvalidValue);
    }
    Ok(())
}

fn check_time(ms: i64, path: &'static str) -> Result<(), AdmError> {
    if (0..=MAX_MS).contains(&ms) {
        Ok(())
    } else {
        Err(AdmError::new(ErrorKind::OutOfRange, path))
    }
}

/// An IANA time zone name: 1–64 of `A-Z a-z 0-9 _ + - /`.
fn is_tz(v: &str) -> bool {
    (1..=TZ_MAX).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'-' | b'/'))
}

/// `[a-z][a-z0-9_]*`, at most `max_len` characters.
pub(crate) fn is_ident(v: &str, max_len: usize) -> bool {
    let mut bytes = v.bytes();
    v.len() <= max_len
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub(crate) fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, AdmError> {
    value
        .as_object()
        .ok_or_else(|| AdmError::new(ErrorKind::WrongType("object"), path))
}

pub(crate) fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, AdmError> {
    value
        .as_str()
        .ok_or_else(|| AdmError::new(ErrorKind::WrongType("string"), path))
}

pub(crate) fn integer(value: &Value, path: &str) -> Result<i64, AdmError> {
    if !(value.is_i64() || value.is_u64()) {
        return Err(AdmError::new(ErrorKind::WrongType("integer"), path));
    }
    value
        .as_i64()
        .ok_or_else(|| AdmError::new(ErrorKind::OutOfRange, path))
}
