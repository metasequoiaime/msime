use serde_json::Value;

pub(super) const FALLS_BACK_ON_READ: bool = cfg!(target_os = "android");

/// 只在 Android 完整文档读取时规范化这一字段，避免严格解码先拒绝文档而让旧会话的臺灣变体粘住。
/// 缺省 s2t 不落盘；未知值在下次正常保存时消失。提交的校验、其他字段与非 Android 读法不变。
/// 账号快照的缺键合并不经过这里，不会清除已选的合法变体。
/// `applies` 允许在桌面测试环境中覆盖 Android 分支。
pub(super) fn settle(document: &mut Value, applies: bool) {
    if !applies {
        return;
    }
    let Some(preferences) = document
        .get_mut("preferences")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if preferences
        .get("traditional_chinese_variant")
        .is_some_and(|value| !matches!(value.as_str(), Some("s2t" | "s2tw" | "s2twp")))
    {
        preferences.remove("traditional_chinese_variant");
    }
}
