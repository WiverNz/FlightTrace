//! FlightTrace desktop shell.
//!
//! A thin Tauri v2 wrapper around the same Axum router and React bundle the
//! CLI serves. The desktop binary:
//!
//! 1. Spawns a Tokio runtime on a background thread.
//! 2. Binds `flighttrace-api` on `127.0.0.1:0` (dynamic loopback port).
//! 3. Points a Tauri WebView at `http://127.0.0.1:<port>/`.
//! 4. Sends a graceful-shutdown signal when the window closes.
//!
//! No business logic, no Tauri commands. If you want to change what the
//! window shows, change the React app — this shell doesn't care.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;

use anyhow::{Context, Result, bail};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use flighttrace_api::{AppState, build_with_static};
use flighttrace_storage::connect;

const DEFAULT_DB: &str = "./data/flighttrace-desktop.db";

fn main() -> Result<()> {
    init_tracing();

    let web_dir = resolve_web_dir()?;
    let database = resolve_database_path();

    // Start the internal Axum server on a background thread so Tauri owns
    // the main thread's event loop (required on macOS + Windows).
    let (server_thread, addr, shutdown_tx) = spawn_internal_server(web_dir, database)?;
    let url = format!("http://{addr}/");
    info!(%url, "desktop shell will point WebView at internal server");

    let run_result = run_tauri(url.clone(), shutdown_tx.clone());

    // Whether Tauri exited normally or errored, signal the server and wait.
    let _ = shutdown_tx.send(true);
    if let Err(err) = server_thread.join() {
        warn!(?err, "internal-server thread join error");
    }

    run_result
}

fn run_tauri(url: String, shutdown_tx: watch::Sender<bool>) -> Result<()> {
    let setup_url = url.clone();
    tauri::Builder::default()
        .setup(move |app| {
            let parsed = setup_url
                .parse()
                .expect("internal-server URL must be a valid URL");
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(parsed),
            )
            .title("FlightTrace")
            .inner_size(1280.0, 800.0)
            .min_inner_size(960.0, 600.0)
            .resizable(true)
            .build()?;
            Ok(())
        })
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Flip the graceful-shutdown watch. Axum finishes its
                // in-flight requests, drops the pool, and the background
                // thread joins.
                let _ = shutdown_tx.send(true);
            }
        })
        .run(tauri::generate_context!())
        .context("tauri event loop exited with an error")
}

fn spawn_internal_server(
    web_dir: PathBuf,
    database: PathBuf,
) -> Result<(JoinHandle<()>, SocketAddr, watch::Sender<bool>)> {
    let (addr_tx, addr_rx) = mpsc::channel::<Result<SocketAddr, String>>();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let shutdown_tx_out = shutdown_tx.clone();

    let handle = std::thread::Builder::new()
        .name("flighttrace-server".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    let _ = addr_tx.send(Err(format!("build tokio runtime: {err}")));
                    return;
                }
            };

            runtime.block_on(async move {
                let bootstrap = async {
                    ensure_parent_dir(&database).await?;
                    let database_url = sqlite_url_for(&database);
                    info!(%database_url, "connecting to FlightTrace SQLite database");
                    let pool = connect(&database_url)
                        .await
                        .with_context(|| format!("connect to SQLite at {database:?}"))?;

                    let listener = TcpListener::bind("127.0.0.1:0")
                        .await
                        .context("bind loopback listener")?;
                    let local = listener.local_addr().context("read listener addr")?;
                    Ok::<_, anyhow::Error>((pool, listener, local))
                };

                let (pool, listener, local) = match bootstrap.await {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = addr_tx.send(Err(format!("{err:#}")));
                        return;
                    }
                };
                let _ = addr_tx.send(Ok(local));
                info!(%local, "FlightTrace internal server listening");

                let router = build_with_static(AppState::new(pool), Some(web_dir));

                let mut rx = shutdown_rx;
                let shutdown_fut = async move {
                    while !*rx.borrow_and_update() {
                        if rx.changed().await.is_err() {
                            return;
                        }
                    }
                };

                if let Err(err) = axum::serve(listener, router)
                    .with_graceful_shutdown(shutdown_fut)
                    .await
                {
                    warn!(%err, "axum server exited with error");
                }
                info!("FlightTrace internal server shut down");
            });
        })
        .context("spawn internal-server thread")?;

    let addr = match addr_rx.recv() {
        Ok(Ok(addr)) => addr,
        Ok(Err(msg)) => bail!("internal server failed to start: {msg}"),
        Err(_) => bail!("internal-server thread died before reporting its address"),
    };

    Ok((handle, addr, shutdown_tx_out))
}

fn resolve_web_dir() -> Result<PathBuf> {
    if let Some(env) = std::env::var_os("FLIGHTTRACE_WEB_DIR") {
        let dir = PathBuf::from(env);
        validate_web_dir(&dir)?;
        return Ok(dir);
    }

    // Fallback: repo-relative path from this crate's manifest. Works for
    // `cargo tauri dev` from a checkout. For `cargo tauri build`, set
    // FLIGHTTRACE_WEB_DIR to point at the bundled web/dist beside the
    // binary — v0.1 doesn't auto-bundle it yet.
    let candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("web")
        .join("dist");
    validate_web_dir(&candidate)?;
    Ok(candidate)
}

fn validate_web_dir(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        bail!(
            "web dir {:?} does not exist — run `npm run build` in web/ or set FLIGHTTRACE_WEB_DIR",
            dir
        );
    }
    if !dir.join("index.html").is_file() {
        bail!(
            "web dir {:?} is missing index.html — is this really a Vite build output?",
            dir
        );
    }
    Ok(())
}

fn resolve_database_path() -> PathBuf {
    std::env::var_os("FLIGHTTRACE_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DB))
}

async fn ensure_parent_dir(database: &Path) -> Result<()> {
    if let Some(parent) = database.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create database parent {parent:?}"))?;
    }
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

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
