use super::*;
use msime_client_services::account::{
    AccountChallenge, AccountProfile, AccountSessionStorage, AccountTokens, AccountUser,
    SavedAccountSession,
};
use msime_client_services::skin::category::SkinCategory;
use msime_client_services::skin::community::{CommunitySkin, CommunitySkinPage};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 合成的访问令牌：结果和错误里都不该出现它。
const ACCESS_TOKEN: &str = "a1a2a3a4a5a6a7a8a9a0b1b2b3b4b5b6b7b8b9b0c1c2c3c4c5c6c7c8c9c0d1d2";
/// 只有 JPEG 开头和结尾标记的合成字节，不是一张能解码的图。
const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0synthetic-jpeg-fixture\xFF\xD9";
/// [`JPEG`] 的标准 base64，事先算好写在这里，不经被测代码得出。
const JPEG_BASE64: &str = "/9j/4HN5bnRoZXRpYy1qcGVnLWZpeHR1cmX/2Q==";
/// 假服务器一页给几款，与真服务器相同。
const PAGE: usize = 20;

/// 假服务器收到的一次发布。
#[derive(Clone, Debug, PartialEq)]
struct Published {
    id: Uuid,
    name: String,
    description: String,
    design: TouchKeyboardSkinDesign,
    category: Option<SkinCategory>,
}

#[derive(Default)]
struct FakeState {
    /// 每次请求的名字和参数，按顺序。
    calls: Vec<String>,
    /// 每次请求带的令牌。
    bearers: Vec<String>,
    published: Vec<Published>,
    listed: Vec<CommunitySkin>,
    /// 设了就让发布和删除都以它失败。
    refuse: Option<AccountError>,
    /// 设了就让列表每一页都说后面还有。
    endless: bool,
    /// 设了就在一次发布成功之后、应答回到会话层之前运行，用来扮演这期间动了会话文件的设置应用。
    after_publish: Option<Box<dyn Fn() + Send>>,
}

/// 只在内存里应答的账号服务器，测试不联网。
#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<FakeState>>);

impl FakeApi {
    fn record(&self, call: String, bearer: Option<&str>) {
        let mut state = self.0.lock().unwrap();
        state.calls.push(call);
        state.bearers.push(bearer.unwrap_or_default().to_owned());
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }

    fn refuse(&self, error: AccountError) {
        self.0.lock().unwrap().refuse = Some(error);
    }

