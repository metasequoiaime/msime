//! Bounded transfers through the Linux session's clipboard tools.

use std::sync::OnceLock;
use std::time::Duration;

use msime_client_core::clipboard::{normalize_text, MAX_TEXT_BYTES};

use super::linux_process::StartupProbe;

pub fn read_text(program: &str, arguments: &[&str]) -> Option<String> {
    super::linux_process::read_text_prefix(
        program,
        arguments,
        MAX_TEXT_BYTES,
        Duration::from_secs(1),
    )
    .map(|text| normalize_text(&text))
}

/// 能否用 `wl-paste` 读剪贴板而不抢焦点。合成器提供 data-control 协议（wlroots 系、KWin）时，`wl-paste` 经它在后台读取；不提供时（GNOME/Mutter），`wl-paste` 只能临时建一个窗口拿到键盘焦点才读得到，每读一次都会打断前台应用里输入法的预编辑，并让 gnome-shell 刷 `meta_window_set_stack_position_no_sync` 断言（#6514）。`wl-paste --watch` 只走 data-control，借它探测，判定规则见 `data_control_verdict`。只有确定的结论在进程内缓存：会话中途合成器不会换，`--watch` 本身不建窗口，探测一次不抢焦点。其余失败（没装 `wl-paste`、一时连不上合成器、被信号结束）这一次按不能用处理、改走 X11，下次读取再探测，免得一次偶然失败让整个进程都不再用 `wl-paste`。
pub fn wayland_paste_is_background() -> bool {
    static BACKGROUND: OnceLock<bool> = OnceLock::new();
    if let Some(&background) = BACKGROUND.get() {
        return background;
    }
    let probe = super::linux_process::probe_stays_running(
        "wl-paste",
        &["--type", "text", "--watch", "true"],
        Duration::from_secs(1),
    );
    match data_control_verdict(&probe) {
        Some(background) => *BACKGROUND.get_or_init(|| background),
        None => false,
    }
}

/// 合成器不提供 data-control 时 wl-clipboard 写在 stderr 里的那段文字：2.2 起是 "Watch mode requires a compositor that supports the data-control protocol"，更早是 "... wlroots data-control protocol"，编译时没带该协议的是 "wl-clipboard was built without support for the data-control protocol"。与 `msime-linux-clipboard-monitor` 的 `WAYLAND_NO_DATA_CONTROL` 一致。
const WAYLAND_NO_DATA_CONTROL: &[u8] = b"data-control protocol";

/// 从一次 `wl-paste --watch` 探测得出可以缓存的结论：`Some(true)` 是一直在运行，合成器提供 data-control；`Some(false)` 是以正数状态退出且 stderr 里有 `WAYLAND_NO_DATA_CONTROL`，合成器不提供。其余结局（起不来、退出码为 0、被信号结束、别的错误信息）说明不了合成器支持什么，返回 `None`，不缓存。
fn data_control_verdict(probe: &StartupProbe) -> Option<bool> {
    match probe {
        StartupProbe::Running => Some(true),
        StartupProbe::Exited {
            code: Some(code),
            stderr,
        } if *code > 0
            && stderr
                .windows(WAYLAND_NO_DATA_CONTROL.len())
                .any(|window| window == WAYLAND_NO_DATA_CONTROL) =>
        {
            Some(false)
        }
        StartupProbe::Exited { .. } | StartupProbe::Failed => None,
    }
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
    use super::{data_control_verdict, targets_mark_secret, StartupProbe};

    fn exited(code: Option<i32>, stderr: &str) -> StartupProbe {
        StartupProbe::Exited {
            code,
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn only_the_missing_data_control_message_is_a_cached_no() {
        assert_eq!(data_control_verdict(&StartupProbe::Running), Some(true));
        // wl-clipboard 2.2 起、2.1 及更早、以及编译时没带该协议时的措辞。
        for message in [
            "Watch mode requires a compositor that supports the data-control protocol\n",
            "Watch mode requires a compositor that supports wlroots data-control protocol\n",
            "wl-clipboard was built without support for the data-control protocol\n",
        ] {
            assert_eq!(
                data_control_verdict(&exited(Some(1), message)),
                Some(false),
                "{message}"
            );
        }
        // 一时连不上合成器、退出码为 0、被信号结束、没装 `wl-paste`：都不缓存，下次再探测。
        assert_eq!(
            data_control_verdict(&exited(Some(1), "Failed to connect to a Wayland server\n")),
            None
        );
        assert_eq!(data_control_verdict(&exited(Some(1), "")), None);
        assert_eq!(
            data_control_verdict(&exited(
                Some(0),
                "Watch mode requires a compositor that supports the data-control protocol\n"
            )),
            None
        );
        assert_eq!(
            data_control_verdict(&exited(
                None,
                "Watch mode requires a compositor that supports the data-control protocol\n"
            )),
            None
        );
        assert_eq!(data_control_verdict(&StartupProbe::Failed), None);
    }

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
