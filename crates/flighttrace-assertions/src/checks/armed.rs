//! `armed: <bool>` — the vehicle must reach the requested armed state.

use flighttrace_core::{AssertionOutcome, Evidence, NormalizedEvent, NormalizedEventKind};

use crate::checks::{CheckOutput, Window};

/// Pass when an `ArmedChanged { armed: <expected> }` event lands inside
/// `window`. The earliest matching event is cited as evidence.
pub fn check_armed(expected: bool, window: Window, events: &[NormalizedEvent]) -> CheckOutput {
    let matching = events.iter().filter(|e| window.contains(e.at)).find(
        |e| matches!(e.kind, NormalizedEventKind::ArmedChanged { armed } if armed == expected),
    );

    match matching {
        Some(event) => {
            let delay = (event.at.0 - window.start.0).as_seconds_f64();
            let verb = if expected { "armed" } else { "disarmed" };
            CheckOutput {
                outcome: AssertionOutcome::Pass,
                detail: format!("Vehicle {verb} {delay:.1}s after step start."),
                evidence: vec![Evidence::Event(event.id)],
                evaluated_at: event.at,
            }
        }
        None => {
            let verb = if expected { "arm" } else { "disarm" };
            CheckOutput {
                outcome: AssertionOutcome::Fail,
                detail: format!("Vehicle did not {verb} within the step window."),
                evidence: vec![Evidence::Note(format!(
                    "No ArmedChanged event with armed={expected} during the step window."
                ))],
                evaluated_at: window.end,
            }
        }
    }
}
