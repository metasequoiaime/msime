//! Controller v2 with injected message transport, independent of platform APIs.
//! Wire: Engine contracts/voice_controller.h at d6a1f6b62b8498cc2d9252aa312147cd06de8b80.
use std::sync::atomic::{AtomicBool, Ordering};
pub const MAX_REPLY: usize = 40 + 65536;
const MAGIC: u32 = 0x3243564d;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Unavailable,
    Busy,
    Stale,
    Denied,
    Cancelled,
}
pub trait Transport {
    /// Exact message exchange, bounded in size/time. Close on uncertainty; never retry.
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, Error>;
    fn pause(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Idle,
    Recording,
    Recognizing,
    Processing,
    Complete,
    Cancelled,
    Failed,
}
impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Recognizing => "recognizing",
            Self::Processing => "processing",
            Self::Complete => "complete",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}
pub struct Update {
    pub phase: Phase,
    pub level: f32,
    pub text: String,
}
struct Reply {
    session: u64,
    update: Update,
}
fn request(controller: u64, id: u64, session: u64, operation: u32, language: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(48 + language.len());
    for value in [MAGIC, 2, operation, language.len() as u32] {
        bytes.extend(value.to_le_bytes());
    }
    for value in [id, session, controller, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(language.as_bytes());
    bytes
}
fn reply(bytes: &[u8], id: u64) -> Result<Reply, Error> {
    if !(40..=MAX_REPLY).contains(&bytes.len()) {
        return Err(Error::Invalid);
    }
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let u64_at = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    let (status, phase, session, length, level) = (
        u32_at(8),
        u32_at(12),
        u64_at(24),
        u32_at(32) as usize,
        u32_at(36),
    );
    if u32_at(0) != MAGIC
        || u32_at(4) != 2
        || u64_at(16) != id
        || status > 5
        || phase > 6
        || length > 65536
        || bytes.len() != 40 + length
        || level > 1000
    {
        return Err(Error::Invalid);
    }
    if status != 0 {
        if length != 0 || level != 0 {
            return Err(Error::Invalid);
        }
        return Err(match status {
            1 => Error::Invalid,
            2 => Error::Denied,
            3 => Error::Busy,
            4 => Error::Stale,
            _ => Error::Unavailable,
        });
    }
    if (phase == 0 && (session != 0 || length != 0 || level != 0))
        || (phase != 0 && session == 0)
        || (phase != 1 && level != 0)
        || (phase >= 5 && length != 0)
    {
        return Err(Error::Invalid);
    }
    let text = std::str::from_utf8(&bytes[40..]).map_err(|_| Error::Invalid)?;
    if text.contains('\0') {
        return Err(Error::Invalid);
    }
    Ok(Reply {
        session,
        update: Update {
            phase: [
                Phase::Idle,
                Phase::Recording,
                Phase::Recognizing,
                Phase::Processing,
                Phase::Complete,
                Phase::Cancelled,
                Phase::Failed,
            ][phase as usize],
            level: level as f32 / 1000.0,
            text: text.to_owned(),
        },
    })
}
/// Retain one transport for the call; dropping it cancels its Server capture.
/// Returns reviewed text only, never injects input or writes the clipboard.
pub fn recognize(
    transport: &mut impl Transport,
    controller: u64,
    language: &str,
    stopped: &AtomicBool,
    cancelled: &AtomicBool,
    mut update: impl FnMut(&Update),
) -> Result<String, Error> {
    if controller >> 32 == 0
        || language.is_empty()
        || language.len() > 64
        || !crate::is_ascii_alphanumeric_dash(language)
    {
        return Err(Error::Invalid);
    }
    let mut id = 0u64;
    let mut call = |transport: &mut dyn Transport, session, operation, language: &str| {
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        id = id.checked_add(1).ok_or(Error::Unavailable)?;
        let bytes = transport.exchange(&request(controller, id, session, operation, language))?;
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        reply(&bytes, id)
    };
    let hello = call(transport, 0, 0, "")?;
    if hello.update.phase != Phase::Idle || hello.session != 0 {
        return Err(Error::Invalid);
    }
    let mut response = call(transport, 0, 1, language)?;
    let session = response.session;
    if session == 0 {
        return Err(Error::Invalid);
    }
    let mut stop_sent = false;
    let mut previous = Phase::Recording;
    loop {
        if response.session != session || response.update.phase < previous {
            return Err(Error::Stale);
        }
        previous = response.update.phase;
        if cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        match response.update.phase {
            Phase::Cancelled => return Err(Error::Cancelled),
            Phase::Failed => return Err(Error::Unavailable),
            Phase::Complete => {
                if response.update.text.is_empty() {
                    return Err(Error::Unavailable);
                }
                update(&response.update);
                return if cancelled.load(Ordering::Acquire) {
                    Err(Error::Cancelled)
                } else {
                    Ok(response.update.text)
                };
            }
            _ => update(&response.update),
        }
        transport.pause();
        let operation = if stopped.load(Ordering::Acquire) && !stop_sent {
            stop_sent = true;
            2
        } else {
            4
        };
        response = call(transport, session, operation, "")?;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    fn frame(id: u64, session: u64, phase: u32, text: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in [MAGIC, 2, 0, phase] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(id.to_le_bytes());
        bytes.extend(session.to_le_bytes());
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(text);
        bytes
    }
    #[test]
    fn exact_little_endian_contract_and_invalid_frames() {
        let encoded = request((7 << 32) | 9, 1, 0, 1, "en-US");
        assert_eq!(&encoded[..16], b"MVC2\x02\0\0\0\x01\0\0\0\x05\0\0\0");
        assert_eq!(&encoded[32..40], &[9, 0, 0, 0, 7, 0, 0, 0]);
        assert_eq!(&encoded[48..], b"en-US");
        let good = frame(1, 7, 4, b"synthetic result");
        assert!(reply(&good, 1).is_ok());
        for length in 0..good.len() {
            assert!(reply(&good[..length], 1).is_err());
        }
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(reply(&trailing, 1).is_err());
        assert!(reply(&good, 2).is_err());
        for text in [b"\xc0\xaf".as_slice(), b"a\0b"] {
            assert!(reply(&frame(1, 7, 4, text), 1).is_err());
        }
        assert!(reply(&frame(1, 7, 4, &vec![b'x'; 65537]), 1).is_err());
        assert!(reply(&frame(1, 7, 5, b"unexpected"), 1).is_err());
    }
    struct Fixture {
        replies: VecDeque<Vec<u8>>,
        operations: Vec<u32>,
    }
    impl Transport for Fixture {
        fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, Error> {
            self.operations
                .push(u32::from_le_bytes(request[8..12].try_into().unwrap()));
            Ok(self.replies.pop_front().unwrap())
        }
        fn pause(&mut self) {}
    }
    #[test]
    fn stop_once_then_poll_without_committing() {
        let mut transport = Fixture {
            replies: [
                frame(1, 0, 0, b""),
                frame(2, 7, 1, b""),
                frame(3, 7, 2, b""),
                frame(4, 7, 3, b""),
                frame(5, 7, 4, b"synthetic result"),
            ]
            .into(),
            operations: vec![],
        };
        let mut phases = vec![];
        assert_eq!(
            recognize(
                &mut transport,
                1 << 32,
                "en",
                &AtomicBool::new(true),
                &AtomicBool::new(false),
                |value| phases.push(value.phase)
            ),
            Ok("synthetic result".into())
        );
        assert_eq!(transport.operations, [0, 1, 2, 4, 4]);
        assert_eq!(
            phases,
            [
                Phase::Recording,
                Phase::Recognizing,
                Phase::Processing,
                Phase::Complete
            ]
        );
    }
    #[test]
    fn cancellation_wrong_session_and_phase_regression_reject_results() {
        let cancelled = AtomicBool::new(false);
        let mut transport = Fixture {
            replies: [frame(1, 0, 0, b""), frame(2, 7, 1, b"")].into(),
            operations: vec![],
        };
        assert_eq!(
            recognize(
                &mut transport,
                1 << 32,
                "en",
                &AtomicBool::new(false),
                &cancelled,
                |_| cancelled.store(true, Ordering::Release)
            ),
            Err(Error::Cancelled)
        );
        for bad in [frame(3, 8, 4, b"synthetic"), frame(3, 7, 1, b"")] {
            let mut transport = Fixture {
                replies: [frame(1, 0, 0, b""), frame(2, 7, 2, b""), bad].into(),
                operations: vec![],
            };
            assert_eq!(
                recognize(
                    &mut transport,
                    1 << 32,
                    "en",
                    &AtomicBool::new(false),
                    &AtomicBool::new(false),
                    |value| assert_ne!(value.phase, Phase::Complete)
                ),
                Err(Error::Stale)
            );
        }
    }
}
