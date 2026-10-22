//! 导出共享设置页、Windows 与 iOS 原生设置使用的公开 AI 目录，不读取配置或凭据。

use msime_client_providers::ai::provider_catalog::{ui_provider_catalog, PROVIDERS};
use std::path::Path;

fn wide(value: &str) -> Result<String, serde_json::Error> {
    Ok(format!("L{}", serde_json::to_string(value)?))
}

fn swift_string(value: &str) -> String {
    let mut result = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            character if character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') => {
                result.push_str(&format!("\\u{{{:x}}}", character as u32));
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = serde_json::to_value(ui_provider_catalog())?;
    let json = format!("{}\n", serde_json::to_string_pretty(&catalog)?);
    let write = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => {
            print!("{json}");
            return Ok(());
        }
        [flag] if flag == "--write" => true,
        [flag] if flag == "--check" => false,
        _ => return Err("用法：export_ai_provider_catalog [--write|--check]".into()),
    };
    let mut header = String::from(
        "#pragma once\n// 由共享 Rust AI 服务目录导出；通过 export_ai_provider_catalog 更新。\n#include <string_view>\n#include <vector>\n\nnamespace msime::ai {\nstruct ProviderPreset {\n  std::wstring_view id;\n  std::wstring_view title;\n  std::wstring_view endpoint;\n  std::wstring_view model;\n  std::vector<std::wstring_view> models;\n  std::wstring_view documentation;\n};\ninline const std::vector<ProviderPreset>& provider_catalog() {\n  static const std::vector<ProviderPreset> table{\n",
    );
    let mut swift = String::from(
        "// 由共享 Rust AI 服务目录导出；通过 export_ai_provider_catalog 更新。\nenum AIProviderCatalog {\n  struct Provider: Sendable {\n    let title: String\n    let endpoint: String\n    let models: [String]\n    let documentation: String\n  }\n\n  static func provider(id: String) -> Provider? { providers[id] }\n\n  private static let providers: [String: Provider] = [\n",
    );
    for preset in PROVIDERS {
        let models = preset
            .models
            .iter()
            .map(|model| wide(model))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        header.push_str(&format!(
            "    {{{}, {}, {}, {}, {{{models}}}, {}}},\n",
            wide(preset.id)?,
            wide(preset.title)?,
            wide(preset.endpoint)?,
            wide(preset.model)?,
            wide(preset.documentation)?
        ));
        let models = preset
            .models
            .iter()
            .map(|model| swift_string(model))
            .collect::<Vec<_>>()
            .join(", ");
        swift.push_str(&format!(
            "    {}: Provider(title: {}, endpoint: {}, models: [{models}], documentation: {}),\n",
            swift_string(preset.id),
            swift_string(preset.title),
            swift_string(preset.endpoint),
            swift_string(preset.documentation),
        ));
    }
    header.push_str("  };\n  return table;\n}\n} // 命名空间 msime::ai\n");
    swift.push_str("  ]\n}\n");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, content, semantic) in [
        (
            "packages/ui/src/settings/ai-provider-catalog.json",
            json,
            true,
        ),
        ("shared/ai/AiProviderCatalog.h", header, false),
        (
            "platforms/ios/App/Services/AIProviderCatalog.swift",
            swift,
            false,
        ),
    ] {
        let path = root.join(relative);
        if write {
            std::fs::write(path, content)?;
        } else {
            let committed = std::fs::read_to_string(path)?;
            let matches = if semantic {
                catalog == serde_json::from_str::<serde_json::Value>(&committed)?
            } else {
                content == committed
            };
            if !matches {
                return Err(format!(
                    "AI 服务目录已过期：{relative}，请运行 export_ai_provider_catalog --write"
                )
                .into());
            }
        }
    }
    println!("AI 服务目录{}完成", if write { "生成" } else { "检查" });
    Ok(())
}
