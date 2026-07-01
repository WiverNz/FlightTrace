//! Router assembly.

use std::path::PathBuf;

use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{any, get, post};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::handlers::{demo, health, runs};
use crate::state::AppState;

/// Default CORS origin allowed in dev (Vite's default port).
pub const DEFAULT_CORS_ORIGIN: &str = "http://localhost:5173";
/// IPv4 loopback CORS origin used by the VS Code browser launch profiles.
pub const LOOPBACK_CORS_ORIGIN: &str = "http://127.0.0.1:5173";

/// Build the API-only router. Equivalent to [`build_with_static`] with no
/// static-file directory.
pub fn build(state: AppState) -> Router {
    build_with_static(state, None)
}

/// Build the router with optional static-file serving.
///
/// When `web_dir` is `Some(dir)`, unmatched non-API requests fall through to
/// `ServeDir::new(dir)` and, failing a real file match, to
/// `ServeFile::new(dir/index.html)` — the standard SPA fallback so that
/// client-side routes like `/runs/<id>` still resolve to the bundled
/// `index.html`.
///
/// `/api/*` paths never fall through: a catch-all route returns the standard
/// 404 JSON envelope for any unknown API URL, keeping the API contract
/// unchanged regardless of static-serving mode.
pub fn build_with_static(state: AppState, web_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(vec![
            DEFAULT_CORS_ORIGIN
                .parse::<HeaderValue>()
                .expect("valid CORS origin"),
            LOOPBACK_CORS_ORIGIN
                .parse::<HeaderValue>()
                .expect("valid loopback CORS origin"),
        ])
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    let mut router = Router::new()
        .route("/health", get(health::get))
        .route("/api/runs", get(runs::list))
        .route("/api/runs/:id", get(runs::get))
        .route("/api/runs/:id/events", get(runs::events))
        .route("/api/runs/:id/assertions", get(runs::assertions))
        .route("/api/runs/:id/telemetry", get(runs::telemetry))
        .route("/api/runs/:id/failure-report", get(runs::failure_report))
        .route("/api/runs/:id/reevaluate", post(runs::reevaluate))
        .route("/api/demo/run", post(demo::create))
        .route("/api/*rest", any(api_not_found));

    if let Some(dir) = web_dir {
        let index = dir.join("index.html");
        let serve = ServeDir::new(&dir).fallback(ServeFile::new(index));
        router = router.fallback_service(serve);
    }

    router
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Fallback handler for `/api/*` paths that don't match a real route. Keeps
/// unknown API URLs returning the JSON error envelope instead of the SPA
/// `index.html`.
async fn api_not_found() -> ApiError {
    ApiError::NotFound
}
