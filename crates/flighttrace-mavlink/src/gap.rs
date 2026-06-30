//! Telemetry-gap detection.
//!
//! The listener's tick loop calls [`check_gap`] every iteration. If we've
//! observed at least one telemetry message and the most recent one is older
//! than `telemetry_gap_threshold`, and we have not already emitted a gap event
//! for this outage, we return a `TelemetryGapDetected` event with the
//! observed gap in seconds. The next telemetry-bearing message clears
//! `session.gap_emitted_at` so future outages can be reported again.

use std::time::{Duration, Instant};

use flighttrace_core::NormalizedEventKind;

use crate::session::SessionState;

/// Returns `Some(NormalizedEventKind::TelemetryGapDetected { gap_sec })`
/// once per outage, or `None` if no gap has occurred / the gap has already
/// been emitted.
pub fn check_gap(
    session: &mut SessionState,
    now: Instant,
    threshold: Duration,
) -> Option<NormalizedEventKind> {
    let last = session.last_telemetry_message_at?;
    let gap = now.saturating_duration_since(last);
    if gap < threshold {
        return None;
    }
    if session.gap_emitted_at.is_some() {
        return None;
    }
    session.gap_emitted_at = Some(now);
    Some(NormalizedEventKind::TelemetryGapDetected {
        gap_sec: gap.as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_gap_when_no_telemetry_yet() {
        let mut s = SessionState::default();
        assert!(check_gap(&mut s, Instant::now(), Duration::from_secs(2)).is_none());
    }

    #[test]
    fn no_gap_when_below_threshold() {
        let mut s = SessionState::default();
        let start = Instant::now();
        s.note_telemetry(start);
        let result = check_gap(
            &mut s,
            start + Duration::from_millis(500),
            Duration::from_secs(2),
        );
        assert!(result.is_none());
    }

    #[test]
    fn emits_gap_once_above_threshold() {
        let mut s = SessionState::default();
        let start = Instant::now();
        s.note_telemetry(start);
        let now = start + Duration::from_secs(3);
        let ev = check_gap(&mut s, now, Duration::from_secs(2)).expect("gap event");
        match ev {
            NormalizedEventKind::TelemetryGapDetected { gap_sec } => {
                assert!((gap_sec - 3.0).abs() < 0.05);
            }
            _ => panic!("unexpected event"),
        }
        // Second tick within the same outage does not re-emit.
        assert!(check_gap(&mut s, now + Duration::from_secs(1), Duration::from_secs(2)).is_none());
    }

    #[test]
    fn next_telemetry_resets_gap_tracker() {
        let mut s = SessionState::default();
        let start = Instant::now();
        s.note_telemetry(start);
        let outage_at = start + Duration::from_secs(3);
        let _ = check_gap(&mut s, outage_at, Duration::from_secs(2));

        // Telemetry returns.
        s.note_telemetry(outage_at + Duration::from_millis(100));
        // Another outage later — should emit again.
        let next_outage = outage_at + Duration::from_secs(5);
        assert!(check_gap(&mut s, next_outage, Duration::from_secs(2)).is_some());
    }
}
