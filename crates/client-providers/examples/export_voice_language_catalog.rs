//! 导出共享 UI 与 Windows 原生设置使用的语音语言目录；不读取偏好或凭据。

use msime_client_providers::voice::language_catalog::LANGUAGES;
use std::path::Path;

fn cxx(value: &str) -> Result<String, serde_json::Error> {
    Ok(serde_json::to_string(value)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generated = format!("{}\n", serde_json::to_string_pretty(LANGUAGES)?);
    let mut header = format!(
        "#pragma once\n// 由共享 Rust 语音语言目录导出；通过 export_voice_language_catalog 更新。\n#include <array>\n#include <string_view>\n\nnamespace msime::voice {{\nstruct VoiceLanguageCatalogEntry {{\n  std::wstring_view id;\n  std::wstring_view provider_value;\n  std::wstring_view provider_title;\n  std::wstring_view system_value;\n  std::wstring_view system_title;\n  std::wstring_view mobile_value;\n  std::wstring_view mobile_title;\n}};\ninline constexpr std::array<VoiceLanguageCatalogEntry, {}> kLanguages = {{{{\n",
        LANGUAGES.len()
    );
    for language in LANGUAGES {
        header.push_str(&format!(
            "  {{L{}, L{}, L{}, L{}, L{}, L{}, L{}}},\n",
            cxx(language.id)?,
            cxx(language.provider_value)?,
            cxx(language.provider_title)?,
            cxx(language.system_value)?,
            cxx(language.system_title)?,
            cxx(language.mobile_value)?,
            cxx(language.mobile_title)?,
        ));
    }
    header.push_str("}};\n} // 命名空间 msime::voice\n");

    let write = match std::env::args().nth(1).as_deref() {
        None => {
            print!("{generated}");
            return Ok(());
        }
        Some("--write") => true,
        Some("--check") => false,
        _ => return Err("用法：export_voice_language_catalog [--write|--check]".into()),
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, content) in [
        (
            "packages/ui/src/voice/voice-language-catalog.json",
            generated,
        ),
        ("shared/voice/VoiceLanguageCatalog.h", header),
    ] {
        let path = root.join(relative);
        if write {
            std::fs::write(path, content)?;
        } else if std::fs::read_to_string(&path)? != content {
            return Err(format!(
                "语音语言目录已过期：{relative}，请运行 export_voice_language_catalog --write"
            )
            .into());
        }
    }
    println!("语音语言目录{}完成", if write { "生成" } else { "检查" });
    Ok(())
}
