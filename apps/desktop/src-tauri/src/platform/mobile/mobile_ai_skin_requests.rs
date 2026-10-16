//! 移动端 AI 皮肤生成任务的登记与取消。登记表本身在 client-services（[`AiSkinRequests`]），C 接口的 `msime_client_ai_skin_generate` 和 `msime_client_ai_skin_cancel` 用的是同一个类型；这里只留 Tauri 命令在工作线程结束后注销请求的那一步。

pub(crate) use msime_client_services::skin::ai::AiSkinRequests;

pub(crate) fn finish_after_worker<T, E>(
    requests: &AiSkinRequests,
    id: &str,
    result: Result<T, E>,
) -> Result<T, E> {
    requests.finish(id);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_failure_still_releases_request_id() {
        let requests = AiSkinRequests::default();
        let _ = requests.begin("synthetic-request").unwrap();
        let failed: Result<(), &str> =
            finish_after_worker(&requests, "synthetic-request", Err("worker"));
        assert_eq!(failed, Err("worker"));
        assert!(requests.begin("synthetic-request").is_ok());
    }
}
