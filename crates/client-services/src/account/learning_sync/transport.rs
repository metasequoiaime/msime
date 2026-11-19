//! learning 域的二进制请求；不实现 Debug，防止输出密文或解密后的内容。

/// 元数据、下载、上传和删除只接受设备编号，由传输层生成固定路径。
pub enum BlobRequest<'a> {
    Metadata {
        device_id: &'a str,
    },
    Download {
        device_id: &'a str,
    },
    Upload {
        device_id: &'a str,
        generation: u64,
        envelope: &'a [u8],
    },
    Delete {
        device_id: &'a str,
    },
}

/// HTTP 错误也保留状态、有限正文和秒数退避；失败不冒充空成功响应。
pub struct BlobResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after_seconds: Option<u64>,
}

pub(crate) const LEARNING_PATH: &str = "/v1/users/me/learning";
