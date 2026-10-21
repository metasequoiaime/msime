//! 导出设置页与原生宿主同步读取的公开语音预设；不读取偏好或凭据。

use msime_client_providers::voice::provider_catalog::{
    ASR_PROVIDERS, DOUBAO_STREAM_ENDPOINTS, POLISH_PROVIDERS,
};
use std::path::Path;

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
    let mut endpoints =
        String::from("  static polishEndpoint(id: string): string {\n    switch (id) {\n");
    let mut models =
        String::from("  static polishModel(id: string): string {\n    switch (id) {\n");
    for preset in POLISH_PROVIDERS {
        let id = serde_json::to_string(preset.id)?;
        endpoints.push_str(&format!(
            "      case {id}:\n        return {};\n",
            serde_json::to_string(preset.endpoint)?
        ));
        models.push_str(&format!(
            "      case {id}:\n        return {};\n",
            serde_json::to_string(preset.model)?
        ));
    }
    let end = "      default:\n        return \"\";\n    }\n  }\n";
    let harmony = format!(
        "// 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。\nexport class VoiceProviderCatalog {{\n{endpoints}{end}\n{models}{end}}}\n"
    );
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
