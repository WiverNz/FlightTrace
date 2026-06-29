//! Persistence for telemetry samples.

use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use flighttrace_core::{MissionRunId, TelemetrySample};

use crate::encoding::{id_to_text, text_to_id, text_to_timestamp, timestamp_to_text};
use crate::error::StorageResult;

/// Repository for telemetry samples.
#[derive(Debug, Clone)]
pub struct TelemetrySampleRepo<'a> {
    pool: &'a SqlitePool,
}

impl<'a> TelemetrySampleRepo<'a> {
    /// Wrap a pool reference.
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a single telemetry sample.
    pub async fn insert(&self, sample: &TelemetrySample) -> StorageResult<()> {
        sqlx::query(
            r#"
            INSERT INTO telemetry_samples
                (run_id, at, altitude_m, vertical_speed_mps, mode, armed, battery_remaining_pct)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(id_to_text(sample.run_id.into_inner()))
        .bind(timestamp_to_text(sample.at)?)
        .bind(sample.altitude_m)
        .bind(sample.vertical_speed_mps)
        .bind(&sample.mode)
        .bind(if sample.armed { 1_i64 } else { 0_i64 })
        .bind(sample.battery_remaining_pct as f64)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// List samples for a run ordered by `at ASC`.
    pub async fn list_for_run(&self, run_id: MissionRunId) -> StorageResult<Vec<TelemetrySample>> {
        let rows = sqlx::query(
            r#"
            SELECT run_id, at, altitude_m, vertical_speed_mps, mode, armed, battery_remaining_pct
            FROM telemetry_samples
            WHERE run_id = ?1
            ORDER BY at ASC
            "#,
        )
        .bind(id_to_text(run_id.into_inner()))
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(row_to_sample).collect()
    }
}

pub(crate) async fn insert_sample_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    sample: &TelemetrySample,
) -> StorageResult<()> {
    sqlx::query(
        r#"
        INSERT INTO telemetry_samples
            (run_id, at, altitude_m, vertical_speed_mps, mode, armed, battery_remaining_pct)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(id_to_text(sample.run_id.into_inner()))
    .bind(timestamp_to_text(sample.at)?)
    .bind(sample.altitude_m)
    .bind(sample.vertical_speed_mps)
    .bind(&sample.mode)
    .bind(if sample.armed { 1_i64 } else { 0_i64 })
    .bind(sample.battery_remaining_pct as f64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn row_to_sample(row: SqliteRow) -> StorageResult<TelemetrySample> {
    let run_id_text: String = row.try_get("run_id")?;
    let at_text: String = row.try_get("at")?;
    let altitude_m: f64 = row.try_get("altitude_m")?;
    let vertical_speed_mps: f64 = row.try_get("vertical_speed_mps")?;
    let mode: String = row.try_get("mode")?;
    let armed_int: i64 = row.try_get("armed")?;
    let battery_remaining_pct: f64 = row.try_get("battery_remaining_pct")?;

    Ok(TelemetrySample {
        run_id: MissionRunId::new(text_to_id(&run_id_text)?),
        at: text_to_timestamp(&at_text)?,
        altitude_m,
        vertical_speed_mps,
        mode,
        armed: armed_int != 0,
        battery_remaining_pct: battery_remaining_pct as f32,
    })
}