    fn refusal(&self) -> Result<(), AccountError> {
        match self.0.lock().unwrap().refuse.clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn skin(id: Uuid, name: &str) -> CommunitySkin {
    CommunitySkin {
        id,
        name: name.into(),
        description: "合成的键盘皮肤".into(),
        author: "Fixture".into(),
        design: TouchKeyboardSkinDesign::default(),
        downloads: 3,
        rating_count: 2,
        rating_average: 4.5,
        owned: true,
        my_rating: 0,
        moderation: Some(CommunityModeration::Pending),
        category: Some(SkinCategory::Cute),
        origin: None,
    }
}

impl AccountApi for FakeApi {
    fn providers(&self) -> Result<HashMap<String, bool>, AccountError> {
        Ok(HashMap::new())
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        panic!("the server never signs in")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        panic!("the server never signs in")
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

impl CommunitySkinApi for FakeApi {
    fn community_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<SkinCategory>,
        exclude_ai_generated: bool,
        token: Option<&str>,
    ) -> Result<CommunitySkinPage, AccountError> {
        self.record(
            format!(
                "list offset={offset} search={search:?} mine={mine} category={} exclude_ai={exclude_ai_generated}",
                category.is_some()
            ),
            token,
        );
        let state = self.0.lock().unwrap();
        let skins: Vec<CommunitySkin> = state
            .listed
            .iter()
            .skip(offset)
            .take(PAGE)
            .cloned()
            .collect();
        Ok(CommunitySkinPage {
            has_more: state.endless || offset + skins.len() < state.listed.len(),
            skins,
        })
    }
    fn community_skin(
        &self,
        _: Uuid,
        _: bool,
        _: Option<&str>,
    ) -> Result<CommunitySkin, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn download_community_skin(
        &self,
        _: Uuid,
        _: bool,
        _: &str,
    ) -> Result<TouchKeyboardSkinDesign, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rate_community_skin(&self, _: Uuid, _: u8, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn publish_community_skin(
        &self,
        id: Uuid,
        name: &str,
        description: &str,
        design: &TouchKeyboardSkinDesign,
        category: Option<SkinCategory>,
        token: &str,
    ) -> Result<(), AccountError> {
        self.record("publish".into(), Some(token));
        self.refusal()?;
        self.0.lock().unwrap().published.push(Published {
            id,
            name: name.into(),
            description: description.into(),
            design: design.clone(),
            category,
        });
        if let Some(settings_app) = &self.0.lock().unwrap().after_publish {
            settings_app();
        }
        Ok(())
    }
    fn unpublish_community_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        self.record(format!("delete {id}"), Some(token));
        self.refusal()
    }
    fn set_community_skin_category(
        &self,
        _: Uuid,
        _: SkinCategory,
        _: &str,
    ) -> Result<CommunitySkin, AccountError> {
        Err(AccountError::Unavailable)
    }
}

fn saved_session() -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: ACCESS_TOKEN.into(),
            refresh_token: "e1e2e3e4e5e6e7e8e9e0f1f2f3f4f5f6f7f8f9f0a1a2a3a4a5a6a7a8a9a0b1b2"
                .into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: "fixture-user".into(),
                display_name: "Fixture".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000,
        session_id: None,
    }
}

/// 一个账号会话目录和一个放照片的目录，`signed_in` 决定会话目录里有没有会话。
struct Fixture {
    account: tempfile::TempDir,
    files: tempfile::TempDir,
    api: FakeApi,
}

impl Fixture {
    fn new(signed_in: bool) -> Self {
        let account = tempfile::tempdir().unwrap();
        if signed_in {
            FileAccountSessionStorage::new(account.path(), AccountSessionFileLayout::Native)
                .save(&saved_session())
                .unwrap();
        }
        Self {
            account,
            files: tempfile::tempdir().unwrap(),
            api: FakeApi::default(),
        }
    }

    fn community(&self) -> KeyboardSkinCommunity<FakeApi> {
        community_at(
            self.api.clone(),
            self.account.path(),
            AccountSessionFileLayout::Native,
        )
    }

    fn publish(&self, request: serde_json::Value) -> Result<PublishedKeyboardSkin, String> {
        let request: PublishKeyboardSkinRequest = serde_json::from_value(request).unwrap();
        publish(&self.community(), &request)
    }

