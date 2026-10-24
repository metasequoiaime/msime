//! 导出设置选择器的按住录音目录，不读取用户偏好或凭据。

use msime_client_providers::voice::hold_key_catalog::hold_keys;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = hold_keys();
    let keys = catalog
        .iter()
        .map(|entry| serde_json::to_string(entry.key))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let mut typescript = format!("// 由共享 Rust 按住录音目录导出；通过 export_voice_hold_key_catalog 更新。\nexport const VOICE_HOLD_KEYS = [{keys}] as const;\n\nexport const VOICE_HOLD_PRESETS = [\n");
    let mut header = format!(
        "#pragma once\n// 由共享 Rust 按住录音目录导出；通过 export_voice_hold_key_catalog 更新。\n#include <array>\n#include <string_view>\n\nnamespace msime::voice {{\nstruct VoiceHoldKeyCatalogEntry {{\n  std::wstring_view key;\n  std::wstring_view label;\n  bool fallback;\n}};\ninline constexpr std::array<VoiceHoldKeyCatalogEntry, {}> kVoiceHoldKeys = {{{{\n",
        catalog.len()
    );
    for entry in &catalog {
        typescript.push_str("  {\n");
        for (name, value) in [
            ("key", entry.key),
            ("macos_label", entry.macos_label),
            ("desktop_label", entry.desktop_label),
            ("other_label", entry.other_label),
            ("macos_action", entry.macos_action),
            ("desktop_action", entry.desktop_action),
            ("other_action", entry.other_action),
        ] {
            typescript.push_str(&format!("    {name}: {},\n", serde_json::to_string(value)?));
        }
        typescript.push_str(&format!("    fallback: {},\n  }},\n", entry.fallback));
        header.push_str(&format!(
            "  {{L{}, L{}, {}}},\n",
            serde_json::to_string(entry.key)?,
            serde_json::to_string(entry.desktop_label)?,
            entry.fallback
        ));
    }
    typescript.push_str("] as const;\n");
    header.push_str("}};\n} // 命名空间 msime::voice\n");
    let write = match std::env::args().nth(1).as_deref() {
        Some("--write") => true,
        Some("--check") => false,
        _ => return Err("用法：export_voice_hold_key_catalog [--write|--check]".into()),
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, content) in [
        (
            "packages/ui/src/settings/voice-hold-key-catalog.ts",
            typescript,
        ),
        ("shared/voice/VoiceHoldKeyCatalog.h", header),
    ] {
        let path = root.join(relative);
        if write {
            std::fs::write(path, content)?;
        } else if std::fs::read_to_string(&path)? != content {
            return Err(format!(
                "按住录音目录已过期：{relative}，请运行 export_voice_hold_key_catalog --write"
            )
            .into());
        }
    }
    println!("按住录音目录{}完成", if write { "生成" } else { "检查" });
    Ok(())
}
