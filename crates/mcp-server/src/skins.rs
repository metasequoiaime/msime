//! AI 助手通过共享安装器创建候选皮肤，清单和资源与社区下载使用同一套校验。
//!
//! 资源支持 PNG/JPEG/APNG/GIF/WebP，预览保持 PNG/JPEG。图片可以是 base64，也可以是本机文件的绝对路径：代理先用代码把图写成文件再给路径，免得几十 KB 的 base64 在转写时出错。这里只写本地皮肤；发布到社区在 `community` 模块，借用设置应用保存的账号会话，服务器自己从不登录。设置应用在用户触发同步时检查服务端能力，再上传扩展包。

use msime_client_core::edition::Edition;
use msime_client_core::preferences::PreferencesStore;
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
    /// 清单基底是否为 `canvas`：仅此时读取九宫格、自由挂件和边距。Web engine 与 macOS、Windows、iOS、Android、HarmonyOS 2in1 和 Linux X11 的对应原生入口通过各自图片解码器绘制贴图；Linux 原生窗口还要求焦点与 classicui 所有权，其他入口沿用各自静态或不透明的可读回退。声明可读不代表目标设备已完成绘制验收。
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
    /// 装饰图是否多帧（frames 大于 1）；这是资源元数据，实际帧播放与待机变换分别取决于宿主入口的能力，不能据此认定所有宿主都已播放动画。
    pub animated: bool,
    /// 装饰框内叠加候选词的声明；绘制覆盖面由各宿主决定，不能把列表中的声明当成设备验收结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_overlay: Option<SkinDecorationCandidateView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinDecorationCandidateView {
    /// 0 跟随高亮，1 起为当前页候选序号；越出当前页时不显示。
    pub index: u8,
    pub font_size_dip: f64,
    pub min_font_size_dip: f64,
    /// 0 使用装饰框宽；缩到最小字号仍放不下时不换行、允许超出。
    pub max_width_dip: f64,
    /// 原始安全颜色声明；不能识别的颜色由绘制器回退为白色。
    pub color: String,
    pub align_h: String,
    pub align_v: String,
    pub offset_x_dip: f64,
    pub offset_y_dip: f64,
    pub strip_badge: bool,
}

