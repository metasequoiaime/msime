//! `msime_client_community` 不联网能验证的部分：本机打包检查、补预览和许可、请求校验，以及未登录时在发请求前就结束的操作。

use super::*;
use std::ffi::CStr;

/// 一张完整的 1×1 PNG：打包会完整解码图片。
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0x60, 0x60,
    0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0xA5, 0xF6, 0x45, 0x40, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];
const ITEM: &str = "10000000-0000-4000-8000-000000000001";

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: the buffer is live for the call, and the returned string is freed below.
    unsafe {
        let raw = msime_client_community(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn failure(code: &str) -> Value {
    json!({ "ok": false, "error": code })
}

/// 一个临时状态根：账号目录里没有会话，`skins` 和 `plugins` 按需创建。
struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }

    fn account(&self) -> PathBuf {
        self.0.path().join("account")
    }

    fn skins(&self) -> PathBuf {
        self.0.path().join("skins")
    }

    fn call(&self, action: Value) -> Value {
        call(json!({
            "directory": self.account(),
            "skins_directory": self.skins(),
            "state_root": self.0.path(),
            "action": action,
        }))
    }

    /// 一个已安装的候选窗皮肤，清单里没有预览和许可，文件夹里放着一张还没引用的图。
    fn install_skin(&self) {
        let skin = self.skins().join("sakura");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(
            skin.join("skin.toml"),
            "schema_version = 1\nid = 'sakura'\nname = '樱花'\nversion = '1.0'\nbase = 'paper'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#fff0f5'\n",
        )
        .unwrap();
    }
}

#[test]
fn a_candidate_skin_is_checked_and_completed_without_the_network() {
    let root = Root::new();
    root.install_skin();
    // 没有预览：分享先要预览。
    assert_eq!(
        root.call(json!({ "operation": "candidate_skin_pack_preview", "skin_id": "sakura" })),
        failure("candidate_skin_preview_required")
    );

    let added = root.call(json!({
        "operation": "candidate_skin_add_preview",
        "skin_id": "sakura",
        "image": BASE64.encode(PNG),
    }));
    assert_eq!(added["ok"], true, "{added}");
    assert_eq!(
        added["value"]["directory"],
        root.skins().to_string_lossy().as_ref()
    );
    assert_eq!(added["value"]["packages"][0]["id"], "sakura");
    assert!(added["value"]["packages"][0]["preview"].is_string());

    // 有了预览，公开分享还缺素材许可；私有的不要。
    assert_eq!(
        root.call(json!({ "operation": "candidate_skin_pack_preview", "skin_id": "sakura" })),
        failure("candidate_skin_license_required")
    );
    let private = root.call(json!({
        "operation": "candidate_skin_pack_preview",
        "skin_id": "sakura",
        "visibility": "private",
    }));
    assert_eq!(private["value"]["suggestedName"], "樱花", "{private}");

    let licensed = root.call(json!({
        "operation": "candidate_skin_add_license",
        "skin_id": "sakura",
        "assets": "CC-BY-4.0",
    }));
    assert_eq!(
        licensed["value"]["packages"][0]["id"], "sakura",
        "{licensed}"
    );
    let public = root.call(json!({
        "operation": "candidate_skin_pack_preview",
        "skin_id": "sakura",
    }));
    assert_eq!(public["value"]["suggestedName"], "樱花", "{public}");
    assert_eq!(public["value"]["license"]["assets"], "CC-BY-4.0");
    assert_eq!(public["value"]["fileCount"], 1);
    assert_eq!(public["value"]["size"], PNG.len());

    assert_eq!(
        root.call(json!({
            "operation": "candidate_skin_add_preview",
            "skin_id": "sakura",
            "image": "not base64!",
        })),
        failure("community_invalid")
    );
}

