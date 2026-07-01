//! FlightTrace CLI entrypoint.
//!
//! Run modes:
//!
//! * **No arguments** — launch the packaged GUI. Auto-detects a `web-dist`
//!   directory (see [`resolve_gui_web_dir`]), binds a free loopback port,
//!   prints the URL, and opens the default browser. This is what a user
//!   double-clicking `flighttrace.exe` from a release archive gets.
//! * `gui` — same as no-arguments, but explicit for scripting.
//! * `demo` — print the deterministic `TAKEOFF_TIMEOUT` bundle to stdout as
//!   JSON.
//! * `serve` — open (or create) a SQLite database, run the embedded
//!   migrations, and start the Axum API on the requested address. Pass
//!   `--mavlink udp://host:port` to additionally bind a read-only MAVLink
//!   UDP listener (issue 4) that persists live SITL traffic into the same
//!   database.
//!
//! Each `serve` flag also reads a matching `FLIGHTTRACE_*` environment
//! variable as a fallback, so a `.env` file can drive local-dev configuration
//! without retyping flags. Precedence is: CLI flag > env var > built-in
//! default.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
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
    /// With no subcommand, run the packaged GUI (see the `gui` subcommand
    /// for the same behaviour explicitly).
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Launch the FlightTrace UI — auto-detects `web-dist`, binds a free
    /// loopback port, prints the URL, opens the default browser. This is
    /// what `flighttrace` with no arguments runs.
    Gui,
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
        // Bare `flighttrace` (double-click, release-archive shape) and the
        // explicit `gui` subcommand share one handler.
        None | Some(Command::Gui) => run_gui(),
        Some(Command::Demo) => run_demo(),
        Some(Command::Serve {
            bind,
            database,
            mavlink,
            mavlink_dialect,
            mission,
            web_dir,
            open,
        }) => run_serve(
            bind,
            &database,
            mavlink.as_deref(),
            &mavlink_dialect,
            mission.as_deref(),
            web_dir,
            open,
            false,
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

/// Default GUI launch — what `flighttrace` with no arguments runs.
///
/// Auto-detects `web-dist`, binds a free loopback port, prints the URL
/// to stdout, and opens the default browser. If no `web-dist` can be
/// located anywhere, returns an actionable error.
fn run_gui() -> Result<()> {
    let web_dir = resolve_gui_web_dir()?;
    let database = std::env::var_os("FLIGHTTRACE_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(GUI_DEFAULT_DATABASE));
    let bind: SocketAddr = "127.0.0.1:0"
        .parse()
        .expect("literal loopback address is valid");
    info!(
        web_dir = %web_dir.display(),
        database = %database.display(),
        "starting FlightTrace GUI",
    );
    run_serve(
        bind,
        &database,
        None,
        DEFAULT_DIALECT,
        None,
        Some(web_dir),
        true,
        true,
    )
}

const GUI_DEFAULT_DATABASE: &str = "./data/flighttrace.db";
const DEFAULT_DIALECT: &str = "common";

/// Resolve a `web-dist` directory for the GUI launch flow. Priority:
///
/// 1. `FLIGHTTRACE_WEB_DIR` env var.
/// 2. `./web-dist` in the current working directory.
/// 3. `<exe-dir>/web-dist` — the release-archive shape.
/// 4. `./web/dist` — repo checkouts.
///
/// Missing → returns the single actionable-message error documented in
/// `docs/issues/13-gui-default-packaged-binary.md`.
fn resolve_gui_web_dir() -> Result<PathBuf> {
    let env_override = std::env::var_os("FLIGHTTRACE_WEB_DIR").map(PathBuf::from);
    let cwd = std::env::current_dir().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from));
    resolve_gui_web_dir_with(env_override, cwd, exe_dir)
}

