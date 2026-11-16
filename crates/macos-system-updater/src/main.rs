//! `msime-system-updater`: launchd runs this as root whenever the inbox holds something (see `lib.rs`). It takes no arguments and reads nothing from its environment, so nothing an unprivileged caller controls reaches it except the contents of the inbox.

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    use msime_macos_system_updater::{drain, prepare, Layout, SystemHost};
    if !rustix::process::geteuid().is_root() {
        eprintln!("msime-system-updater: must be started by launchd as root");
        return std::process::ExitCode::from(1);
    }
    let layout = Layout::system();
    if let Err(error) = prepare(&layout) {
        eprintln!("msime-system-updater: cannot prepare its directories: {error}");
        return std::process::ExitCode::from(1);
    }
    for (name, outcome) in drain(&layout, &SystemHost) {
        eprintln!(
            "msime-system-updater: {}: {outcome}",
            name.to_string_lossy()
        );
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("msime-system-updater: only macOS has /Library/Input Methods");
    std::process::ExitCode::from(1)
}