    /// 在照片目录里写一个文件，返回它的绝对路径。
    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.files.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn published(&self) -> Vec<Published> {
        self.api.0.lock().unwrap().published.clone()
    }
}

/// 代理会发来的一份请求：五个必填颜色，其余都不写。
fn request() -> serde_json::Value {
    serde_json::json!({
        "name": "薄荷汽水",
        "design": {
            "background": "#D8F0E4",
            "keyBackground": "#fafff9",
            "keyForeground": "#173D30",
            "accent": "#245A43",
            "actionBackground": "#245A43"
        }
    })
}

fn with(mut request: serde_json::Value, key: &str, value: serde_json::Value) -> serde_json::Value {
    request[key] = value;
    request
}

fn with_design(
    mut request: serde_json::Value,
    key: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    request["design"][key] = value;
    request
}

#[test]
fn without_a_session_nothing_is_read_or_sent() {
    let fixture = Fixture::new(false);
    // 照片路径并不存在：没登录时先回答登录，说明没有去读它。
    let missing = fixture.files.path().join("missing.jpg");
    let error = fixture
        .publish(with(
            request(),
            "photo_file",
            missing.to_str().unwrap().into(),
        ))
        .unwrap_err();
    assert_eq!(error, SIGN_IN);
    assert_eq!(list(&fixture.community()).unwrap_err(), SIGN_IN);
    assert_eq!(
        delete(
            &fixture.community(),
            &CommunityKeyboardSkinRequest {
                id: Uuid::new_v4().to_string()
            }
        )
        .unwrap_err(),
        SIGN_IN
    );
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn a_design_is_published_as_the_phone_document() {
    let fixture = Fixture::new(true);
    let published = fixture
        .publish(serde_json::json!({
            "name": "  薄荷汽水  ",
            "description": " 清爽的绿色\n适合夏天 ",
            "category": "cute",
            "design": {
                "background": "#D8F0E4",
                "keyBackground": "#fafff9",
                "keyForeground": "#173D30",
                "accent": "#245A43",
                "actionBackground": "#000001",
                "gradientEnd": "#EEF6DD",
                "customBorderColor": "#B6D8C5",
                "cornerRadius": 14,
                "borderWidth": 0.5,
                "shadow": 0.08,
                "pattern": 2,
                "monospaced": true,
                "keyShape": "pebble",
                "keyMaterial": "glass",
                "keyOpacity": 0.6,
                "gradientHorizontal": true,
                "patternOpacity": 0.1,
                "photoShade": 0.3,
                "photoPosition": 0.5
            }
        }))
        .unwrap();
    assert_eq!(published.name, "薄荷汽水");
    assert_eq!(published.photo_bytes, None);
    let value = serde_json::to_value(&published).unwrap();
    crate::assert_required_fields_present::<PublishedKeyboardSkin>(&value);
    // 没有照片时 photo_bytes 是 null，不是缺字段。
    assert!(value["photo_bytes"].is_null());

    let state = fixture.api.0.lock().unwrap();
    assert_eq!(state.calls, ["publish"]);
    assert_eq!(state.bearers, [ACCESS_TOKEN]);
    let sent = &state.published[0];
    assert_eq!(sent.id.hyphenated().to_string(), published.id);
    assert_eq!(sent.id.get_version_num(), 4);
    assert_eq!(sent.name, "薄荷汽水");
    assert_eq!(sent.description, "清爽的绿色\n适合夏天");
    assert_eq!(sent.category, Some(SkinCategory::Cute));
    assert_eq!(
        sent.design,
        TouchKeyboardSkinDesign {
            background: 0xD8F0E4,
            key_background: 0xFAFFF9,
            key_foreground: 0x173D30,
            accent: 0x245A43,
            action_background: 0x000001,
            corner_radius: 14.0,
            border_width: 0.5,
            shadow: 0.08,
            pattern: 2,
            monospaced: true,
            key_shape: Some(TouchSkinKeyShape::Pebble),
            key_material: Some(TouchSkinKeyMaterial::Glass),
            key_opacity: Some(0.6),
            gradient_end: Some(0xEEF6DD),
            gradient_horizontal: Some(true),
            pattern_opacity: Some(0.1),
            custom_border_color: Some(0xB6D8C5),
            photo: None,
            photo_shade: Some(0.3),
            photo_position: Some(0.5),
        }
    );
    // 令牌不出现在结果里。
    assert!(!value.to_string().contains(ACCESS_TOKEN));
}

#[test]
fn keys_left_out_take_the_phone_defaults() {
    let fixture = Fixture::new(true);
    fixture.publish(request()).unwrap();
    let sent = fixture.published().remove(0);
    assert_eq!(sent.description, "");
    assert_eq!(sent.category, None);
    assert_eq!(
        sent.design,
        TouchKeyboardSkinDesign {
            background: 0xD8F0E4,
            key_background: 0xFAFFF9,
            key_foreground: 0x173D30,
            accent: 0x245A43,
            action_background: 0x245A43,
            ..TouchKeyboardSkinDesign::default()
        }
    );
}

#[test]
fn colours_are_six_hex_digits_after_a_hash() {
    let fixture = Fixture::new(true);
    for (key, value) in [
        ("background", "D8F0E4"),
        ("keyBackground", "#FFF"),
        ("keyForeground", "#173D3G"),
        ("accent", "#245A43FF"),
        ("actionBackground", "green"),
        ("gradientEnd", "14217444"),
        ("customBorderColor", ""),
    ] {
        let error = fixture
            .publish(with_design(request(), key, value.into()))
            .unwrap_err();
        assert!(
            error.contains(&format!("design.{key}")) && error.contains("#RRGGBB"),
            "{key}: {error}"
        );
    }
    assert!(fixture.api.calls().is_empty());
}

#[test]
fn a_value_out_of_range_is_refused_not_adjusted() {
    let fixture = Fixture::new(true);
    for (key, value) in [
        ("cornerRadius", 20.5),
        ("cornerRadius", -1.0),
        ("borderWidth", 2.1),
        ("shadow", 0.41),
        ("pattern", 4.0),
        ("keyOpacity", 0.2),
        ("keyOpacity", 1.01),
        ("patternOpacity", 0.6),
        ("photoShade", 0.9),
        ("photoPosition", 1.5),
    ] {
        let value = if key == "pattern" {
            serde_json::json!(value as u8)
        } else {
            serde_json::json!(value)
        };
        let error = fixture
            .publish(with_design(request(), key, value))
            .unwrap_err();
        assert_eq!(error, OUT_OF_RANGE, "{key}");
    }
    for range in [
        "cornerRadius 0 to 20",
        "borderWidth 0 to 2",
        "shadow 0 to 0.4",
        "pattern 0 to 3",
        "keyOpacity 0.25 to 1",
        "patternOpacity 0 to 0.5",
        "photoShade 0 to 0.8",
        "photoPosition 0 to 1",
    ] {
        assert!(OUT_OF_RANGE.contains(range), "{range}");
    }
    assert!(fixture.api.calls().is_empty());

    // 边界上的值照原样发出去。
    let mut edge = request();
    for (key, value) in [
        ("cornerRadius", 20.0),
        ("borderWidth", 2.0),
        ("shadow", 0.4),
        ("keyOpacity", 0.25),
        ("patternOpacity", 0.5),
        ("photoShade", 0.8),
        ("photoPosition", 0.0),
    ] {
        edge = with_design(edge, key, serde_json::json!(value));
    }
    fixture.publish(edge).unwrap();
    let design = fixture.published().remove(0).design;
    assert_eq!(design.corner_radius, 20.0);
    assert_eq!(design.key_opacity, Some(0.25));
    assert_eq!(design.photo_position, Some(0.0));
}

#[test]
fn unknown_keys_shapes_and_categories_are_refused() {
    let parse = |value: serde_json::Value| {
        serde_json::from_value::<PublishKeyboardSkinRequest>(value).map(|_| ())
    };
    assert!(parse(request()).is_ok());
    // 照片只能从 photo_file 来。
    assert!(parse(with_design(request(), "photo", JPEG_BASE64.into())).is_err());
    assert!(parse(with_design(request(), "keyShape", "square".into())).is_err());
    assert!(parse(with_design(request(), "keyMaterial", "metal".into())).is_err());
    assert!(parse(with_design(request(), "key_shape", "rounded".into())).is_err());
    assert!(parse(with(request(), "category", "animals".into())).is_err());
    assert!(parse(with(request(), "visibility", "private".into())).is_err());
    assert!(parse(with(request(), "origin", "ai".into())).is_err());
    let mut missing = request();
    missing["design"].as_object_mut().unwrap().remove("accent");
    assert!(parse(missing).is_err());
    for shape in ["rounded", "capsule", "ticket", "pebble"] {
        assert!(parse(with_design(request(), "keyShape", shape.into())).is_ok());
    }
    for material in ["flat", "raised", "glass", "paper"] {
        assert!(parse(with_design(request(), "keyMaterial", material.into())).is_ok());
    }
}

#[test]
fn the_listing_text_is_checked_before_anything_is_sent() {
    let fixture = Fixture::new(true);
    for name in ["", "   ", &"长".repeat(33)] {
        let error = fixture
            .publish(with(request(), "name", name.into()))
            .unwrap_err();
        assert!(error.contains("1 to 32 characters"), "{name:?}: {error}");
    }
    let error = fixture
        .publish(with(request(), "description", "长".repeat(281).into()))
        .unwrap_err();
    assert!(error.contains("at most 280 characters"), "{error}");
    for (key, value) in [
        ("name", "薄荷\n汽水"),
        ("name", "薄荷\u{7}汽水"),
        ("description", "第一行\r\n第二行"),
        ("description", "响铃\u{7}"),
    ] {
        let error = fixture
            .publish(with(request(), key, value.into()))
            .unwrap_err();
        assert!(error.contains("control characters"), "{key}: {error}");
    }
    assert!(fixture.api.calls().is_empty());

    // 32 个字符的标题、280 个字符带换行和制表符的简介都能发。
    let description = format!("第一行\n\t第二行{}", "长".repeat(280 - 8));
    let longest = with(
        with(request(), "name", "长".repeat(32).into()),
        "description",
        description.clone().into(),
    );
    fixture.publish(longest).unwrap();
    assert_eq!(fixture.published()[0].description, description);
}

#[test]
fn a_jpeg_file_reaches_the_community_as_standard_base64() {
    let fixture = Fixture::new(true);
    let photo = fixture.file("photo.jpg", JPEG);
    let published = fixture
        .publish(with(request(), "photo_file", photo.clone().into()))
        .unwrap();
    assert_eq!(published.photo_bytes, Some(JPEG.len() as u64));
    let sent = fixture.published().remove(0);
    assert_eq!(sent.design.photo.as_deref(), Some(JPEG_BASE64));
    // 手机发布用的同一个校验接受这份文档。
    assert!(sent.design.validate());
    // 结果里没有照片的内容，也没有它的路径。
    let result = serde_json::to_string(&published).unwrap();
    assert!(!result.contains(JPEG_BASE64));
    assert!(!result.contains(&photo));
}

#[test]
fn a_photo_at_the_size_limit_passes_the_shared_validation() {
    let fixture = Fixture::new(true);
    let mut bytes = vec![0_u8; MAX_PHOTO_BYTES as usize];
    bytes[..JPEG_START.len()].copy_from_slice(&JPEG_START);
    let photo = fixture.file("largest.jpg", &bytes);
    let published = fixture
        .publish(with(request(), "photo_file", photo.into()))
        .unwrap();
    assert_eq!(published.photo_bytes, Some(MAX_PHOTO_BYTES));
    assert!(fixture.published()[0].design.validate());
}

#[test]
fn a_photo_file_is_only_read_when_it_is_a_plain_small_jpeg() {
    let fixture = Fixture::new(true);
    let refused = |file: &str| {
        fixture
            .publish(with(request(), "photo_file", file.into()))
            .unwrap_err()
    };

    let error = refused("photo.jpg");
    assert!(error.contains("is not an absolute path"), "{error}");
    let missing = fixture.files.path().join("missing.jpg");
    let error = refused(missing.to_str().unwrap());
    assert!(error.contains("does not exist"), "{error}");
    let error = refused(fixture.files.path().to_str().unwrap());
    assert!(error.contains("is not a regular file"), "{error}");

    // 超过上限的文件在读之前就拒绝；稀疏文件不占盘。
    let huge = fixture.files.path().join("huge.jpg");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(MAX_PHOTO_BYTES + 1)
        .unwrap();
    let error = refused(huge.to_str().unwrap());
    assert!(error.contains("larger than 512,000 bytes"), "{error}");

    // 不是 JPEG 的内容说清楚要什么，错误里不带文件内容。
    for (name, bytes) in [
        ("picture.png", &b"\x89PNG\r\n\x1A\nsynthetic-png"[..]),
        ("picture.gif", &b"GIF89asynthetic-gif"[..]),
        ("empty.jpg", &b""[..]),
        ("short.jpg", &b"\xFF\xD8"[..]),
        ("notes.jpg", &b"synthetic-secret-text"[..]),
    ] {
        let error = refused(&fixture.file(name, bytes));
        assert_eq!(error, NOT_A_JPEG, "{name}");
    }
    assert!(NOT_A_JPEG
        .contains("baseline JPEG no larger than 1024 pixels on its long side and 512,000 bytes"));

    let real = fixture.file("photo.jpg", JPEG);
    #[cfg(unix)]
    {
        let link = fixture.files.path().join("link.jpg");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let error = refused(link.to_str().unwrap());
        assert!(error.contains("is a symbolic link"), "{error}");
        // 符号链接在上一级目录时也拒绝。
        let linked = fixture.files.path().join("linked");
        std::os::unix::fs::symlink(fixture.files.path(), &linked).unwrap();
        // root 在只有 root 能写的目录里建的链接，Linux 上算系统链接（`msime_path_trust::is_root_only_link`），以 root 运行的测试容器里跳过这一项。
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&linked).unwrap().uid() != 0 {
            let through = linked.join("photo.jpg");
            let error = refused(through.to_str().unwrap());
            assert!(error.contains("symbolic link on the way"), "{error}");
        }
    }
    // 到这里一个请求都没发。
    assert!(fixture.api.calls().is_empty());

    // 同一个文件经正常路径给出就能发。
    fixture
        .publish(with(request(), "photo_file", real.into()))
        .unwrap();
}

#[test]
fn an_id_is_only_taken_to_repeat_a_publish() {
    let fixture = Fixture::new(true);
    let id = "7b0b8c0e-2f6f-4a55-9d0c-0d6f5b9d7a11";
    let published = fixture
        .publish(with(
            request(),
            "id",
            format!(" {} ", id.to_uppercase()).into(),
        ))
        .unwrap();
    assert_eq!(published.id, id);
    assert_eq!(fixture.published()[0].id, Uuid::parse_str(id).unwrap());
    for bad in ["", "not-an-id", "00000000-0000-0000-0000-000000000000"] {
        let error = fixture
            .publish(with(request(), "id", bad.into()))
            .unwrap_err();
        assert!(
            error.contains("leave id out for a new skin"),
            "{bad}: {error}"
        );
    }
    // 不给 id 时每次都是新的。
    let first = fixture.publish(request()).unwrap().id;
    let second = fixture.publish(request()).unwrap().id;
    assert_ne!(first, second);
    assert_eq!(fixture.api.calls().len(), 3);
}

#[test]
fn publish_refusals_say_what_to_do() {
    let fixture = Fixture::new(true);
    let photo = fixture.file("photo.jpg", JPEG);
    let id = "7b0b8c0e-2f6f-4a55-9d0c-0d6f5b9d7a11";
    let refused = |error: AccountError| {
        fixture.api.refuse(error);
        fixture
            .publish(with(
                with(request(), "photo_file", photo.clone().into()),
                "id",
                id.into(),
            ))
            .unwrap_err()
    };

    // 409 有两种来源，两种都说。
    let conflict = refused(AccountError::Conflict);
    assert!(
        conflict.contains("maximum of 50 keyboard skins"),
        "{conflict}"
    );
    assert!(
        conflict.contains("delete_community_keyboard_skin"),
        "{conflict}"
    );
    assert!(
        conflict.contains("the id belongs to a different skin"),
        "{conflict}"
    );

    // 没拿到明确回答时说明可能已经发出去了，并给出再发一次用的 id。
    for error in [AccountError::Unavailable, AccountError::NotFound] {
        let unavailable = refused(error);
        assert!(
            unavailable.contains("may or may not have been published"),
            "{unavailable}"
        );
        assert!(unavailable.contains(id), "{unavailable}");
        assert!(
            unavailable.contains("list_community_keyboard_skins"),
            "{unavailable}"
        );
    }

    assert!(refused(AccountError::Invalid).contains("1024 pixels"));
    assert!(refused(AccountError::BlockedContent).contains("name or description"));
    assert!(refused(AccountError::ScreeningUnavailable).contains("try again later"));
    assert!(refused(AccountError::RateLimited).contains("wait a minute"));
    assert!(refused(AccountError::Banned).contains("banned"));
    assert!(refused(AccountError::Forbidden).contains("does not allow this account"));
    // 会话过期由会话层处理，这里只看说法。
    let unused = Uuid::parse_str(id).unwrap();
    assert_eq!(publish_problem(AccountError::Unauthorized, unused), SIGN_IN);

    // 审计行的用词：没拿到明确结果的不记成被拒。
    assert_eq!(
        publish_outcome(&Err::<(), _>(refused(AccountError::Unavailable))),
        "unconfirmed"
    );
    assert_eq!(
        publish_outcome(&Err::<(), _>(refused(AccountError::Conflict))),
        "refused"
    );
    assert_eq!(publish_outcome(&Ok::<_, String>(())), "published");

    // 哪一种拒绝都不带照片内容、照片路径和令牌。
    for error in [
        AccountError::Conflict,
        AccountError::Unavailable,
        AccountError::Invalid,
    ] {
        let text = refused(error);
        assert!(!text.contains(JPEG_BASE64), "{text}");
        assert!(!text.contains(&photo), "{text}");
        assert!(!text.contains(ACCESS_TOKEN), "{text}");
    }
}

#[test]
fn a_session_that_changed_during_a_publish_leaves_the_outcome_unknown() {
    let id = "7b0b8c0e-2f6f-4a55-9d0c-0d6f5b9d7a11";
    // 设置应用在请求途中做的三件事：退出登录、换成另一个账号、留下一份读不出来的会话。
    let signed_out: fn(&FileAccountSessionStorage) = |storage| storage.clear().unwrap();
    let switched: fn(&FileAccountSessionStorage) = |storage| {
        let mut other = saved_session();
        other.tokens.user.id = "another-user".into();
        storage.save(&other).unwrap();
    };
    let unreadable: fn(&FileAccountSessionStorage) = |storage| {
        let mut broken = saved_session();
        broken.expires_at_unix_ms = u64::MAX;
        storage.save(&broken).unwrap();
    };
    for (change, cause) in [
        (signed_out, "signed out or switched accounts"),
        (switched, "signed out or switched accounts"),
        (unreadable, "account session"),
    ] {
        let fixture = Fixture::new(true);
        let photo = fixture.file("photo.jpg", JPEG);
        let storage = FileAccountSessionStorage::new(
            fixture.account.path(),
            AccountSessionFileLayout::Native,
        );
        fixture.api.0.lock().unwrap().after_publish = Some(Box::new(move || change(&storage)));
        let result = fixture.publish(with(
            with(request(), "photo_file", photo.clone().into()),
            "id",
            id.into(),
        ));
        // 服务器已经收下了这一款。
        assert_eq!(fixture.published().len(), 1, "{cause}");
        assert_eq!(publish_outcome(&result), "unconfirmed", "{cause}");
        let error = result.unwrap_err();
        assert!(error.contains(cause), "{error}");
        assert!(
            error.contains("may or may not have been published"),
            "{error}"
        );
        assert!(error.contains(id), "{error}");
        assert!(error.contains("list_community_keyboard_skins"), "{error}");
        assert!(!error.contains("try again"), "{error}");
        assert!(!error.contains(JPEG_BASE64), "{error}");
        assert!(!error.contains(&photo), "{error}");
        assert!(!error.contains(ACCESS_TOKEN), "{error}");
    }
}

#[test]
fn the_accounts_own_keyboard_skins_are_listed_page_by_page() {
    let fixture = Fixture::new(true);
    let ids: Vec<Uuid> = (0..45).map(|_| Uuid::new_v4()).collect();
    let mut skins: Vec<CommunitySkin> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| skin(*id, &format!("皮肤 {index}")))
        .collect();
    skins[1].moderation = Some(CommunityModeration::Removed);
    skins[1].category = None;
    skins[2].moderation = None;
    fixture.api.0.lock().unwrap().listed = skins;

