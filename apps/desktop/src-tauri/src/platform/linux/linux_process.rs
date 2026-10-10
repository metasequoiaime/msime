//! Bounded, silent output capture for Linux session tools.
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Whether an executable file is available in an absolute directory listed by `PATH`.
pub fn program_available(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| {
            directory.is_absolute()
                && fs::metadata(directory.join(name)).is_ok_and(|metadata| metadata.is_file())
        })
    })
}

/// Run a session command without capturing output, enforcing a hard deadline.
/// The child is always reaped so a timed-out helper cannot remain attached to
/// the desktop command that launched it.
pub fn run_status(program: &str, arguments: &[&str], timeout: Duration) -> bool {
    let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
    run_status_os(OsStr::new(program), &arguments, timeout)
}

/// Start a launcher such as xdg-open, which in its generic mode runs the
/// handler in the foreground. An early failing exit is a failure; a launcher
/// still running after `check` has opened the handler, so it is left to run
/// and reaped on its own thread instead of being killed at a deadline.
pub fn launch(program: &str, arguments: &[&str], check: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + check;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = std::thread::Builder::new()
                    .name("msime-launcher-reaper".to_owned())
                    .spawn(move || {
                        let _ = child.wait();
                    });
                return true;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// 命令在 `window` 内是否一直没有退出。到时仍在运行就结束并回收它，返回 `true`；起不来或提前退出（不论退出码）返回 `false`。用来探测只有在条件不满足时才会立即退出的常驻命令。
pub fn stays_running(program: &str, arguments: &[&str], window: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + window;
    let running = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => break true,
            Err(_) => break false,
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    running
}

/// Run a command with one path argument without requiring the path to be UTF-8.
pub fn run_status_path(program: &str, argument: &Path, timeout: Duration) -> bool {
    run_status_os(OsStr::new(program), &[argument.as_os_str()], timeout)
}

fn run_status_os(program: &OsStr, arguments: &[&OsStr], timeout: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

pub fn read_text(
    program: &str,
    arguments: &[&str],
    max_bytes: usize,
    timeout: Duration,
) -> Option<String> {
    read_text_bounded(program, arguments, max_bytes, timeout, false)
}

pub fn read_text_prefix(
    program: &str,
    arguments: &[&str],
    max_bytes: usize,
    timeout: Duration,
) -> Option<String> {
    read_text_bounded(program, arguments, max_bytes, timeout, true)
}

fn read_text_bounded(
    program: &str,
    arguments: &[&str],
    max_bytes: usize,
    timeout: Duration,
    prefix: bool,
) -> Option<String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let result = (|| {
        let mut output = child.stdout.take()?;
        let flags = fcntl_getfl(&output).ok()?;
        fcntl_setfl(&output, flags | OFlags::NONBLOCK).ok()?;
        let deadline = Instant::now() + timeout;
        let mut bytes = Vec::with_capacity(max_bytes + 1);
        let mut buffer = [0; 8192];
        let mut eof = false;
        loop {
            if Instant::now() >= deadline {
                return None;
            }
            if !eof {
                let remaining = (max_bytes + 1 - bytes.len()).min(buffer.len());
                match output.read(&mut buffer[..remaining]) {
                    Ok(0) => eof = true,
                    Ok(count) => {
                        bytes.extend_from_slice(&buffer[..count]);
                        if prefix && bytes.len() >= max_bytes {
                            bytes.truncate(max_bytes);
                            let end = match std::str::from_utf8(&bytes) {
                                Ok(_) => bytes.len(),
                                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                                Err(_) => return None,
                            };
                            bytes.truncate(end);
                            let text = String::from_utf8(bytes).ok()?;
                            return (!text.contains('\0')).then_some(text);
                        }
                        if bytes.len() > max_bytes {
                            return None;
                        }
                        continue;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => return None,
                }
            }
            if eof {
                if let Some(status) = child.try_wait().ok()? {
                    if !status.success() {
                        return None;
                    }
                    let text = String::from_utf8(bytes).ok()?;
                    return (!text.contains('\0')).then_some(text);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    // Reap the process on every path, including a full pipe, failed decoding or
    // timeout. Nonblocking reads also cover descendants holding stdout open.
    if result.is_none() || prefix {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

// Send private input through a bounded anonymous pipe with no diagnostics.
pub fn write_input(program: &str, arguments: &[&str], bytes: &[u8], timeout: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let result = (|| {
        let mut input = child.stdin.take()?;
        let flags = fcntl_getfl(&input).ok()?;
        fcntl_setfl(&input, flags | OFlags::NONBLOCK).ok()?;
        let deadline = Instant::now() + timeout;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            if Instant::now() >= deadline {
                return None;
            }
            match input.write(remaining) {
                Ok(0) => return None,
                Ok(count) => remaining = &remaining[count..],
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
        // EOF lets the tool finish consuming the private payload.
        drop(input);
        loop {
            if let Some(status) = child.try_wait().ok()? {
                return status.success().then_some(());
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result.is_some()
}

#[cfg(test)]
mod tests {
    use super::run_status;
    use std::time::Duration;

    #[test]
    fn run_status_reports_success_and_failure_without_output() {
        assert!(run_status(
            "/bin/sh",
            &["-c", "exit 0"],
            Duration::from_secs(1)
        ));
        assert!(!run_status(
            "/bin/sh",
            &["-c", "exit 7"],
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn run_status_terminates_a_stalled_command() {
        assert!(!run_status(
            "/bin/sh",
            &["-c", "sleep 1"],
            Duration::from_millis(20)
        ));
    }

    #[test]
    fn launch_fails_only_on_an_early_failing_exit() {
        let check = Duration::from_secs(1);
        assert!(super::launch("/bin/sh", &["-c", "exit 0"], check));
        assert!(!super::launch("/bin/sh", &["-c", "exit 4"], check));
        assert!(!super::launch("/nonexistent/msime-launcher", &[], check));
        // A launcher that keeps running is a handler that opened, not a hang.
        let started = std::time::Instant::now();
        assert!(super::launch(
            "/bin/sh",
            &["-c", "sleep 1"],
            Duration::from_millis(20)
        ));
        assert!(started.elapsed() < Duration::from_millis(900));
    }

    #[test]
    fn stays_running_distinguishes_an_early_exit() {
        let window = Duration::from_secs(2);
        assert!(!super::stays_running("/bin/sh", &["-c", "exit 1"], window));
        assert!(!super::stays_running("/bin/sh", &["-c", "exit 0"], window));
        assert!(!super::stays_running(
            "/nonexistent/msime-probe",
            &[],
            window
        ));
        let started = std::time::Instant::now();
        assert!(super::stays_running(
            "/bin/sh",
            &["-c", "sleep 5"],
            Duration::from_millis(200)
        ));
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[cfg(unix)]
    #[test]
    fn run_status_path_accepts_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(OsString::from_vec(vec![b's', b'y', b'n', 0x80]));
        std::fs::create_dir(&path).unwrap();
        assert!(super::run_status_path(
            "/bin/ls",
            &path,
            Duration::from_secs(1)
        ));
    }
}
