//! 阿里云机器翻译 OpenAPI：配置、ACS3 签名与严格响应解析，不执行网络请求。

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const REGIONS: [(&str, &str); 2] = [("cn-hangzhou", "杭州"), ("cn-hongkong", "香港")];
pub const EDITIONS: [(&str, &str); 2] = [("general", "通用版"), ("professional", "专业版")];
pub const SCENES: [(&str, &str); 6] = [
    ("title", "商品标题"),
    ("description", "商品描述"),
    ("communication", "商品沟通"),
    ("medical", "医疗"),
    ("social", "社交"),
    ("finance", "金融"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub access_key_id: String,
    pub access_key_secret: String,
    pub region: String,
    pub edition: String,
    pub scene: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            access_key_id: String::new(),
            access_key_secret: String::new(),
            region: "cn-hangzhou".into(),
            edition: "general".into(),
            scene: "social".into(),
        }
    }
}
impl Preferences {
    pub fn issue(&self) -> Option<&'static str> {
        if !self.access_key_id.is_empty()
            && (!crate::is_bounded_ascii_identifier(&self.access_key_id, 4096)
                || !super::is_valid_credential(&self.access_key_id)
                || self.access_key_id.chars().all(|c| c == '*'))
        {
            return Some("AccessKey ID 只能包含英文字母、数字、- 和 _，不能使用示例占位符");
        }
        if !self.access_key_secret.is_empty()
            && (!super::is_valid_credential(&self.access_key_secret)
                || self.access_key_secret.chars().all(|c| c == '*'))
        {
            return Some("AccessKey Secret 无效，不能使用控制字符或示例占位符");
        }
        if !REGIONS.iter().any(|(id, _)| *id == self.region) {
            return Some("请选择杭州或香港地域");
        }
        if !EDITIONS.iter().any(|(id, _)| *id == self.edition) {
            return Some("请选择通用版或专业版");
        }
        if !SCENES.iter().any(|(id, _)| *id == self.scene) {
            return Some("请选择有效的专业版场景");
        }
        None
    }
    pub fn ready(&self) -> bool {
        self.issue().is_none()
            && !self.access_key_id.is_empty()
            && !self.access_key_secret.is_empty()
    }
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Serialize)]
pub struct HttpRequest {
    pub url: String,
    pub method: &'static str,
    pub headers: std::collections::BTreeMap<String, String>,
    pub body_utf8: String,
    pub timeout_ms: u32,
    pub max_response_bytes: usize,
}

