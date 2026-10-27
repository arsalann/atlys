//! Renders the JSON Schema of each record type from [`crate::types::TYPES`].
//! The rendered files are committed in `spec/adm/v0/types/`.

use serde_json::{Map, Value, json};

use crate::types::{self, FieldKind, RecordType, TimeShape};

/// JSON Schema dialect of the ADM schemas.
pub const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// File name of a record type's schema, for example `health.steps.schema.json`.
pub fn file_name(record_type: &RecordType) -> String {
    format!("{}.schema.json", record_type.name)
}

/// The JSON Schema of a complete record draft of this type.
pub fn render(record_type: &RecordType) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in record_type.fields {
        let mut schema = kind_schema(field.kind);
        schema.insert("description".into(), field.description.into());
        properties.insert(field.name.into(), Value::Object(schema));
        if field.required {
            required.push(field.name);
        }
    }
    let mut out = Map::new();
    out.insert("$schema".into(), DIALECT.into());
    out.insert("title".into(), record_type.name.into());
    out.insert("description".into(), record_type.description.into());
    out.insert(
        "x-sensitivity".into(),
        record_type.sensitivity.as_str().into(),
    );
    out.insert(
        "allOf".into(),
        json!([{ "$ref": "../record-draft.schema.json" }]),
    );
    out.insert(
        "properties".into(),
        json!({
            "type": { "const": record_type.name },
            "data": {
                "type": "object",
                "additionalProperties": false,
                "required": required,
                "properties": properties,
            },
        }),
    );
    match record_type.time {
        TimeShape::Interval => {
            out.insert("required".into(), json!(["end_ms"]));
        }
        TimeShape::Instant => {
            out.insert("not".into(), json!({ "required": ["end_ms"] }));
        }
        TimeShape::Either => {}
    }
    Value::Object(out)
}

/// Pretty JSON with a trailing newline, as committed.
pub fn to_file(schema: &Value) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("JSON values always serialize");
    text.push('\n');
    text
}

fn kind_schema(kind: FieldKind) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |key: &str, value: Value| {
        out.insert(key.into(), value);
    };
    match kind {
        FieldKind::Int { min, max } => {
            put("type", "integer".into());
            if min != i64::MIN {
                put("minimum", min.into());
            }
            if max != i64::MAX {
                put("maximum", max.into());
            }
        }
        FieldKind::Num {
            min,
            max,
            min_exclusive,
        } => {
            put("type", "number".into());
            put(
                if min_exclusive {
                    "exclusiveMinimum"
                } else {
                    "minimum"
                },
                number(min),
            );
            if max.is_finite() {
                put("maximum", number(max));
            }
        }
        FieldKind::AnyNum => put("type", "number".into()),
        FieldKind::Bool => put("type", "boolean".into()),
        FieldKind::Enum(values) => put("enum", json!(values)),
        FieldKind::Text { max_len } => {
            put("type", "string".into());
            put("minLength", 1.into());
            put("maxLength", max_len.into());
        }
        FieldKind::Name => {
            put("type", "string".into());
            put("pattern", types::NAME_PATTERN.into());
            put("maxLength", types::NAME_MAX.into());
        }
        FieldKind::Object => put("type", "object".into()),
        FieldKind::Any => {}
    }
    out
}

/// Whole numbers render as JSON integers (`300`, not `300.0`).
fn number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}