    let listed = list(&fixture.community()).unwrap();
    assert_eq!(listed.skins.len(), 45);
    assert_eq!(
        listed.skins[0],
        CommunityKeyboardSkinView {
            id: ids[0].hyphenated().to_string(),
            name: "皮肤 0".into(),
            description: "合成的键盘皮肤".into(),
            category: Some(Category::Cute),
            moderation: Some(Moderation::Pending),
            downloads: 3,
            rating_count: 2,
            rating_average: 4.5,
        }
    );
    assert_eq!(listed.skins[1].moderation, Some(Moderation::Removed));
    assert_eq!(listed.skins[1].category, None);
    assert_eq!(listed.skins[2].moderation, None);
    assert_eq!(listed.skins[44].id, ids[44].hyphenated().to_string());
    // 只列自己的，一页接一页，不筛分类，也不按来源排除。
    assert_eq!(
        fixture.api.calls(),
        [0, 20, 40].map(|offset| format!(
            "list offset={offset} search=\"\" mine=true category=false exclude_ai=false"
        ))
    );
    assert!(fixture
        .api
        .0
        .lock()
        .unwrap()
        .bearers
        .iter()
        .all(|bearer| bearer == ACCESS_TOKEN));

    let value = serde_json::to_value(&listed).unwrap();
    crate::assert_required_fields_present::<CommunityKeyboardSkinList>(&value);
    crate::assert_required_fields_present::<CommunityKeyboardSkinView>(&value["skins"][0]);
    assert_eq!(value["skins"][0]["moderation"], "pending");
    assert_eq!(value["skins"][0]["category"], "cute");
    // 列表不带设计、照片、作者和来源。
    for absent in ["design", "photo", "author", "origin", "owned"] {
        assert!(value["skins"][0].get(absent).is_none(), "{absent}");
    }
    assert!(!value.to_string().contains(ACCESS_TOKEN));
}

