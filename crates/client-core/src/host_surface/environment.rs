//! 「关于」页「系统信息」里由宿主在运行时填进 [`HostCapabilities`](super::HostCapabilities) 的几项：发行版、内核、桌面会话、输入法框架和设备型号。
//!
//! 这里只做解析和清洗，读文件、读环境变量留给宿主。这些文本会被用户原样复制进公开的问题报告，所以只接受短的、不含控制字符的文本；读到别的就不报告，而不是报一个猜测。它们都描述机器和会话，不含账号、输入内容或任何路径。

/// 单项文本的长度上限，按字符计。发行版名（`Ubuntu 26.04.1 LTS`、`openSUSE Tumbleweed`）、设备型号都远短于它，超出说明读到的不是这类东西。
const MAX_LABEL_CHARS: usize = 80;

fn clean_label(raw: &str) -> Option<String> {
    let value = raw.trim();
    (!value.is_empty()
        && value.chars().count() <= MAX_LABEL_CHARS
        && !value.chars().any(char::is_control))
    .then(|| value.to_owned())
}

/// 只由可见 ASCII 组成的短标识（内核版本、`XDG_SESSION_TYPE`、`XDG_CURRENT_DESKTOP`）。
fn clean_token(raw: &str) -> Option<String> {
    let value = raw.trim();
    (!value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .then(|| value.to_owned())
}

/// os-release 的值：可以用双引号或单引号括起，双引号内 `\"`、`\\`、`` \` ``、`\$` 是转义（os-release(5)）。
fn os_release_value(raw: &str) -> String {
    let raw = raw.trim();
    let quoted = |quote: char| raw.len() >= 2 && raw.starts_with(quote) && raw.ends_with(quote);
    if quoted('\'') {
        return raw[1..raw.len() - 1].to_owned();
    }
    if !quoted('"') {
        return raw.to_owned();
    }
    let mut value = String::new();
    let mut characters = raw[1..raw.len() - 1].chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                value.push(escaped);
            }
        } else {
            value.push(character);
        }
    }
    value
}

/// `/etc/os-release`（或 `/usr/lib/os-release`）描述的发行版：优先 `PRETTY_NAME`，没有时用 `NAME` 加 `VERSION`。
pub fn os_release_name(document: &str) -> Option<String> {
    let mut pretty_name = None;
    let mut name = None;
    let mut version = None;
    for line in document.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let slot = match key {
            "PRETTY_NAME" => &mut pretty_name,
            "NAME" => &mut name,
            "VERSION" => &mut version,
            _ => continue,
        };
        *slot = clean_label(&os_release_value(value));
    }
    pretty_name.or_else(|| match (name, version) {
        (Some(name), Some(version)) => clean_label(&format!("{name} {version}")),
        (name, _) => name,
    })
}

/// 内核版本，即 `/proc/sys/kernel/osrelease` 的内容（`7.0.0-38-generic`）。
pub fn kernel_release(raw: &str) -> Option<String> {
    clean_token(raw)
}

/// 桌面会话：`XDG_CURRENT_DESKTOP` 和 `XDG_SESSION_TYPE`，写成 `ubuntu:GNOME (wayland)`。只有其中一项时只报那一项，两项都没有或都不像标识时不报。
pub fn desktop_session(
    current_desktop: Option<&str>,
    session_type: Option<&str>,
) -> Option<String> {
    match (
        current_desktop.and_then(clean_token),
        session_type.and_then(clean_token),
    ) {
        (Some(desktop), Some(session)) => Some(format!("{desktop} ({session})")),
        (Some(desktop), None) => Some(desktop),
        (None, Some(session)) => Some(session),
        (None, None) => None,
    }
}

/// 运行中的 Linux 宿主在候选面板状态文件里写下的输入法框架（`{"host": "ibus" | "fcitx5", ...}`，见 [`CandidatePanelLimit::status_file`](super::CandidatePanelLimit::status_file)）。它说的是水杉实际挂在哪个框架上，比从 `GTK_IM_MODULE` 之类的环境变量猜更准；认不出的名字不报。
pub fn input_method_framework_from_host_status(document: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(document).ok()?;
    match value.get("host")?.as_str()? {
        "ibus" => Some("IBus".to_owned()),
        "fcitx5" => Some("Fcitx5".to_owned()),
        _ => None,
    }
}

