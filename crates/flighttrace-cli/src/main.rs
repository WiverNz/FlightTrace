//! FlightTrace CLI entrypoint.
//!
//! `demo` prints the deterministic `TAKEOFF_TIMEOUT` bundle to stdout as JSON.
//! `serve` opens (or creates) a SQLite database, runs the embedded
//! migrations, and starts the Axum API on the requested address. Pass
//! `--mavlink udp://host:port` to additionally bind a read-only MAVLink UDP
//! listener (issue 4) that persists live SITL traffic into the same database.
//!
//! Each `serve` flag also reads a matching `FLIGHTTRACE_*` environment
//! variable as a fallback, so a `.env` file can drive local-dev configuration
//! without retyping flags. Precedence is: CLI flag > env var > built-in
//! default.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use flighttrace_api::{AppState, build_with_static};
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
        /// Address to bind the HTTP server to. Env: `FLIGHTTRACE_BIND`.
        #[arg(long, env = "FLIGHTTRACE_BIND", default_value = "127.0.0.1:8080")]
        bind: SocketAddr,
        /// Path to the SQLite database file. Parent directory is created if
        /// missing. Env: `FLIGHTTRACE_DATABASE`.
        #[arg(
            long,
            env = "FLIGHTTRACE_DATABASE",
            default_value = "./data/flighttrace.db"
        )]
        database: PathBuf,
        /// Optional MAVLink UDP endpoint to ingest read-only telemetry from
        /// (e.g. `udp://127.0.0.1:14550`). Absent = demo-only mode, exactly
        /// like before issue 4. Env: `FLIGHTTRACE_MAVLINK`.
        #[arg(long, env = "FLIGHTTRACE_MAVLINK")]
        mavlink: Option<String>,
        /// MAVLink dialect to decode incoming traffic with. PX4 also speaks
        /// `common`, so the default is fine for both stacks. Env:
        /// `FLIGHTTRACE_MAVLINK_DIALECT`.
        #[arg(long, env = "FLIGHTTRACE_MAVLINK_DIALECT", default_value = "common")]
        mavlink_dialect: String,
        /// Path to a mission YAML to evaluate against when each MAVLink
        /// session ends. Without it, ingestion still classifies sessions
        /// from the in-band `MissionFailed` flag but produces no
        /// `FailureReport`. Env: `FLIGHTTRACE_MISSION`.
        #[arg(long, env = "FLIGHTTRACE_MISSION")]
        mission: Option<PathBuf>,
        /// Directory that holds the prebuilt web UI (typically
        /// `./web/dist`). When set, the API server also serves the SPA at
        /// `http://<bind>/` with client-side routing fallback. Absent =
        /// API-only. Env: `FLIGHTTRACE_WEB_DIR`.
        #[arg(long, env = "FLIGHTTRACE_WEB_DIR")]
        web_dir: Option<PathBuf>,
        /// Open the default browser at `http://<bind>/` once the server
        /// finishes binding. No-op in headless environments. Env:
        /// `FLIGHTTRACE_OPEN`.
        #[arg(long, env = "FLIGHTTRACE_OPEN")]
        open: bool,
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
            web_dir,
            open,
        } => run_serve(
            bind,
            &database,
            mavlink.as_deref(),
            &mavlink_dialect,
            mission.as_deref(),
            web_dir,
            open,
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
    web_dir: Option<PathBuf>,
    open_browser: bool,
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

    // Same fail-fast treatment for --web-dir: bad paths trip on startup, not
    // on the first browser request.
    let web_dir = match web_dir {
        Some(dir) => Some(validate_web_dir(dir)?),
        None => None,
    };

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
        if let Some(dir) = web_dir.as_ref() {
            info!(%local, web_dir = %dir.display(), "FlightTrace API listening + serving UI");
        } else {
            info!(%local, "FlightTrace API listening");
        }

        if open_browser {
            let url = format!("http://{local}/");
            tokio::spawn(async move {
                open_url_in_browser(&url);
            });
        }

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

        let router = build_with_static(AppState::new(pool.clone()), web_dir.clone());
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

/// Ensures `--web-dir` points at a directory that has a readable `index.html`.
/// Same fail-fast intent as the mission YAML preflight.
fn validate_web_dir(dir: PathBuf) -> Result<PathBuf> {
    let metadata = std::fs::metadata(&dir).with_context(|| format!("read --web-dir {dir:?}"))?;
    if !metadata.is_dir() {
        anyhow::bail!("--web-dir {:?} is not a directory", dir);
    }
    let index = dir.join("index.html");
    if !index.is_file() {
        anyhow::bail!(
            "--web-dir {:?} is missing an index.html — did you run `npm run build` in web/?",
            dir
        );
    }
    Ok(dir)
}

/// Platform-conditional "open URL in default browser" helper. Best-effort:
/// errors are logged and never propagate — a headless CI or a missing
/// launcher must not take the server down.
fn open_url_in_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let attempt = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "macos")]
    let attempt = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let attempt = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let attempt: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no known browser launcher on this platform",
    ));

    match attempt {
        Ok(_) => info!(%url, "requested browser open"),
        Err(err) => warn!(%err, %url, "could not open browser (continuing headless)"),
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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    const CONFIG_ENV_VARS: [&str; 7] = [
        "FLIGHTTRACE_BIND",
        "FLIGHTTRACE_DATABASE",
        "FLIGHTTRACE_MAVLINK",
        "FLIGHTTRACE_MAVLINK_DIALECT",
        "FLIGHTTRACE_MISSION",
        "FLIGHTTRACE_WEB_DIR",
        "FLIGHTTRACE_OPEN",
    ];
    type ServeConfig = (
        SocketAddr,
        PathBuf,
        Option<String>,
        String,
        Option<PathBuf>,
        Option<PathBuf>,
        bool,
    );

    struct EnvGuard {
        values: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn cleared() -> Self {
            let values = CONFIG_ENV_VARS
                .into_iter()
                .map(|name| (name, std::env::var_os(name)))
                .collect();
            for name in CONFIG_ENV_VARS {
                // SAFETY: all tests in this module serialize environment
                // mutation with ENV_LOCK and restore it before releasing it.
                unsafe { std::env::remove_var(name) };
            }
            Self { values }
        }

        fn set(&self, name: &'static str, value: &str) {
            assert!(CONFIG_ENV_VARS.contains(&name));
            // SAFETY: all tests in this module serialize environment mutation
            // with ENV_LOCK and restore it before releasing it.
            unsafe { std::env::set_var(name, value) };
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.values {
                // SAFETY: the ENV_LOCK guard still exists while this guard is
                // dropped, so no test in this module can observe this restore.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    fn parse_serve(args: &[&str]) -> ServeConfig {
        let cli = Cli::try_parse_from(args).expect("serve arguments should parse");
        match cli.command {
            Command::Serve {
                bind,
                database,
                mavlink,
                mavlink_dialect,
                mission,
                web_dir,
                open,
            } => (
                bind,
                database,
                mavlink,
                mavlink_dialect,
                mission,
                web_dir,
                open,
            ),
            Command::Demo => panic!("expected serve command"),
        }
    }

    #[test]
    fn serve_uses_defaults_without_env_or_flags() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let _env = EnvGuard::cleared();

        let config = parse_serve(&["flighttrace", "serve"]);

        assert_eq!(config.0, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.1, PathBuf::from("./data/flighttrace.db"));
        assert_eq!(config.2, None);
        assert_eq!(config.3, "common");
        assert_eq!(config.4, None);
        assert_eq!(config.5, None);
        assert!(!config.6);
    }

    #[test]
    fn serve_uses_environment_when_flags_are_absent() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let env = EnvGuard::cleared();
        env.set("FLIGHTTRACE_BIND", "127.0.0.1:8088");
        env.set("FLIGHTTRACE_DATABASE", "./data/env.db");
        env.set("FLIGHTTRACE_MAVLINK_DIALECT", "ardupilotmega");

        let config = parse_serve(&["flighttrace", "serve"]);

        assert_eq!(config.0, "127.0.0.1:8088".parse().unwrap());
        assert_eq!(config.1, PathBuf::from("./data/env.db"));
        assert_eq!(config.3, "ardupilotmega");
    }

    #[test]
    fn serve_cli_flags_override_environment() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let env = EnvGuard::cleared();
        env.set("FLIGHTTRACE_BIND", "127.0.0.1:8088");
        env.set("FLIGHTTRACE_DATABASE", "./data/env.db");
        env.set("FLIGHTTRACE_MAVLINK", "udp://127.0.0.1:14550");
        env.set("FLIGHTTRACE_MAVLINK_DIALECT", "common");
        env.set("FLIGHTTRACE_MISSION", "examples/missions/env.yaml");

        let config = parse_serve(&[
            "flighttrace",
            "serve",
            "--bind",
            "127.0.0.1:8089",
            "--database",
            "./data/cli.db",
            "--mavlink",
            "udp://127.0.0.1:14560",
            "--mavlink-dialect",
            "px4",
            "--mission",
            "examples/missions/cli.yaml",
        ]);

        assert_eq!(config.0, "127.0.0.1:8089".parse().unwrap());
        assert_eq!(config.1, PathBuf::from("./data/cli.db"));
        assert_eq!(config.2.as_deref(), Some("udp://127.0.0.1:14560"));
        assert_eq!(config.3, "px4");
        assert_eq!(config.4, Some(PathBuf::from("examples/missions/cli.yaml")));
    }

    #[test]
    fn serve_reads_optional_mavlink_and_mission_from_environment() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let env = EnvGuard::cleared();
        env.set("FLIGHTTRACE_MAVLINK", "udp://127.0.0.1:14550");
        env.set(
            "FLIGHTTRACE_MISSION",
            "examples/missions/simple_takeoff.yaml",
        );

        let config = parse_serve(&["flighttrace", "serve"]);

        assert_eq!(config.2.as_deref(), Some("udp://127.0.0.1:14550"));
        assert_eq!(
            config.4,
            Some(PathBuf::from("examples/missions/simple_takeoff.yaml"))
        );
    }

    #[test]
    fn serve_reads_web_dir_and_open_from_environment() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let env = EnvGuard::cleared();
        env.set("FLIGHTTRACE_WEB_DIR", "./web/dist");
        env.set("FLIGHTTRACE_OPEN", "true");

        let config = parse_serve(&["flighttrace", "serve"]);

        assert_eq!(config.5, Some(PathBuf::from("./web/dist")));
        assert!(config.6);
    }

    #[test]
    fn serve_cli_flags_override_web_dir_environment() {
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let env = EnvGuard::cleared();
        env.set("FLIGHTTRACE_WEB_DIR", "./env/dist");
        env.set("FLIGHTTRACE_OPEN", "true");

        let config = parse_serve(&["flighttrace", "serve", "--web-dir", "./cli/dist"]);

        assert_eq!(config.5, Some(PathBuf::from("./cli/dist")));
        // --open is a bool flag; env stays authoritative when the CLI flag is
        // absent (no way to explicitly say "off" without --open=false, and
        // clap treats a bare --open as true).
        assert!(config.6);
    }
}