#[test]
fn a_listing_that_never_ends_is_cut_off() {
    let fixture = Fixture::new(true);
    {
        let mut state = fixture.api.0.lock().unwrap();
        state.listed = (0..300)
            .map(|index| skin(Uuid::new_v4(), &format!("皮肤 {index}")))
            .collect();
        state.endless = true;
    }
    let listed = list(&fixture.community()).unwrap();
    assert_eq!(listed.skins.len(), MAX_LISTED);
    assert_eq!(fixture.api.calls().len(), MAX_PAGES);

    // 说后面还有却给了空页，也到此为止。
    let empty = Fixture::new(true);
    empty.api.0.lock().unwrap().endless = true;
    assert!(list(&empty.community()).unwrap().skins.is_empty());
    assert_eq!(empty.api.calls().len(), 1);
}

#[test]
fn a_keyboard_skin_is_deleted_by_its_id() {
    let fixture = Fixture::new(true);
    let id = Uuid::new_v4();
    let deleted = delete(
        &fixture.community(),
        &CommunityKeyboardSkinRequest {
            id: format!(" {} ", id.hyphenated()),
        },
    )
    .unwrap();
    assert_eq!(deleted.id, id.hyphenated().to_string());
    crate::assert_required_fields_present::<DeletedCommunityKeyboardSkin>(
        &serde_json::to_value(&deleted).unwrap(),
    );
    assert_eq!(fixture.api.calls(), [format!("delete {id}")]);

    for bad in ["not-an-id", "00000000-0000-0000-0000-000000000000"] {
        let error = delete(
            &fixture.community(),
            &CommunityKeyboardSkinRequest { id: bad.into() },
        )
        .unwrap_err();
        assert!(
            error.contains("list_community_keyboard_skins gives the ids"),
            "{error}"
        );
    }
    assert_eq!(fixture.api.calls().len(), 1);

    // 不是自己的或已经删掉的，说去哪里查。
    fixture.api.refuse(AccountError::NotFound);
    let error = delete(
        &fixture.community(),
        &CommunityKeyboardSkinRequest { id: id.to_string() },
    )
    .unwrap_err();
    assert!(error.contains("has no such keyboard skin"), "{error}");
    assert!(error.contains("list_community_keyboard_skins"), "{error}");
    fixture.api.refuse(AccountError::Unavailable);
    let error = delete(
        &fixture.community(),
        &CommunityKeyboardSkinRequest { id: id.to_string() },
    )
    .unwrap_err();
    assert!(error.contains("try again later"), "{error}");
    assert!(
        !error.contains("may or may not have been published"),
        "{error}"
    );
}

