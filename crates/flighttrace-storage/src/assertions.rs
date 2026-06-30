//! Persistence for assertion results.

use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use flighttrace_core::{AssertionOutcome, AssertionResult, AssertionResultId, MissionRunId};

use crate::encoding::{
    id_to_text, text_to_id, text_to_timestamp, text_to_unit_enum, timestamp_to_text,
    unit_enum_to_text,
};
use crate::error::StorageResult;

/// Repository for assertion results.
#[derive(Debug, Clone)]
pub struct AssertionResultRepo<'a> {
    pool: &'a SqlitePool,
}

impl<'a> AssertionResultRepo<'a> {
    /// Wrap a pool reference.
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a single assertion result.
    pub async fn insert(&self, result: &AssertionResult) -> StorageResult<()> {
        sqlx::query(
            r#"
            INSERT INTO assertion_results (id, run_id, name, outcome, detail, evaluated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(id_to_text(result.id.into_inner()))
        .bind(id_to_text(result.run_id.into_inner()))
        .bind(&result.name)
        .bind(unit_enum_to_text(&result.outcome)?)
        .bind(&result.detail)
        .bind(timestamp_to_text(result.evaluated_at)?)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Replace every assertion result for a run with the supplied set, in one
    /// transaction. Used by the assertion engine when it (re-)evaluates a run.
    pub async fn replace_for_run(
        &self,
        run_id: MissionRunId,
        results: &[AssertionResult],
    ) -> StorageResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM assertion_results WHERE run_id = ?1")
            .bind(id_to_text(run_id.into_inner()))
            .execute(&mut *tx)
            .await?;
        for result in results {
            insert_assertion_in_tx(&mut tx, result).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// List assertion results for a run ordered by `evaluated_at ASC`.
    pub async fn list_for_run(&self, run_id: MissionRunId) -> StorageResult<Vec<AssertionResult>> {
        let rows = sqlx::query(
            r#"
            SELECT id, run_id, name, outcome, detail, evaluated_at
            FROM assertion_results
            WHERE run_id = ?1
            ORDER BY evaluated_at ASC, id ASC
            "#,
        )
        .bind(id_to_text(run_id.into_inner()))
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(row_to_assertion).collect()
    }
}

pub(crate) async fn insert_assertion_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    result: &AssertionResult,
) -> StorageResult<()> {
    sqlx::query(
        r#"
        INSERT INTO assertion_results (id, run_id, name, outcome, detail, evaluated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(id_to_text(result.id.into_inner()))
    .bind(id_to_text(result.run_id.into_inner()))
    .bind(&result.name)
    .bind(unit_enum_to_text(&result.outcome)?)
    .bind(&result.detail)
    .bind(timestamp_to_text(result.evaluated_at)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn row_to_assertion(row: SqliteRow) -> StorageResult<AssertionResult> {
    let id_text: String = row.try_get("id")?;
    let run_id_text: String = row.try_get("run_id")?;
    let name: String = row.try_get("name")?;
    let outcome_text: String = row.try_get("outcome")?;
    let detail: String = row.try_get("detail")?;
    let evaluated_at_text: String = row.try_get("evaluated_at")?;

    let outcome: AssertionOutcome = text_to_unit_enum(&outcome_text)?;

    Ok(AssertionResult {
        id: AssertionResultId::new(text_to_id(&id_text)?),
        run_id: MissionRunId::new(text_to_id(&run_id_text)?),
        name,
        outcome,
        detail,
        evaluated_at: text_to_timestamp(&evaluated_at_text)?,
    })
}
