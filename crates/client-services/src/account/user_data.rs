//! 云端数据：`GET /v1/users/me/data` 汇总各分区，`GET /v1/users/me/data/export` 把本人的数据流式导出成 zip，`DELETE /v1/users/me/data` 清空所选分区（msime-cloud `internal/account/user_data.go`）。
//!
//! 删除成功后服务端把 phrases 置为 `[]`、settings 置为 `{}` 并推进修订号，词库写入一条 `reset` 变更；本模块不碰本机的同步状态，同步状态机自己识别「云端被清空」。

use super::*;
use std::collections::HashSet;

pub const DATA_PATH: &str = "/v1/users/me/data";
pub const DATA_EXPORT_PATH: &str = "/v1/users/me/data/export";
/// 能删除的分区（服务端 `userDataDeletable`）。账号、会话和社区作品分别走注销账号、移除设备和作品管理。
pub const DATA_SECTIONS: [&str; 5] = ["preferences", "dictionary", "phrases", "clipboard", "voice"];
/// 导出包的上限。包里最大的是 `dictionary.ndjson`，整份词库快照最多 512 MiB，压缩后只会更小；其余几个 JSON 文件各在 1 MiB 以内。
pub const MAX_DATA_EXPORT_BYTES: u64 = 640 * 1024 * 1024;
/// 服务端导出路由的时限是 2 分钟，客户端多等 10 秒。
const DATA_EXPORT_TIMEOUT: Duration = Duration::from_secs(130);
const MAX_DATA_SECTIONS: usize = 32;
/// 服务端的 `Content-Disposition` 读不出合适的文件名时用它。
const DEFAULT_EXPORT_FILENAME: &str = "msime-data.zip";

/// 汇总里的一个分区。`bytes` 是服务端报告的行大小之和，作统计用，不是精确的磁盘占用。服务端以后新增的分区 id 原样保留。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDataSection {
    pub id: String,
    pub bytes: i64,
    pub items: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDataSummary {
    pub bytes: i64,
    pub sections: Vec<AccountDataSection>,
}

/// 写好的导出包：`path` 是宿主给的目标路径，`bytes` 是文件大小，`filename` 是服务端建议的文件名。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDataExport {
    pub path: String,
    pub bytes: u64,
    pub filename: String,
}

/// 要删除的分区：非空、不重复，每个都是 [`DATA_SECTIONS`] 之一。
pub fn valid_data_sections(sections: &[String]) -> bool {
    let mut seen = HashSet::new();
    !sections.is_empty()
        && sections.iter().all(|section| {
            DATA_SECTIONS.contains(&section.as_str()) && seen.insert(section.as_str())
        })
}

fn valid_section_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

