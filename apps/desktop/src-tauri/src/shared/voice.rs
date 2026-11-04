//! Voice capture plumbing shared by the hosts that own a microphone session.

#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) mod voice_output;
#[cfg(any(unix, target_os = "windows"))]
pub(crate) mod voice_sessions;
