//! Validation errors with stable codes (RFC-0001, "Errors").

use std::fmt;

/// What is wrong with a value. [`ErrorKind::code`] is stable and appears in
/// the synthetic fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not JSON, a duplicate key, or a value of the wrong shape at the top level.
    Malformed(String),
    /// Written with a newer ADM version than this reader supports.
    UnsupportedVersion(String),
    /// The record type is not defined in this ADM version.
    UnknownType(String),
    /// A required field is absent.
    MissingField,
    /// A field is not defined for this object.
    UnknownField,
    /// A field has the wrong JSON type.
    WrongType(&'static str),
    /// A number is outside its allowed range.
    OutOfRange,
    /// A string is empty, too long, not an allowed value, or badly formatted.
    InvalidValue,
    /// `end_ms` is before `start_ms`.
    InvalidTime,
}

impl ErrorKind {
    /// The stable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "malformed",
            Self::UnsupportedVersion(_) => "unsupported_version",
            Self::UnknownType(_) => "unknown_type",
            Self::MissingField => "missing_field",
            Self::UnknownField => "unknown_field",
            Self::WrongType(_) => "wrong_type",
            Self::OutOfRange => "out_of_range",
            Self::InvalidValue => "invalid_value",
            Self::InvalidTime => "invalid_time",
        }
    }
}

/// An ADM validation error: what is wrong, and where (for example
/// `records[2].data.count`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmError {
    pub kind: ErrorKind,
    pub path: String,
}

impl AdmError {
    pub(crate) fn new(kind: ErrorKind, path: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }

    pub(crate) fn malformed(detail: impl Into<String>) -> Self {
        Self::new(ErrorKind::Malformed(detail.into()), "")
    }

    /// The stable error code.
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }

    /// Prefixes the path, for errors inside a nested object or array.
    pub(crate) fn within(mut self, prefix: &str) -> Self {
        self.path = if self.path.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}.{}", self.path)
        };
        self
    }
}

impl fmt::Display for AdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.path.is_empty() {
            write!(f, "{}: ", self.path)?;
        }
        match &self.kind {
            ErrorKind::Malformed(detail) => write!(f, "malformed: {detail}"),
            ErrorKind::UnsupportedVersion(v) => {
                write!(f, "ADM version {v:?} is newer than this app supports")
            }
            ErrorKind::UnknownType(t) => write!(f, "unknown record type {t:?}"),
            ErrorKind::MissingField => f.write_str("missing required field"),
            ErrorKind::UnknownField => f.write_str("field is not allowed here"),
            ErrorKind::WrongType(expected) => write!(f, "expected {expected}"),
            ErrorKind::OutOfRange => f.write_str("value out of range"),
            ErrorKind::InvalidValue => f.write_str("invalid value"),
            ErrorKind::InvalidTime => f.write_str("ends before it starts"),
        }
    }
}

impl std::error::Error for AdmError {}
