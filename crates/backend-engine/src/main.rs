//! `msime-backend-engine <resources> <scratch>`: read one request from stdin, write one response line to stdout, exit. msime-cloud starts one process per request (`internal/engine/client.go` there), so nothing is shared between requests or users; the protocol itself is `msime_engine::backend`.
//!
//! Every outcome, a failure included, is a JSON line on stdout and exit status 0. Nothing goes to stderr: a panic message could carry a path or the user's input, and the server must never forward either.

use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use msime_client_core::chinese_conversion::simplified_to_traditional;
use msime_engine::backend::{execute_raw, BackendError, MAXIMUM_REQUEST_BYTES};

fn main() {
    panic::set_hook(Box::new(|_| {}));
    let mut arguments = std::env::args_os().skip(1);
    let resources = arguments.next().map(PathBuf::from).unwrap_or_default();
    let scratch = arguments.next().map(PathBuf::from).unwrap_or_default();
    let mut raw = Vec::new();
    // One byte past the limit is enough to know the request is too large.
    let response = match std::io::stdin()
        .take(MAXIMUM_REQUEST_BYTES as u64 + 1)
        .read_to_end(&mut raw)
    {
        Ok(_) => panic::catch_unwind(AssertUnwindSafe(|| {
            execute_raw(&raw, &resources, &scratch, simplified_to_traditional)
        }))
        .unwrap_or_else(|_| BackendError::EngineFailure.response()),
        Err(_) => BackendError::InvalidRequest.response(),
    };
    let mut stdout = std::io::stdout().lock();
    // The server reads whatever arrives; a closed pipe has no one left to tell.
    let _ = writeln!(stdout, "{response}");
}
