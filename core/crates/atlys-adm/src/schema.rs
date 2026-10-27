//! Renders the JSON Schema of each record type from [`crate::types::TYPES`].
//! The rendered files are committed in `spec/adm/v0/types/`.

use serde_json::{Map, Value, json};

use crate::types::{FieldKind, RecordType, TimeShape};

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
    let value = match kind {
        FieldKind::Int { min, max } => json!({ "type": "integer", "minimum": min, "maximum": max }),
        FieldKind::Num {
            min,
            max,
            min_exclusive,
        } => {
            let bound = if min_exclusive {
                "exclusiveMinimum"
            } else {
                "minimum"
            };
            json!({ "type": "number", bound: number(min), "maximum": number(max) })
        }
        FieldKind::Bool => json!({ "type": "boolean" }),
        FieldKind::Enum(values) => json!({ "enum": values }),
        FieldKind::Text { max_len } => {
            json!({ "type": "string", "minLength": 1, "maxLength": max_len })
        }
        FieldKind::Ident { max_len } => {
            json!({ "type": "string", "pattern": "^[a-z][a-z0-9_]*$", "maxLength": max_len })
        }
    };
    match value {
        Value::Object(map) => map,
        _ => unreachable!("every kind renders an object"),
    }
}

/// Whole numbers render as JSON integers (`300`, not `300.0`).
fn number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}
