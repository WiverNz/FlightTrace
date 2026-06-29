//! Handlers under `/api/runs`.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use uuid::Uuid;

use flighttrace_core::MissionRunId;
use flighttrace_storage::{
    AssertionResultRepo, FailureReportRepo, MissionRunRepo, NormalizedEventRepo,
};

use crate::dto::{AssertionResultDto, FailureReportDto, MissionRunDto, NormalizedEventDto};
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

async fn ensure_run_exists(state: &AppState, id: MissionRunId) -> ApiResult<()> {
    let run = MissionRunRepo::new(state.pool()).get(id).await?;
    if run.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(())
}
