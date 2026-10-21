//! 导出共享设置页同步读取的公开语音预设；不读取偏好或凭据。

use msime_client_providers::voice::provider_catalog::{
    ASR_PROVIDERS, DOUBAO_STREAM_ENDPOINTS, POLISH_PROVIDERS,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = serde_json::json!({
        "asr": ASR_PROVIDERS,
        "polish": POLISH_PROVIDERS,
        "doubao_stream_endpoints": DOUBAO_STREAM_ENDPOINTS,
    });
    let generated = format!("{}\n", serde_json::to_string_pretty(&catalog)?);
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => print!("{generated}"),
        [flag] if flag == "--check" => {
            let committed = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../packages/ui/src/voice/voice-provider-catalog.json"),
            )?;
            if catalog != serde_json::from_str::<serde_json::Value>(&committed)? {
                return Err(
                    "共享语音目录已过期，请重新运行 export_voice_provider_catalog 导出 JSON".into(),
                );
            }
            println!("共享语音目录与 Rust 预设一致");
        }
        _ => return Err("用法：export_voice_provider_catalog [--check]".into()),
    }
    Ok(())
}
