//! The Atlys Data Model (ADM) v0: record drafts, batches, stored records, and
//! validation. Specified in RFC-0001 (`spec/rfcs/0001-data-model.md`); the
//! JSON Schemas and synthetic fixtures live in `spec/adm/v0/`.

#![forbid(unsafe_code)]

mod batch;
pub mod draft;
mod error;
mod record;
pub mod schema;
mod strict;
pub mod time;
pub mod types;

pub use batch::Batch;
pub use error::{AdmError, ErrorKind};
pub use record::{Deletion, Record, RecordDraft, Source};
pub use types::{RecordType, Sensitivity, TimeShape};

/// Version of the ADM implemented by this crate. Batches and record JSON
/// carry it; see RFC-0001, "Versioning".
pub const ADM_VERSION: &str = "0.1";
