//! Embed the FlightTrace icon into `flighttrace.exe` on Windows builds
//! so Explorer, the taskbar, and pinned shortcuts stop showing the
//! generic default. No-op for every other target.
//!
//! `winresource` compiles `icons/flighttrace.ico` into a Windows
//! resource block via `rc.exe` (MSVC) or `windres` (mingw), which
//! means:
//!   * native Windows builds (our release CI runners): Just Works.
//!   * cross-compiling to Windows from Linux: needs `mingw-w64` in
//!     PATH; documented but not required for CI.

fn main() {
    // `cfg!(target_os = ...)` in a build script evaluates against the
    // *host*, so use the cargo-provided env var to check the *target*.
    // This lets the icon embed correctly even when cross-compiling.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/flighttrace.ico");
        // Cheap identification for the Details tab in Explorer's file
        // properties. Version fields are read from Cargo automatically.
        res.set("ProductName", "FlightTrace");
        res.set("FileDescription", "FlightTrace mission debugger");
        res.set("CompanyName", "FlightTrace contributors");
        res.compile()
            .expect("compile flighttrace.exe Windows resource block");
    }

    println!("cargo:rerun-if-changed=icons/flighttrace.ico");
    println!("cargo:rerun-if-changed=build.rs");
}
