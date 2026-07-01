//! HTTP API for FlightTrace built on Axum.
//!
//! The crate exposes a single [`router::build`] function that takes a
//! storage-backed [`state::AppState`] and returns an `axum::Router`. The CLI
//! `serve` subcommand owns the runtime, address binding, and pool wiring;
//! this crate stays runtime-agnostic so it can be tested with
//! `tower::ServiceExt::oneshot`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod dto;
pub mod error;
pub mod handlers;
pub mod router;
pub mod state;

pub use error::{ApiError, ApiResult};
pub use router::{DEFAULT_CORS_ORIGIN, LOOPBACK_CORS_ORIGIN, build, build_with_static};
pub use state::AppState;
