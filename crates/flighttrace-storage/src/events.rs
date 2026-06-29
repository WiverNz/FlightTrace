//! Persistence for normalized events.
//!
//! `normalized_events` stores each event as `(kind, payload)` where `payload`
//! is the variant-specific JSON object *without* the `kind` discriminator.
//! When loading we re-merge them so they round-trip through the
//! internally-tagged enum serde representation.

use serde_json::{Map, Value};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use flighttrace_core::{EventId, MissionRunId, NormalizedEvent, NormalizedEventKind};

use crate::encoding::{id_to_text, text_to_id, text_to_timestamp, timestamp_to_text};
use crate::error::{StorageError, StorageResult};

/// Repository for normalized events.
#[derive(Debug, Clone)]
pub struct NormalizedEventRepo<'a> {
    pool: &'a SqlitePool,
}

impl<'a> NormalizedEventRepo<'a> {
    /// Wrap a pool reference.
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a single event.
    pub async fn insert(&self, event: &NormalizedEvent) -> StorageResult<()> {
        let (kind, payload) = split_kind_and_payload(&event.kind)?;
        sqlx::query(
            r#"
            INSERT INTO normalized_events (id, run_id, at, kind, payload)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(id_to_text(event.id.into_inner()))
        .bind(id_to_text(event.run_id.into_inner()))
        .bind(timestamp_to_text(event.at)?)
        .bind(kind)
        .bind(payload)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// List events for a run ordered by `at ASC`.
    pub async fn list_for_run(&self, run_id: MissionRunId) -> StorageResult<Vec<NormalizedEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, run_id, at, kind, payload FROM normalized_events
            WHERE run_id = ?1
            ORDER BY at ASC, id ASC
            "#,
        )
        .bind(id_to_text(run_id.into_inner()))
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(row_to_event).collect()
    }
}

pub(crate) async fn insert_event_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    event: &NormalizedEvent,
) -> StorageResult<()> {
    let (kind, payload) = split_kind_and_payload(&event.kind)?;
    sqlx::query(
        r#"
        INSERT INTO normalized_events (id, run_id, at, kind, payload)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(id_to_text(event.id.into_inner()))
    .bind(id_to_text(event.run_id.into_inner()))
    .bind(timestamp_to_text(event.at)?)
    .bind(kind)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn split_kind_and_payload(kind: &NormalizedEventKind) -> StorageResult<(String, String)> {
    let mut value = serde_json::to_value(kind)?;
    let object = value.as_object_mut().ok_or_else(|| {
        StorageError::Sqlx(sqlx::Error::Decode(
            "expected internally-tagged event kind to serialize as an object".into(),
        ))
    })?;

    let kind_value = object.remove("kind").ok_or_else(|| {
        StorageError::Sqlx(sqlx::Error::Decode(
            "event kind missing the `kind` discriminator".into(),
        ))
    })?;
    let kind_str = match kind_value {
        Value::String(s) => s,
        other => {
            return Err(StorageError::Sqlx(sqlx::Error::Decode(
                format!("`kind` was not a string: {other}").into(),
            )));
        }
    };

    let payload_json = serde_json::to_string(&Value::Object(std::mem::take(object)))?;
    Ok((kind_str, payload_json))
}

fn merge_kind_and_payload(kind: &str, payload: &str) -> StorageResult<NormalizedEventKind> {
    let mut payload_value: Value = serde_json::from_str(payload)?;
    let object = match &mut payload_value {
        Value::Object(map) => map,
        _ => {
            let mut empty = Map::new();
            empty.insert("kind".to_string(), Value::String(kind.to_string()));
            let merged = Value::Object(empty);
            return Ok(serde_json::from_value(merged)?);
        }
    };
    object.insert("kind".to_string(), Value::String(kind.to_string()));
    Ok(serde_json::from_value(payload_value)?)
}

fn row_to_event(row: SqliteRow) -> StorageResult<NormalizedEvent> {
    let id_text: String = row.try_get("id")?;
    let run_id_text: String = row.try_get("run_id")?;
    let at_text: String = row.try_get("at")?;
    let kind_text: String = row.try_get("kind")?;
    let payload_text: String = row.try_get("payload")?;

    Ok(NormalizedEvent {
        id: EventId::new(text_to_id(&id_text)?),
        run_id: MissionRunId::new(text_to_id(&run_id_text)?),
        at: text_to_timestamp(&at_text)?,
        kind: merge_kind_and_payload(&kind_text, &payload_text)?,
    })
}
