//! FlightTrace CLI entrypoint.
//!
//! `demo` prints the deterministic `TAKEOFF_TIMEOUT` bundle to stdout as JSON.
//! `serve` opens (or creates) a SQLite database, runs the embedded
//! migrations, and starts the Axum API on the requested address.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

use flighttrace_api::{AppState, build};
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
    },
}

fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    match cli.command {
        Command::Demo => run_demo(),
        Command::Serve { bind, database } => run_serve(bind, &database),
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

fn run_serve(bind: SocketAddr, database: &Path) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

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

        let router = build(AppState::new(pool));
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .context("axum server")?;

        Ok::<(), anyhow::Error>(())
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
