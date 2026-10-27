//! Inbox merge and the record browser over UniFFI.

use std::time::{Duration, Instant};

use atlys_vault::inbox;
use atlys_vault::records::{self, Cursor};

use crate::{AtlysError, VaultSession};

/// What one `merge_inbox` call did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MergeReport {
    pub files_merged: u32,
    pub files_replayed: u32,
    pub files_quarantined: u32,
    /// Files written by a newer Atlys version; they wait for an update.
    pub files_need_update: u32,
    /// Files left pending after a transient error.
    pub files_failed: u32,
    /// Files not reached before the time budget ran out.
    pub files_remaining: u32,
    pub records_added: u32,
    pub records_updated: u32,
    pub records_deleted_at_source: u32,
}

impl From<records::MergeReport> for MergeReport {
    fn from(r: records::MergeReport) -> Self {
        Self {
            files_merged: r.files_merged,
            files_replayed: r.files_replayed,
            files_quarantined: r.files_quarantined,
            files_need_update: r.files_need_update,
            files_failed: r.files_failed,
            files_remaining: r.files_remaining,
            records_added: r.records_added,
            records_updated: r.records_updated,
            records_deleted_at_source: r.records_deleted_at_source,
        }
    }
}

/// Record counts per type and subtype.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordTypeSummary {
    pub record_type: String,
    /// The source's type name inside generic types, for example
    /// `HKQuantityTypeIdentifierStepCount`.
    pub subtype: Option<String>,
    pub count: u64,
    pub deleted_at_source: u64,
    pub first_start_ms: i64,
    pub last_start_ms: i64,
}

/// Records and the latest merge per collector.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CollectorSummary {
    pub collector: String,
    pub count: u64,
    pub last_ingested_ms: i64,
    pub last_start_ms: i64,
}

/// One row of a record list.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RecordRow {
    pub id: String,
    pub record_type: String,
    pub subtype: Option<String>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub tz: Option<String>,
    pub deleted_at_source: bool,
    pub preview: String,
}

/// Where the next page starts: the last row's start time and ID.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecordCursor {
    pub start_ms: i64,
    pub id: String,
}

/// Inbox state: batches waiting to merge and quarantined files.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InboxStatus {
    pub pending_files: u32,
    pub quarantined_files: u32,
}

#[uniffi::export]
impl VaultSession {
    /// Merges pending inbox files for at most `max_seconds`. Call again while
    /// `files_remaining > 0`; the session is busy during each call.
    pub fn merge_inbox(&self, max_seconds: f64) -> Result<MergeReport, AtlysError> {
        let deadline = Instant::now() + Duration::from_secs_f64(max_seconds.clamp(0.0, 600.0));
        self.with(|v| Ok(v.merge_inbox(Some(deadline))?.into()))
    }

    /// Total number of stored records.
    pub fn record_count(&self) -> Result<u64, AtlysError> {
        self.with(|v| Ok(v.store().record_count()?))
    }

    /// Record counts per type and subtype.
    pub fn record_types(&self) -> Result<Vec<RecordTypeSummary>, AtlysError> {
        self.with(|v| {
            Ok(v.store()
                .record_types()?
                .into_iter()
                .map(|t| RecordTypeSummary {
                    record_type: t.record_type,
                    subtype: t.subtype,
                    count: t.count,
                    deleted_at_source: t.deleted_at_source,
                    first_start_ms: t.first_start_ms,
                    last_start_ms: t.last_start_ms,
                })
                .collect())
        })
    }

    /// Record counts and latest times per collector.
    pub fn collector_summaries(&self) -> Result<Vec<CollectorSummary>, AtlysError> {
        self.with(|v| {
            Ok(v.store()
                .collector_summaries()?
                .into_iter()
                .map(|c| CollectorSummary {
                    collector: c.collector,
                    count: c.count,
                    last_ingested_ms: c.last_ingested_ms,
                    last_start_ms: c.last_start_ms,
                })
                .collect())
        })
    }

    /// A page of records of one type and subtype, newest first.
    pub fn records(
        &self,
        record_type: String,
        subtype: Option<String>,
        after: Option<RecordCursor>,
        limit: u32,
    ) -> Result<Vec<RecordRow>, AtlysError> {
        let after = after.map(|c| Cursor {
            start_ms: c.start_ms,
            id: c.id,
        });
        self.with(|v| {
            Ok(v.store()
                .records(
                    &record_type,
                    subtype.as_deref(),
                    after.as_ref(),
                    limit.min(500),
                )?
                .into_iter()
                .map(|r| RecordRow {
                    id: r.id,
                    record_type: r.record_type,
                    subtype: r.subtype,
                    start_ms: r.start_ms,
                    end_ms: r.end_ms,
                    tz: r.tz,
                    deleted_at_source: r.deleted_at_source,
                    preview: r.preview,
                })
                .collect())
        })
    }

    /// One record's JSON (RFC-0001 "Record JSON"), pretty-printed, or `nil`
    /// if there is no record with this ID.
    pub fn record_json(&self, id: String) -> Result<Option<String>, AtlysError> {
        self.with(|v| {
            Ok(v.store().record(&id)?.map(|r| {
                serde_json::to_string_pretty(&r.to_json()).expect("JSON values always serialize")
            }))
        })
    }

    /// Pending and quarantined inbox files.
    pub fn inbox_status(&self) -> Result<InboxStatus, AtlysError> {
        self.with(|v| {
            let paths = v.paths();
            let pending = inbox::pending(&paths.inbox()).map_err(atlys_vault::VaultError::from)?;
            let quarantined = inbox::quarantine_counts(&paths.quarantine())
                .map_err(atlys_vault::VaultError::from)?
                .values()
                .sum();
            Ok(InboxStatus {
                pending_files: pending.len() as u32,
                quarantined_files: quarantined,
            })
        })
    }
}
