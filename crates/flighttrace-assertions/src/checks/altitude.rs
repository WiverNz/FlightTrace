//! `altitude_at_least_m` — vehicle must reach a target altitude in the window.

use flighttrace_core::{
    AssertionOutcome, Evidence, NormalizedEvent, NormalizedEventKind, TelemetrySample,
};

use crate::checks::{CheckOutput, Window};

/// Pass when either an `AltitudeUpdated` event or a telemetry sample inside
/// `window` reports an altitude ≥ `target_m`. Otherwise fail and cite the
/// maximum altitude actually reached.
pub fn check_altitude_at_least_m(
    target_m: f64,
    window: Window,
    events: &[NormalizedEvent],
    samples: &[TelemetrySample],
) -> CheckOutput {
    // Sweep events first so a passing verdict points at the precise altitude
    // event that crossed the threshold.
    let mut max_event_alt: Option<(f64, &NormalizedEvent)> = None;
    for event in events.iter().filter(|e| window.contains(e.at)) {
        if let NormalizedEventKind::AltitudeUpdated { altitude_m } = event.kind {
            if altitude_m >= target_m {
                return CheckOutput {
                    outcome: AssertionOutcome::Pass,
                    detail: format!("Reached {altitude_m:.1} m (target {target_m:.1} m)."),
                    evidence: vec![Evidence::Event(event.id)],
                    evaluated_at: event.at,
                };
            }
            if max_event_alt.is_none_or(|(m, _)| altitude_m > m) {
                max_event_alt = Some((altitude_m, event));
            }
        }
    }

    // Fall back to telemetry samples.
    let mut max_sample: Option<&TelemetrySample> = None;
    for sample in samples.iter().filter(|s| window.contains(s.at)) {
        if sample.altitude_m >= target_m {
            return CheckOutput {
                outcome: AssertionOutcome::Pass,
                detail: format!(
                    "Reached {altitude:.1} m (target {target_m:.1} m).",
                    altitude = sample.altitude_m,
                ),
                evidence: vec![Evidence::Sample {
                    at: sample.at,
                    field: "altitude_m".into(),
                    value: format!("{:.1}", sample.altitude_m),
                }],
                evaluated_at: sample.at,
            };
        }
        if max_sample.is_none_or(|s| sample.altitude_m > s.altitude_m) {
            max_sample = Some(sample);
        }
    }

    // Failure path — report the highest altitude we saw and the evidence for it.
    let max_observed = max_event_alt
        .map(|(m, _)| m)
        .or(max_sample.map(|s| s.altitude_m))
        .unwrap_or(0.0);

    let mut evidence: Vec<Evidence> = Vec::new();
    if let Some((_, ev)) = max_event_alt {
        evidence.push(Evidence::Event(ev.id));
    }
    if let Some(sample) = max_sample {
        evidence.push(Evidence::Sample {
            at: sample.at,
            field: "altitude_m".into(),
            value: format!("{:.1}", sample.altitude_m),
        });
    }
    // Cite vehicle warnings observed during the climb — the engine surfaces
    // these as `Evidence::Statustext` so the UI's evidence list renders the
    // message text directly rather than just an event reference.
    for event in events.iter().filter(|e| window.contains(e.at)) {
        if let NormalizedEventKind::StatustextReceived { severity, text } = &event.kind
            && *severity <= 4
        {
            evidence.push(Evidence::Statustext {
                at: event.at,
                text: text.clone(),
            });
        }
    }
    if evidence.is_empty() {
        evidence.push(Evidence::Note(
            "No altitude observations recorded during the step window.".into(),
        ));
    }

    CheckOutput {
        outcome: AssertionOutcome::Fail,
        detail: format!("max_altitude={max_observed:.1}m target={target_m:.1}m"),
        evidence,
        evaluated_at: window.end,
    }
}
