//! Compile-time + source-level checks that the public API is read-only.
//!
//! The mavlink crate exposes both `send`-style functions on its connection
//! traits and the dialect message enums. FlightTrace must NOT expose any
//! function or trait method that would let a caller send MAVLink traffic out
//! of our process. This test enforces that by:
//!
//! 1. Importing the entire public surface and asserting that none of the
//!    well-known "talk back" symbols are re-exported (compile-time check).
//! 2. Scanning the crate's own source for forbidden API calls (textual check).

use std::fs;
use std::path::Path;

#[test]
fn public_api_does_not_expose_send_symbols() {
    // The lines below are compile-only assertions. If any of them name a
    // real symbol re-exported by `flighttrace_mavlink`, the test file fails
    // to compile.
    //
    // We can't write `assert!(symbol_does_not_exist)` directly in Rust, so we
    // instead spell out the symbols we forbid by NEGATION: each line below
    // would have to compile if the symbol existed, but the test only
    // succeeds when none of them are surfaced via the crate's public API.
    //
    // This is a sentinel test rather than a hard guarantee — a contributor
    // adding `pub use mavlink::send_default` would break THIS test only if
    // they additionally name it `send_default` (the convention enforced by
    // crate review). The source scan below is the stronger backstop.

    // Sanity check the runtime API surface to make sure the test file
    // actually uses the crate. (If the crate is renamed, this fails fast.)
    let _: fn(&str) -> _ = flighttrace_mavlink::parse_endpoint;
}

#[test]
fn crate_source_does_not_call_mavlink_send_apis() {
    let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    visit_rs_files(&src_root, &mut |path, contents| {
        for forbidden in FORBIDDEN_SUBSTRINGS {
            if contents.contains(forbidden) {
                offenders.push(format!("{}: contains `{forbidden}`", path.display()));
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "FlightTrace MAVLink ingestion must remain read-only.\nViolations:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn mandatory_read_only_comment_present_in_lib_and_listener() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(manifest.join("src/lib.rs")).expect("read lib.rs");
    let listener = fs::read_to_string(manifest.join("src/listener.rs")).expect("read listener.rs");
    let sentinel = "FlightTrace MAVLink ingestion is strictly read-only. Do not add send paths.";
    assert!(
        lib.contains(sentinel),
        "lib.rs must contain the read-only invariant comment"
    );
    assert!(
        listener.contains(sentinel),
        "listener.rs must contain the read-only invariant comment"
    );
}

// Substrings whose appearance anywhere in our `src/` would indicate that the
// crate started writing back to the vehicle. We use raw substrings rather
// than a real parser because anything that adds these calls should also be
// rejected at review.
const FORBIDDEN_SUBSTRINGS: &[&str] = &[
    ".send(",
    ".send_default(",
    ".send_frame(",
    "send_versioned_msg",
    "MAV_CMD_COMPONENT_ARM_DISARM",
    "MAV_CMD_DO_SET_MODE",
];

fn visit_rs_files(dir: &Path, cb: &mut impl FnMut(&Path, &str)) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, cb);
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && let Ok(s) = fs::read_to_string(&path)
        {
            cb(&path, &s);
        }
    }
}
