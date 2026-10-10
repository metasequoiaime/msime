//! Bounded, silent output capture for Linux session tools.
use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
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

/// 常驻命令在探测窗口内的结局，见 `probe_stays_running`。
#[derive(Debug, PartialEq, Eq)]
pub enum StartupProbe {
    /// 到时仍在运行。
    Running,
    /// 窗口内自己退出了。`code` 是退出码，被信号结束时为 `None`；`stderr` 是它退出前写下的 stderr 开头（最多 `STARTUP_PROBE_STDERR_BYTES` 字节）。
    Exited { code: Option<i32>, stderr: Vec<u8> },
    /// 起不来（例如没装），或者等待它时出错。
    Failed,
}

/// `probe_stays_running` 最多保留的 stderr 字节数，足够装下一条错误信息。
pub const STARTUP_PROBE_STDERR_BYTES: usize = 4096;

/// 运行一个常驻命令最多 `window`，看它是一直运行，还是提前退出、退出时说了什么。到时仍在运行就结束并回收它。用来探测只有在条件不满足时才会立即退出的常驻命令：调用方靠退出码和 stderr 区分「条件不满足」和一次偶然的失败。stderr 接到非阻塞管道，只在命令退出后读一次已写下的内容，不等 EOF：命令起的子进程可能继承这个 stderr 并一直开着它。
pub fn probe_stays_running(program: &str, arguments: &[&str], window: Duration) -> StartupProbe {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return StartupProbe::Failed;
    };
    let errors = child.stderr.take().filter(|errors| {
        fcntl_getfl(errors)
            .and_then(|flags| fcntl_setfl(errors, flags | OFlags::NONBLOCK))
            .is_ok()
    });
    let deadline = Instant::now() + window;
    let probe = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stderr = Vec::new();
                if let Some(mut errors) = errors {
                    let mut buffer = [0; STARTUP_PROBE_STDERR_BYTES];
                    while stderr.len() < STARTUP_PROBE_STDERR_BYTES {
                        let remaining = STARTUP_PROBE_STDERR_BYTES - stderr.len();
                        match errors.read(&mut buffer[..remaining]) {
                            Ok(0) => break,
                            Ok(count) => stderr.extend_from_slice(&buffer[..count]),
                            Err(error) if error.kind() == ErrorKind::Interrupted => {}
                            Err(_) => break,
                        }
                    }
                }
                break StartupProbe::Exited {
                    code: status.code(),
                    stderr,
                };
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => break StartupProbe::Running,
            Err(_) => break StartupProbe::Failed,
        }
    };
    kill_process_group(&child);
    let _ = child.kill();
    let _ = child.wait();
    probe
}

/// Run a command with one path argument without requiring the path to be UTF-8.
pub fn run_status_path(program: &str, argument: &Path, timeout: Duration) -> bool {
    run_status_os(OsStr::new(program), &[argument.as_os_str()], timeout)
}

fn run_status_os(program: &OsStr, arguments: &[&OsStr], timeout: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .process_group(0)
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
                kill_process_group(&child);
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

/// Helpers used for bounded commands own their process group so a child that
/// inherits a pipe cannot survive the request and keep that pipe open forever.
#[cfg(unix)]
fn kill_process_group(child: &Child) {
    if let Some(pid) = rustix::process::Pid::from_raw(child.id() as _) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_: &Child) {}

fn read_text_bounded(
    program: &str,
    arguments: &[&str],
    max_bytes: usize,
    timeout: Duration,
    prefix: bool,
) -> Option<String> {
    let mut child = Command::new(program)
        .args(arguments)
        .process_group(0)
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
        kill_process_group(&child);
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

// Send private input through a bounded anonymous pipe with no diagnostics.
pub fn write_input(program: &str, arguments: &[&str], bytes: &[u8], timeout: Duration) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .process_group(0)
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
        kill_process_group(&child);
        let _ = child.kill();
    }
    let _ = child.wait();
    result.is_some()
}