/// 设备型号：DMI 的厂商（`/sys/class/dmi/id/sys_vendor`）和产品名（`product_name`）。产品名已经以厂商开头时不再重复厂商。
pub fn device_model(vendor: Option<&str>, product: Option<&str>) -> Option<String> {
    let vendor = vendor.and_then(clean_label);
    let product = product.and_then(clean_label);
    match (vendor, product) {
        (Some(vendor), Some(product)) if product.starts_with(&vendor) => Some(product),
        (Some(vendor), Some(product)) => clean_label(&format!("{vendor} {product}")),
        (vendor, product) => product.or(vendor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_prefers_the_pretty_name() {
        let document = "NAME=\"Ubuntu\"\nVERSION=\"26.04.1 LTS (Synthetic)\"\nID=ubuntu\nPRETTY_NAME=\"Ubuntu 26.04.1 LTS\"\n";
        assert_eq!(
            os_release_name(document).as_deref(),
            Some("Ubuntu 26.04.1 LTS")
        );
    }

    #[test]
    fn os_release_falls_back_to_name_and_version() {
        assert_eq!(
            os_release_name("NAME=Fedora\nVERSION='44 (Workstation)'\n").as_deref(),
            Some("Fedora 44 (Workstation)")
        );
        assert_eq!(
            os_release_name("NAME=\"Arch Linux\"\n").as_deref(),
            Some("Arch Linux")
        );
    }

    #[test]
    fn os_release_unescapes_double_quoted_values() {
        assert_eq!(
            os_release_name("PRETTY_NAME=\"Synthetic \\\"Edge\\\" 1.0\"\n").as_deref(),
            Some("Synthetic \"Edge\" 1.0")
        );
    }

    #[test]
    fn os_release_reports_nothing_it_cannot_vouch_for() {
        assert_eq!(os_release_name(""), None);
        assert_eq!(os_release_name("ID=ubuntu\n"), None);
        assert_eq!(os_release_name("PRETTY_NAME=\"\"\n"), None);
        let long = format!("PRETTY_NAME=\"{}\"\n", "x".repeat(MAX_LABEL_CHARS + 1));
        assert_eq!(os_release_name(&long), None);
        assert_eq!(os_release_name("PRETTY_NAME=\"a\u{7}b\"\n"), None);
    }

    #[test]
    fn kernel_release_is_one_printable_token() {
        assert_eq!(
            kernel_release("7.0.0-38-generic\n").as_deref(),
            Some("7.0.0-38-generic")
        );
        assert_eq!(kernel_release(""), None);
        assert_eq!(kernel_release("7.0 generic"), None);
    }

    #[test]
    fn desktop_session_joins_the_desktop_and_the_session_type() {
        assert_eq!(
            desktop_session(Some("ubuntu:GNOME"), Some("wayland")).as_deref(),
            Some("ubuntu:GNOME (wayland)")
        );
        assert_eq!(desktop_session(Some("KDE"), None).as_deref(), Some("KDE"));
        assert_eq!(desktop_session(None, Some("x11")).as_deref(), Some("x11"));
        assert_eq!(desktop_session(Some(""), Some("tty session")), None);
        assert_eq!(desktop_session(None, None), None);
    }

    #[test]
    fn framework_comes_from_the_running_host_status() {
        assert_eq!(
            input_method_framework_from_host_status(r#"{"host":"ibus","limit":"gnome_shell"}"#)
                .as_deref(),
            Some("IBus")
        );
        assert_eq!(
            input_method_framework_from_host_status(r#"{"host":"fcitx5","limit":null}"#).as_deref(),
            Some("Fcitx5")
        );
        assert_eq!(
            input_method_framework_from_host_status(r#"{"host":"other"}"#),
            None
        );
        assert_eq!(
            input_method_framework_from_host_status(r#"{"limit":null}"#),
            None
        );
        assert_eq!(input_method_framework_from_host_status("not json"), None);
    }

    #[test]
    fn device_model_names_the_vendor_once() {
        assert_eq!(
            device_model(Some("SyntheticVendor"), Some("Model 14")).as_deref(),
            Some("SyntheticVendor Model 14")
        );
        assert_eq!(
            device_model(Some("SyntheticVendor"), Some("SyntheticVendor Model 14\n")).as_deref(),
            Some("SyntheticVendor Model 14")
        );
        assert_eq!(
            device_model(None, Some("Model 14")).as_deref(),
            Some("Model 14")
        );
        assert_eq!(
            device_model(Some("SyntheticVendor"), Some("")).as_deref(),
            Some("SyntheticVendor")
        );
        assert_eq!(device_model(None, None), None);
    }
}
