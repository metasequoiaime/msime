#![cfg(feature = "runtime")]

use msime_local_translation::{Error, Translator};
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
#[ignore = "需要已校验的真实权重；通过 MSIME_TRANSLATION_TEST_MODEL 指定路径"]
fn real_model_translates_multiple_languages_and_rejects_cancelled_work() {
    let path = std::env::var("MSIME_TRANSLATION_TEST_MODEL").expect("model path required");
    let model = Translator::load(Path::new(&path)).expect("load verified model");
    let input = "这是一段用于测试离线翻译的合成文本。";
    for (language, expected) in [
        ("en", "synthetic"),
        ("ja", "オフライン"),
        ("fr", "synthétique"),
    ] {
        let output = model
            .translate(
                input,
                language,
                Instant::now() + Duration::from_secs(30),
                &|| false,
            )
            .expect("translation");
        assert!(output.contains(expected), "language={language}");
    }
    assert_eq!(
        model.translate(
            input,
            "en",
            Instant::now() + Duration::from_secs(30),
            &|| true
        ),
        Err(Error::Cancelled)
    );
    assert_eq!(
        model.translate(input, "en", Instant::now(), &|| false),
        Err(Error::Timeout)
    );
    assert_eq!(
        model.translate(
            input,
            "xx",
            Instant::now() + Duration::from_secs(30),
            &|| false
        ),
        Err(Error::InvalidInput)
    );
    // 在预填充已经开始后取消，必须丢弃这次生成；下一次请求使用干净上下文。
    let checks = std::cell::Cell::new(0);
    assert_eq!(
        model.translate(
            input,
            "en",
            Instant::now() + Duration::from_secs(30),
            &|| {
                checks.set(checks.get() + 1);
                checks.get() > 2
            }
        ),
        Err(Error::Cancelled)
    );
    let output = model
        .translate(
            "离线翻译测试。",
            "en",
            Instant::now() + Duration::from_secs(30),
            &|| false,
        )
        .unwrap();
    assert!(output.to_lowercase().contains("translation"));
}
