//! App 内反馈：`POST /v1/feedback`，multipart 请求体是一个 `payload`（JSON）加最多 3 个 `screenshots`（msime-cloud `internal/account/feedback.go`）。登录账号和匿名账号都可以提交。
//!
//! 发送前按服务端的规则校验：正文去掉首尾空白后码点数和 UTF-16 单元数都不超过 500，只允许 `\n` 和 `\t` 两种控制字符；`edition` 是 0–32 个 `[a-z0-9._-]`；诊断信息的键过白名单、控制字符换成空格、按字节截到 256、空值丢掉。截图每张不超过 1 MiB，只认 PNG 和 JPEG 的文件头，再只读头部的尺寸（单边不超过 8192、总像素不超过 8388608）；不重新编码，服务端自己会重新编码。

use super::*;
use std::collections::BTreeMap;

pub const FEEDBACK_PATH: &str = "/v1/feedback";
/// 反馈类型（服务端 `feedbackTypes`）。
pub const FEEDBACK_TYPES: [&str; 3] = ["bug", "suggestion", "dictionary"];
/// 诊断信息允许的键（服务端 `feedbackDiagnosticKeys`）。
pub const FEEDBACK_DIAGNOSTIC_KEYS: [&str; 9] = [
    "device",
    "os",
    "app_version",
    "edition",
    "scheme",
    "keyboard_layout",
    "skin",
    "ime_enabled",
    "ime_default",
];
/// 服务端认的平台（`sessionPlatforms`）。
pub const FEEDBACK_PLATFORMS: [&str; 6] =
    ["android", "ios", "macos", "windows", "linux", "harmony"];
pub const MAX_FEEDBACK_SCREENSHOTS: usize = 3;
pub const MAX_FEEDBACK_SCREENSHOT_BYTES: u64 = 1024 * 1024;
const MAX_FEEDBACK_TEXT: usize = 500;
const MAX_FEEDBACK_DIAGNOSTIC_BYTES: usize = 256;
const MAX_FEEDBACK_SCREENSHOT_SIDE: u32 = 8192;
const MAX_FEEDBACK_SCREENSHOT_PIXELS: u64 = 8 << 20;
/// 服务端覆盖三张截图上传和重新编码的时限。
const FEEDBACK_TIMEOUT: Duration = Duration::from_secs(60);

/// 校验过、可以发送的反馈。用 [`prepare_feedback`] 构造。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountFeedback {
    payload: Vec<u8>,
    screenshots: Vec<AccountAvatarImage>,
}

/// 宿主给的一份反馈，字段与服务端的 `payload` 相同；`screenshots` 是截图文件的绝对路径。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountFeedbackInput {
    pub kind: String,
    pub text: String,
    pub platform: String,
    pub app_version: String,
    pub edition: String,
    pub diagnostics: BTreeMap<String, String>,
    pub screenshots: Vec<std::path::PathBuf>,
}

/// 服务端收下反馈后的回执。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountFeedbackReceipt {
    pub id: String,
    pub status: String,
}

fn valid_edition(edition: &str) -> bool {
    edition.len() <= 32
        && edition.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

/// 1–`maximum` 个码点（去掉首尾空白后至少 1 个），控制字符只允许 `allowed` 里的，与服务端的 `resourceText` 相同。
fn valid_text(value: &str, maximum: usize, allowed: &[char]) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= maximum
        && !crate::has_disallowed_control_with_allowed(value, allowed)
}

/// 诊断信息：只留白名单里的键，控制字符换成空格，按字节截到 256，去掉首尾空白后为空的丢掉。
fn clean_diagnostics(diagnostics: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    diagnostics
        .iter()
        .filter(|(key, _)| FEEDBACK_DIAGNOSTIC_KEYS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            let mut value: String = value
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect();
            truncate_at_char_boundary(&mut value, MAX_FEEDBACK_DIAGNOSTIC_BYTES);
            (!value.trim().is_empty()).then(|| (key.clone(), value))
        })
        .collect()
}

