//! Helpers for converting domain values to and from their TEXT column form.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::BorrowedFormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

use flighttrace_core::Timestamp;

use crate::error::StorageError;

/// Fixed-width RFC 3339 format used for `at` columns so SQLite's lexicographic
/// `ORDER BY` matches chronological order. Subseconds are always rendered with
/// nine digits.
const STORAGE_TIMESTAMP_FORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9]Z");

/// Render a UUID-based id as its canonical hyphenated string.
pub(crate) fn id_to_text(id: Uuid) -> String {
    id.to_string()
}

/// Parse a TEXT column back into a [`Uuid`].
pub(crate) fn text_to_id(raw: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(raw).map_err(|err| StorageError::Sqlx(sqlx::Error::Decode(Box::new(err))))
}

/// Render a [`Timestamp`] as the storage-canonical RFC 3339 string.
///
/// Storage timestamps always carry nanosecond precision so the column is
/// fixed-width and orders correctly under SQLite's lexicographic comparison.
pub(crate) fn timestamp_to_text(ts: Timestamp) -> Result<String, StorageError> {
    ts.0.format(STORAGE_TIMESTAMP_FORMAT)
        .map_err(|err| StorageError::Sqlx(sqlx::Error::Decode(Box::new(err))))
}

/// Parse a TEXT column back into a [`Timestamp`].
///
/// Accepts any RFC 3339 input — the storage format is a strict subset, but
/// the parser is permissive in case rows were written by a different tool.
pub(crate) fn text_to_timestamp(raw: &str) -> Result<Timestamp, StorageError> {
    OffsetDateTime::parse(raw, &Rfc3339)
        .map(Timestamp)
        .map_err(|err| StorageError::Sqlx(sqlx::Error::Decode(Box::new(err))))
}

/// Encode a unit-variant enum (e.g. `MissionStatus`, `FailureReason`) as its
/// canonical variant name. Panics in debug if used on a non-unit variant.
pub(crate) fn unit_enum_to_text<T: Serialize>(value: &T) -> Result<String, StorageError> {
    match serde_json::to_value(value)? {
        Value::String(s) => Ok(s),
        other => Err(StorageError::Sqlx(sqlx::Error::Decode(
            format!("expected unit enum, got {other:?}").into(),
        ))),
    }
}

/// Decode a TEXT column back into a unit-variant enum.
pub(crate) fn text_to_unit_enum<T: DeserializeOwned>(raw: &str) -> Result<T, StorageError> {
    Ok(serde_json::from_value(Value::String(raw.to_string()))?)
}
