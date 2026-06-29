//! Persist and load a full mission run bundle.

use sqlx::SqlitePool;

use flighttrace_core::{
    AssertionResult, FailureReport, MissionRun, MissionRunId, NormalizedEvent, TelemetrySample,
};

use crate::assertions::{AssertionResultRepo, insert_assertion_in_tx};
use crate::error::{StorageError, StorageResult};
use crate::events::{NormalizedEventRepo, insert_event_in_tx};
use crate::failure::{FailureReportRepo, insert_report_in_tx};
use crate::runs::{MissionRunRepo, insert_run_inner};
use crate::telemetry::{TelemetrySampleRepo, insert_sample_in_tx};

/// Owned, fully-hydrated mission run with all child collections.
///
/// Mirrors `flighttrace_demo::DemoRun` but lives in `flighttrace-storage` so
/// the storage crate does not need to depend on `flighttrace-demo`.
#[derive(Debug, Clone, PartialEq)]
pub struct RunBundle {
    /// Headline metadata for the run.
    pub run: MissionRun,
    /// Normalized events in chronological order.
    pub events: Vec<NormalizedEvent>,
    /// Telemetry samples in chronological order.
    pub telemetry: Vec<TelemetrySample>,
    /// Assertion verdicts.
    pub assertions: Vec<AssertionResult>,
    /// Failure report explaining why the run failed.
    pub failure_report: FailureReport,
}

/// Persist a full bundle in a single transaction. Returns the run id.
pub async fn persist_bundle(pool: &SqlitePool, bundle: &RunBundle) -> StorageResult<MissionRunId> {
    let mut tx = pool.begin().await?;

    insert_run_inner(&mut tx, &bundle.run).await?;
    for event in &bundle.events {
        insert_event_in_tx(&mut tx, event).await?;
    }
    for sample in &bundle.telemetry {
        insert_sample_in_tx(&mut tx, sample).await?;
    }
    for result in &bundle.assertions {
        insert_assertion_in_tx(&mut tx, result).await?;
    }
    insert_report_in_tx(&mut tx, &bundle.failure_report).await?;

    tx.commit().await?;
    Ok(bundle.run.id)
}

/// Load a full bundle by run id. Returns `None` if the run does not exist.
///
/// If the run exists but has no failure report this returns
/// [`StorageError::NotFound`] because v0.1 only persists bundles that include
/// one. Once issue 4 (live ingestion) lands this will be relaxed.
pub async fn load_bundle(pool: &SqlitePool, id: MissionRunId) -> StorageResult<Option<RunBundle>> {
    let run = match MissionRunRepo::new(pool).get(id).await? {
        Some(run) => run,
        None => return Ok(None),
    };

    let events = NormalizedEventRepo::new(pool).list_for_run(id).await?;
    let telemetry = TelemetrySampleRepo::new(pool).list_for_run(id).await?;
    let assertions = AssertionResultRepo::new(pool).list_for_run(id).await?;
    let failure_report = FailureReportRepo::new(pool)
        .get_for_run(id)
        .await?
        .ok_or(StorageError::NotFound)?;

    Ok(Some(RunBundle {
        run,
        events,
        telemetry,
        assertions,
        failure_report,
    }))
}
