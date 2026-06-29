//! Persistence for failure reports.

use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use flighttrace_core::{Evidence, FailureReason, FailureReport, FailureReportId, MissionRunId};

use crate::encoding::{
    id_to_text, text_to_id, text_to_timestamp, text_to_unit_enum, timestamp_to_text,
    unit_enum_to_text,
};
use crate::error::StorageResult;

/// Repository for failure reports.
#[derive(Debug, Clone)]
pub struct FailureReportRepo<'a> {
    pool: &'a SqlitePool,
}

impl<'a> FailureReportRepo<'a> {
    /// Wrap a pool reference.
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a failure report.
    pub async fn insert(&self, report: &FailureReport) -> StorageResult<()> {
        let evidence_json = serde_json::to_string(&report.evidence)?;
        sqlx::query(
            r#"
            INSERT INTO failure_reports
                (id, run_id, reason, summary, evidence_json, likely_cause, generated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(id_to_text(report.id.into_inner()))
        .bind(id_to_text(report.run_id.into_inner()))
        .bind(unit_enum_to_text(&report.reason)?)
        .bind(&report.summary)
        .bind(evidence_json)
        .bind(&report.likely_cause)
        .bind(timestamp_to_text(report.generated_at)?)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Fetch the failure report for a run, if any.
    pub async fn get_for_run(&self, run_id: MissionRunId) -> StorageResult<Option<FailureReport>> {
        let row = sqlx::query(
            r#"
            SELECT id, run_id, reason, summary, evidence_json, likely_cause, generated_at
            FROM failure_reports WHERE run_id = ?1
            "#,
        )
        .bind(id_to_text(run_id.into_inner()))
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row_to_report(row)?)),
            None => Ok(None),
        }
    }
}

pub(crate) async fn insert_report_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    report: &FailureReport,
) -> StorageResult<()> {
    let evidence_json = serde_json::to_string(&report.evidence)?;
    sqlx::query(
        r#"
        INSERT INTO failure_reports
            (id, run_id, reason, summary, evidence_json, likely_cause, generated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(id_to_text(report.id.into_inner()))
    .bind(id_to_text(report.run_id.into_inner()))
    .bind(unit_enum_to_text(&report.reason)?)
    .bind(&report.summary)
    .bind(evidence_json)
    .bind(&report.likely_cause)
    .bind(timestamp_to_text(report.generated_at)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn row_to_report(row: SqliteRow) -> StorageResult<FailureReport> {
    let id_text: String = row.try_get("id")?;
    let run_id_text: String = row.try_get("run_id")?;
    let reason_text: String = row.try_get("reason")?;
    let summary: String = row.try_get("summary")?;
    let evidence_json: String = row.try_get("evidence_json")?;
    let likely_cause: String = row.try_get("likely_cause")?;
    let generated_at_text: String = row.try_get("generated_at")?;

    let reason: FailureReason = text_to_unit_enum(&reason_text)?;
    let evidence: Vec<Evidence> = serde_json::from_str(&evidence_json)?;

    Ok(FailureReport {
        id: FailureReportId::new(text_to_id(&id_text)?),
        run_id: MissionRunId::new(text_to_id(&run_id_text)?),
        reason,
        summary,
        evidence,
        likely_cause,
        generated_at: text_to_timestamp(&generated_at_text)?,
    })
}
