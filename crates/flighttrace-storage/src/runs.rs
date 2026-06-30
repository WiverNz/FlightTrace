//! Persistence for [`MissionRun`] and its phase transitions.

use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use flighttrace_core::{
    FailureReason, MissionPhase, MissionRun, MissionRunId, MissionSource, MissionStatus,
    PhaseTransition,
};

use crate::encoding::{
    id_to_text, text_to_id, text_to_timestamp, text_to_unit_enum, timestamp_to_text,
    unit_enum_to_text,
};
use crate::error::StorageResult;

/// Repository for mission run headlines and their phase transitions.
#[derive(Debug, Clone)]
pub struct MissionRunRepo<'a> {
    pool: &'a SqlitePool,
}

impl<'a> MissionRunRepo<'a> {
    /// Wrap a pool reference.
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a mission run and its phase transitions in a single transaction.
    pub async fn insert(&self, run: &MissionRun) -> StorageResult<()> {
        let mut tx = self.pool.begin().await?;
        insert_run_inner(&mut tx, run).await?;
        tx.commit().await?;
        Ok(())
    }

    /// List mission runs in `started_at DESC` order with `limit`/`offset`.
    pub async fn list(&self, limit: u32, offset: u32) -> StorageResult<Vec<MissionRun>> {
        let rows = sqlx::query(
            r#"
            SELECT id, vehicle, mission, source, status, failure_reason, started_at, ended_at
            FROM mission_runs
            ORDER BY started_at DESC, id ASC
            LIMIT ?1 OFFSET ?2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool)
        .await?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(row_to_run_with_phases(self.pool, row).await?);
        }
        Ok(runs)
    }

    /// Fetch a single run (with phases). Returns `None` when missing.
    pub async fn get(&self, id: MissionRunId) -> StorageResult<Option<MissionRun>> {
        let row = sqlx::query(
            r#"
            SELECT id, vehicle, mission, source, status, failure_reason, started_at, ended_at
            FROM mission_runs WHERE id = ?1
            "#,
        )
        .bind(id_to_text(id.into_inner()))
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_run_with_phases(self.pool, row).await?)),
            None => Ok(None),
        }
    }

    /// Delete a run. Cascades to all child rows. Returns `true` if a row was removed.
    pub async fn delete(&self, id: MissionRunId) -> StorageResult<bool> {
        let result = sqlx::query("DELETE FROM mission_runs WHERE id = ?1")
            .bind(id_to_text(id.into_inner()))
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

pub(crate) async fn insert_run_inner(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run: &MissionRun,
) -> StorageResult<()> {
    let failure_reason_text = match run.failure_reason {
        Some(reason) => Some(unit_enum_to_text(&reason)?),
        None => None,
    };
    let ended_at_text = match run.ended_at {
        Some(ts) => Some(timestamp_to_text(ts)?),
        None => None,
    };

    sqlx::query(
        r#"
        INSERT INTO mission_runs (id, vehicle, mission, source, status, failure_reason, started_at, ended_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(id_to_text(run.id.into_inner()))
    .bind(&run.vehicle)
    .bind(&run.mission)
    .bind(unit_enum_to_text(&run.source)?)
    .bind(unit_enum_to_text(&run.status)?)
    .bind(failure_reason_text)
    .bind(timestamp_to_text(run.started_at)?)
    .bind(ended_at_text)
    .execute(&mut **tx)
    .await?;

    for (ordinal, phase) in run.phases.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO mission_phase_transitions (run_id, ordinal, at, phase)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(id_to_text(run.id.into_inner()))
        .bind(ordinal as i64)
        .bind(timestamp_to_text(phase.at)?)
        .bind(unit_enum_to_text(&phase.phase)?)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn row_to_run_with_phases(pool: &SqlitePool, row: SqliteRow) -> StorageResult<MissionRun> {
    let id_text: String = row.try_get("id")?;
    let id = MissionRunId::new(text_to_id(&id_text)?);

    let vehicle: String = row.try_get("vehicle")?;
    let mission: String = row.try_get("mission")?;
    let source_text: String = row.try_get("source")?;
    let status_text: String = row.try_get("status")?;
    let failure_reason_text: Option<String> = row.try_get("failure_reason")?;
    let started_at_text: String = row.try_get("started_at")?;
    let ended_at_text: Option<String> = row.try_get("ended_at")?;

    let source: MissionSource = text_to_unit_enum(&source_text)?;
    let status: MissionStatus = text_to_unit_enum(&status_text)?;
    let failure_reason: Option<FailureReason> = match failure_reason_text {
        Some(text) => Some(text_to_unit_enum(&text)?),
        None => None,
    };
    let started_at = text_to_timestamp(&started_at_text)?;
    let ended_at = match ended_at_text {
        Some(text) => Some(text_to_timestamp(&text)?),
        None => None,
    };

    let phase_rows = sqlx::query(
        r#"
        SELECT at, phase FROM mission_phase_transitions
        WHERE run_id = ?1
        ORDER BY ordinal ASC
        "#,
    )
    .bind(id_to_text(id.into_inner()))
    .fetch_all(pool)
    .await?;

    let mut phases = Vec::with_capacity(phase_rows.len());
    for row in phase_rows {
        let at_text: String = row.try_get("at")?;
        let phase_text: String = row.try_get("phase")?;
        let phase: MissionPhase = text_to_unit_enum(&phase_text)?;
        phases.push(PhaseTransition {
            at: text_to_timestamp(&at_text)?,
            phase,
        });
    }

    Ok(MissionRun {
        id,
        vehicle,
        mission,
        source,
        status,
        failure_reason,
        started_at,
        ended_at,
        phases,
    })
}