fn validate_summary(summary: &AccountDataSummary) -> Result<(), AccountError> {
    if summary.bytes < 0
        || summary.sections.len() > MAX_DATA_SECTIONS
        || summary
            .sections
            .iter()
            .any(|section| !valid_section_id(&section.id) || section.bytes < 0 || section.items < 0)
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// `Content-Disposition: attachment; filename="msime-data-2026-10-16.zip"` 里的文件名；不是 1–64 字节、只含字母数字和 `._-`、以 `.zip` 结尾的普通文件名时用 [`DEFAULT_EXPORT_FILENAME`]。
fn export_filename(header: Option<&str>) -> String {
    header
        .and_then(|value| {
            value
                .split(';')
                .map(str::trim)
                .find_map(|part| part.strip_prefix("filename="))
        })
        .map(|name| name.trim_matches('"'))
        .filter(|name| {
            name.len() <= 64
                && name.ends_with(".zip")
                && !name.starts_with('.')
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
        .unwrap_or(DEFAULT_EXPORT_FILENAME)
        .to_owned()
}

/// 导出包能不能按 zip 打开：只读中央目录，不解压内容。服务端开始写响应后出错只能中断连接，宿主拿到的会是截断的包，读不出中央目录。
fn readable_zip<R: std::io::Read + std::io::Seek>(reader: R) -> bool {
    zip::ZipArchive::new(reader).is_ok_and(|archive| !archive.is_empty())
}

impl BackendAccountClient {
    pub(super) fn data_summary(
        &self,
        access_token: &str,
    ) -> Result<AccountDataSummary, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let summary = self.json_detailed::<AccountDataSummary, ()>(
            Method::GET,
            DATA_PATH,
            Some(access_token),
            None,
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
        )?;
        validate_summary(&summary)?;
        Ok(summary)
    }

    pub(super) fn data_delete(
        &self,
        sections: &[String],
        access_token: &str,
    ) -> Result<(), AccountFailure> {
        if !valid_data_sections(sections) || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        #[derive(Serialize)]
        struct Body<'a> {
            sections: &'a [String],
        }
        let body = serde_json::to_vec(&Body { sections }).map_err(|_| AccountError::Invalid)?;
        self.request_detailed(
            Method::DELETE,
            DATA_PATH,
            Some(access_token),
            Some(body),
            MAX_JSON_BYTES,
            MAX_JSON_BYTES,
            Duration::from_secs(30),
            "application/json",
        )
        .map(|_| ())
    }

    /// 把导出包流式写到 `destination`（绝对路径，父目录不能是符号链接）。先写进同目录的临时文件，读得出中央目录才换到目标位置，所以失败时目标位置上原有的文件不动、也不留下半个包。
    pub(super) fn data_export_to_file(
        &self,
        destination: &Path,
        access_token: &str,
    ) -> Result<AccountDataExport, AccountFailure> {
        if !destination.is_absolute() || !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let parent = destination.parent().ok_or(AccountError::Invalid)?;
        #[cfg(unix)]
        let name = destination.file_name().ok_or(AccountError::Invalid)?;
        #[cfg(not(unix))]
        destination.file_name().ok_or(AccountError::Invalid)?;
        if !parent.is_absolute() {
            return Err(AccountError::Invalid.into());
        }
        crate::storage::reject_symlink(parent).map_err(|_| AccountError::Invalid)?;
        let mut response = self
            .http()?
            .get(self.url(DATA_EXPORT_PATH)?)
            .header(reqwest::header::ACCEPT, "application/zip")
            .bearer_auth(access_token)
            .timeout(DATA_EXPORT_TIMEOUT)
            .send()
            .map_err(|error| AccountFailure::from_send_error(&error))?;
        if !response.status().is_success() {
            return Err(failure_from_response(response));
        }
        let zip_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/zip"));
        if !zip_type
            || response
                .content_length()
                .is_some_and(|length| length > MAX_DATA_EXPORT_BYTES)
        {
            return Err(AccountError::Unavailable.into());
        }
        let filename = export_filename(
            response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok()),
        );
        fn copy(
            response: &mut reqwest::blocking::Response,
            file: &mut std::fs::File,
        ) -> std::io::Result<u64> {
            let bytes =
                std::io::copy(&mut response.by_ref().take(MAX_DATA_EXPORT_BYTES + 1), file)?;
            if bytes == 0 || bytes > MAX_DATA_EXPORT_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "data export size is invalid",
                ));
            }
            file.flush()?;
            Ok(bytes)
        }
        #[cfg(unix)]
        let bytes = {
            let directory = crate::storage::open_private_directory(parent)
                .map_err(|_| AccountError::Unavailable)?;
            let temporary = std::ffi::OsString::from(format!(
                ".msime-data-export-{}.partial",
                uuid::Uuid::new_v4().simple()
            ));
            let (bytes, _) =
                crate::storage::write_private_file_at_with(&directory, &temporary, |mut file| {
                    let bytes = copy(&mut response, &mut file)?;
                    Ok::<_, std::io::Error>((file, bytes))
                })
                .map_err(|_| AccountError::Unavailable)?;
            let published = crate::storage::open_private_file_at(&directory, &temporary)
                .ok()
                .is_some_and(readable_zip)
                && crate::storage::rename_private_entry(&directory, &temporary, &directory, name)
                    .is_ok();
            if !published {
                let _ = crate::storage::remove_private_file_at(&directory, &temporary);
                return Err(AccountError::Unavailable.into());
            }
            bytes
        };
        #[cfg(not(unix))]
        let bytes = {
            use std::io::Seek;
            let mut temporary = tempfile::Builder::new()
                .prefix(".msime-data-export-")
                .tempfile_in(parent)
                .map_err(|_| AccountError::Unavailable)?;
            let bytes = copy(&mut response, temporary.as_file_mut())
                .map_err(|_| AccountError::Unavailable)?;
            temporary
                .as_file()
                .sync_all()
                .and_then(|()| temporary.as_file_mut().rewind())
                .map_err(|_| AccountError::Unavailable)?;
            if !readable_zip(temporary.as_file_mut()) {
                return Err(AccountError::Unavailable.into());
            }
            temporary
                .persist(destination)
                .map_err(|_| AccountError::Unavailable)?;
            bytes
        };
        Ok(AccountDataExport {
            path: destination.to_string_lossy().into_owned(),
            bytes,
            filename,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_sections_once_each_can_be_deleted() {
        let sections = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        assert!(valid_data_sections(&sections(&["voice", "phrases"])));
        assert!(valid_data_sections(&sections(&DATA_SECTIONS)));
        assert!(!valid_data_sections(&[]));
        assert!(!valid_data_sections(&sections(&["phrases", "phrases"])));
        assert!(!valid_data_sections(&sections(&["community"])));
        assert!(!valid_data_sections(&sections(&["avatar"])));
    }

    #[test]
    fn the_export_filename_comes_from_the_server_when_it_is_plain() {
        assert_eq!(
            export_filename(Some(r#"attachment; filename="msime-data-2026-10-16.zip""#)),
            "msime-data-2026-10-16.zip"
        );
        for header in [
            None,
            Some("attachment"),
            Some(r#"attachment; filename="../evil.zip""#),
            Some(r#"attachment; filename="report.exe""#),
            Some(r#"attachment; filename=".zip""#),
        ] {
            assert_eq!(
                export_filename(header),
                DEFAULT_EXPORT_FILENAME,
                "{header:?}"
            );
        }
    }

    #[test]
    fn unknown_summary_sections_are_kept() {
        let summary: AccountDataSummary = serde_json::from_value(serde_json::json!({
            "bytes": 10,
            "sections": [{ "id": "preferences", "bytes": 4, "items": 1 }, { "id": "future_section", "bytes": 6, "items": 2 }],
        }))
        .unwrap();
        assert!(validate_summary(&summary).is_ok());
        let mut negative = summary.clone();
        negative.sections[0].items = -1;
        assert_eq!(validate_summary(&negative), Err(AccountError::Unavailable));
        let mut bad_id = summary;
        bad_id.sections[1].id = "Bad Id".into();
        assert_eq!(validate_summary(&bad_id), Err(AccountError::Unavailable));
    }
}
