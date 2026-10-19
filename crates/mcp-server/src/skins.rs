//! AI 助手通过共享安装器创建候选皮肤，清单和资源与社区下载使用同一套校验。
//!
//! 资源支持 PNG/JPEG/APNG/GIF/WebP，预览保持 PNG/JPEG。MCP 只写本地皮肤，
//! 不持有账号会话；宿主在用户触发同步时检查服务端能力，再上传扩展包。

use msime_client_core::uuid::Uuid;
use msime_client_services::skin::candidate_community::{self, CandidateSkinPackage};
use msime_client_services::skin::{candidate_sync, catalog};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub const SKIN_DIRECTORY: &str = "skins";

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinList {
    pub skins: Vec<SkinView>,
    /// Folders under the skin directory that are not a usable skin, with the reason the input method gives.
    pub issues: Vec<SkinIssueView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinView {
    /// The folder name and the manifest id.
    pub id: String,
    pub schema_version: u8,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The candidate layouts the skin supports: horizontal, vertical.
    pub layouts: Vec<String>,
    /// The colour modes the skin supports: dark, light.
    pub themes: Vec<String>,
    /// 清单基底是否为 `canvas`：仅此时读取九宫格、自由挂件和边距。Web engine 与 macOS、Windows、iOS、Android、HarmonyOS 原生候选窗通过各自图片解码器绘制贴图，尚无对应绘制能力的宿主（目前是 Linux）使用不透明的可读回退。
    pub canvas: bool,
    /// 不阻止加载的提示，例如非 canvas 基底上忽略的专用字段，或需要宿主绘制能力的 canvas 贴图；没有提示时是空列表。始终输出：输出 schema 把它列为必填，省略会让按 schema 校验结构化结果的客户端（如 Claude Code）拒绝整个列表。
    pub notices: Vec<String>,
    /// 有限的 schema v2 动效契约。版本 1 皮肤报告为 `none` 预设。
    pub animation: SkinAnimationView,
    /// 候选窗上方的装饰图，皮肤没有装饰图时省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decoration: Option<SkinDecorationView>,
    /// Whether the desktop app has synced this skin to the user's cloud library.
    pub synced: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinDecorationView {
    /// 候选窗在屏幕上时整张装饰图的待机动效：none、float、sway、breathe 或 bounce。
    pub motion: String,
    /// 待机动效一个周期的毫秒数。
    pub motion_period_ms: u16,
    /// 装饰图的帧数：静态 PNG、JPEG 为 1，APNG（以及手动装进皮肤目录的 GIF、WebP）为它的动画帧数。
    pub frames: u32,
    /// 装饰图是不是多帧的 APNG（frames 大于 1）。能播帧动画的宿主（目前是 macOS）播放它，其他宿主只画第一帧。
    pub animated: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinAnimationView {
    pub enter: String,
    pub exit: String,
    pub selection: String,
    pub duration_ms: u16,
    pub stagger_ms: u16,
    /// 三个预设各自是否由清单的 `[animation]` 表写出。没写的预设是补上的默认值：版本 2 皮肤省略时 Windows 和网页仍按默认组合播放，macOS 只播放写出来的预设，所以要在 macOS 上动起来必须显式写出。
    pub declared: SkinAnimationDeclaredView,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinAnimationDeclaredView {
    pub enter: bool,
    pub exit: bool,
    pub selection: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinIssueView {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CreateSkinRequest {
    /// The skin's folder name, which must also be the manifest's `id`: lowercase ASCII letters, digits, `_`, `-` and `.`, starting with a letter or digit, at most 64 bytes, and not a reserved id: a built-in theme (system, shuishan, light, paper, night, ink), an msime-windows built-in look (fluent, wechat, wechat_scroll, graphite, willow_green, autumn_osmanthus, microsoft), canvas or default.
    pub package_id: String,
    /// The whole `skin.toml`, at most 64 KiB.
    pub manifest: String,
    /// 清单引用的全部资源，以包内相对路径为键、标准 base64 为值。支持 PNG/JPEG/APNG/GIF/WebP，预览限 PNG/JPEG；最多 37 张，每张 1 MiB、每边 2048 像素，合计 2 MiB 和 8,000,000 像素，预览最多 256 KiB。 动画最多 120 帧，每帧按整张画布计入像素预算。
    pub images: BTreeMap<String, String>,
    /// Replace an existing skin of this id whole. Without it an existing skin is left alone and the call refused.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CreatedSkin {
    pub id: String,
    /// Whether an existing skin of this id was replaced.
    pub replaced: bool,
}

fn root(state_dir: &Path) -> PathBuf {
    state_dir.join(SKIN_DIRECTORY)
}

pub fn list(state_dir: &Path) -> SkinList {
    let root = root(state_dir);
    let synced = candidate_sync::synced_packages(&state_dir.join(candidate_sync::STATE_FILE));
    let catalog = catalog::scan(&root);
    SkinList {
        skins: catalog
            .packages
            .into_iter()
            .map(|package| SkinView {
                synced: synced.contains_key(&package.id),
                id: package.id,
                schema_version: package.schema_version,
                name: package.name,
                version: package.version,
                description: package.description,
                layouts: package.layouts,
                themes: package.themes,
                canvas: package.canvas,
                notices: package.notices,
                animation: SkinAnimationView {
                    enter: package.animation.enter.id().into(),
                    exit: package.animation.exit.id().into(),
                    selection: package.animation.selection.id().into(),
                    duration_ms: package.animation.duration_ms,
                    stagger_ms: package.animation.stagger_ms,
                    declared: SkinAnimationDeclaredView {
                        enter: package.animation.declared.enter,
                        exit: package.animation.declared.exit,
                        selection: package.animation.declared.selection,
                    },
                },
                decoration: package
                    .decoration_image
                    .is_some()
                    .then(|| SkinDecorationView {
                        motion: package.decoration_motion.id().into(),
                        motion_period_ms: package.decoration_motion_period_ms,
                        frames: package.decoration_frames,
                        animated: package.decoration_frames > 1,
                    }),
            })
            .collect(),
        issues: catalog
            .issues
            .into_iter()
            .map(|issue| SkinIssueView {
                folder: issue.folder,
                reason: issue.reason,
            })
            .collect(),
    }
}

pub fn create(state_dir: &Path, request: &CreateSkinRequest) -> Result<CreatedSkin, String> {
    let root = root(state_dir);
    let replaced =
        request.replace && std::fs::symlink_metadata(root.join(&request.package_id)).is_ok();
    let package = CandidateSkinPackage {
        // Not read by install; the gallery sets it to the publication it downloaded.
        id: Uuid::nil(),
        package_id: request.package_id.clone(),
        manifest: request.manifest.clone(),
        files: request.images.clone(),
    };
    match candidate_community::install(&root, &package, request.replace) {
        Ok(id) => Ok(CreatedSkin { id, replaced }),
        Err("candidate_skin_package") => Err(manifest_problem(request)),
        Err(code) => Err(explain(code).to_owned()),
    }
}

fn explain(code: &str) -> &'static str {
    match code {
        "candidate_skin_exists" => "a skin with this package_id is already installed; list_candidate_skins shows it. Pass replace: true to replace it whole, or choose another id",
        "candidate_skin_file_path" => "images must hold 1 to 37 entries, the preview and any decoration, background, border_image and decorations images counted together, each a relative path inside the skin folder other than skin.toml, unique ignoring case",
        "candidate_skin_file_type" => "resources must be .png, .jpg, .jpeg, .apng, .gif or .webp; the preview must be .png, .jpg or .jpeg, and stylesheets are not accepted",
        "candidate_skin_too_large" => "too large: the manifest may be 64 KiB, each image 1 MiB and 2048 pixels on each side, all images 2 MiB and 8,000,000 pixels together (each animated frame counts as a whole image), an animation at most 120 frames, and the preview 256 KiB",
        "candidate_skin_image_invalid" => "an image is not valid standard base64, or its bytes (every animation frame) cannot be decoded using the format its extension names: it may be truncated or corrupted, so generate it again and pass the whole base64",
        "storage" => "the skin folder could not be written",
        _ => "the skin was refused",
    }
}

const BASE_RULES: &str = "skin.toml: base must be system, a built-in theme (shuishan, light, paper, night, ink), an msime-windows built-in look (fluent, wechat, wechat_scroll, graphite, willow_green, autumn_osmanthus, microsoft), or canvas with schema_version = 2";

/// Why `install` refused the manifest, in words an agent can act on. `install` reports every manifest problem with one code, so the manifest is loaded again here, beside empty stand-ins for the images, by the rule the catalog lists skins by.
fn manifest_problem(request: &CreateSkinRequest) -> String {
    if !catalog::is_external_id(&request.package_id) {
        return "package_id must be lowercase ASCII letters, digits, `_`, `-` and `.`, start with a letter or digit, be at most 64 bytes, and not be a reserved id: a built-in theme (system, shuishan, light, paper, night, ink), an msime-windows built-in look (fluent, wechat, wechat_scroll, graphite, willow_green, autumn_osmanthus, microsoft), canvas or default".into();
    }
    const SHARING_RULES: &str = "a skin must set `preview` to a .png, .jpg or .jpeg image, must not set `toolbar_stylesheet`, and images must hold exactly the images the manifest references";
    let Ok(scratch) = tempfile::tempdir() else {
        return SHARING_RULES.into();
    };
    let folder = scratch.path().join(&request.package_id);
    if std::fs::create_dir(&folder).is_err()
        || std::fs::write(folder.join("skin.toml"), &request.manifest).is_err()
    {
        return SHARING_RULES.into();
    }
    for path in request.images.keys() {
        // Only plain relative paths are stood in for; install already refused anything else with its own code.
        if !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            continue;
        }
        let stand_in = folder.join(path);
        if let Some(parent) = stand_in.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(stand_in, []);
    }
    match catalog::load_package(scratch.path(), &request.package_id) {
        Err(reason) if reason == "manifest id does not match folder" => {
            "the manifest's id must equal package_id".into()
        }
        Err(reason) if reason == "base must be system or a built-in theme" => BASE_RULES.into(),
        Err(reason) => format!("skin.toml: {reason}"),
        Ok(_) => SHARING_RULES.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGBgAAAABQABpfZFQAAAAABJRU5ErkJggg==";

    fn manifest(id: &str, name: &str) -> String {
        format!(
            "schema_version = 1\nid = '{id}'\nname = '{name}'\nversion = '1.0'\nbase = 'night'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal', 'vertical']\nthemes = ['dark', 'light']\n[candidate_window]\nmin_width_dip = 200\n[candidate.dark]\naccent = '#ff8800'\n"
        )
    }

    fn request(id: &str, name: &str, replace: bool) -> CreateSkinRequest {
        CreateSkinRequest {
            package_id: id.into(),
            manifest: manifest(id, name),
            images: BTreeMap::from([("preview.png".into(), PNG.into())]),
            replace,
        }
    }

    #[test]
    fn every_field_the_output_schema_requires_is_in_the_listing() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), &request("sunset", "Sunset", false)).unwrap();
        let listed = serde_json::to_value(list(state.path())).unwrap();
        let skin = &listed["skins"][0];
        let schema = serde_json::to_value(rmcp::schemars::schema_for!(SkinView)).unwrap();
        let required = schema["required"]
            .as_array()
            .expect("SkinView has required fields");
        for field in required {
            let field = field.as_str().unwrap();
            assert!(
                skin.get(field).is_some(),
                "the output schema requires {field}, but the listing left it out: {skin}"
            );
        }
        assert_eq!(skin["notices"], serde_json::json!([]));
    }

    #[test]
    fn a_created_skin_is_listed_as_the_catalog_reads_it() {
        let state = tempfile::tempdir().unwrap();
        assert_eq!(
            create(state.path(), &request("sunset", "Sunset", false)).unwrap(),
            CreatedSkin {
                id: "sunset".into(),
                replaced: false
            }
        );
        assert!(state.path().join("skins/sunset/preview.png").is_file());
        let listed = list(state.path());
        assert!(listed.issues.is_empty());
        assert_eq!(
            listed.skins,
            [SkinView {
                id: "sunset".into(),
                schema_version: 1,
                name: "Sunset".into(),
                version: "1.0".into(),
                description: None,
                layouts: vec!["horizontal".into(), "vertical".into()],
                themes: vec!["dark".into(), "light".into()],
                canvas: false,
                notices: Vec::new(),
                animation: SkinAnimationView {
                    enter: "none".into(),
                    exit: "none".into(),
                    selection: "none".into(),
                    duration_ms: 0,
                    stagger_ms: 0,
                    declared: SkinAnimationDeclaredView {
                        enter: false,
                        exit: false,
                        selection: false,
                    },
                },
                decoration: None,
                synced: false,
            }]
        );
    }

    #[test]
    fn the_listing_tells_declared_presets_from_filled_in_defaults() {
        let state = tempfile::tempdir().unwrap();
        for (id, animation) in [
            ("implicit", ""),
            ("enter_only", "[animation]\nenter = 'slide_up'\n"),
        ] {
            let manifest = format!(
                "schema_version = 2\nid = '{id}'\nname = 'Motion'\nversion = '2.0'\nbase = 'system'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n{animation}"
            );
            create(
                state.path(),
                &CreateSkinRequest {
                    package_id: id.into(),
                    manifest,
                    images: BTreeMap::from([("preview.png".into(), PNG.into())]),
                    replace: false,
                },
            )
            .unwrap();
        }
        let listed = list(state.path());
        let skin = |id: &str| listed.skins.iter().find(|skin| skin.id == id).unwrap();
        let animation = |id: &str| &skin(id).animation;
        // 版本 2 的动效不限于 canvas：base = system 的清单照样按版本 2 列出。
        assert_eq!(skin("implicit").schema_version, 2);
        assert!(!skin("implicit").canvas);
        // 省略 `[animation]` 的版本 2 皮肤照样报出默认组合，但标明一个都没写，macOS 不会播放。
        let implicit = animation("implicit");
        assert_eq!(implicit.enter, "fade_scale");
        assert_eq!(
            implicit.declared,
            SkinAnimationDeclaredView {
                enter: false,
                exit: false,
                selection: false,
            }
        );
        let enter_only = animation("enter_only");
        assert_eq!(enter_only.enter, "slide_up");
        assert_eq!(enter_only.selection, "spring");
        assert_eq!(
            enter_only.declared,
            SkinAnimationDeclaredView {
                enter: true,
                exit: false,
                selection: false,
            }
        );
    }

    /// 2×2、3 帧、无限循环的 APNG，默认图是第一帧。
    const APNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAACGFjVEwAAAADAAAAAM7tusAAAAAaZmNUTAAAAAAAAAACAAAAAgAAAAAAAAAAAAEACgAA6FTcAAAAABFJREFUeNpj+M/A8B+EGWAMAEfKB/katvGpAAAAGmZjVEwAAAABAAAAAgAAAAIAAAAAAAAAAAABAAoAAHMnNtQAAAASZmRBVAAAAAJ42mNg+A+FMAYAQ84H+cMAaa8AAAAaZmNUTAAAAAMAAAACAAAAAgAAAAAAAAAAAAEACgAAnrHlPQAAABRmZEFUAAAABHjaY2Bg+P8fgqEMAD/SB/mZNdnKAAAAAElFTkSuQmCC";

    #[test]
    fn the_listing_reports_decoration_motion_and_frames() {
        let state = tempfile::tempdir().unwrap();
        for (id, image, motion) in [
            ("whale", APNG, "motion = 'sway'\nmotion_period_ms = 2400\n"),
            ("still", PNG, ""),
        ] {
            let manifest = format!(
                "schema_version = 2\nid = '{id}'\nname = 'Whale'\nversion = '1.0'\nbase = 'system'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = 'whale.png'\ntop_inset_dip = 40\nwidth_dip = 60\n{motion}"
            );
            create(
                state.path(),
                &CreateSkinRequest {
                    package_id: id.into(),
                    manifest,
                    images: BTreeMap::from([
                        ("preview.png".into(), PNG.into()),
                        ("whale.png".into(), image.into()),
                    ]),
                    replace: false,
                },
            )
            .unwrap();
        }
        let listed = list(state.path());
        let decoration = |id: &str| {
            listed
                .skins
                .iter()
                .find(|skin| skin.id == id)
                .unwrap()
                .decoration
                .as_ref()
                .unwrap()
        };
        assert_eq!(
            decoration("whale"),
            &SkinDecorationView {
                motion: "sway".into(),
                motion_period_ms: 2400,
                frames: 3,
                animated: true,
            }
        );
        assert_eq!(
            decoration("still"),
            &SkinDecorationView {
                motion: "none".into(),
                motion_period_ms: 3000,
                frames: 1,
                animated: false,
            }
        );
    }

    /// A version 2 manifest on `base`, with `extra` tables after `[candidate_window]`.
    fn v2_manifest(id: &str, base: &str, extra: &str) -> String {
        format!(
            "schema_version = 2\nid = '{id}'\nname = 'Panel'\nversion = '1.0'\nbase = '{base}'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n{extra}"
        )
    }

    const CANVAS_TABLES: &str = "[candidate_window.border_image]\nsource = 'panel.png'\nslice_dip = [8]\n[[candidate_window.decorations]]\nimage = 'leaf.png'\nanchor = 'top-left'\nwidth_dip = 24\nheight_dip = 24\n";

    #[test]
    fn a_canvas_skin_with_a_border_image_and_widgets_is_created_and_listed() {
        let state = tempfile::tempdir().unwrap();
        create(
            state.path(),
            &CreateSkinRequest {
                package_id: "panel".into(),
                manifest: v2_manifest("panel", "canvas", CANVAS_TABLES),
                images: BTreeMap::from([
                    ("preview.png".into(), PNG.into()),
                    ("panel.png".into(), PNG.into()),
                    ("leaf.png".into(), PNG.into()),
                ]),
                replace: false,
            },
        )
        .unwrap();
        let listed = list(state.path());
        assert!(listed.issues.is_empty(), "{:?}", listed.issues);
        let skin = &listed.skins[0];
        assert_eq!(skin.id, "panel");
        assert_eq!(skin.schema_version, 2);
        assert!(skin.canvas);
        assert_eq!(skin.notices.len(), 1, "{:?}", skin.notices);
        assert!(skin.notices[0].contains("not drawn"), "{:?}", skin.notices);
        assert_eq!(skin.animation.enter, "fade_scale");

        // 九宫格面板和挂件可超过旧协议的三张限制，仍须完整引用。
        let crowded = CreateSkinRequest {
            package_id: "crowded".into(),
            manifest: v2_manifest(
                "crowded",
                "canvas",
                &format!("{CANVAS_TABLES}[[candidate_window.decorations]]\nimage = 'star.png'\nwidth_dip = 12\nheight_dip = 12\n"),
            ),
            images: BTreeMap::from([
                ("preview.png".into(), PNG.into()),
                ("panel.png".into(), PNG.into()),
                ("leaf.png".into(), PNG.into()),
                ("star.png".into(), PNG.into()),
            ]),
            replace: false,
        };
        assert_eq!(create(state.path(), &crowded).unwrap().id, "crowded");
    }

    #[test]
    fn a_version_2_skin_on_another_base_keeps_its_animation_and_ignores_canvas_keys() {
        let state = tempfile::tempdir().unwrap();
        // msime-windows ignores the canvas-only tables under any other base, so their images are neither required nor accepted; [animation] and decoration motion stay version 2 features of every base.
        create(
            state.path(),
            &CreateSkinRequest {
                package_id: "lively".into(),
                manifest: v2_manifest(
                    "lively",
                    "system",
                    &format!("{CANVAS_TABLES}[animation]\nenter = 'slide_up'\n"),
                ),
                images: BTreeMap::from([("preview.png".into(), PNG.into())]),
                replace: false,
            },
        )
        .unwrap();
        let listed = list(state.path());
        let skin = &listed.skins[0];
        assert_eq!(skin.schema_version, 2);
        assert!(!skin.canvas);
        assert_eq!(skin.notices.len(), 1, "{:?}", skin.notices);
        assert!(
            skin.notices[0].contains("border_image"),
            "{:?}",
            skin.notices
        );
        assert_eq!(skin.animation.enter, "slide_up");
        assert!(skin.animation.declared.enter);
    }

    #[test]
    fn a_skin_on_the_wechat_scroll_look_is_created() {
        let state = tempfile::tempdir().unwrap();
        let mut scroll = request("scroll", "Scroll", false);
        scroll.manifest = scroll.manifest.replace("'night'", "'wechat_scroll'");
        create(state.path(), &scroll).unwrap();
        let listed = list(state.path());
        assert_eq!(listed.skins[0].id, "scroll");
        assert!(!listed.skins[0].canvas);
    }

    #[test]
    fn an_existing_skin_is_replaced_only_when_asked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), &request("sunset", "Sunset", false)).unwrap();
        let refused = create(state.path(), &request("sunset", "Dusk", false)).unwrap_err();
        assert!(refused.contains("replace: true"), "{refused}");
        assert_eq!(list(state.path()).skins[0].name, "Sunset");
        assert!(
            create(state.path(), &request("sunset", "Dusk", true))
                .unwrap()
                .replaced
        );
        assert_eq!(list(state.path()).skins[0].name, "Dusk");
    }

    #[test]
    fn a_refused_manifest_says_what_is_wrong() {
        let state = tempfile::tempdir().unwrap();
        let mut mismatched = request("sunset", "Sunset", false);
        mismatched.package_id = "dusk".into();
        assert!(create(state.path(), &mismatched)
            .unwrap_err()
            .contains("must equal package_id"));

        let mut unsupported = request("sunset", "Sunset", false);
        unsupported.manifest = unsupported.manifest.replace("'night'", "'custom'");
        assert_eq!(create(state.path(), &unsupported).unwrap_err(), BASE_RULES);

        // msime-windows 的规则：canvas 只用于版本 2。
        let mut static_canvas = request("sunset", "Sunset", false);
        static_canvas.manifest = static_canvas.manifest.replace("'night'", "'canvas'");
        assert_eq!(
            create(state.path(), &static_canvas).unwrap_err(),
            "skin.toml: base = canvas requires schema_version 2"
        );

        let mut no_preview = request("sunset", "Sunset", false);
        no_preview.manifest = no_preview.manifest.replace("preview = 'preview.png'\n", "");
        assert!(create(state.path(), &no_preview)
            .unwrap_err()
            .contains("must set `preview`"));

        let mut reserved = request("night", "Night", false);
        reserved.manifest = manifest("night", "Night");
        assert!(create(state.path(), &reserved)
            .unwrap_err()
            .contains("reserved id"));
        for id in ["canvas", "wechat_scroll", "default"] {
            let mut reserved = request(id, "Reserved", false);
            reserved.manifest = manifest(id, "Reserved");
            let refused = create(state.path(), &reserved).unwrap_err();
            assert!(refused.contains("reserved id"), "{id}: {refused}");
            assert!(refused.contains("canvas or default"), "{id}: {refused}");
        }

        let mut not_png = request("sunset", "Sunset", false);
        not_png.images = BTreeMap::from([("preview.png".into(), "bm90IGEgcG5n".into())]);
        assert!(create(state.path(), &not_png)
            .unwrap_err()
            .contains("cannot be decoded"));

        // 签名和 IHDR 完好、像素数据和 IEND 都缺失：只看签名的检查会把它当成 PNG 装进去。
        let mut truncated = request("sunset", "Sunset", false);
        truncated.images = BTreeMap::from([("preview.png".into(), PNG[..44].into())]);
        assert!(create(state.path(), &truncated)
            .unwrap_err()
            .contains("cannot be decoded using the format"));

        assert!(list(state.path()).skins.is_empty());
    }

    #[test]
    fn a_synced_skin_is_marked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), &request("sunset", "Sunset", false)).unwrap();
        std::fs::write(
            state.path().join(candidate_sync::STATE_FILE),
            serde_json::to_vec(&serde_json::json!({
                "user_id": "user",
                "packages": { "sunset": { "cloud_id": Uuid::new_v4(), "cloud_digest": "a", "local_digest": "b" } }
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(list(state.path()).skins[0].synced);
    }
}
