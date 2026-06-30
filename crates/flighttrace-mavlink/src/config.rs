//! Configuration for the MAVLink ingestion listener.
//!
//! The CLI surfaces only two knobs (`--mavlink <endpoint>` and
//! `--mavlink-dialect`); the timeouts in [`MavlinkConfig`] use the defaults
//! issue 4 specifies but are exposed here so tests can shrink them.

use std::time::Duration;

use crate::error::{MavlinkIngestError, MavlinkIngestResult};

/// MAVLink dialect choice. PX4 does not have a separate dialect — it speaks
/// `common`, so [`Dialect::Px4`] resolves to the `common` message set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// MAVLink `common` dialect — works for PX4 and ArduPilot's shared messages.
    Common,
    /// MAVLink `ardupilotmega` dialect — superset of `common` with ArduPilot extensions.
    ArdupilotMega,
    /// PX4 — same wire format as `common`.
    Px4,
}

impl Dialect {
    /// Parse a CLI-friendly dialect name.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "common" => Some(Self::Common),
            "ardupilotmega" | "ardu" | "ardupilot" => Some(Self::ArdupilotMega),
            "px4" => Some(Self::Px4),
            _ => None,
        }
    }

    /// Short name used in logs.
    pub fn name(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::ArdupilotMega => "ardupilotmega",
            Self::Px4 => "px4",
        }
    }
}

/// Tunables for the ingestion loop.
///
/// The defaults match issue 4: `session_idle_timeout=30s`,
/// `heartbeat_timeout=3s`, `telemetry_gap_threshold=2s`,
/// `telemetry_batch_interval=250ms`.
#[derive(Debug, Clone)]
pub struct MavlinkConfig {
    /// Raw endpoint string for diagnostics (`udp://host:port`).
    pub endpoint: String,
    /// Parsed UDP bind / connect address (`host:port`, no scheme).
    pub udp_addr: String,
    /// Dialect to decode messages with.
    pub dialect: Dialect,
    /// Maximum gap between HEARTBEATs before logging a stale-link warning.
    pub heartbeat_timeout: Duration,
    /// Telemetry stream gap that triggers a single `TelemetryGapDetected` event.
    pub telemetry_gap_threshold: Duration,
    /// Idle time after which the current session is closed (status = Succeeded
    /// unless a MissionFailed event already exists).
    pub session_idle_timeout: Duration,
    /// How often the listener flushes the in-flight telemetry batch.
    pub telemetry_batch_interval: Duration,
}

impl MavlinkConfig {
    /// Build a config from a CLI endpoint + dialect.
    pub fn new(endpoint: &str, dialect: Dialect) -> MavlinkIngestResult<Self> {
        let udp_addr = parse_endpoint(endpoint)?;
        Ok(Self {
            endpoint: endpoint.to_string(),
            udp_addr,
            dialect,
            heartbeat_timeout: Duration::from_secs(3),
            telemetry_gap_threshold: Duration::from_secs(2),
            session_idle_timeout: Duration::from_secs(30),
            telemetry_batch_interval: Duration::from_millis(250),
        })
    }

    /// Address string the `mavlink` crate's `udpin:` connection expects.
    pub fn mavlink_connect_address(&self) -> String {
        format!("udpin:{}", self.udp_addr)
    }
}

/// Parse a `udp://host:port` endpoint into a bare `host:port` string.
///
/// Accepts uppercase / mixed-case schemes; rejects anything that isn't UDP.
pub fn parse_endpoint(endpoint: &str) -> MavlinkIngestResult<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err(MavlinkIngestError::InvalidEndpoint(
            endpoint.to_string(),
            "endpoint is empty".to_string(),
        ));
    }

    let lower = trimmed.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("udp://") {
        &trimmed[trimmed.len() - rest.len()..]
    } else if let Some(rest) = lower.strip_prefix("udpin://") {
        &trimmed[trimmed.len() - rest.len()..]
    } else {
        return Err(MavlinkIngestError::InvalidEndpoint(
            endpoint.to_string(),
            "must start with `udp://`".to_string(),
        ));
    };

    // Strip any trailing path / query; we only want host:port.
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host_port.is_empty() {
        return Err(MavlinkIngestError::InvalidEndpoint(
            endpoint.to_string(),
            "missing host:port".to_string(),
        ));
    }
    if !host_port.contains(':') {
        return Err(MavlinkIngestError::InvalidEndpoint(
            endpoint.to_string(),
            "missing `:port`".to_string(),
        ));
    }

    Ok(host_port.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loopback_endpoint() {
        assert_eq!(
            parse_endpoint("udp://127.0.0.1:14550").unwrap(),
            "127.0.0.1:14550"
        );
    }

    #[test]
    fn parses_wildcard_endpoint() {
        assert_eq!(
            parse_endpoint("udp://0.0.0.0:14550").unwrap(),
            "0.0.0.0:14550"
        );
    }

    #[test]
    fn parses_uppercase_scheme() {
        assert_eq!(
            parse_endpoint("UDP://127.0.0.1:14550").unwrap(),
            "127.0.0.1:14550"
        );
    }

    #[test]
    fn rejects_tcp_scheme() {
        let err = parse_endpoint("tcp://127.0.0.1:14550").unwrap_err();
        assert!(matches!(err, MavlinkIngestError::InvalidEndpoint(..)));
    }

    #[test]
    fn rejects_missing_port() {
        let err = parse_endpoint("udp://127.0.0.1").unwrap_err();
        assert!(matches!(err, MavlinkIngestError::InvalidEndpoint(..)));
    }

    #[test]
    fn rejects_empty() {
        let err = parse_endpoint("").unwrap_err();
        assert!(matches!(err, MavlinkIngestError::InvalidEndpoint(..)));
    }

    #[test]
    fn dialect_parse_defaults() {
        assert_eq!(Dialect::parse("common"), Some(Dialect::Common));
        assert_eq!(
            Dialect::parse("ardupilotmega"),
            Some(Dialect::ArdupilotMega)
        );
        assert_eq!(Dialect::parse("PX4"), Some(Dialect::Px4));
        assert_eq!(Dialect::parse("nope"), None);
    }

    #[test]
    fn mavlink_address_uses_udpin_prefix() {
        let cfg = MavlinkConfig::new("udp://127.0.0.1:14550", Dialect::Common).unwrap();
        assert_eq!(cfg.mavlink_connect_address(), "udpin:127.0.0.1:14550");
    }
}
