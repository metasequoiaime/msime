//! 导出共享设置页与 Windows 原生设置同步使用的公开 AI 目录，不读取配置或凭据。

use msime_client_providers::ai::provider_catalog::{ui_provider_catalog, PROVIDERS};
use std::path::Path;

fn wide(value: &str) -> Result<String, serde_json::Error> {
    Ok(format!("L{}", serde_json::to_string(value)?))
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
    }
    header.push_str("  };\n  return table;\n}\n} // 命名空间 msime::ai\n");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, content, semantic) in [
        (
            "packages/ui/src/settings/ai-provider-catalog.json",
            json,
            true,
        ),
        ("shared/ai/AiProviderCatalog.h", header, false),
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
