//! 真实权重验收入口；只接受合成测试文本，不用于产品调用。
use msime_local_translation::Translator;
use std::path::Path;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("missing model path")?;
    let started = Instant::now();
    let model = Translator::load(Path::new(&path))?;
    println!("load_ms={}", started.elapsed().as_millis());
    for language in ["en", "ja", "fr"] {
        let started = Instant::now();
        let result = model.translate(
            "这是一段用于测试离线翻译的合成文本。",
            language,
            started + Duration::from_secs(30),
            &|| false,
        )?;
        println!(
            "{language}: {result} ({} ms)",
            started.elapsed().as_millis()
        );
    }
    Ok(())
}
