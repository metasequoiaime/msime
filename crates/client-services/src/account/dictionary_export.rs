//! 云词库导出写进文件（#8201 的 P7）：`GET /v1/users/me/dictionaries/{kind}/export?format=` 的正文边下载边写进宿主给的路径，不放进内存，也不进 C 响应。对正文的要求与 [`BackendAccountClient::export_dictionary`] 相同：非空、UTF-8、不含 NUL，最多 384 MiB。

use super::*;
use std::io;

/// 一次读多少字节来校验和写入。
const COPY_CHUNK_BYTES: usize = 64 * 1024;

fn invalid_export(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// 把导出正文从 `input` 抄进 `output`，返回写入的字节数。按块校验 UTF-8 和 NUL：一个字符跨两块时，前一块末尾那几个字节留到下一块再判断，所以写进去的始终是完整的字符。超过 [`MAX_DICTIONARY_EXPORT_BYTES`]、为空、不是 UTF-8 或含 NUL 时返回错误，调用方丢掉写了一半的文件。
pub(super) fn copy_dictionary_export(input: impl Read, output: &mut impl Write) -> io::Result<u64> {
    let mut input = input.take(MAX_DICTIONARY_EXPORT_BYTES as u64 + 1);
    let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
    // 上一块末尾还不完整的字符留在 buffer 开头的字节数，最多 3 个。
    let mut pending = 0;
    let mut total = 0_u64;
    loop {
        let read = match input.read(&mut buffer[pending..]) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if read == 0 {
            break;
        }
        let filled = pending + read;
        let complete = match std::str::from_utf8(&buffer[..filled]) {
            Ok(_) => filled,
            // 末尾只是字符还没收完，不是坏字节。
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return Err(invalid_export("dictionary export is not UTF-8")),
        };
        if buffer[..complete].contains(&0) {
            return Err(invalid_export("dictionary export contains NUL"));
        }
        output.write_all(&buffer[..complete])?;
        total += complete as u64;
        buffer.copy_within(complete..filled, 0);
        pending = filled - complete;
    }
    if pending != 0 {
        return Err(invalid_export("dictionary export ends inside a character"));
    }
    if total == 0 || total > MAX_DICTIONARY_EXPORT_BYTES as u64 {
        return Err(invalid_export("dictionary export size is invalid"));
    }
    Ok(total)
}

impl BackendAccountClient {
    /// 与 [`Self::export_dictionary`] 相同的请求，正文边下载边写进 `destination`，返回写入的字节数。`destination` 的上级目录必须是绝对路径、不经过符号链接；Unix 上经不跟随符号链接的目录句柄写一个只有本用户能读的文件，其他平台先写同目录的临时文件再换入。正文不合格、超过上限或写入失败时是 `Unavailable`，`destination` 不出现。
    pub fn export_dictionary_to_file(
        &self,
        kind: DictionaryKind,
        format: &str,
        destination: &Path,
        access_token: &str,
    ) -> Result<u64, AccountError> {
        if !matches!(format, "standard" | "windows")
            || !destination.is_absolute()
            || !crate::text::is_lower_hex(access_token, 64)
        {
            return Err(AccountError::Invalid);
        }
        let parent = destination
            .parent()
            .filter(|parent| parent.is_absolute())
            .ok_or(AccountError::Invalid)?;
        let name = destination.file_name().ok_or(AccountError::Invalid)?;
        crate::storage::reject_symlink(parent).map_err(|_| AccountError::Invalid)?;
        let path = format!(
            "/v1/users/me/dictionaries/{}/export?format={format}",
            crate::cloud::dictionary::kind_path(kind)
        );
        let mut response = self
            .prepare(
                Method::GET,
                &path,
                Some(access_token),
                None,
                MAX_JSON_BYTES,
                "text/plain",
            )?
            .timeout(Duration::from_secs(600))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        if !response.status().is_success() {
            return Err(error_from_response(response));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_DICTIONARY_EXPORT_BYTES as u64)
        {
            return Err(AccountError::Unavailable);
        }
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(parent)
                .map_err(|_| AccountError::Unavailable)?;
            let (bytes, _) =
                crate::storage::write_private_file_at_with(&directory, name, |mut file| {
                    let bytes = copy_dictionary_export(&mut response, &mut file)?;
                    Ok::<_, io::Error>((file, bytes))
                })
                .map_err(|_| AccountError::Unavailable)?;
            Ok(bytes)
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            // 用 tempfile 缺省的 `.tmp` 前缀：进程中途被杀时留下的临时文件，host-api 下次清理中转目录时一并删掉。
            let mut temporary =
                tempfile::NamedTempFile::new_in(parent).map_err(|_| AccountError::Unavailable)?;
            let bytes = copy_dictionary_export(&mut response, temporary.as_file_mut())
                .and_then(|bytes| temporary.as_file().sync_all().map(|()| bytes))
                .map_err(|_| AccountError::Unavailable)?;
            temporary
                .persist(destination)
                .map_err(|_| AccountError::Unavailable)?;
            Ok(bytes)
        }
    }
}

#[cfg(test)]
#[path = "dictionary_export_tests.rs"]
mod tests;
