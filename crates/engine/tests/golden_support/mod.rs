//! Support for `tests/golden.rs`. The recorder's JSON shapes are the contract (`tools/engine-golden/recorder.cpp`); this module reproduces them from the Rust engine and compares structurally, so key order and absent-versus-default fields never cause a false diff.

pub mod diff;
pub mod fixture;
pub mod real;
pub mod replay;
pub mod snapshot;

use std::path::{Path, PathBuf};

/// `crates/engine/tests/golden`, found from the directory of whichever crate compiles this module: the engine's own `tests/golden.rs`, or the C ABI replay in `crates/engine-capi/tests/golden_replay.rs`, which includes this module by path. Both crates sit side by side under `crates/`.
pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate directory sits under crates/")
        .join("engine")
        .join("tests")
        .join("golden")
}