/// 读一张截图：超过 1 MiB 是 `account_too_large`；不是普通文件、不是 PNG 或 JPEG 是 `account_invalid`；头部声明的尺寸超过服务端上限是 `account_too_large`。
fn read_screenshot(path: &Path) -> Result<AccountAvatarImage, AccountFailure> {
    if !path.is_absolute() {
        return Err(AccountError::Invalid.into());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| AccountError::Invalid)?;
    if metadata.len() > MAX_FEEDBACK_SCREENSHOT_BYTES {
        return Err(AccountFailure::too_large());
    }
    let image = read_account_avatar_upload(path)?;
    let format = match image.content_type {
        "image/png" => image::ImageFormat::Png,
        _ => image::ImageFormat::Jpeg,
    };
    let (width, height) =
        image::ImageReader::with_format(std::io::Cursor::new(&image.bytes), format)
            .into_dimensions()
            .map_err(|_| AccountError::Invalid)?;
    if width == 0 || height == 0 {
        return Err(AccountError::Invalid.into());
    }
    if width > MAX_FEEDBACK_SCREENSHOT_SIDE
        || height > MAX_FEEDBACK_SCREENSHOT_SIDE
        || u64::from(width) * u64::from(height) > MAX_FEEDBACK_SCREENSHOT_PIXELS
    {
        return Err(AccountFailure::too_large());
    }
    Ok(image)
}

/// 按服务端的规则校验并整理一份反馈，读进截图。正文去掉首尾空白后发送。
pub fn prepare_feedback(input: &AccountFeedbackInput) -> Result<AccountFeedback, AccountFailure> {
    let text = input.text.trim();
    if !FEEDBACK_TYPES.contains(&input.kind.as_str())
        || !valid_text(text, MAX_FEEDBACK_TEXT, &['\n', '\t'])
        || !crate::is_bounded_utf16(text, MAX_FEEDBACK_TEXT)
        || !FEEDBACK_PLATFORMS.contains(&input.platform.as_str())
        || !valid_text(&input.app_version, 64, &[])
        || !valid_edition(&input.edition)
        || input.screenshots.len() > MAX_FEEDBACK_SCREENSHOTS
    {
        return Err(AccountError::Invalid.into());
    }
    #[derive(Serialize)]
    struct Payload<'a> {
        #[serde(rename = "type")]
        kind: &'a str,
        text: &'a str,
        platform: &'a str,
        app_version: &'a str,
        edition: &'a str,
        diagnostics: BTreeMap<String, String>,
    }
    let payload = serde_json::to_vec(&Payload {
        kind: &input.kind,
        text,
        platform: &input.platform,
        app_version: &input.app_version,
        edition: &input.edition,
        diagnostics: clean_diagnostics(&input.diagnostics),
    })
    .map_err(|_| AccountError::Invalid)?;
    let screenshots = input
        .screenshots
        .iter()
        .map(|path| read_screenshot(path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AccountFeedback {
        payload,
        screenshots,
    })
}

