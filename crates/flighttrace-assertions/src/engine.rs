//! Top-level evaluator that turns a [`MissionSpec`] + a run's events and
//! samples into [`AssertionResult`]s and (when something failed) a
//! [`FailureReport`].
//!
//! The evaluator is pure: same inputs in, same outputs out. There is no I/O,
//! no clock dependency, and no shared state — callers persist the results
//! themselves.

use time::Duration;

use flighttrace_core::{
    AssertionResult, FailureReport, MissionRun, NormalizedEvent, TelemetrySample, Timestamp,
};

use crate::checks::{
    CheckOutput, Window, altitude::check_altitude_at_least_m, armed::check_armed,
    gap::check_no_telemetry_gap_longer_than_sec, heartbeat::check_heartbeat_within_sec,
    mode::check_mode_not_changes,
};
use crate::failure::{CheckKind, assertion_id_for, build_failure_report};
use crate::schema::{MissionSpec, StepSpec};

/// All the read-only context a mission spec needs to be evaluated.
#[derive(Debug)]
pub struct EvaluationInput<'a> {
    /// The mission run being evaluated.
    pub run: &'a MissionRun,
    /// Every normalized event recorded for this run.
    pub events: &'a [NormalizedEvent],
    /// Every telemetry sample recorded for this run.
    pub samples: &'a [TelemetrySample],
}

/// What the engine produced for a single run.
#[derive(Debug, Clone)]
pub struct EvaluationOutput {
    /// One [`AssertionResult`] per check declared in the spec, in spec order.
    pub results: Vec<AssertionResult>,
    /// Present iff at least one assertion failed.
    pub failure_report: Option<FailureReport>,
}

/// Internal record of a check that failed, kept in deterministic order so the
/// failure-report builder can pick a primary cause.
#[derive(Debug, Clone)]
pub struct FailedCheck {
    /// Spec-order index of the step the check belongs to.
    pub step_idx: usize,
    /// Which check failed.
    pub kind: CheckKind,
    /// Verdict, detail, evidence, and evaluated_at returned by the check.
    pub output: CheckOutput,
}

/// Evaluate `spec` against `input`. See module docs for semantics.
pub fn evaluate(spec: &MissionSpec, input: &EvaluationInput<'_>) -> EvaluationOutput {
    let mut results: Vec<AssertionResult> = Vec::new();
    let mut failures: Vec<FailedCheck> = Vec::new();

    for (step_idx, step) in spec.steps.iter().enumerate() {
        let window = window_for_step(input, step);
        evaluate_step(input, step, step_idx, window, &mut results, &mut failures);
    }

    let failure_report = build_failure_report(input.run, input.events, &failures);

    EvaluationOutput {
        results,
        failure_report,
    }
}

fn evaluate_step(
    input: &EvaluationInput<'_>,
    step: &StepSpec,
    step_idx: usize,
    window: Window,
    results: &mut Vec<AssertionResult>,
    failures: &mut Vec<FailedCheck>,
) {
    // Order is fixed by CheckKind::priority so the failure-report's choice of
    // primary cause is deterministic.
    if let Some(threshold) = step.expect.heartbeat_within_sec {
        let out = check_heartbeat_within_sec(threshold, window, input.events);
        record(
            input,
            step,
            step_idx,
            CheckKind::HeartbeatWithinSec,
            out,
            results,
            failures,
        );
    }
    if let Some(expected) = step.expect.armed {
        let out = check_armed(expected, window, input.events);
        record(
            input,
            step,
            step_idx,
            CheckKind::Armed,
            out,
            results,
            failures,
        );
    }
    if let Some(target) = step.expect.altitude_at_least_m {
        let out = check_altitude_at_least_m(target, window, input.events, input.samples);
        record(
            input,
            step,
            step_idx,
            CheckKind::AltitudeAtLeastM,
            out,
            results,
            failures,
        );
    }
    if let Some(enabled) = step.expect.mode_not_changes {
        let out = check_mode_not_changes(enabled, window, input.events);
        record(
            input,
            step,
            step_idx,
            CheckKind::ModeNotChanges,
            out,
            results,
            failures,
        );
    }
    if let Some(threshold) = step.expect.no_telemetry_gap_longer_than_sec {
        let out = check_no_telemetry_gap_longer_than_sec(threshold, window, input.events);
        record(
            input,
            step,
            step_idx,
            CheckKind::NoTelemetryGap,
            out,
            results,
            failures,
        );
    }
}

fn record(
    input: &EvaluationInput<'_>,
    step: &StepSpec,
    step_idx: usize,
    kind: CheckKind,
    output: CheckOutput,
    results: &mut Vec<AssertionResult>,
    failures: &mut Vec<FailedCheck>,
) {
    let name = format!("{}.{}", step.name, kind.as_str());
    let id = assertion_id_for(input.run.id, step_idx, kind);
    results.push(AssertionResult {
        id,
        run_id: input.run.id,
        name,
        outcome: output.outcome,
        detail: output.detail.clone(),
        evaluated_at: output.evaluated_at,
    });
    if matches!(output.outcome, flighttrace_core::AssertionOutcome::Fail) {
        failures.push(FailedCheck {
            step_idx,
            kind,
            output,
        });
    }
}

/// Compute the time window a step's checks operate over.
///
/// Without `within_sec`, the window stretches to the run's `ended_at` (if
/// set) or to the latest event/sample timestamp — whichever is later — so an
/// in-progress run still has a sensible upper bound.
fn window_for_step(input: &EvaluationInput<'_>, step: &StepSpec) -> Window {
    let start = input.run.started_at;
    let end = match step.expect.within_sec {
        Some(secs) => Timestamp(start.0 + Duration::seconds_f64(secs)),
        None => default_end(input),
    };
    Window { start, end }
}

fn default_end(input: &EvaluationInput<'_>) -> Timestamp {
    let max_event = input.events.iter().map(|e| e.at.0).max();
    let max_sample = input.samples.iter().map(|s| s.at.0).max();
    let max_observation = match (max_event, max_sample) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    let ended = input.run.ended_at.map(|t| t.0);
    let upper = match (ended, max_observation) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => input.run.started_at.0,
    };
    Timestamp(upper)
}