#[test]
fn the_descriptions_tell_keyboard_skins_from_candidate_window_skins() {
    for word in [
        "touch keyboard on iOS, Android and HarmonyOS",
        "This is not a candidate-window skin",
        "This server never signs in or handles passwords",
        "always public at once",
        "cannot be edited afterwards, only deleted with delete_community_keyboard_skin and published again",
        "ask the user in plain words to confirm that everyone may see it and that they made the photo or have the right to share it",
        "Do not publish AI-generated artwork with this tool",
        "cornerRadius (0 to 20",
        "keyOpacity (0.25 to 1)",
        "must already be a JPEG no larger than 1024 pixels on its long side and 512,000 bytes",
        "does not convert, scale or re-encode images",
        "an account keeps at most 50 keyboard skins",
        "Keep keyForeground clearly readable on keyBackground",
        "Only offered when the user started the server with --allow-write",
    ] {
        assert!(PUBLISH_DESCRIPTION.contains(word), "{word}");
    }
    assert!(LIST_DESCRIPTION.contains("not the candidate-window skins list_community_skins lists"));
    assert!(LIST_DESCRIPTION.contains("this server never signs in itself"));
    assert!(DELETE_DESCRIPTION.contains("Confirm with the user first"));
    assert!(DELETE_DESCRIPTION.contains("delete_community_skin instead"));
}
