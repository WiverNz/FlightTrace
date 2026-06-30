//! `mode_not_changes` — the flight mode must remain stable across the window.

use flighttrace_core::{AssertionOutcome, Evidence, NormalizedEvent, NormalizedEventKind};

use crate::checks::{CheckOutput, Window};

/// Pass when every `ModeChanged` event inside `window` reports the same mode
/// value (i.e. the mode is set but never changes underneath the step).
///
/// `enabled = false` short-circuits to a Pass — the YAML author opted out.
pub fn check_mode_not_changes(
    enabled: bool,
    window: Window,
    events: &[NormalizedEvent],
) -> CheckOutput {
    if !enabled {
        return CheckOutput {
            outcome: AssertionOutcome::Pass,
            detail: "mode_not_changes disabled by spec.".into(),
            evidence: Vec::new(),
            evaluated_at: window.end,
        };
    }

    let mut current_mode: Option<&str> = None;
    let mut first_event_id = None;
    for event in events.iter().filter(|e| window.contains(e.at)) {
        if let NormalizedEventKind::ModeChanged { mode } = &event.kind {
            match current_mode {
                None => {
                    current_mode = Some(mode.as_str());
                    first_event_id = Some(event.id);
                }
                Some(prev) if prev == mode.as_str() => {
                    // Repeated set — still stable.
                }
                Some(prev) => {
                    return CheckOutput {
                        outcome: AssertionOutcome::Fail,
                        detail: format!("Flight mode changed from {prev} to {mode}."),
                        evidence: vec![Evidence::Event(event.id)],
                        evaluated_at: event.at,
                    };
                }
            }
        }
    }

    let detail = match current_mode {
        Some(mode) => format!("Flight mode stayed on {mode} during the step window."),
        None => "No mode changes during the step window.".into(),
    };
    let evidence = first_event_id
        .map(|id| vec![Evidence::Event(id)])
        .unwrap_or_default();

    CheckOutput {
        outcome: AssertionOutcome::Pass,
        detail,
        evidence,
        evaluated_at: window.end,
    }
}