/// Pure form of [`resolve_gui_web_dir`] with the environment injected —
/// makes the priority chain unit-testable with temp directories.
fn resolve_gui_web_dir_with(
    env_override: Option<PathBuf>,
    cwd: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    let mut attempted: Vec<(String, PathBuf)> = Vec::new();

    if let Some(path) = env_override {
        if is_valid_web_dir(&path) {
            return Ok(path);
        }
        attempted.push(("$FLIGHTTRACE_WEB_DIR".into(), path));
    } else {
        attempted.push(("$FLIGHTTRACE_WEB_DIR".into(), PathBuf::from("(unset)")));
    }

    if let Some(cwd) = cwd.as_ref() {
        let p = cwd.join("web-dist");
        if is_valid_web_dir(&p) {
            return Ok(p);
        }
        attempted.push(("./web-dist".into(), p));

        let p = cwd.join("web").join("dist");
        if is_valid_web_dir(&p) {
            return Ok(p);
        }
        // Second CWD candidate pushed after the exe-dir one below so the
        // final list reads in the priority order documented above.
    }

    if let Some(exe) = exe_dir.as_ref() {
        let p = exe.join("web-dist");
        if is_valid_web_dir(&p) {
            return Ok(p);
        }
        attempted.push(("<exe-dir>/web-dist".into(), p));
    }

    if let Some(cwd) = cwd.as_ref() {
        let p = cwd.join("web").join("dist");
        if is_valid_web_dir(&p) {
            return Ok(p);
        }
        attempted.push(("./web/dist".into(), p));
    }

    let tried = attempted
        .iter()
        .enumerate()
        .map(|(i, (label, path))| format!("  {}. {label} ({})", i + 1, path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(anyhow!(
        "No web UI found. Place web-dist next to the binary, set \
         FLIGHTTRACE_WEB_DIR, or run `flighttrace serve` for API-only mode.\n\
         \n\
         Tried:\n{tried}",
    ))
}

fn is_valid_web_dir(dir: &Path) -> bool {
    dir.is_dir() && dir.join("index.html").is_file()
}

// Serve has always taken one argument per CLI flag; issue 13 added one more
// for the stdout URL announce. Bundling them into a struct would just
// invert cost (define the struct at every call site instead of listing
// args) and this is a private single-caller-per-mode function.
#[allow(clippy::too_many_arguments)]
fn run_serve(
    bind: SocketAddr,
    database: &Path,
    mavlink_endpoint: Option<&str>,
    mavlink_dialect: &str,
    mission_path: Option<&Path>,
    web_dir: Option<PathBuf>,
    open_browser: bool,
    announce_stdout: bool,
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

        // The GUI path picks a random port, so print the bound URL to
        // stdout — that's the single line the user needs to open the app.
        // Tracing goes to stderr; keep stdout a clean surface.
        if announce_stdout {
            println!("FlightTrace UI: http://{local}/");
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
            Some(Command::Serve {
                bind,
                database,
                mavlink,
                mavlink_dialect,
                mission,
                web_dir,
                open,
            }) => (
                bind,
                database,
                mavlink,
                mavlink_dialect,
                mission,
                web_dir,
                open,
            ),
            other => panic!("expected serve command, got {other:?}"),
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

    // -------- issue 13: GUI-by-default packaged binary --------

    #[test]
    fn no_arguments_parse_as_default_gui() {
        let cli = Cli::try_parse_from(["flighttrace"]).expect("bare invocation parses");
        assert!(
            cli.command.is_none(),
            "expected no subcommand, got {:?}",
            cli.command
        );
    }

    #[test]
    fn explicit_gui_subcommand_parses() {
        let cli = Cli::try_parse_from(["flighttrace", "gui"]).expect("gui subcommand parses");
        assert!(matches!(cli.command, Some(Command::Gui)));
    }

    #[test]
    fn existing_demo_and_serve_subcommands_still_parse() {
        let cli =
            Cli::try_parse_from(["flighttrace", "demo"]).expect("demo subcommand still parses");
        assert!(matches!(cli.command, Some(Command::Demo)));

        // Existing full-form serve invocation must be unaffected.
        let _lock = ENV_LOCK.lock().expect("environment test lock");
        let _env = EnvGuard::cleared();
        let cfg = parse_serve(&[
            "flighttrace",
            "serve",
            "--bind",
            "127.0.0.1:8080",
            "--database",
            "./data/flighttrace.db",
        ]);
        assert_eq!(cfg.0, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(cfg.1, PathBuf::from("./data/flighttrace.db"));
    }

    #[test]
    fn help_and_version_short_circuit_without_starting_server() {
        // clap returns a DisplayHelp / DisplayVersion "error" that renders on
        // stdout and short-circuits the run — verifying the kind is enough.
        let err = Cli::try_parse_from(["flighttrace", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);

        let err = Cli::try_parse_from(["flighttrace", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    fn scratch_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("flighttrace-gui-test-{}", uuid_stub()));
        std::fs::create_dir_all(&path).expect("create scratch dir");
        path
    }

    fn uuid_stub() -> String {
        // Avoid pulling in the uuid crate just for tests. Pids collide across
        // parallel test runs but we combine with a monotonic counter for
        // uniqueness within a process.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!(
            "{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }

    struct DirGuard(PathBuf);
    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_valid_web_dir(root: &Path, sub: &str) -> PathBuf {
        let dir = root.join(sub);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("index.html"), "<!doctype html>").expect("write index");
        dir
    }

    #[test]
    fn gui_web_dir_prefers_env_var_when_valid() {
        let root = scratch_dir();
        let _guard = DirGuard(root.clone());
        let env_dir = make_valid_web_dir(&root, "env");
        let cwd_dir = make_valid_web_dir(&root, "web-dist");
        let resolved = resolve_gui_web_dir_with(Some(env_dir.clone()), Some(root.clone()), None)
            .expect("env override wins");
        assert_eq!(resolved, env_dir);
        // Sanity: with the env override removed, the CWD candidate takes over.
        let fallback = resolve_gui_web_dir_with(None, Some(root.clone()), None)
            .expect("cwd/web-dist fallback");
        assert_eq!(fallback, cwd_dir);
    }

    #[test]
    fn gui_web_dir_falls_back_to_exe_dir() {
        let root = scratch_dir();
        let _guard = DirGuard(root.clone());
        // CWD has no web-dist so the second candidate (cwd/web-dist) misses.
        let cwd_only = root.join("no-web-here");
        std::fs::create_dir_all(&cwd_only).expect("mkdir cwd");
        // Exe dir sits at `root`, so <exe-dir>/web-dist = root/web-dist. Populate.
        let exe_web = make_valid_web_dir(&root, "web-dist");
        let resolved = resolve_gui_web_dir_with(None, Some(cwd_only), Some(root.clone()))
            .expect("exe-dir candidate hits");
        assert_eq!(resolved, exe_web);
    }

    #[test]
    fn gui_web_dir_falls_back_to_repo_web_dist() {
        let root = scratch_dir();
        let _guard = DirGuard(root.clone());
        let web_dist = make_valid_web_dir(&root, "web/dist");
        let resolved =
            resolve_gui_web_dir_with(None, Some(root.clone()), None).expect("repo dev fallback");
        assert_eq!(resolved, web_dist);
    }

    #[test]
    fn gui_web_dir_missing_returns_actionable_error() {
        let root = scratch_dir();
        let _guard = DirGuard(root.clone());
        // Nothing valid anywhere.
        let err = resolve_gui_web_dir_with(None, Some(root.clone()), Some(root.clone()))
            .expect_err("no candidate should succeed");
        let msg = format!("{err:#}");
        assert!(msg.contains("No web UI found"), "message = {msg}");
        assert!(msg.contains("FLIGHTTRACE_WEB_DIR"), "message = {msg}");
        assert!(msg.contains("flighttrace serve"), "message = {msg}");
        // Every candidate must be enumerated in the diagnostic.
        for expected in [
            "$FLIGHTTRACE_WEB_DIR",
            "./web-dist",
            "<exe-dir>/web-dist",
            "./web/dist",
        ] {
            assert!(
                msg.contains(expected),
                "expected {expected} in diagnostic:\n{msg}"
            );
        }
    }

    #[test]
    fn is_valid_web_dir_requires_index_html() {
        let root = scratch_dir();
        let _guard = DirGuard(root.clone());
        assert!(!is_valid_web_dir(&root), "empty dir isn't valid");
        assert!(!is_valid_web_dir(&root.join("does-not-exist")));
        let good = make_valid_web_dir(&root, "good");
        assert!(is_valid_web_dir(&good));
    }
}