pub fn request(
    config: &Preferences,
    text: &str,
    source: &str,
    target: &str,
    timestamp: i64,
    nonce: &str,
) -> Option<HttpRequest> {
    if !config.enabled
        || !config.ready()
        || !super::is_valid_source_text(text)
        || !super::is_supported_translation_pair(source, target)
        || !crate::is_bounded_ascii_identifier(nonce, 128)
        || nonce.is_empty()
        || timestamp < 0
    {
        return None;
    }
    let utc = time::OffsetDateTime::from_unix_timestamp(timestamp).ok()?;
    if !(1970..=9999).contains(&utc.year()) {
        return None;
    }
    let date = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        utc.year(),
        utc.month() as u8,
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second()
    );
    let host = match config.region.as_str() {
        "cn-hangzhou" => "mt.cn-hangzhou.aliyuncs.com",
        "cn-hongkong" => "mt.aliyuncs.com",
        _ => return None,
    };
    let action = if config.edition == "professional" {
        "Translate"
    } else {
        "TranslateGeneral"
    };
    let scene = if config.edition == "professional" {
        &config.scene
    } else {
        "general"
    };
    let body_utf8: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("FormatType", "text"),
            ("Scene", scene),
            ("SourceLanguage", source),
            ("SourceText", text),
            ("TargetLanguage", target),
        ])
        .finish();
    let payload_hash = hex::encode(Sha256::digest(body_utf8.as_bytes()));
    let mut headers = std::collections::BTreeMap::from([
        (
            "content-type".into(),
            "application/x-www-form-urlencoded; charset=utf-8".into(),
        ),
        ("host".into(), host.into()),
        ("x-acs-action".into(), action.into()),
        ("x-acs-content-sha256".into(), payload_hash.clone()),
        ("x-acs-date".into(), date),
        ("x-acs-signature-nonce".into(), nonce.into()),
        ("x-acs-version".into(), "2018-10-12".into()),
    ]);
    let signed_headers = headers.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical_headers = headers
        .iter()
        .map(|(key, value)| format!("{key}:{value}\n"))
        .collect::<String>();
    let canonical = format!("POST\n/\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
    let to_sign = format!(
        "ACS3-HMAC-SHA256\n{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(config.access_key_secret.as_bytes()).ok()?;
    mac.update(to_sign.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    headers.insert(
        "Authorization".into(),
        format!(
            "ACS3-HMAC-SHA256 Credential={},SignedHeaders={signed_headers},Signature={signature}",
            config.access_key_id
        ),
    );
    headers.insert("Accept".into(), "application/json".into());
    Some(HttpRequest {
        url: format!("https://{host}/"),
        method: "POST",
        headers,
        body_utf8,
        timeout_ms: 2500,
        max_response_bytes: 262144,
    })
}

/// 合法空回答是 Some("")；错误、类型错误与超限回复是 None，不能负缓存成成功。
pub fn parse(body: &[u8]) -> Option<String> {
    if body.len() > 262144 {
        return None;
    }
    let root: Value = serde_json::from_slice(body).ok()?;
    if root.get("Code")?.as_u64()? != 200 {
        return None;
    }
    let text = root.get("Data")?.get("Translated")?.as_str()?;
    if text.len() > 4096 {
        return None;
    }
    Some(super::format_translation_gloss(text).unwrap_or_default())
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Catalog,
    Validate {
        config: Preferences,
    },
    Request {
        config: Preferences,
        text: String,
        source_language: String,
        target_language: String,
    },
    Parse {
        body_base64: String,
    },
}

pub fn execute(bytes: &[u8]) -> Result<Value, &'static str> {
    let operation: Operation =
        serde_json::from_slice(bytes).map_err(|_| "invalid Aliyun translation request")?;
    match operation {
        Operation::Catalog => Ok(
            json!({"regions":REGIONS, "editions":EDITIONS, "scenes":SCENES, "defaults":Preferences::default()}),
        ),
        Operation::Validate { config } => {
            Ok(json!({"issue":config.issue(), "ready":config.ready()}))
        }
        Operation::Request {
            config,
            text,
            source_language,
            target_language,
        } => {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            let nonce = uuid::Uuid::new_v4().simple().to_string();
            serde_json::to_value(request(
                &config,
                &text,
                &source_language,
                &target_language,
                now,
                &nonce,
            ))
            .map_err(|_| "invalid Aliyun descriptor")
        }
        Operation::Parse { body_base64 } => {
            let bytes = BASE64
                .decode(body_base64)
                .map_err(|_| "invalid Aliyun response encoding")?;
            let result = parse(&bytes);
            Ok(json!({"valid": result.is_some(), "translation": result}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Preferences {
        Preferences {
            enabled: true,
            access_key_id: "synthetic-id".into(),
            access_key_secret: "synthetic-secret".into(),
            ..Default::default()
        }
    }
    #[test]
    fn signs_exact_form_bytes_for_the_official_endpoint() {
        let value = request(
            &config(),
            "合成 &+测试",
            "zh",
            "en",
            1704067200,
            "synthetic-nonce",
        )
        .unwrap();
        assert_eq!(value.url, "https://mt.cn-hangzhou.aliyuncs.com/");
        assert_eq!(value.body_utf8, "FormatType=text&Scene=general&SourceLanguage=zh&SourceText=%E5%90%88%E6%88%90+%26%2B%E6%B5%8B%E8%AF%95&TargetLanguage=en");
        // 固定答案由独立 Python hashlib/hmac 脚本按官方 ACS3 公式计算。
        assert_eq!(value.headers["Authorization"], "ACS3-HMAC-SHA256 Credential=synthetic-id,SignedHeaders=content-type;host;x-acs-action;x-acs-content-sha256;x-acs-date;x-acs-signature-nonce;x-acs-version,Signature=64be86f887987f5025bf13b70be2ee308a3a148e7583c7129360b9d8830c2471");
        assert_eq!(value.headers["x-acs-date"], "2024-01-01T00:00:00Z");
        assert_ne!(
            value.headers["Authorization"],
            request(
                &config(),
                "不同合成值",
                "zh",
                "en",
                1704067200,
                "synthetic-nonce"
            )
            .unwrap()
            .headers["Authorization"]
        );
    }
    #[test]
    fn professional_action_region_and_scene_are_explicit() {
        let mut config = config();
        config.edition = "professional".into();
        config.region = "cn-hongkong".into();
        config.scene = "medical".into();
        let value = request(&config, "合成", "zh", "en", 1704067200, "nonce").unwrap();
        assert_eq!(value.url, "https://mt.aliyuncs.com/");
        assert_eq!(value.headers["x-acs-action"], "Translate");
        assert!(value.body_utf8.contains("Scene=medical"));
        config.region = "evil.invalid".into();
        assert!(request(&config, "合成", "zh", "en", 1704067200, "nonce").is_none());
    }
    #[test]
    fn rejects_incomplete_credentials_and_invalid_parameters_without_a_request() {
        let mut config = config();
        config.enabled = false;
        assert!(request(&config, "合成", "zh", "en", 1704067200, "nonce").is_none());
        config.enabled = true;
        config.access_key_secret.clear();
        assert!(!config.ready());
        config.access_key_secret = "<placeholder>".into();
        assert!(config.issue().is_some());
        for (text, source, target, nonce) in [
            ("", "zh", "en", "nonce"),
            ("合成\0", "zh", "en", "nonce"),
            ("合成", "zh", "invalid", "nonce"),
            ("合成", "zh", "en", "bad\r\nnonce"),
        ] {
            assert!(request(
                &super::tests::config(),
                text,
                source,
                target,
                1704067200,
                nonce
            )
            .is_none());
        }
    }
    #[test]
    fn response_errors_are_not_successful_empty_translations() {
        assert!(parse(b"{\"Code\":200,\"Data\":{\"Translated\":\"\xff\"}}").is_none());
        assert_eq!(
            parse(br#"{"Code":200,"Data":{"Translated":""}}"#),
            Some(String::new())
        );
        assert_eq!(
            parse(br#"{"Code":200,"Data":{"Translated":"synthetic\nanswer"}}"#),
            Some("synthetic answer".into())
        );
        for body in [
            r#"{"Code":10013,"Data":{"Translated":"stale"}}"#,
            r#"{"Code":"200","Data":{"Translated":"wrong"}}"#,
            r#"{"Code":200,"Data":{}}"#,
            r#"{"Code":200,"Data":{"Translated":null}}"#,
            "not json",
        ] {
            assert!(parse(body.as_bytes()).is_none());
        }
    }
    #[test]
    fn preferences_credentials_stay_local_and_are_redacted() {
        let preferences = crate::preferences::Preferences {
            aliyun_translation: config(),
            ..Default::default()
        };
        let export = crate::settings_document::export_document(&preferences).unwrap();
        assert!(!export.contains("aliyun_translation"));
        let diagnostic = preferences.redacted_for_diagnostics().to_string();
        assert!(!diagnostic.contains("synthetic-secret"));
        assert!(!diagnostic.contains("synthetic-id"));
        let retained = preferences.restored_to_defaults();
        assert_eq!(
            retained.aliyun_translation.access_key_secret,
            "synthetic-secret"
        );
        assert!(!retained.aliyun_translation.enabled);
    }
}
