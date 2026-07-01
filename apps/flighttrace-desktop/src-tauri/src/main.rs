//! FlightTrace desktop shell.
//!
//! Tauri v2 wrapper around the same Axum router and React bundle the CLI
//! serves. The shell:
//!
//! 1. Spawns a Tokio runtime on a background thread.
//! 2. Binds `flighttrace-api` on `127.0.0.1:0` (dynamic loopback port).
//! 3. Points a Tauri WebView at `http://127.0.0.1:<port>/`.
//! 4. Sends a graceful-shutdown signal when the window closes.
//!
//! Web assets and the SQLite path are resolved via `AppHandle::path()` from
//! inside Tauri's `setup()` callback, so the same binary works from a repo
//! checkout, a `cargo tauri build` bundle, or a portable folder that just
//! drops `web-dist/` next to the executable. See
//! `docs/issues/09-desktop-packaging-hardening.md` for the precise
//! priority chain.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use anyhow::{Context, Result, anyhow, bail};
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use flighttrace_api::{AppState, build_with_static};
use flighttrace_storage::connect;

fn main() -> Result<()> {
    init_tracing();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Shared holder for the server thread so we can join it after Tauri's
    // event loop exits.
    let server_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    let setup_thread = server_thread.clone();
    let setup_shutdown_rx = shutdown_rx.clone();
    let close_shutdown_tx = shutdown_tx.clone();

    let run_result = tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();

            // Both paths need AppHandle::path(), so resolve inside setup().
            let web_dir = resolve_web_dir(&handle).map_err(|err| {
                error!("{err}");
                Box::<dyn std::error::Error>::from(format!("{err:#}"))
            })?;
            let database = resolve_database_path(&handle);

            info!(web_dir = %web_dir.display(), database = %database.display(), "desktop startup paths");

            let (thread, addr) = spawn_internal_server(
                web_dir,
                database,
                setup_shutdown_rx.clone(),
            )
            .map_err(|err| Box::<dyn std::error::Error>::from(format!("{err:#}")))?;

            *setup_thread.lock().expect("server_thread lock") = Some(thread);

            let url = format!("http://{addr}/");
            info!(%url, "pointing WebView at internal server");

            let parsed = url
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
                let _ = close_shutdown_tx.send(true);
            }
        })
        .run(tauri::generate_context!());

    // Whether Tauri exited normally or errored, signal the server and wait.
    let _ = shutdown_tx.send(true);
    let joined = server_thread.lock().expect("server_thread lock").take();
    if let Some(thread) = joined
        && let Err(err) = thread.join()
    {
        warn!(?err, "internal-server thread join error");
    }

    run_result.context("tauri event loop exited with an error")
}

