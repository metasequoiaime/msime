//! learning 账号端点：控制消息用 JSON，信封按原始字节传输，不接受任意路径。

use super::transport::{BlobRequest, BlobResponse, LEARNING_PATH};
use crate::account::validate::{read_bounded_response, MAX_ERROR_BODY_BYTES};
use crate::account::{AccountError, BackendAccountClient};
use msime_client_core::cloud::learning_e2e::{
    HttpRequest, HttpResponse, HEADER_BYTES, MAX_ENVELOPE_BYTES, MAX_GENERATION,
};
use reqwest::{header, Method};
use std::time::Duration;

const MAX_METADATA_BYTES: usize = 1024 * 1024;

fn device_id(value: &str) -> bool {
    crate::text::is_lower_hex(value, 32)
}

fn control_method(request: &HttpRequest) -> Result<Method, AccountError> {
    let valid = if request.method == "GET" {
        request
            .path
            .strip_prefix(&format!("{LEARNING_PATH}?device_id="))
            .is_some_and(device_id)
    } else if request.method == "POST" {
        request.path == format!("{LEARNING_PATH}/key")
            || request.path == format!("{LEARNING_PATH}/rekey")
    } else {
        let devices = request
            .path
            .strip_prefix(&format!("{LEARNING_PATH}/devices/"))
            .is_some_and(device_id);
        let wraps = request
            .path
            .strip_prefix(&format!("{LEARNING_PATH}/wraps/"))
            .is_some_and(device_id);
        match request.method.as_str() {
            "PUT" => devices || wraps,
            "DELETE" => devices,
            _ => false,
        }
    };
    if !valid {
        return Err(AccountError::Invalid);
    }
    Method::from_bytes(request.method.as_bytes()).map_err(|_| AccountError::Invalid)
}

impl BackendAccountClient {
    /// 只把驱动生成的 learning 控制路径交给同源账号传输。
    pub fn learning_e2e(
        &self,
        request: &HttpRequest,
        access_token: &str,
    ) -> Result<HttpResponse, AccountError> {
        let method = control_method(request)?;
        let response = self
            .prepare(
                method,
                &request.path,
                Some(access_token),
                request.body.as_ref().map(|body| body.as_bytes().to_vec()),
                MAX_METADATA_BYTES,
                "application/json",
            )?
            .timeout(Duration::from_secs(60))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let status = response.status().as_u16();
        let bytes = if response.status().is_success() {
            read_bounded_response(response, MAX_METADATA_BYTES)?
        } else {
            crate::bounded_io::read_bounded(response, MAX_ERROR_BODY_BYTES).unwrap_or_default()
        };
        let body = String::from_utf8(bytes).map_err(|_| AccountError::Unavailable)?;
        Ok(HttpResponse { status, body })
    }

    /// 原始二进制信封不经 base64 或 UTF-8；限额与服务端一致。
    pub fn learning_blob(
        &self,
        request: &BlobRequest<'_>,
        access_token: &str,
    ) -> Result<BlobResponse, AccountError> {
        let (method, path, body, limit) = match request {
            BlobRequest::Metadata { device_id: id } if device_id(id) => (
                Method::GET,
                format!("{LEARNING_PATH}?device_id={id}"),
                None,
                MAX_METADATA_BYTES,
            ),
            BlobRequest::Download { device_id: id } if device_id(id) => (
                Method::GET,
                format!("{LEARNING_PATH}/blobs/{id}"),
                None,
                MAX_ENVELOPE_BYTES,
            ),
            BlobRequest::Upload {
                device_id: id,
                generation,
                envelope,
            } if device_id(id)
                && (1..=MAX_GENERATION).contains(generation)
                && (HEADER_BYTES + 16..=MAX_ENVELOPE_BYTES).contains(&envelope.len()) =>
            {
                (
                    Method::PUT,
                    format!("{LEARNING_PATH}/blobs/{id}?revision={generation}"),
                    Some(*envelope),
                    MAX_METADATA_BYTES,
                )
            }
            BlobRequest::Delete { device_id: id } if device_id(id) => (
                Method::DELETE,
                format!("{LEARNING_PATH}/blobs/{id}"),
                None,
                MAX_METADATA_BYTES,
            ),
            _ => return Err(AccountError::Invalid),
        };
        // 这里不给 prepare JSON 正文，避免把二进制错标成 application/json。
        let mut prepared = self.prepare(
            method,
            &path,
            Some(access_token),
            None,
            0,
            "application/octet-stream, application/json",
        )?;
        if let Some(bytes) = body {
            prepared = prepared
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes.to_vec());
        }
        let response = prepared
            .timeout(Duration::from_secs(60))
            .send()
            .map_err(|_| AccountError::Unavailable)?;
        let status = response.status().as_u16();
        let retry_after_seconds = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        let body = if response.status().is_success() {
            read_bounded_response(response, limit)?
        } else {
            crate::bounded_io::read_bounded(response, MAX_ERROR_BODY_BYTES).unwrap_or_default()
        };
        Ok(BlobResponse {
            status,
            body,
            retry_after_seconds,
        })
    }
}

#[cfg(test)]
mod tests;
