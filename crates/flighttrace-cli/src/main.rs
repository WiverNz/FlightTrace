//! FlightTrace CLI entrypoint.
//!
//! `demo` prints the deterministic `TAKEOFF_TIMEOUT` bundle to stdout as JSON.
//! `serve` opens (or creates) a SQLite database, runs the embedded
//! migrations, and starts the Axum API on the requested address. Pass
//! `--mavlink udp://host:port` to additionally bind a read-only MAVLink UDP
//! listener (issue 4) that persists live SITL traffic into the same database.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use flighttrace_api::{AppState, build};
use flighttrace_assertions::load_mission_file;
use flighttrace_mavlink::{Dialect, MavlinkConfig, run_ingestion};
use flighttrace_storage::connect;

#[derive(Debug, Parser)]
#[command(
    name = "flighttrace",
    version,
    about = "FlightTrace — a mission debugger for PX4/ArduPilot autonomous missions.",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the deterministic TAKEOFF_TIMEOUT demo mission run as pretty JSON.
    Demo,
    /// Start the FlightTrace HTTP API backed by a local SQLite database.
    Serve {
        /// Address to bind the HTTP server to.
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: SocketAddr,
        /// Path to the SQLite database file. Parent directory is created if missing.
        #[arg(long, default_value = "./data/flighttrace.db")]
        database: PathBuf,
        /// Optional MAVLink UDP endpoint to ingest read-only telemetry from
        /// (e.g. `udp://127.0.0.1:14550`). Absent = demo-only mode, exactly
        /// like before issue 4.
        #[arg(long)]
        mavlink: Option<String>,
        /// MAVLink dialect to decode incoming traffic with. PX4 also speaks
        /// `common`, so the default is fine for both stacks.
        #[arg(long, default_value = "common")]
        mavlink_dialect: String,
        /// Path to a mission YAML to evaluate against when each MAVLink
        /// session ends. Without it, ingestion still classifies sessions
        /// from the in-band `MissionFailed` flag but produces no
        /// `FailureReport`.
        #[arg(long)]
        mission: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    match cli.command {
        Command::Demo => run_demo(),
        Command::Serve {
            bind,
            database,
            mavlink,
            mavlink_dialect,
            mission,
        } => run_serve(
            bind,
            &database,
            mavlink.as_deref(),
            &mavlink_dialect,
            mission.as_deref(),
        ),
    }
}

fn init_tracing() {
    // Logs go to stderr so stdout stays a clean JSON pipe for `demo`.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn run_demo() -> Result<()> {
    info!("generating deterministic TAKEOFF_TIMEOUT demo run");
    let bundle = flighttrace_demo::generate_failed_takeoff_run();
    let json =
        serde_json::to_string_pretty(&bundle).context("serialize demo bundle as pretty JSON")?;
    println!("{json}");
    Ok(())
}

fn run_serve(
    bind: SocketAddr,
    database: &Path,
    mavlink_endpoint: Option<&str>,
    mavlink_dialect: &str,
    mission_path: Option<&Path>,
) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    // Parse the mission spec up front so a typo fails on startup rather than
    // on session close, hours later.
    let mission_spec = match mission_path {
        Some(path) => {
            Some(load_mission_file(path).with_context(|| format!("load mission spec {path:?}"))?)
        }
        None => None,
    };
    if mission_spec.is_some() && mavlink_endpoint.is_none() {
        warn!(
            "--mission was supplied without --mavlink; the spec will be ignored because no MAVLink session is being ingested"
        );
    }

    let mavlink_config = match mavlink_endpoint {
        Some(endpoint) => {
            let dialect = Dialect::parse(mavlink_dialect).with_context(|| {
                format!(
                    "invalid --mavlink-dialect `{mavlink_dialect}` (expected one of: common, ardupilotmega, px4)"
                )
            })?;
            let mut config = MavlinkConfig::new(endpoint, dialect)
                .with_context(|| format!("parse --mavlink endpoint `{endpoint}`"))?;
            config.mission = mission_spec;
            Some(config)
        }
        None => None,
    };

    runtime.block_on(async move {
        if let Some(parent) = database.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create database parent directory {parent:?}"))?;
        }

        let database_url = sqlite_url_for(database);
        info!(%database_url, "connecting to FlightTrace SQLite database");
        let pool = connect(&database_url)
            .await
            .with_context(|| format!("connect to SQLite database at {database:?}"))?;

        let listener = TcpListener::bind(bind)
            .await
            .with_context(|| format!("bind HTTP listener to {bind}"))?;
        let local = listener
            .local_addr()
            .context("read bound listener address")?;
        info!(%local, "FlightTrace API listening");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        // Signal-handling task: flips the watch on Ctrl+C / SIGTERM.
        tokio::spawn({
            let shutdown_tx = shutdown_tx.clone();
            async move {
                shutdown_signal().await;
                let _ = shutdown_tx.send(true);
            }
        });

        // Optional MAVLink ingestion: spawned alongside Axum, shares the pool.
        let mavlink_handle = mavlink_config.map(|config| {
            let pool: SqlitePool = pool.clone();
            let mut rx = shutdown_rx.clone();
            let shutdown_fut = async move {
                while !*rx.borrow_and_update() {
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
            };
            tokio::spawn(async move {
                if let Err(err) = run_ingestion(pool, config, shutdown_fut).await {
                    warn!(%err, "MAVLink ingestion exited with error");
                }
            })
        });

        // Axum graceful-shutdown future.
        let axum_shutdown = {
            let mut rx = shutdown_rx.clone();
            async move {
                while !*rx.borrow_and_update() {
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
            }
        };

        let router = build(AppState::new(pool.clone()));
        let serve_result = axum::serve(listener, router)
            .with_graceful_shutdown(axum_shutdown)
            .await
            .context("axum server");

        // Ensure mavlink listener is signalled if axum exited early.
        let _ = shutdown_tx.send(true);

        if let Some(handle) = mavlink_handle
            && let Err(err) = handle.await
        {
            warn!(%err, "MAVLink ingestion task join error");
        }

        drop(shutdown_rx);

        serve_result
    })?;

    Ok(())
}

fn sqlite_url_for(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if raw.starts_with("sqlite:") {
        raw.into_owned()
    } else {
        format!("sqlite://{raw}")
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!(%err, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(err) => {
                tracing::warn!(%err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received");
}
