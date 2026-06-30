//! `no_telemetry_gap_longer_than_sec` — telemetry must keep flowing.

use flighttrace_core::{AssertionOutcome, Evidence, NormalizedEvent, NormalizedEventKind};

use crate::checks::{CheckOutput, Window};

/// Pass when no `TelemetryGapDetected` event inside `window` reports a gap
/// larger than `max_gap_sec`. The MAVLink listener emits these events when
/// telemetry stalls; the demo synthesizes them in the same way for testing.
pub fn check_no_telemetry_gap_longer_than_sec(
    max_gap_sec: f64,
    window: Window,
    events: &[NormalizedEvent],
) -> CheckOutput {
    let mut worst: Option<(f64, &NormalizedEvent)> = None;
    for event in events.iter().filter(|e| window.contains(e.at)) {
        if let NormalizedEventKind::TelemetryGapDetected { gap_sec } = event.kind
            && worst.is_none_or(|(prev, _)| gap_sec > prev)
        {
            worst = Some((gap_sec, event));
        }
    }

    match worst {
        Some((gap, event)) if gap > max_gap_sec => CheckOutput {
            outcome: AssertionOutcome::Fail,
            detail: format!("Telemetry gap of {gap:.1}s exceeded the {max_gap_sec:.1}s budget."),
            evidence: vec![Evidence::Event(event.id)],
            evaluated_at: event.at,
        },
        Some((gap, event)) => CheckOutput {
            outcome: AssertionOutcome::Pass,
            detail: format!("Largest telemetry gap was {gap:.1}s (budget {max_gap_sec:.1}s)."),
            evidence: vec![Evidence::Event(event.id)],
            evaluated_at: event.at,
        },
        None => CheckOutput {
            outcome: AssertionOutcome::Pass,
            detail: format!("No telemetry gaps detected (budget {max_gap_sec:.1}s)."),
            evidence: Vec::new(),
            evaluated_at: window.end,
        },
    }
}