#[cfg(test)]
mod tests {
    use super::{run_status, write_input};
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

    #[cfg(unix)]
    #[test]
    fn run_status_does_not_leave_descendants_after_timeout() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("alive");
        let command = format!(
            "(sleep 0.2; echo alive > {}) & sleep 30",
            marker.to_str().unwrap()
        );
        assert!(!run_status(
            "/bin/sh",
            &["-c", &command],
            Duration::from_millis(50)
        ));
        std::thread::sleep(Duration::from_millis(400));
        assert!(!marker.exists(), "a descendant survived run_status cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn write_input_does_not_leave_descendants_after_timeout() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("alive");
        let command = format!(
            "cat >/dev/null; (sleep 0.2; echo alive > {}) & sleep 30",
            marker.to_str().unwrap()
        );
        assert!(!write_input(
            "/bin/sh",
            &["-c", &command],
            b"payload",
            Duration::from_millis(50)
        ));
        std::thread::sleep(Duration::from_millis(400));
        assert!(
            !marker.exists(),
            "a descendant survived write_input cleanup"
        );
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
    fn probe_stays_running_reports_how_the_command_ended() {
        use super::{probe_stays_running, StartupProbe};

        let window = Duration::from_secs(2);
        assert_eq!(
            probe_stays_running("/bin/sh", &["-c", "echo nope >&2; exit 1"], window),
            StartupProbe::Exited {
                code: Some(1),
                stderr: b"nope\n".to_vec(),
            }
        );
        assert_eq!(
            probe_stays_running("/bin/sh", &["-c", "exit 0"], window),
            StartupProbe::Exited {
                code: Some(0),
                stderr: Vec::new(),
            }
        );
        // 被信号结束没有退出码。
        assert_eq!(
            probe_stays_running("/bin/sh", &["-c", "kill -9 $$"], window),
            StartupProbe::Exited {
                code: None,
                stderr: Vec::new(),
            }
        );
        // 退出后由它的子进程继续开着 stderr，也不会让探测等到 EOF。
        let started = std::time::Instant::now();
        assert_eq!(
            probe_stays_running(
                "/bin/sh",
                &["-c", "echo held >&2; sleep 5 & exit 1"],
                window
            ),
            StartupProbe::Exited {
                code: Some(1),
                stderr: b"held\n".to_vec(),
            }
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        // 只保留 stderr 的开头。
        let StartupProbe::Exited { stderr, .. } = probe_stays_running(
            "/bin/sh",
            &["-c", "head -c 10000 /dev/zero >&2; exit 1"],
            window,
        ) else {
            panic!("命令应当提前退出");
        };
        assert_eq!(stderr.len(), super::STARTUP_PROBE_STDERR_BYTES);
        assert_eq!(
            probe_stays_running("/nonexistent/msime-probe", &[], window),
            StartupProbe::Failed
        );
        let started = std::time::Instant::now();
        assert_eq!(
            probe_stays_running("/bin/sh", &["-c", "sleep 5"], Duration::from_millis(200)),
            StartupProbe::Running
        );
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

    #[cfg(unix)]
    #[test]
    fn read_text_does_not_leave_descendants_holding_stdout() {
        let root = tempfile::tempdir().unwrap();
        let started = root.path().join("started");
        let alive = root.path().join("alive");
        let command = format!(
            "sleep 30 & (sleep 0.2; echo alive > {}) & echo started > {}; exit 0",
            alive.to_str().unwrap(),
            started.to_str().unwrap()
        );
        assert!(super::read_text(
            "/bin/sh",
            &["-c", &command],
            1024,
            Duration::from_millis(100)
        )
        .is_none());
        assert!(started.is_file());
        for _ in 0..100 {
            if alive.is_file() {
                panic!("a descendant survived read_text cleanup");
            }
            if !started.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!alive.is_file(), "a descendant survived read_text cleanup");
    }
}
