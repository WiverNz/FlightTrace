//! `POST /api/demo/run` — persist a fresh, freshly-timestamped demo run.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use flighttrace_core::{
    AssertionResult, AssertionResultId, EventId, Evidence, FailureReport, FailureReportId,
    MissionRun, MissionRunId, NormalizedEvent, PhaseTransition, TelemetrySample, Timestamp,
};
use flighttrace_demo::generate_failed_takeoff_run;
use flighttrace_storage::{RunBundle, persist_bundle};

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoCreated {
    id: String,
}

/// `POST /api/demo/run` — create and persist a new demo failed run.
///
/// The deterministic demo bundle is rewritten so every id is fresh and the
/// timeline starts at "now". Evidence that points at a specific event id is
/// remapped to the newly-minted id so foreign links inside the bundle stay
/// intact.
pub async fn create(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let bundle = build_fresh_demo_bundle();
    let id = persist_bundle(state.pool(), &bundle).await?;
    Ok((
        StatusCode::CREATED,
        Json(DemoCreated { id: id.to_string() }),
    ))
}

fn build_fresh_demo_bundle() -> RunBundle {
    let demo = generate_failed_takeoff_run();
    let now = OffsetDateTime::now_utc();
    let delta = now - demo.run.started_at.0;

    let new_run_id = MissionRunId::new(Uuid::new_v4());
    let new_report_id = FailureReportId::new(Uuid::new_v4());

    let mut event_id_map: HashMap<EventId, EventId> = HashMap::with_capacity(demo.events.len());
    for event in &demo.events {
        event_id_map.insert(event.id, EventId::new(Uuid::new_v4()));
    }

    let events: Vec<NormalizedEvent> = demo
        .events
        .into_iter()
        .map(|event| NormalizedEvent {
            id: event_id_map[&event.id],
            run_id: new_run_id,
            at: shift(event.at, delta),
            kind: event.kind,
        })
        .collect();

    let telemetry: Vec<TelemetrySample> = demo
        .telemetry
        .into_iter()
        .map(|sample| TelemetrySample {
            run_id: new_run_id,
            at: shift(sample.at, delta),
            altitude_m: sample.altitude_m,
            vertical_speed_mps: sample.vertical_speed_mps,
            mode: sample.mode,
            armed: sample.armed,
            battery_remaining_pct: sample.battery_remaining_pct,
        })
        .collect();

    let assertions: Vec<AssertionResult> = demo
        .assertions
        .into_iter()
        .map(|result| AssertionResult {
            id: AssertionResultId::new(Uuid::new_v4()),
            run_id: new_run_id,
            name: result.name,
            outcome: result.outcome,
            detail: result.detail,
            evaluated_at: shift(result.evaluated_at, delta),
        })
        .collect();

    let phases: Vec<PhaseTransition> = demo
        .run
        .phases
        .into_iter()
        .map(|phase| PhaseTransition {
            at: shift(phase.at, delta),
            phase: phase.phase,
        })
        .collect();

    let run = MissionRun {
        id: new_run_id,
        vehicle: demo.run.vehicle,
        mission: demo.run.mission,
        source: demo.run.source,
        status: demo.run.status,
        failure_reason: demo.run.failure_reason,
        started_at: shift(demo.run.started_at, delta),
        ended_at: demo.run.ended_at.map(|ts| shift(ts, delta)),
        phases,
    };

    let evidence: Vec<Evidence> = demo
        .failure_report
        .evidence
        .into_iter()
        .map(|ev| remap_evidence(ev, &event_id_map, delta))
        .collect();

    let failure_report = FailureReport {
        id: new_report_id,
        run_id: new_run_id,
        reason: demo.failure_report.reason,
        summary: demo.failure_report.summary,
        evidence,
        likely_cause: demo.failure_report.likely_cause,
        generated_at: shift(demo.failure_report.generated_at, delta),
    };

    RunBundle {
        run,
        events,
        telemetry,
        assertions,
        failure_report,
    }
}

fn shift(ts: Timestamp, delta: Duration) -> Timestamp {
    Timestamp(ts.0 + delta)
}

fn remap_evidence(
    evidence: Evidence,
    event_id_map: &HashMap<EventId, EventId>,
    delta: Duration,
) -> Evidence {
    match evidence {
        Evidence::Note(text) => Evidence::Note(text),
        Evidence::Event(id) => Evidence::Event(event_id_map.get(&id).copied().unwrap_or(id)),
        Evidence::Sample { at, field, value } => Evidence::Sample {
            at: shift(at, delta),
            field,
            value,
        },
        Evidence::Statustext { at, text } => Evidence::Statustext {
            at: shift(at, delta),
            text,
        },
    }
}
