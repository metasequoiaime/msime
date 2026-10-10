//! Bounded transfers through the Linux session's clipboard tools.

use std::sync::OnceLock;
use std::time::Duration;

use msime_client_core::clipboard::{normalize_text, MAX_TEXT_BYTES};

pub fn read_text(program: &str, arguments: &[&str]) -> Option<String> {
    super::linux_process::read_text_prefix(
        program,
        arguments,
        MAX_TEXT_BYTES,
        Duration::from_secs(1),
    )
    .map(|text| normalize_text(&text))
}

/// 能否用 `wl-paste` 读剪贴板而不抢焦点。合成器提供 data-control 协议（wlroots 系、KWin）时，`wl-paste` 经它在后台读取；不提供时（GNOME/Mutter），`wl-paste` 只能临时建一个窗口拿到键盘焦点才读得到，每读一次都会打断前台应用里输入法的预编辑，并让 gnome-shell 刷 `meta_window_set_stack_position_no_sync` 断言（#6514）。`wl-paste --watch` 只走 data-control，没有它就立即以非零状态退出，借此判断；没装 `wl-paste` 也算不能用。结果在进程内缓存：会话中途合成器不会换，`--watch` 本身不建窗口，探测一次不抢焦点。
pub fn wayland_paste_is_background() -> bool {
    static BACKGROUND: OnceLock<bool> = OnceLock::new();
    *BACKGROUND.get_or_init(|| {
        super::linux_process::stays_running(
            "wl-paste",
            &["--type", "text", "--watch", "true"],
            Duration::from_secs(1),
        )
    })
}

pub fn write_text(program: &str, arguments: &[&str], text: &str) -> bool {
    super::linux_process::write_input(program, arguments, text.as_bytes(), Duration::from_secs(2))
}

/// Whether the clipboard owner marks its offer as a secret. KeePassXC, KWallet
/// and Bitwarden set `x-kde-passwordManagerHint`; its presence is the signal.
/// The nspasteboard.org names match what the macOS reader refuses.
pub fn targets_mark_secret(targets: &str) -> bool {
    targets.lines().map(str::trim).any(|target| {
        matches!(
            target,
            "x-kde-passwordManagerHint"
                | "org.nspasteboard.ConcealedType"
                | "org.nspasteboard.TransientType"
        )
    })
}

pub fn offers_secret(program: &str, arguments: &[&str]) -> bool {
    super::linux_process::read_text_prefix(program, arguments, 4096, Duration::from_secs(1))
        .is_some_and(|targets| targets_mark_secret(&targets))
}

#[cfg(test)]
mod tests {
    use super::targets_mark_secret;

    #[test]
    fn password_manager_targets_mark_the_copy_secret() {
        assert!(targets_mark_secret(
            "text/plain\nx-kde-passwordManagerHint\nUTF8_STRING\n"
        ));
        assert!(targets_mark_secret(
            "TARGETS\r\norg.nspasteboard.ConcealedType\r\n"
        ));
        assert!(!targets_mark_secret("text/plain\nUTF8_STRING\nTARGETS\n"));
        assert!(!targets_mark_secret("x-kde-passwordManagerHint-other\n"));
        assert!(!targets_mark_secret(""));
    }
}
