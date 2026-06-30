//! Router assembly.

use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::handlers::{demo, health, runs};
use crate::state::AppState;

/// Default CORS origin allowed in dev (Vite's default port).
pub const DEFAULT_CORS_ORIGIN: &str = "http://localhost:5173";

/// Build the full FlightTrace API router with CORS and tracing layers.
pub fn build(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            DEFAULT_CORS_ORIGIN
                .parse::<HeaderValue>()
                .expect("valid CORS origin"),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    Router::new()
        .route("/health", get(health::get))
        .route("/api/runs", get(runs::list))
        .route("/api/runs/:id", get(runs::get))
        .route("/api/runs/:id/events", get(runs::events))
        .route("/api/runs/:id/assertions", get(runs::assertions))
        .route("/api/runs/:id/telemetry", get(runs::telemetry))
        .route("/api/runs/:id/failure-report", get(runs::failure_report))
        .route("/api/runs/:id/reevaluate", post(runs::reevaluate))
        .route("/api/demo/run", post(demo::create))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
