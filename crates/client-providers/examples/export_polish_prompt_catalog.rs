//! 生成各宿主同步读取的润色目录，防止手工复制的名称和提示词漂移。

use msime_client_providers::voice::polish_catalog::POLISH_PROMPT_PRESETS;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let write = match args.as_slice() {
        [flag] if flag == "--write" => true,
        [flag] if flag == "--check" => false,
        _ => return Err("用法：export_polish_prompt_catalog --write|--check".into()),
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = serde_json::to_value(POLISH_PROMPT_PRESETS)?;
    let json = format!("{}\n", serde_json::to_string_pretty(&catalog)?);
    let prompts: serde_json::Map<_, _> = POLISH_PROMPT_PRESETS
        .iter()
        .map(|preset| (preset.id.to_owned(), serde_json::json!(preset.prompt)))
        .collect();
    let python = format!(
        "# 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。\nPROMPTS = {}\n",
        serde_json::to_string_pretty(&prompts)?
    );
    let ids = POLISH_PROMPT_PRESETS
        .iter()
        .map(|preset| serde_json::to_string(preset.id))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let typescript = format!("export const POLISH_PRESET_IDS = [{ids}] as const;\n");
    let titles = POLISH_PROMPT_PRESETS
        .iter()
        .map(|preset| serde_json::to_string(preset.title))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let java = format!(
        "package app.msime.android;\n\n// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。\nfinal class VoicePolishPresetCatalog {{\n    private VoicePolishPresetCatalog() {{}}\n\n    static String[] ids() {{\n        return new String[] {{{ids}}};\n    }}\n\n    static String[] titles() {{\n        return new String[] {{{titles}}};\n    }}\n}}\n"
    );
    let mut swift = String::from(
        "// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。\nenum VoicePolishPresetCatalog {\n  static let builtIns: [(id: String, title: String)] = [\n",
    );
    for preset in POLISH_PROMPT_PRESETS {
        swift.push_str(&format!("    ({:?}, {:?}),\n", preset.id, preset.title));
    }
    swift.push_str("  ]\n}\n");
    let mut header = format!(
        "#pragma once\n// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。\n#include <array>\n#include <string_view>\n\nnamespace msime::windows {{\nstruct PolishPromptCatalogEntry {{\n  std::string_view id;\n  std::string_view prompt;\n  std::string_view title;\n}};\ninline constexpr std::array<PolishPromptCatalogEntry, {}> kPolishPromptPresets = {{{{\n",
        POLISH_PROMPT_PRESETS.len()
    );
    let mut wide_header = format!(
        "#pragma once\n// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。\n#include <array>\n#include <string_view>\n\nnamespace msime::windows {{\nstruct PolishPromptWideCatalogEntry {{\n  std::wstring_view id;\n  std::wstring_view name;\n  std::wstring_view prompt;\n}};\ninline constexpr std::array<PolishPromptWideCatalogEntry, {}> kPolishPromptPresetsWide = {{{{\n",
        POLISH_PROMPT_PRESETS.len()
    );
    for preset in POLISH_PROMPT_PRESETS {
        if preset.prompt.contains(")MSIME_POLISH\"") {
            return Err("提示词包含 C++ raw string 分隔符".into());
        }
        header.push_str(&format!(
            "  {{{}, R\"MSIME_POLISH({})MSIME_POLISH\", {}}},\n",
            serde_json::to_string(preset.id)?,
            preset.prompt,
            serde_json::to_string(preset.title)?
        ));
        wide_header.push_str(&format!(
            "  {{L{}, L{}, LR\"MSIME_POLISH({})MSIME_POLISH\"}},\n",
            serde_json::to_string(preset.id)?,
            serde_json::to_string(preset.title)?,
            preset.prompt
        ));
    }
    header.push_str("}};\n} // 命名空间 msime::windows\n");
    wide_header.push_str("}};\n} // 命名空间 msime::windows\n");
    for (relative, generated, semantic) in [
        (
            "packages/ui/src/voice/polish-prompt-catalog.json",
            json,
            true,
        ),
        (
            "packages/ui/src/voice/polish-preset-ids.ts",
            typescript,
            false,
        ),
        (
            "platforms/linux/scripts/msime_voice_polish_prompts.py",
            python,
            false,
        ),
        (
            "platforms/ios/App/Services/VoicePolishPresetCatalog.swift",
            swift,
            false,
        ),
        (
            "platforms/android/java/app/msime/android/voice/VoicePolishPresetCatalog.java",
            java,
            false,
        ),
        ("shared/voice/PolishPromptCatalog.h", header, false),
        ("shared/voice/PolishPromptWideCatalog.h", wide_header, false),
    ] {
        let path = root.join(relative);
        if write {
            std::fs::write(path, generated)?;
        } else {
            let committed = std::fs::read_to_string(path)?;
            let matches = if semantic {
                serde_json::from_str::<serde_json::Value>(&committed)? == catalog
            } else {
                committed == generated
            };
            if !matches {
                return Err(format!(
                    "润色目录已过期：{relative}，请运行 export_polish_prompt_catalog --write"
                )
                .into());
            }
        }
    }
    println!("润色目录{}完成", if write { "生成" } else { "检查" });
    Ok(())
}
