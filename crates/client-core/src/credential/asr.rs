//! Batch ASR probes use synthetic silence, never microphone or user audio.
use crate::credential::probe::ProbeResult;
use serde_json::Value;
use std::{io::Read, time::Duration};

pub trait Transport {
    /// `format` 是 [`crate::voice::provider::asr_request_format`] 的取值，决定请求体怎么拼。
    fn upload(
        &self,
        format: &str,
        endpoint: &str,
        token: &str,
        model: &str,
        wav: Vec<u8>,
    ) -> Option<u16>;
}
pub struct HttpTransport;
impl Transport for HttpTransport {
    fn upload(
        &self,
        format: &str,
        endpoint: &str,
        token: &str,
        model: &str,
        wav: Vec<u8>,
    ) -> Option<u16> {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .ok()?;
        let request = client.post(endpoint).bearer_auth(token);
        let request = if format == crate::voice::provider::ASR_REQUEST_CHAT_AUDIO {
            request.json(&crate::voice::provider::chat_audio_request_body(
                model, &wav,
            ))
        } else {
            let file = reqwest::blocking::multipart::Part::bytes(wav)
                .file_name("credential-test.wav")
                .mime_str("audio/wav")
                .ok()?;
            let form = reqwest::blocking::multipart::Form::new()
                .part("file", file)
                .text("model", model.to_owned());
            request.multipart(form)
        };
        let response = request.send().ok()?;
        let status = response.status().as_u16();
        let bytes = std::io::copy(&mut response.take(256 * 1024 + 1), &mut std::io::sink()).ok()?;
        (bytes <= 256 * 1024).then_some(status)
    }
}

fn silent_wav() -> Vec<u8> {
    const DATA_BYTES: u32 = 16000 * 2;
    let mut wav = Vec::with_capacity(44 + DATA_BYTES as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + DATA_BYTES).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1_u16.to_le_bytes()); // mono
    wav.extend_from_slice(&16000_u32.to_le_bytes());
    wav.extend_from_slice(&32000_u32.to_le_bytes()); // bytes/s
    wav.extend_from_slice(&2_u16.to_le_bytes()); // block alignment
    wav.extend_from_slice(&16_u16.to_le_bytes()); // bits/sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&DATA_BYTES.to_le_bytes());
    wav.resize(44 + DATA_BYTES as usize, 0);
    wav
}

pub fn test(config: &Value, transport: &impl Transport) -> ProbeResult {
    let get = |key| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    use crate::voice::provider::{ASR_REQUEST_CHAT_AUDIO, ASR_REQUEST_MULTIPART};
    // 只测整句上传的服务；流式、本地和系统识别不走这里。
    let format = match crate::voice::provider::asr_request_format(get("provider")) {
        Some(format @ (ASR_REQUEST_MULTIPART | ASR_REQUEST_CHAT_AUDIO)) => format,
        _ => {
            return ProbeResult {
                ok: false,
                message: "此识别服务的凭据测试尚未接入。".into(),
            }
        }
    };
    let (token, endpoint, model) = (get("token"), get("endpoint"), get("model"));
    if !crate::credential::usable_token(token) {
        return ProbeResult {
            ok: false,
            message: "请先填写有效的 API Key。".into(),
        };
    }
    if !crate::credential::valid_https_endpoint_and_model(endpoint, model) {
        return ProbeResult {
            ok: false,
            message: "请填写有效的 HTTPS 转写接口地址和模型名。".into(),
        };
    }
    let status = transport.upload(format, endpoint, token, model, silent_wav());
    let ok = matches!(status, Some(200..=299));
    // The reference treats successful processing of silent audio as success;
    // no transcript is required or returned to the settings window.
    let message = match status {
        Some(200..=299) => "连接成功，API Key 和模型配置有效。",
        Some(401 | 403) => "认证失败，请检查 API Key 和访问权限。",
        Some(429) => "服务限流或额度不足，请稍后重试。",
        Some(_) => "转写测试未成功，请检查接口地址、模型和服务对静音音频的支持。",
        None => "连接失败或响应超出限制，请检查网络后重试。",
    };
    ProbeResult {
        ok,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fake(Option<u16>);
    impl Transport for Fake {
        fn upload(
            &self,
            _: &str,
            endpoint: &str,
            token: &str,
            model: &str,
            wav: Vec<u8>,
        ) -> Option<u16> {
            assert_eq!(endpoint, "https://fixture.invalid/asr");
            assert_eq!(token, "synthetic-key");
            assert_eq!(model, "fixture-model");
            assert_eq!(wav, silent_wav());
            self.0
        }
    }
    fn config(provider: &str) -> Value {
        json!({"provider":provider,"token":"synthetic-key","model":"fixture-model","endpoint":"https://fixture.invalid/asr"})
    }
    #[test]
    fn asr_probe_silence_is_one_second_pcm_wav() {
        let wav = silent_wav();
        assert_eq!(wav.len(), 32044);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 32036);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert!(wav[44..].iter().all(|byte| *byte == 0));
    }
    #[test]
    fn asr_probe_status_and_provider_routing() {
        for provider in [
            "openai",
            "siliconflow",
            "groq",
            "everyapi",
            "mistral",
            "bailian",
        ] {
            assert!(test(&config(provider), &Fake(Some(200))).ok);
            for status in [None, Some(301), Some(401), Some(403), Some(429), Some(500)] {
                let result = test(&config(provider), &Fake(status));
                assert!(!result.ok);
                assert!(!result.message.contains("synthetic-key"));
            }
        }
    }
    #[test]
    fn asr_probe_sends_each_provider_its_request_format() {
        struct Format(std::cell::RefCell<Vec<String>>);
        impl Transport for Format {
            fn upload(&self, format: &str, _: &str, _: &str, _: &str, _: Vec<u8>) -> Option<u16> {
                self.0.borrow_mut().push(format.to_owned());
                Some(200)
            }
        }
        let transport = Format(Default::default());
        assert!(test(&config("bailian"), &transport).ok);
        assert!(test(&config("openai"), &transport).ok);
        assert_eq!(*transport.0.borrow(), ["chat_audio", "multipart"]);
    }
    #[test]
    fn asr_probe_rejects_invalid_config_without_upload() {
        struct Never;
        impl Transport for Never {
            fn upload(&self, _: &str, _: &str, _: &str, _: &str, _: Vec<u8>) -> Option<u16> {
                panic!("unexpected upload")
            }
        }
        for (field, value) in [
            ("provider", "doubao"),
            ("provider", "system"),
            ("token", ""),
            ("token", "a\r\nb"),
            ("model", ""),
            ("endpoint", "http://fixture.invalid/asr"),
            ("endpoint", "wss://fixture.invalid/asr"),
            ("endpoint", "https://user:pass@fixture.invalid/asr"),
            ("endpoint", "https:///asr"),
        ] {
            let mut config = config("openai");
            config[field] = json!(value);
            assert!(!test(&config, &Never).ok);
        }
    }
}
