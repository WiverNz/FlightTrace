//! `heartbeat_within_sec` — bound the gap between consecutive HEARTBEATs.

use flighttrace_core::{AssertionOutcome, Evidence, NormalizedEvent, NormalizedEventKind};

use crate::checks::{CheckOutput, Window};

/// Pass when every consecutive pair of HEARTBEAT events inside `window` is
/// separated by ≤ `max_gap_sec`, and the first HEARTBEAT arrives within
/// `max_gap_sec` of the window start.
pub fn check_heartbeat_within_sec(
    max_gap_sec: f64,
    window: Window,
    events: &[NormalizedEvent],
) -> CheckOutput {
    let heartbeats: Vec<&NormalizedEvent> = events
        .iter()
        .filter(|e| window.contains(e.at))
        .filter(|e| matches!(e.kind, NormalizedEventKind::HeartbeatReceived))
        .collect();

    if heartbeats.is_empty() {
        return CheckOutput {
            outcome: AssertionOutcome::Fail,
            detail: format!("No HEARTBEAT received within {max_gap_sec:.1}s of step start."),
            evidence: vec![Evidence::Note(
                "Expected at least one HEARTBEAT during the step window.".into(),
            )],
            evaluated_at: window.end,
        };
    }

    // Check arrival latency of the first heartbeat.
    let first = heartbeats[0];
    let first_delay = (first.at.0 - window.start.0).as_seconds_f64();
    if first_delay > max_gap_sec {
        return CheckOutput {
            outcome: AssertionOutcome::Fail,
            detail: format!(
                "First HEARTBEAT arrived {first_delay:.1}s after step start (limit {max_gap_sec:.1}s)."
            ),
            evidence: vec![Evidence::Event(first.id)],
            evaluated_at: first.at,
        };
    }

    // Check inter-arrival gaps.
    let mut max_gap = first_delay;
    let mut worst_event = first;
    for pair in heartbeats.windows(2) {
        let gap = (pair[1].at.0 - pair[0].at.0).as_seconds_f64();
        if gap > max_gap {
            max_gap = gap;
            worst_event = pair[1];
        }
    }

    if max_gap > max_gap_sec {
        CheckOutput {
            outcome: AssertionOutcome::Fail,
            detail: format!("Largest HEARTBEAT gap was {max_gap:.1}s (limit {max_gap_sec:.1}s)."),
            evidence: vec![Evidence::Event(worst_event.id)],
            evaluated_at: worst_event.at,
        }
    } else {
        CheckOutput {
            outcome: AssertionOutcome::Pass,
            detail: format!(
                "Heartbeat received {first_delay:.1}s after step start (limit {max_gap_sec:.1}s)."
            ),
            evidence: vec![Evidence::Event(first.id)],
            evaluated_at: first.at,
        }
    }
}