impl AccountFeedback {
    /// 要发的 `payload` 部分（JSON），测试和自己发 multipart 的宿主看它。
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl BackendAccountClient {
    pub(super) fn send_feedback(
        &self,
        feedback: &AccountFeedback,
        access_token: &str,
    ) -> Result<AccountFeedbackReceipt, AccountFailure> {
        use reqwest::blocking::multipart::{Form, Part};
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let payload = Part::bytes(feedback.payload.clone())
            .mime_str("application/json")
            .map_err(|_| AccountError::Invalid)?;
        let mut form = Form::new().part("payload", payload);
        for (index, screenshot) in feedback.screenshots.iter().enumerate() {
            let extension = if screenshot.content_type == "image/png" {
                "png"
            } else {
                "jpg"
            };
            let part = Part::bytes(screenshot.bytes.clone())
                .file_name(format!("screenshot-{}.{extension}", index + 1))
                .mime_str(screenshot.content_type)
                .map_err(|_| AccountError::Invalid)?;
            form = form.part("screenshots", part);
        }
        let response = self
            .http()
            .post(self.url(FEEDBACK_PATH)?)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(access_token)
            .timeout(FEEDBACK_TIMEOUT)
            .multipart(form)
            .send()
            .map_err(|error| AccountFailure::from_send_error(&error))?;
        let bytes = read_detailed_response(response, MAX_JSON_BYTES)?;
        let receipt: AccountFeedbackReceipt =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        if !crate::text::is_bounded_ascii_identifier(&receipt.id, 64)
            || !crate::text::is_bounded_text(&receipt.status, 32)
        {
            return Err(AccountError::Unavailable.into());
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn input() -> AccountFeedbackInput {
        AccountFeedbackInput {
            kind: "bug".into(),
            text: "  候选窗不见了\n\t复现步骤见下  ".into(),
            platform: "macos".into(),
            app_version: "0.9.0".into(),
            edition: "full".into(),
            ..Default::default()
        }
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::GrayImage::new(width, height)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    #[test]
    fn the_payload_is_trimmed_and_diagnostics_are_cleaned() {
        let mut value = input();
        value.diagnostics = BTreeMap::from([
            ("device".to_owned(), "Mac\u{7}Book".to_owned()),
            ("os".to_owned(), "   ".to_owned()),
            ("skin".to_owned(), "长".repeat(100)),
            ("password".to_owned(), "不该出现".to_owned()),
        ]);
        let feedback = prepare_feedback(&value).unwrap();
        let payload: Value = serde_json::from_slice(feedback.payload()).unwrap();
        assert_eq!(payload["text"], "候选窗不见了\n\t复现步骤见下");
        assert_eq!(payload["type"], "bug");
        let diagnostics = payload["diagnostics"].as_object().unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics["device"], "Mac Book");
        assert_eq!(diagnostics["skin"].as_str().unwrap().len(), 255);
    }

    #[test]
    fn fields_follow_the_server_rules() {
        let invalid = |change: &dyn Fn(&mut AccountFeedbackInput)| {
            let mut value = input();
            change(&mut value);
            prepare_feedback(&value).unwrap_err().code()
        };
        assert_eq!(
            invalid(&|value| value.kind = "praise".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.text = " \n ".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.text = "回车\r换行".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.text = "字".repeat(501)),
            "account_invalid"
        );
        // 码点数不超过 500，但 UTF-16 单元数超过。
        assert_eq!(
            invalid(&|value| value.text = "😀".repeat(251)),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.platform = "web".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.app_version = "".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.app_version = "1\u{0}".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.edition = "Full".into()),
            "account_invalid"
        );
        assert_eq!(
            invalid(&|value| value.edition = "a".repeat(33)),
            "account_invalid"
        );
        let mut empty_edition = input();
        empty_edition.edition = String::new();
        assert!(prepare_feedback(&empty_edition).is_ok());
    }

    #[test]
    fn screenshots_are_checked_by_header_and_size() {
        let directory = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = directory.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let with = |paths: Vec<std::path::PathBuf>| {
            let mut value = input();
            value.screenshots = paths;
            prepare_feedback(&value)
        };
        let small = write("small.png", &png(1080, 2340));
        assert_eq!(with(vec![small.clone()]).unwrap().screenshots.len(), 1);
        assert_eq!(
            with(vec![write("wide.png", &png(8193, 10))])
                .unwrap_err()
                .code(),
            "account_too_large"
        );
        assert_eq!(
            with(vec![write("many.png", &png(4096, 2049))])
                .unwrap_err()
                .code(),
            "account_too_large"
        );
        assert_eq!(
            with(vec![write("text.png", b"not an image")])
                .unwrap_err()
                .code(),
            "account_invalid"
        );
        let mut large = png(10, 10);
        large.resize(MAX_FEEDBACK_SCREENSHOT_BYTES as usize + 1, 0);
        assert_eq!(
            with(vec![write("large.png", &large)]).unwrap_err().code(),
            "account_too_large"
        );
        assert_eq!(
            with(vec![small.clone(), small.clone(), small.clone(), small])
                .unwrap_err()
                .code(),
            "account_invalid"
        );
        assert_eq!(
            with(vec!["relative.png".into()]).unwrap_err().code(),
            "account_invalid"
        );
    }
}