/// Priority chain for locating the built React bundle:
///
/// 1. `FLIGHTTRACE_WEB_DIR` env var.
/// 2. `resource_dir()/web-dist` — Tauri v2 bundled resource (see
///    `tauri.conf.json` `bundle.resources`).
/// 3. `resource_dir()/_up_/web-dist` — some macOS bundle layouts.
/// 4. `resource_dir()/web/dist` — bundler that keeps the source relative
///    structure.
/// 5. `<exe_dir>/web-dist` — portable release: user drops `web-dist/`
///    next to the binary.
/// 6. `CARGO_MANIFEST_DIR/../../../web/dist` — compile-time repo path,
///    works for `cargo tauri dev` from a checkout.
fn resolve_web_dir(handle: &AppHandle) -> Result<PathBuf> {
    let mut attempted: Vec<(String, PathBuf, Option<String>)> = Vec::new();

    let try_candidate =
        |label: &str, path: PathBuf, attempted: &mut Vec<(String, PathBuf, Option<String>)>| {
            match validate_web_dir(&path) {
                Ok(()) => Some(path),
                Err(reason) => {
                    attempted.push((label.to_string(), path, Some(reason)));
                    None
                }
            }
        };

    if let Some(env) = std::env::var_os("FLIGHTTRACE_WEB_DIR") {
        let path = PathBuf::from(env);
        if let Some(hit) = try_candidate("$FLIGHTTRACE_WEB_DIR", path, &mut attempted) {
            return Ok(hit);
        }
    } else {
        attempted.push((
            "$FLIGHTTRACE_WEB_DIR".into(),
            PathBuf::new(),
            Some("not set".into()),
        ));
    }

    if let Ok(res_dir) = handle.path().resource_dir() {
        for sub in ["web-dist", "_up_/web-dist", "web/dist"] {
            let candidate = res_dir.join(sub);
            let label = format!("<resource-dir>/{sub}");
            if let Some(hit) = try_candidate(&label, candidate, &mut attempted) {
                return Ok(hit);
            }
        }
    } else {
        attempted.push((
            "<resource-dir>".into(),
            PathBuf::new(),
            Some("Tauri could not resolve resource_dir()".into()),
        ));
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let candidate = parent.join("web-dist");
        if let Some(hit) = try_candidate("<exe-dir>/web-dist", candidate, &mut attempted) {
            return Ok(hit);
        }
    }

    let repo_candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("web")
        .join("dist");
    if let Some(hit) = try_candidate("<repo>/web/dist", repo_candidate, &mut attempted) {
        return Ok(hit);
    }

    Err(anyhow!(format_missing_web_dir(&attempted)))
}

fn format_missing_web_dir(attempted: &[(String, PathBuf, Option<String>)]) -> String {
    let mut out = String::from("web assets could not be located. Tried:\n");
    for (i, (label, path, reason)) in attempted.iter().enumerate() {
        let display_path = if path.as_os_str().is_empty() {
            String::new()
        } else {
            format!(" ({})", path.display())
        };
        let reason = reason.as_deref().unwrap_or("missing");
        out.push_str(&format!("  {}. {label}{display_path} → {reason}\n", i + 1));
    }
    out.push_str(
        "\nFix one of the following:\n\
         - run `cd web && npm run build` from the repo root, then re-run\n  \
           the desktop app from the repo checkout;\n\
         - set FLIGHTTRACE_WEB_DIR to a directory that contains index.html;\n\
         - drop a `web-dist/` folder next to the flighttrace-desktop binary.",
    );
    out
}

fn validate_web_dir(dir: &Path) -> std::result::Result<(), String> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    if !dir.join("index.html").is_file() {
        return Err(format!("{} is missing index.html", dir.display()));
    }
    Ok(())
}

/// SQLite path priority:
/// 1. `FLIGHTTRACE_DATABASE` env var.
/// 2. `AppHandle::path().app_data_dir()/flighttrace-desktop.db` — proper
///    per-OS location (`~/.local/share/com.flighttrace.desktop/` on Linux,
///    `~/Library/Application Support/com.flighttrace.desktop/` on macOS,
///    `%APPDATA%\com.flighttrace.desktop\` on Windows).
/// 3. `./data/flighttrace-desktop.db` — CWD fallback if `app_data_dir()`
///    fails (rare).
fn resolve_database_path(handle: &AppHandle) -> PathBuf {
    if let Some(env) = std::env::var_os("FLIGHTTRACE_DATABASE") {
        return PathBuf::from(env);
    }

    match handle.path().app_data_dir() {
        Ok(dir) => dir.join("flighttrace-desktop.db"),
        Err(err) => {
            warn!(%err, "app_data_dir() unavailable, using ./data/ fallback");
            PathBuf::from("./data/flighttrace-desktop.db")
        }
    }
}

fn spawn_internal_server(
    web_dir: PathBuf,
    database: PathBuf,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<(JoinHandle<()>, SocketAddr)> {
    let (addr_tx, addr_rx) = mpsc::channel::<Result<SocketAddr, String>>();

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

    Ok((handle, addr))
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
