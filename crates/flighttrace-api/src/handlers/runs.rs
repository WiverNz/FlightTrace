//! Handlers under `/api/runs`.

use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use uuid::Uuid;

use flighttrace_assertions::{EvaluationInput, evaluate, load_mission_file};
use flighttrace_core::{MissionRun, MissionRunId, MissionStatus};
use flighttrace_storage::{
    AssertionResultRepo, FailureReportRepo, MissionRunRepo, NormalizedEventRepo,
    TelemetrySampleRepo,
};

use crate::dto::{
    AssertionResultDto, FailureReportDto, MissionRunDto, NormalizedEventDto, ReevaluateBody,
    ReevaluateResponse, TelemetrySampleDto,
};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Default page size when the client does not specify `limit`.
const DEFAULT_LIMIT: u32 = 50;
/// Maximum page size accepted from a client.
const MAX_LIMIT: u32 = 200;

/// Pagination query for `GET /api/runs`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Page size; capped at [`MAX_LIMIT`].
    pub limit: Option<u32>,
    /// Number of rows to skip before the page.
    pub offset: Option<u32>,
}

/// `GET /api/runs?limit&offset` — list mission runs newest-first.
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<MissionRunDto>>> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let runs = MissionRunRepo::new(state.pool())
        .list(limit, offset)
        .await?;
    Ok(Json(runs.into_iter().map(MissionRunDto::from).collect()))
}

/// `GET /api/runs/:id` — fetch a single mission run.
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<MissionRunDto>> {
    let id = MissionRunId::new(id);
    let run = MissionRunRepo::new(state.pool())
        .get(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(run.into()))
}

/// `GET /api/runs/:id/events` — list events for a run.
pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<NormalizedEventDto>>> {
    let id = MissionRunId::new(id);
    ensure_run_exists(&state, id).await?;
    let events = NormalizedEventRepo::new(state.pool())
        .list_for_run(id)
        .await?;
    Ok(Json(
        events.into_iter().map(NormalizedEventDto::from).collect(),
    ))
}

/// `GET /api/runs/:id/assertions` — list assertion results for a run.
pub async fn assertions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<AssertionResultDto>>> {
    let id = MissionRunId::new(id);
    ensure_run_exists(&state, id).await?;
    let results = AssertionResultRepo::new(state.pool())
        .list_for_run(id)
        .await?;
    Ok(Json(
        results.into_iter().map(AssertionResultDto::from).collect(),
    ))
}

/// `GET /api/runs/:id/telemetry` — list telemetry samples for a run.
pub async fn telemetry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<TelemetrySampleDto>>> {
    let id = MissionRunId::new(id);
    ensure_run_exists(&state, id).await?;
    let samples = TelemetrySampleRepo::new(state.pool())
        .list_for_run(id)
        .await?;
    Ok(Json(
        samples.into_iter().map(TelemetrySampleDto::from).collect(),
    ))
}

/// `GET /api/runs/:id/failure-report` — fetch the failure report for a run.
pub async fn failure_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FailureReportDto>> {
    let id = MissionRunId::new(id);
    ensure_run_exists(&state, id).await?;
    let report = FailureReportRepo::new(state.pool())
        .get_for_run(id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(report.into()))
}

/// `POST /api/runs/:id/reevaluate` — re-run the assertion engine against a
/// run's stored events and samples.
///
/// The handler resolves a mission YAML (body's `missionPath` overrides the
/// run's tagged mission file under `examples/missions/`), evaluates it, and
/// rewrites the run's assertion results, failure report, and headline
/// status to match the new verdict.
pub async fn reevaluate(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Option<Json<ReevaluateBody>>,
) -> ApiResult<Json<ReevaluateResponse>> {
    let id = MissionRunId::new(id);
    let body = body.map(|Json(b)| b).unwrap_or_default();

    let run = MissionRunRepo::new(state.pool())
        .get(id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let path = resolve_mission_path(&run, body.mission_path.as_deref())?;
    let spec = load_mission_file(&path)?;

    let events = NormalizedEventRepo::new(state.pool())
        .list_for_run(id)
        .await?;
    let samples = TelemetrySampleRepo::new(state.pool())
        .list_for_run(id)
        .await?;

    let output = evaluate(
        &spec,
        &EvaluationInput {
            run: &run,
            events: &events,
            samples: &samples,
        },
    );

    let assertion_repo = AssertionResultRepo::new(state.pool());
    assertion_repo.replace_for_run(id, &output.results).await?;

    let failure_repo = FailureReportRepo::new(state.pool());
    let run_repo = MissionRunRepo::new(state.pool());
    match &output.failure_report {
        Some(report) => {
            failure_repo.upsert_for_run(report).await?;
            let ended_at = run.ended_at.unwrap_or(report.generated_at);
            run_repo
                .update_status(
                    id,
                    MissionStatus::Failed,
                    Some(report.reason),
                    Some(ended_at),
                )
                .await?;
        }
        None => {
            failure_repo.delete_for_run(id).await?;
            // Don't flip a still-running mission to Succeeded — only conclude
            // runs that have already ended.
            if run.ended_at.is_some() {
                run_repo
                    .update_status(id, MissionStatus::Succeeded, None, run.ended_at)
                    .await?;
            }
        }
    }

    Ok(Json(ReevaluateResponse {
        assertions: output
            .results
            .into_iter()
            .map(AssertionResultDto::from)
            .collect(),
        failure_report: output.failure_report.map(FailureReportDto::from),
    }))
}

/// Resolve the on-disk mission YAML for a re-evaluation request.
///
/// Explicit `mission_path` wins; otherwise we look under
/// `examples/missions/<run.mission>.yaml`. The run's `mission` field is the
/// label set when the run was ingested, so this assumes mission YAMLs are
/// kept in the standard examples directory under a matching basename.
fn resolve_mission_path(run: &MissionRun, override_path: Option<&str>) -> ApiResult<PathBuf> {
    if let Some(path) = override_path {
        if path.is_empty() {
            return Err(ApiError::BadRequest(
                "missionPath must not be empty".to_string(),
            ));
        }
        return Ok(PathBuf::from(path));
    }
    if run.mission.is_empty() {
        return Err(ApiError::BadRequest(
            "run has no tagged mission; supply missionPath in the request body".to_string(),
        ));
    }
    Ok(PathBuf::from(format!(
        "examples/missions/{}.yaml",
        run.mission
    )))
}

async fn ensure_run_exists(state: &AppState, id: MissionRunId) -> ApiResult<()> {
    let run = MissionRunRepo::new(state.pool()).get(id).await?;
    if run.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(())
}