impl From<&catalog::SkinDecorationCandidate> for SkinDecorationCandidateView {
    fn from(value: &catalog::SkinDecorationCandidate) -> Self {
        Self {
            index: value.index,
            font_size_dip: value.font_size_dip,
            min_font_size_dip: value.min_font_size_dip,
            max_width_dip: value.max_width_dip,
            color: value.color.clone(),
            align_h: match value.align_h {
                catalog::DecorationAlign::Left => "left",
                catalog::DecorationAlign::Center => "center",
                catalog::DecorationAlign::Right => "right",
            }
            .into(),
            align_v: match value.align_v {
                catalog::DecorationVerticalAlign::Top => "top",
                catalog::DecorationVerticalAlign::Center => "center",
                catalog::DecorationVerticalAlign::Bottom => "bottom",
            }
            .into(),
            offset_x_dip: value.offset_x_dip,
            offset_y_dip: value.offset_y_dip,
            strip_badge: value.strip_badge,
        }
    }
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
    /// 以标准 base64 给出的资源，以包内相对路径为键。支持 PNG/JPEG/APNG/GIF/WebP，预览限 PNG/JPEG；与 image_files 合计最多 64 张（含工具栏图片），每张 1 MiB、每边 2048 像素，合计 2 MiB 和 8,000,000 像素，预览最多 256 KiB。动画最多 120 帧，每帧按整张画布计入像素预算。社区上传沿用服务端的 37 张上限，图片工具栏另需明确能力声明。
    #[serde(default)]
    pub images: BTreeMap<String, String>,
    /// 以本机文件给出的资源，以包内相对路径为键、文件的绝对路径为值。服务器只读这些文件：路径上不能有符号链接，必须是普通文件，超过 1 MiB 的在读之前就拒绝；读进来的字节和 images 里的 base64 走同一套检查和上限。同一个包内路径不能两边都给。
    #[serde(default)]
    pub image_files: BTreeMap<String, String>,
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

/// 指定一款已安装的皮肤。
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct SkinRequest {
    /// 皮肤的 id，即 list_candidate_skins 给出的 id，也是皮肤目录里的文件夹名。
    pub package_id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeletedSkin {
    pub id: String,
    /// 设置应用是否把它同步到过用户的云端皮肤库。同步过的，设置应用下一次同步时把库里私有的那份一起删掉；公开发布的那份保留，删除前在另一台设备上改过的那份会被下载回来。
    pub synced: bool,
}

/// get_candidate_skin 给出的一款已安装皮肤：改好清单后连同图片交给 create_candidate_skin（replace: true）重装。
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinFiles {
    pub id: String,
    /// `skin.toml` 全文。
    pub manifest: String,
    /// 皮肤文件夹在本机的绝对路径。
    pub directory: String,
    /// 清单引用的每张图片；文件夹里清单没引用的文件不列。
    pub images: Vec<SkinFileView>,
    /// 设置应用是否把它同步到过用户的云端皮肤库。
    pub synced: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct SkinFileView {
    /// 清单里写的包内路径，也是 create_candidate_skin 的 images 和 image_files 用的键。
    pub path: String,
    /// 这张图在本机的绝对路径，可以原样作为 image_files 的值。
    pub file: String,
    /// 文件的字节数。
    pub bytes: u64,
    /// 按内容认出的编码：png、apng（多于一帧的 PNG）、jpeg、gif 或 webp；认不出时为 null。
    pub format: Option<String>,
    /// 一帧的宽，像素；认不出时为 null。
    pub width: Option<u32>,
    /// 一帧的高，像素；认不出时为 null。
    pub height: Option<u32>,
    /// 动画帧数，静态图为 1。
    pub frames: u32,
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
                        candidate_overlay: package.decoration_candidate.as_ref().map(Into::into),
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

/// 装一款皮肤。`linux_host_options` 是 Linux 宿主读皮肤目录的运行时选项文档（[`crate::config::Config::linux_host_options`]），装好后把新目录发布进去。
pub fn create(
    state_dir: &Path,
    linux_host_options: Option<&Path>,
    request: &CreateSkinRequest,
) -> Result<CreatedSkin, String> {
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
    let local_files: BTreeMap<String, PathBuf> = request
        .image_files
        .iter()
        .map(|(path, file)| (path.clone(), PathBuf::from(file)))
        .collect();
    match candidate_community::install_with_local_files(
        &root,
        &package,
        &local_files,
        request.replace,
    ) {
        Ok(id) => {
            republish(linux_host_options, &root);
            Ok(CreatedSkin { id, replaced })
        }
        Err("candidate_skin_package") => Err(manifest_problem(request)),
        Err("candidate_skin_file_unreadable") => Err(local_file_problem(&request.image_files)),
        Err(code) => Err(explain(code).to_owned()),
    }
}

/// 读一款已安装皮肤的清单原文和它引用的图片，规则与 list_candidate_skins 列出的皮肤相同。
pub fn get(state_dir: &Path, request: &SkinRequest) -> Result<SkinFiles, String> {
    let id = request.package_id.as_str();
    if !catalog::is_external_id(id) {
        return Err(format!("{id:?} is not a skin the user installed: built-in themes, msime-windows built-in looks, canvas and default have no skin folder"));
    }
    let root = root(state_dir);
    let directory = root.join(id);
    let files = catalog::package_files(&root, id).map_err(|reason| {
        if std::fs::symlink_metadata(&directory).is_err() {
            format!("no skin {id} is installed; list_candidate_skins shows the installed skins")
        } else {
            format!("{id} cannot be read as a skin ({reason}); list_candidate_skins lists it under issues with the reason")
        }
    })?;
    let synced = candidate_sync::synced_packages(&state_dir.join(candidate_sync::STATE_FILE))
        .contains_key(id);
    Ok(SkinFiles {
        id: id.to_owned(),
        manifest: files.manifest,
        directory: directory.display().to_string(),
        images: files
            .images
            .into_iter()
            .map(|image| SkinFileView {
                file: directory.join(&image.path).display().to_string(),
                path: image.path,
                bytes: image.bytes,
                format: image.format.map(str::to_owned),
                width: image.dimensions.map(|(width, _)| width),
                height: image.dimensions.map(|(_, height)| height),
                frames: image.frames,
            })
            .collect(),
        synced,
    })
}

/// 删掉一款用户装的皮肤。偏好里选着它（浅色或深色槽位，无论当前主题是不是 custom）时拒绝，不顺手改偏好：改偏好要走 update_preferences 的修订号比较，删除不该绕开它。同步状态由共享的 `candidate_sync::delete_installed` 保留，见那里的说明。
///
/// `linux_host_options` 同 [`create`]：删掉后把新目录发布给 Linux 宿主。
pub fn delete(
    state_dir: &Path,
    linux_host_options: Option<&Path>,
    edition: &'static Edition,
    request: &SkinRequest,
) -> Result<DeletedSkin, String> {
    let id = request.package_id.as_str();
    if !catalog::is_external_id(id) {
        return Err(format!("{id:?} is not a skin the user installed: built-in themes, msime-windows built-in looks, canvas and default cannot be deleted, and a skin id is lowercase ASCII letters, digits, `_`, `-` and `.`"));
    }
    let custom = PreferencesStore::for_edition(state_dir, edition)
        .load()
        .map_err(|error| error.to_string())?
        .preferences
        .custom_theme;
    let slots: Vec<&str> = [
        ("candidate_skin", &custom.candidate_skin),
        ("candidate_skin_dark", &custom.candidate_skin_dark),
    ]
    .into_iter()
    .filter(|(_, skin)| skin.as_deref() == Some(id))
    .map(|(slot, _)| slot)
    .collect();
    if !slots.is_empty() {
        return Err(format!("{id} is chosen as {} in the preferences; choose another skin there, or set it to null, with update_preferences first", slots.join(" and ")));
    }
    let root = root(state_dir);
    match candidate_sync::delete_installed(&root, &state_dir.join(candidate_sync::STATE_FILE), id) {
        Ok(synced) => {
            republish(linux_host_options, &root);
            Ok(DeletedSkin {
                id: id.to_owned(),
                synced,
            })
        }
        Err("candidate_skin_not_installed") => Err(format!(
            "no skin {id} is installed; list_candidate_skins shows the installed skins"
        )),
        Err("candidate_skin_package") => Err(format!("{id} in the skin directory is not a folder (it is a file or a symbolic link), which this tool does not remove")),
        Err(_) => Err("the skin folder could not be removed".into()),
    }
}

/// 皮肤目录写过之后，把重新扫描的结果发布给从运行时选项读皮肤的 Linux 宿主，与设置应用装好、删掉皮肤后的重扫（`rescan_skin_catalog`）相同。同名替换也要发布：宿主画的配色、装饰带尺寸和圆角都在目录条目里，条目不变它们就一直画旧包。
///
/// 和设置应用一样尽力而为：皮肤已经写好了，发布不成不让这次调用失败（重试只会再装一遍），只在 stderr 记一行，设置页下一次重扫时补上。
fn republish(linux_host_options: Option<&Path>, root: &Path) {
    let Some(options) = linux_host_options else {
        return;
    };
    if let Err(error) = crate::runtime_options::republish_skin_catalog(options, root) {
        eprintln!("msime-mcp: the skin catalog was not republished: {error}");
    }
}

const LOCAL_FILE_RULES: &str = "each image_files value must be the absolute path of a regular file on this computer that this user can read, with no symbolic link anywhere on the way (system links such as /tmp on macOS excepted); write the image there and pass that path";

/// 哪个本机文件读不出来、为什么。安装器对每种读不出来的情况只给一个代码，这里按路径做几项便宜的检查，好让代理知道该改哪一项；检查不出的（例如路径上某一级是符号链接）给出整条规则。
fn local_file_problem(image_files: &BTreeMap<String, String>) -> String {
    for (key, file) in image_files {
        let path = Path::new(file);
        let problem = if !path.is_absolute() {
            "is not an absolute path"
        } else {
            match std::fs::symlink_metadata(path) {
                Err(_) => "does not exist or cannot be reached",
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    "is a symbolic link; pass the file it points to"
                }
                Ok(metadata) if !metadata.is_file() => "is not a regular file",
                Ok(_) => continue,
            }
        };
        return format!("image_files[{key:?}] = {file:?} {problem}; {LOCAL_FILE_RULES}");
    }
    format!("an image file cannot be read: {LOCAL_FILE_RULES}")
}

fn explain(code: &str) -> &'static str {
    match code {
        "candidate_skin_exists" => "a skin with this package_id is already installed; list_candidate_skins shows it. Pass replace: true to replace it whole, or choose another id",
        "candidate_skin_file_path" => "images and image_files together must hold 1 to 64 entries, including preview, candidate and toolbar images, each a relative path inside the skin folder other than skin.toml, unique ignoring case and given in only one of the two",
        "candidate_skin_file_type" => "resources must be .png, .jpg, .jpeg, .apng, .gif or .webp; the preview must be .png, .jpg or .jpeg, and stylesheets are not accepted",
        "candidate_skin_too_large" => "too large: the manifest may be 64 KiB, each image 1 MiB (a larger file in image_files is refused before it is read) and 2048 pixels on each side, all images 2 MiB and 8,000,000 pixels together (each animated frame counts as a whole image), an animation at most 120 frames, and the preview 256 KiB",
        "candidate_skin_image_invalid" => "an image is not valid standard base64, or its bytes (every animation frame) cannot be decoded using the format its extension names: it may be truncated or corrupted, so generate it again with code, write it to a file and pass its path in image_files",
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
    const SHARING_RULES: &str = "a skin must set `preview` to a .png, .jpg or .jpeg image, must not set `toolbar_stylesheet`, and images and image_files together must hold exactly the images the manifest references";
    let Ok(scratch) = tempfile::tempdir() else {
        return SHARING_RULES.into();
    };
    let folder = scratch.path().join(&request.package_id);
    if std::fs::create_dir(&folder).is_err()
        || std::fs::write(folder.join("skin.toml"), &request.manifest).is_err()
    {
        return SHARING_RULES.into();
    }
    for path in request.images.keys().chain(request.image_files.keys()) {
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
            image_files: BTreeMap::new(),
            replace,
        }
    }

    #[test]
    fn every_field_the_output_schema_requires_is_in_the_listing() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
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
            create(state.path(), None, &request("sunset", "Sunset", false)).unwrap(),
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
                None,
                &CreateSkinRequest {
                    package_id: id.into(),
                    manifest,
                    images: BTreeMap::from([("preview.png".into(), PNG.into())]),
                    image_files: BTreeMap::new(),
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
            ("whale", APNG, "motion = 'sway'\nmotion_period_ms = 2400\ncandidate_overlay = true\ncandidate_overlay_index = 0\n"),
            ("still", PNG, ""),
        ] {
            let manifest = format!(
                "schema_version = 2\nid = '{id}'\nname = 'Whale'\nversion = '1.0'\nbase = 'system'\npreview = 'preview.png'\n[supports]\nlayouts = ['horizontal']\nthemes = ['light']\n[candidate_window]\n[candidate_window.decoration]\nimage = 'whale.png'\ntop_inset_dip = 40\nwidth_dip = 60\n{motion}"
            );
            create(
                state.path(),
                None,
                &CreateSkinRequest {
                    package_id: id.into(),
                    manifest,
                    images: BTreeMap::from([
                        ("preview.png".into(), PNG.into()),
                        ("whale.png".into(), image.into()),
                    ]),
                    image_files: BTreeMap::new(),
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
                candidate_overlay: Some(SkinDecorationCandidateView {
                    index: 0,
                    font_size_dip: 16.0,
                    min_font_size_dip: 8.0,
                    max_width_dip: 0.0,
                    color: String::new(),
                    align_h: "center".into(),
                    align_v: "center".into(),
                    offset_x_dip: 0.0,
                    offset_y_dip: 0.0,
                    strip_badge: true,
                }),
            }
        );
        assert_eq!(
            decoration("still"),
            &SkinDecorationView {
                motion: "none".into(),
                motion_period_ms: 3000,
                frames: 1,
                animated: false,
                candidate_overlay: None,
            }
        );
        let json = serde_json::to_value(&listed).unwrap();
        let still = json["skins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|skin| skin["id"] == "still")
            .unwrap();
        assert!(still["decoration"].get("candidate_overlay").is_none());
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
            None,
            &CreateSkinRequest {
                package_id: "panel".into(),
                manifest: v2_manifest("panel", "canvas", CANVAS_TABLES),
                images: BTreeMap::from([
                    ("preview.png".into(), PNG.into()),
                    ("panel.png".into(), PNG.into()),
                    ("leaf.png".into(), PNG.into()),
                ]),
                image_files: BTreeMap::new(),
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
            image_files: BTreeMap::new(),
            replace: false,
        };
        assert_eq!(create(state.path(), None, &crowded).unwrap().id, "crowded");
    }

    #[test]
    fn a_version_2_skin_on_another_base_keeps_its_animation_and_ignores_canvas_keys() {
        let state = tempfile::tempdir().unwrap();
        // msime-windows ignores the canvas-only tables under any other base, so their images are neither required nor accepted; [animation] and decoration motion stay version 2 features of every base.
        create(
            state.path(),
            None,
            &CreateSkinRequest {
                package_id: "lively".into(),
                manifest: v2_manifest(
                    "lively",
                    "system",
                    &format!("{CANVAS_TABLES}[animation]\nenter = 'slide_up'\n"),
                ),
                images: BTreeMap::from([("preview.png".into(), PNG.into())]),
                image_files: BTreeMap::new(),
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
        create(state.path(), None, &scroll).unwrap();
        let listed = list(state.path());
        assert_eq!(listed.skins[0].id, "scroll");
        assert!(!listed.skins[0].canvas);
    }

    #[test]
    fn an_existing_skin_is_replaced_only_when_asked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
        let refused = create(state.path(), None, &request("sunset", "Dusk", false)).unwrap_err();
        assert!(refused.contains("replace: true"), "{refused}");
        assert_eq!(list(state.path()).skins[0].name, "Sunset");
        assert!(
            create(state.path(), None, &request("sunset", "Dusk", true))
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
        assert!(create(state.path(), None, &mismatched)
            .unwrap_err()
            .contains("must equal package_id"));

        let mut unsupported = request("sunset", "Sunset", false);
        unsupported.manifest = unsupported.manifest.replace("'night'", "'custom'");
        assert_eq!(
            create(state.path(), None, &unsupported).unwrap_err(),
            BASE_RULES
        );

        // msime-windows 的规则：canvas 只用于版本 2。
        let mut static_canvas = request("sunset", "Sunset", false);
        static_canvas.manifest = static_canvas.manifest.replace("'night'", "'canvas'");
        assert_eq!(
            create(state.path(), None, &static_canvas).unwrap_err(),
            "skin.toml: base = canvas requires schema_version 2"
        );

        let mut no_preview = request("sunset", "Sunset", false);
        no_preview.manifest = no_preview.manifest.replace("preview = 'preview.png'\n", "");
        assert!(create(state.path(), None, &no_preview)
            .unwrap_err()
            .contains("must set `preview`"));

        let mut reserved = request("night", "Night", false);
        reserved.manifest = manifest("night", "Night");
        assert!(create(state.path(), None, &reserved)
            .unwrap_err()
            .contains("reserved id"));
        for id in ["canvas", "wechat_scroll", "default"] {
            let mut reserved = request(id, "Reserved", false);
            reserved.manifest = manifest(id, "Reserved");
            let refused = create(state.path(), None, &reserved).unwrap_err();
            assert!(refused.contains("reserved id"), "{id}: {refused}");
            assert!(refused.contains("canvas or default"), "{id}: {refused}");
        }

        let mut not_png = request("sunset", "Sunset", false);
        not_png.images = BTreeMap::from([("preview.png".into(), "bm90IGEgcG5n".into())]);
        assert!(create(state.path(), None, &not_png)
            .unwrap_err()
            .contains("cannot be decoded"));

        // 签名和 IHDR 完好、像素数据和 IEND 都缺失：只看签名的检查会把它当成 PNG 装进去。
        let mut truncated = request("sunset", "Sunset", false);
        truncated.images = BTreeMap::from([("preview.png".into(), PNG[..44].into())]);
        assert!(create(state.path(), None, &truncated)
            .unwrap_err()
            .contains("cannot be decoded using the format"));

        assert!(list(state.path()).skins.is_empty());
    }

    /// 与 [`PNG`] 同一张 1×1 PNG 的字节，写成文件给 image_files 用。
    const PNG_BYTES: &[u8] = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0d\x49\x44\x41\x54\x78\x9c\x63\x60\x60\x60\x60\x00\x00\x00\x05\x00\x01\xa5\xf6\x45\x40\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82";

    /// `request` 的预览改由 `file` 给出。
    fn from_file(id: &str, file: &Path) -> CreateSkinRequest {
        let mut request = request(id, "From file", false);
        request.images.clear();
        request.image_files =
            BTreeMap::from([("preview.png".into(), file.to_str().unwrap().into())]);
        request
    }

    #[test]
    fn images_given_as_files_are_installed_as_their_bytes() {
        let state = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let file = files.path().join("preview.png");
        std::fs::write(&file, PNG_BYTES).unwrap();
        assert_eq!(
            create(state.path(), None, &from_file("sunset", &file)).unwrap(),
            CreatedSkin {
                id: "sunset".into(),
                replaced: false
            }
        );
        assert_eq!(
            std::fs::read(state.path().join("skins/sunset/preview.png")).unwrap(),
            PNG_BYTES
        );
        // 文件只被读，不被移动或改写。
        assert_eq!(std::fs::read(&file).unwrap(), PNG_BYTES);
        assert_eq!(list(state.path()).skins[0].name, "From file");
        // 参数里只给 image_files、不给 images 也能解析。
        let request: CreateSkinRequest = serde_json::from_value(serde_json::json!({
            "package_id": "dusk",
            "manifest": manifest("dusk", "Dusk"),
            "image_files": { "preview.png": file.to_str().unwrap() },
        }))
        .unwrap();
        assert!(request.images.is_empty());
        assert_eq!(create(state.path(), None, &request).unwrap().id, "dusk");
    }

    #[test]
    fn an_unreadable_image_file_is_named_with_what_is_wrong() {
        let state = tempfile::tempdir().unwrap();
        let files = tempfile::tempdir().unwrap();
        let refused =
            |file: &Path| create(state.path(), None, &from_file("sunset", file)).unwrap_err();
        let error = refused(Path::new("preview.png"));
        assert!(error.contains("is not an absolute path"), "{error}");
        let error = refused(&files.path().join("missing.png"));
        assert!(error.contains("does not exist"), "{error}");
        let error = refused(files.path());
        assert!(error.contains("is not a regular file"), "{error}");
        #[cfg(unix)]
        {
            let real = files.path().join("real.png");
            std::fs::write(&real, PNG_BYTES).unwrap();
            let link = files.path().join("link.png");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let error = refused(&link);
            assert!(error.contains("is a symbolic link"), "{error}");
            // 符号链接在上一级目录时，便宜的检查看不出来，给出整条规则。
            let linked = files.path().join("linked");
            std::os::unix::fs::symlink(files.path(), &linked).unwrap();
            // root 在只有 root 能写的目录里建的链接，Linux 上算系统链接（`msime_path_trust::is_root_only_link`），以 root 运行的测试容器里跳过这一项。
            use std::os::unix::fs::MetadataExt;
            if std::fs::symlink_metadata(&linked).unwrap().uid() != 0 {
                let error = refused(&linked.join("real.png"));
                assert!(
                    error.contains("no symbolic link anywhere on the way"),
                    "{error}"
                );
            }
        }
        // 同一个路径两边都给。
        let file = files.path().join("preview.png");
        std::fs::write(&file, PNG_BYTES).unwrap();
        let mut both = from_file("sunset", &file);
        both.images = BTreeMap::from([("preview.png".into(), PNG.into())]);
        let error = create(state.path(), None, &both).unwrap_err();
        assert!(error.contains("given in only one of the two"), "{error}");
        // 坏图与 base64 一样完整解码后拒绝，并建议写成文件重来。
        std::fs::write(&file, &PNG_BYTES[..40]).unwrap();
        let error = create(state.path(), None, &from_file("sunset", &file)).unwrap_err();
        assert!(error.contains("pass its path in image_files"), "{error}");
        assert!(list(state.path()).skins.is_empty());
    }

    #[test]
    fn a_skin_is_deleted_unless_it_is_chosen_or_not_the_users() {
        use msime_client_core::preferences::Preferences;
        let state = tempfile::tempdir().unwrap();
        let edition = Edition::full();
        let delete = |id: &str| {
            delete(
                state.path(),
                None,
                edition,
                &SkinRequest {
                    package_id: id.into(),
                },
            )
        };
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
        create(state.path(), None, &request("dusk", "Dusk", false)).unwrap();

        // 偏好里选着它时拒绝，哪怕当前主题不是 custom。
        let store = PreferencesStore::new(state.path());
        let mut preferences = Preferences::default();
        preferences.custom_theme.candidate_skin_dark = Some("sunset".into());
        store.save(0, preferences).unwrap();
        let error = delete("sunset").unwrap_err();
        assert!(error.contains("chosen as candidate_skin_dark"), "{error}");
        assert!(state.path().join("skins/sunset").is_dir());

        for (id, expected) in [
            ("night", "cannot be deleted"),
            ("fluent", "cannot be deleted"),
            ("../skins", "cannot be deleted"),
            ("missing", "no skin missing is installed"),
        ] {
            let error = delete(id).unwrap_err();
            assert!(error.contains(expected), "{id}: {error}");
        }
        std::fs::write(state.path().join("skins/stray"), b"not a skin").unwrap();
        assert!(delete("stray").unwrap_err().contains("is not a folder"));

        let deleted = delete("dusk").unwrap();
        assert_eq!(
            deleted,
            DeletedSkin {
                id: "dusk".into(),
                synced: false
            }
        );
        crate::assert_required_fields_present::<DeletedSkin>(
            &serde_json::to_value(&deleted).unwrap(),
        );
        assert!(!state.path().join("skins/dusk").exists());
        let listed = list(state.path());
        assert_eq!(listed.skins.len(), 1);
        assert_eq!(listed.skins[0].id, "sunset");
    }

    #[test]
    fn deleting_a_synced_skin_says_so_and_keeps_the_sync_state() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
        let sync_state = state.path().join(candidate_sync::STATE_FILE);
        std::fs::write(
            &sync_state,
            serde_json::to_vec(&serde_json::json!({
                "user_id": "user",
                "packages": { "sunset": { "cloud_id": Uuid::new_v4(), "cloud_digest": "a", "local_digest": "b" } }
            }))
            .unwrap(),
        )
        .unwrap();
        let before = std::fs::read(&sync_state).unwrap();
        let deleted = delete(
            state.path(),
            None,
            Edition::full(),
            &SkinRequest {
                package_id: "sunset".into(),
            },
        )
        .unwrap();
        assert!(deleted.synced);
        // 同步状态留着，设置应用下一次同步才知道要把云端那份一起删掉，而不是当成新包下载回来。
        assert_eq!(std::fs::read(&sync_state).unwrap(), before);
        assert!(list(state.path()).skins.is_empty());
    }

    #[test]
    fn an_installed_skin_is_read_back_and_recreated_from_its_own_files() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
        let get = |id: &str| {
            get(
                state.path(),
                &SkinRequest {
                    package_id: id.into(),
                },
            )
        };
        let files = get("sunset").unwrap();
        assert_eq!(files.manifest, manifest("sunset", "Sunset"));
        assert!(!files.synced);
        // 逐段拼接：Windows 上读回的路径用反斜杠分隔。
        let directory = state.path().join("skins").join("sunset");
        let preview = directory.join("preview.png");
        assert_eq!(files.directory, directory.display().to_string());
        assert_eq!(
            files.images,
            [SkinFileView {
                path: "preview.png".into(),
                file: preview.display().to_string(),
                bytes: PNG_BYTES.len() as u64,
                format: Some("png".into()),
                width: Some(1),
                height: Some(1),
                frames: 1,
            }]
        );
        let value = serde_json::to_value(&files).unwrap();
        crate::assert_required_fields_present::<SkinFiles>(&value);
        crate::assert_required_fields_present::<SkinFileView>(&value["images"][0]);

        // 改了清单，图片原样指回已安装的文件，整包替换。
        let mut edited = request("sunset", "Sunset", true);
        edited.manifest = files.manifest.replace("Sunset", "Afterglow");
        edited.images.clear();
        edited.image_files = files
            .images
            .iter()
            .map(|image| (image.path.clone(), image.file.clone()))
            .collect();
        assert!(create(state.path(), None, &edited).unwrap().replaced);
        assert_eq!(list(state.path()).skins[0].name, "Afterglow");
        assert_eq!(std::fs::read(&preview).unwrap(), PNG_BYTES);

        for (id, expected) in [
            ("night", "is not a skin the user installed"),
            ("missing", "no skin missing is installed"),
        ] {
            let error = get(id).unwrap_err();
            assert!(error.contains(expected), "{id}: {error}");
        }
        std::fs::create_dir(state.path().join("skins/broken")).unwrap();
        let error = get("broken").unwrap_err();
        assert!(error.contains("under issues"), "{error}");
    }

    #[test]
    fn a_synced_skin_is_marked() {
        let state = tempfile::tempdir().unwrap();
        create(state.path(), None, &request("sunset", "Sunset", false)).unwrap();
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

    /// Linux 的 IBus 和 Fcitx5 宿主只从运行时选项里的皮肤目录知道装了什么、画什么颜色：每次装、同名替换和删除都要重新发布它，被拒绝的写入不碰文档。
    #[test]
    fn every_skin_write_republishes_the_catalog_the_linux_hosts_read() {
        let state = tempfile::tempdir().unwrap();
        let options = state.path().join("runtime-options.json");
        std::fs::write(
            &options,
            br#"{"api_version":1,"resources":"/resources","preferences":{}}"#,
        )
        .unwrap();
        let document = || -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(&options).unwrap()).unwrap()
        };
        let accent = |id: &str| {
            document()["candidate_skin_catalog"]["packages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|package| package["id"] == id)
                .map(|package| package["candidate"]["dark"]["accent"].clone())
        };

        create(
            state.path(),
            Some(&options),
            &request("sunset", "Sunset", false),
        )
        .unwrap();
        assert_eq!(accent("sunset"), Some("#FF8800".into()));

        // 同名替换成别的配色：id 和选择都没变，宿主要靠目录条目变了才画新包。
        let mut recoloured = request("sunset", "Sunset", true);
        recoloured.manifest = recoloured.manifest.replace("#ff8800", "#00aaff");
        assert!(
            create(state.path(), Some(&options), &recoloured)
                .unwrap()
                .replaced
        );
        assert_eq!(accent("sunset"), Some("#00AAFF".into()));

        // 被拒绝的写入不碰文档。
        let before = std::fs::read(&options).unwrap();
        create(
            state.path(),
            Some(&options),
            &request("sunset", "Dusk", false),
        )
        .unwrap_err();
        assert_eq!(std::fs::read(&options).unwrap(), before);

        // 不给文档（macOS、Windows）时什么都不发布。
        create(state.path(), None, &request("dusk", "Dusk", false)).unwrap();
        assert_eq!(std::fs::read(&options).unwrap(), before);

        delete(
            state.path(),
            Some(&options),
            Edition::full(),
            &SkinRequest {
                package_id: "sunset".into(),
            },
        )
        .unwrap();
        assert_eq!(accent("sunset"), None);
        assert_eq!(accent("dusk"), Some("#FF8800".into()));
        // 文档里别的键原样保留。
        assert_eq!(document()["resources"], "/resources");
    }
}
