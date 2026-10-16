//! Real Rust client against the portable C++ peer; no device or native pipe.
//!
//! Build the peer and run this with `platforms/windows/tests/tools/voice-wire-interop.sh`,
//! which compiles voice_wire_peer.cpp and puts its path in `MSIME_VOICE_WIRE_PEER`.
//! Without that variable there is nothing to talk to, so both tests report why
//! they did nothing and return rather than failing an ordinary `cargo test`.

use msime_client_providers::voice::controller;
use msime_client_providers::voice::controller::{Error, Phase, Transport};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct Peer {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Vec<u8>>,
    reader: Option<JoinHandle<()>>,
    operations: Vec<u32>,
}
impl Peer {
    fn new(peer: &std::ffi::OsStr, mode: &str) -> Self {
        let mut child = Command::new(peer)
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        let (sender, responses) = mpsc::channel();
        let reader = thread::spawn(move || loop {
            let mut prefix = [0; 4];
            if output.read_exact(&mut prefix).is_err() {
                break;
            }
            let length = u32::from_le_bytes(prefix) as usize;
            if !(40..=controller::MAX_REPLY).contains(&length) {
                break;
            }
            let mut bytes = vec![0; length];
            if output.read_exact(&mut bytes).is_err() || sender.send(bytes).is_err() {
                break;
            }
        });
        Self {
            child,
            input,
            responses,
            reader: Some(reader),
            operations: Vec::new(),
        }
    }
}
impl Transport for Peer {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, Error> {
        self.operations
            .push(u32::from_le_bytes(request[8..12].try_into().unwrap()));
        self.input
            .write_all(&(request.len() as u32).to_le_bytes())
            .and_then(|()| self.input.write_all(request))
            .and_then(|()| self.input.flush())
            .map_err(|_| Error::Unavailable)?;
        self.responses
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Unavailable)
    }
    fn pause(&mut self) {}
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.reader.take().unwrap().join().unwrap();
    }
}

/// The C++ peer is built by a separate script, so its absence is "not run
/// here", not a failure. Returning the path keeps the check and the use of it
/// in one place.
fn peer_binary() -> Option<std::ffi::OsString> {
    match std::env::var_os("MSIME_VOICE_WIRE_PEER") {
        Some(path) => Some(path),
        None => {
            eprintln!(
                "skipped: set MSIME_VOICE_WIRE_PEER to the compiled peer, or run \
                 platforms/windows/tests/tools/voice-wire-interop.sh"
            );
            None
        }
    }
}

#[test]
fn cpp_dispatcher_returns_reviewed_unicode_without_commit() {
    let Some(peer_path) = peer_binary() else {
        return;
    };
    let mut peer = Peer::new(&peer_path, "ok");
    let stopped = AtomicBool::new(false);
    let mut phases = Vec::new();
    let text = controller::recognize(
        &mut peer,
        (7 << 32) | 9,
        "en-US",
        &stopped,
        &AtomicBool::new(false),
        |update| {
            phases.push(update.phase.name());
            if update.phase == Phase::Recording {
                assert_eq!(update.level, 0.5);
                stopped.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(text, "synthetic 测试 🌲");
    assert_eq!(
        phases,
        ["recording", "recognizing", "processing", "complete"]
    );
    assert_eq!(peer.operations, [0, 1, 2, 4, 4]);
}

#[test]
fn cpp_terminal_errors_and_identity_mismatch_never_deliver_final_text() {
    let Some(peer_path) = peer_binary() else {
        return;
    };
    for (mode, expected) in [
        ("wrong-id", Error::Invalid),
        ("wrong-session", Error::Stale),
        ("cancelled", Error::Cancelled),
        ("failed", Error::Unavailable),
        ("disconnect", Error::Unavailable),
    ] {
        let mut peer = Peer::new(&peer_path, mode);
        let mut complete = false;
        let result = controller::recognize(
            &mut peer,
            (7 << 32) | 9,
            "en-US",
            &AtomicBool::new(true),
            &AtomicBool::new(false),
            |update| {
                complete |= update.phase == Phase::Complete;
            },
        );
        assert_eq!(result, Err(expected), "{mode}");
        assert!(!complete, "{mode}");
        assert_eq!(peer.operations, [0, 1, 2, 4], "{mode}");
    }
}
