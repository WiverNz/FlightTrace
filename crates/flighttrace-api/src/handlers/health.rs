//! `GET /health` — liveness probe.

use axum::Json;
use serde::Serialize;

/// Crate version baked in at compile time. Reported as the `version` field.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON body for `GET /health`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    /// Always `"ok"` while the process is serving requests.
    pub status: &'static str,
    /// `flighttrace-api` crate version (e.g. `"0.1.0"`).
    pub version: &'static str,
}

/// `GET /health` returns `{"status":"ok","version":"<crate version>"}`.
pub async fn get() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: VERSION,
    })
}
