//! 导出设置页与原生宿主同步读取的公开语音预设；不读取偏好或凭据。

use msime_client_providers::voice::provider::{
    asr_request_format, ASR_REQUEST_CHAT_AUDIO, ASR_REQUEST_MULTIPART,
};
use msime_client_providers::voice::provider_catalog::{
    ASR_PROVIDERS, DOUBAO_STREAM_ENDPOINTS, POLISH_PROVIDERS,
};
use std::collections::BTreeMap;
use std::path::Path;

fn harmony_method<'a>(
    name: &str,
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<String, serde_json::Error> {
    let mut method = format!("  static {name}(id: string): string {{\n    switch (id) {{\n");
    for (id, value) in values {
        method.push_str(&format!(
            "      case {}:\n        return {};\n",
            serde_json::to_string(id)?,
            serde_json::to_string(value)?
        ));
    }
    method.push_str("      default:\n        return \"\";\n    }\n  }\n");
    Ok(method)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = serde_json::json!({
        "asr": ASR_PROVIDERS,
        "polish": POLISH_PROVIDERS,
        "doubao_stream_endpoints": DOUBAO_STREAM_ENDPOINTS,
    });
    let generated = format!("{}\n", serde_json::to_string_pretty(&catalog)?);
    let args: Vec<_> = std::env::args().skip(1).collect();
    let write = match args.as_slice() {
        [] => {
            print!("{generated}");
            return Ok(());
        }
        [flag] if flag == "--write" => true,
        [flag] if flag == "--check" => false,
        _ => return Err("用法：export_voice_provider_catalog [--write|--check]".into()),
    };
    let http_asr: Vec<_> = ASR_PROVIDERS
        .iter()
        .filter_map(|preset| {
            let format = asr_request_format(preset.id)?;
            matches!(format, ASR_REQUEST_MULTIPART | ASR_REQUEST_CHAT_AUDIO)
                .then_some((preset, format))
        })
        .collect();
    let methods = [
        harmony_method(
            "polishEndpoint",
            POLISH_PROVIDERS.iter().map(|p| (p.id, p.endpoint)),
        )?,
        harmony_method(
            "polishModel",
            POLISH_PROVIDERS.iter().map(|p| (p.id, p.model)),
        )?,
        harmony_method(
            "httpAsrEndpoint",
            http_asr.iter().map(|(p, _)| (p.id, p.endpoint)),
        )?,
        harmony_method(
            "httpAsrModel",
            http_asr.iter().map(|(p, _)| (p.id, p.model)),
        )?,
        harmony_method(
            "httpAsrRequestFormat",
            http_asr.iter().map(|(p, format)| (p.id, *format)),
        )?,
    ]
    .join("\n");
    let harmony = format!(
        "// 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。\nexport class VoiceProviderCatalog {{\n{methods}}}\n"
    );
    let models = serde_json::json!({
        "asr": ASR_PROVIDERS.iter().map(|p| (p.id, p.model)).collect::<BTreeMap<_, _>>(),
        "polish": POLISH_PROVIDERS.iter().map(|p| (p.id, p.model)).collect::<BTreeMap<_, _>>(),
    });
    let endpoints = serde_json::json!({
        "asr": ASR_PROVIDERS.iter().map(|p| (p.id, p.endpoint)).collect::<BTreeMap<_, _>>(),
        "polish": POLISH_PROVIDERS.iter().map(|p| (p.id, p.endpoint)).collect::<BTreeMap<_, _>>(),
    });
    let formats: BTreeMap<_, _> = http_asr.iter().map(|(p, format)| (p.id, *format)).collect();
    let python = format!(
        "# 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。\nDEFAULT_MODELS = {}\nDEFAULT_ENDPOINTS = {}\nREQUEST_FORMATS = {}\nASR_PROVIDERS = set(DEFAULT_MODELS[\"asr\"])\nPOLISH_PROVIDERS = set(DEFAULT_MODELS[\"polish\"])\n",
        serde_json::to_string_pretty(&models)?,
        serde_json::to_string_pretty(&endpoints)?,
        serde_json::to_string_pretty(&formats)?
    );
    let mut header = format!(
        "#pragma once\n// 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。\n#include <array>\n#include <string_view>\n\nnamespace msime::voice {{\nstruct VoiceProviderCatalogEntry {{\n  std::string_view id;\n  std::string_view title;\n  std::string_view endpoint;\n  std::string_view model;\n}};\ninline constexpr std::array<VoiceProviderCatalogEntry, {}> kAsrProviders = {{{{\n",
        ASR_PROVIDERS.len()
    );
    for preset in ASR_PROVIDERS {
        header.push_str(&format!(
            "  {{{}, {}, {}, {}}},\n",
            serde_json::to_string(preset.id)?,
            serde_json::to_string(preset.title)?,
            serde_json::to_string(preset.endpoint)?,
            serde_json::to_string(preset.model)?
        ));
    }
    header.push_str("}};\n} // 命名空间 msime::voice\n");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (relative, content, semantic) in [
        (
            "packages/ui/src/voice/voice-provider-catalog.json",
            generated,
            true,
        ),
        (
            "platforms/harmony/entry/src/main/ets/keyboard/input/VoiceProviderCatalog.ts",
            harmony,
            false,
        ),
        (
            "platforms/linux/scripts/msime_voice_provider_catalog.py",
            python,
            false,
        ),
        ("shared/voice/VoiceProviderCatalog.h", header, false),
    ] {
        let path = root.join(relative);
        if write {
            std::fs::write(path, content)?;
        } else {
            let committed = std::fs::read_to_string(path)?;
            let matches = if semantic {
                catalog == serde_json::from_str::<serde_json::Value>(&committed)?
            } else {
                committed == content
            };
            if !matches {
                return Err(format!(
                    "语音服务目录已过期：{relative}，请运行 export_voice_provider_catalog --write"
                )
                .into());
            }
        }
    }
    println!("语音服务目录{}完成", if write { "生成" } else { "检查" });
    Ok(())
}