#[test]
fn an_installed_plugin_pack_is_checked_without_the_network() {
    let root = Root::new();
    let pack = root.0.path().join("plugins/command_table/signature");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("plugin.toml"),
        "schema_version = 1\nkind = 'command_table'\nid = 'signature'\nname = '签名'\nversion = '1.0'\nlicense = 'CC0-1.0'\ndescription = '常用签名'\n[[commands]]\ntrigger = 'sig'\ntitle = '签名'\ntemplate = '张三 {date}'\n",
    )
    .unwrap();
    let preview = root.call(json!({
        "operation": "plugin_pack_preview",
        "kind": "command_table",
        "plugin_id": "signature",
    }));
    assert_eq!(preview["ok"], true, "{preview}");
    assert_eq!(preview["value"]["suggestedName"], "签名");
    assert_eq!(preview["value"]["suggestedDescription"], "常用签名");
    assert_eq!(preview["value"]["version"], "1.0");
    assert_eq!(preview["value"]["license"], "CC0-1.0");
    assert_eq!(preview["value"]["fileCount"], 1);

    let missing = root.call(json!({
        "operation": "plugin_pack_preview",
        "kind": "command_table",
        "plugin_id": "absent",
    }));
    assert_eq!(missing["ok"], false, "{missing}");
    assert_ne!(missing["error"], "community_invalid", "{missing}");
}

#[test]
fn bad_requests_fail_before_the_network() {
    let root = Root::new();
    let invalid = failure("community_invalid");
    for action in [
        json!({ "operation": "unknown" }),
        json!({ "operation": "candidate_skin_detail", "id": "not-a-uuid" }),
        json!({ "operation": "candidate_skin_rate", "id": ITEM, "stars": 6 }),
        json!({ "operation": "plugin_rate", "id": ITEM, "stars": 0 }),
        json!({ "operation": "plugin_list", "offset": 0, "search": "", "extra": 1 }),
        json!({ "operation": "resource_list", "kind": "dictionary", "scope": "all", "search": "", "offset": 0 }),
        json!({ "operation": "report", "kind": "plugins", "id": ITEM, "reason": "Other" }),
        json!({ "operation": "report", "kind": "plugins", "id": "nil", "reason": "其他" }),
    ] {
        assert_eq!(root.call(action.clone()), invalid, "{action}");
    }
    assert_eq!(
        call(json!({ "directory": "relative", "action": { "operation": "candidate_skin_sync" } })),
        invalid
    );
    assert_eq!(
        call(json!({
            "directory": root.account(),
            "skins_directory": "skins",
            "action": { "operation": "candidate_skin_sync" },
        })),
        invalid
    );
    // 本机操作缺了它要的目录。
    for action in [
        json!({ "operation": "candidate_skin_sync" }),
        json!({ "operation": "candidate_skin_pack_preview", "skin_id": "sakura" }),
        json!({ "operation": "plugin_pack_preview", "kind": "command_table", "plugin_id": "signature" }),
    ] {
        assert_eq!(
            call(json!({ "directory": root.account(), "action": action })),
            invalid,
            "{action}"
        );
    }
}

#[test]
fn signed_out_calls_that_need_the_account_stop_before_the_network() {
    let root = Root::new();
    let unauthorized = failure("community_unauthorized");
    for action in [
        json!({ "operation": "candidate_skin_sync" }),
        json!({ "operation": "candidate_skin_install", "id": ITEM, "replace": false }),
        json!({ "operation": "candidate_skin_rate", "id": ITEM, "stars": 5 }),
        json!({ "operation": "candidate_skin_set_visibility", "id": ITEM, "visibility": "private" }),
        json!({ "operation": "candidate_skin_unpublish", "id": ITEM }),
        json!({ "operation": "candidate_skin_list", "offset": 0, "search": "", "mine": true }),
        json!({ "operation": "plugin_install", "id": ITEM, "kind": "command_table", "plugin_id": "signature" }),
        json!({ "operation": "plugin_delete", "id": ITEM }),
        json!({ "operation": "resource_save", "id": ITEM, "saved": true }),
        json!({ "operation": "resource_list", "kind": "reply", "scope": "saved", "search": "", "offset": 0 }),
        json!({ "operation": "report", "kind": "candidate-skins", "id": ITEM, "reason": "其他", "detail": "合成" }),
    ] {
        assert_eq!(root.call(action.clone()), unauthorized, "{action}");
    }
    // 没有下载，皮肤根目录没被建出来。
    assert!(!root.skins().exists());
}
