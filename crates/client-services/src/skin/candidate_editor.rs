//! 设置页「制作皮肤」把编辑好的候选窗皮肤装进皮肤目录。
//!
//! 页面只交出 `skin.toml` 的全文和它引用的图片，安装走 [`candidate_community::install`]，与社区下载和 MCP 的 `create_candidate_skin` 是同一条校验与原子替换路径，所以编辑器存下的包和其他来源的包遵守完全相同的规则。[`check`] 不写皮肤目录，只在临时目录里按目录扫描的规则读一遍清单，供页面从一个基底外观（例如 `base = "wechat"`）取得补齐后的配色，也给安装失败时说明清单哪里不对。

use super::candidate_community::{self, CandidateSkinPackage, MAX_MANIFEST_BYTES};
use super::catalog::{self, SkinSummary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use uuid::Uuid;

const ID: &str = "candidate_skin_id";
const PACKAGE: &str = "candidate_skin_package";
const TOO_LARGE: &str = "candidate_skin_too_large";
const STORAGE: &str = "storage";

/// 编辑器交出的一款皮肤：文件夹名（也是清单的 `id`）、`skin.toml` 全文，以及按包内相对路径列出的图片（标准 base64）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinDraft {
    pub package_id: String,
    pub manifest: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// 页面能说明的一种失败：`code` 是 `candidate_skin_*` 或 `storage`，`detail` 是清单没通过目录扫描时扫描给出的原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidateSkinFailure {
    pub code: &'static str,
    pub detail: Option<String>,
}

impl CandidateSkinFailure {
    pub fn code(code: &'static str) -> Self {
        Self { code, detail: None }
    }
}

/// 装好的皮肤：文件夹名，以及是否整体替换了同名的旧皮肤。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreatedCandidateSkin {
    pub id: String,
    pub replaced: bool,
}

fn check_id(draft: &CandidateSkinDraft) -> Result<(), CandidateSkinFailure> {
    if catalog::is_external_id(&draft.package_id) {
        Ok(())
    } else {
        Err(CandidateSkinFailure::code(ID))
    }
}

/// 不碰皮肤目录，按目录扫描的规则读一遍这款皮肤，返回扫描会列出的那一项（基底外观补齐的颜色、圆角都在里面）。
///
/// 图片可以为空：页面用只写了 `base` 的清单取一个外观的配色时不带图片。带图片时按社区包的规则逐张解码，超限或损坏照样以对应的 `candidate_skin_*` 拒绝。
pub fn check(draft: &CandidateSkinDraft) -> Result<SkinSummary, CandidateSkinFailure> {
    check_id(draft)?;
    if draft.manifest.len() > MAX_MANIFEST_BYTES {
        return Err(CandidateSkinFailure::code(TOO_LARGE));
    }
    let files = if draft.files.is_empty() {
        Vec::new()
    } else {
        candidate_community::decode_files(&draft.files).map_err(CandidateSkinFailure::code)?
    };
    let scratch = tempfile::tempdir().map_err(|_| CandidateSkinFailure::code(STORAGE))?;
    let folder = scratch.path().join(&draft.package_id);
    std::fs::create_dir(&folder).map_err(|_| CandidateSkinFailure::code(STORAGE))?;
    std::fs::write(folder.join("skin.toml"), draft.manifest.as_bytes())
        .map_err(|_| CandidateSkinFailure::code(STORAGE))?;
    for (path, bytes) in files {
        // `decode_files` 已经按 `safe_resource` 检查过每个路径，这里建出的目录都在包内。
        let destination = folder.join(path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CandidateSkinFailure::code(STORAGE))?;
        }
        std::fs::write(destination, bytes).map_err(|_| CandidateSkinFailure::code(STORAGE))?;
    }
    catalog::load_package(scratch.path(), &draft.package_id).map_err(|reason| {
        CandidateSkinFailure {
            code: PACKAGE,
            detail: Some(reason),
        }
    })
}

/// 把这款皮肤装进 `root`。没有 `replace` 时同名皮肤保持原样，返回 `candidate_skin_exists`，页面据此请用户确认覆盖。
///
/// 清单被拒时（`candidate_skin_package`）再用 [`check`] 读一遍，把扫描给出的原因放进 `detail`；`detail` 为空说明清单本身能读，是分享规则没通过：缺少 PNG/JPEG 预览图、图片与清单引用的不一致，或写了样式表。
pub fn create(
    root: &Path,
    draft: &CandidateSkinDraft,
    replace: bool,
) -> Result<CreatedCandidateSkin, CandidateSkinFailure> {
    check_id(draft)?;
    let existed = std::fs::symlink_metadata(root.join(&draft.package_id)).is_ok();
    let package = CandidateSkinPackage {
        // install 不读它；社区下载时它是那次发布的 id。
        id: Uuid::nil(),
        package_id: draft.package_id.clone(),
        manifest: draft.manifest.clone(),
        files: draft.files.clone(),
    };
    match candidate_community::install(root, &package, replace) {
        Ok(id) => Ok(CreatedCandidateSkin {
            id,
            replaced: replace && existed,
        }),
        Err(PACKAGE) => Err(CandidateSkinFailure {
            code: PACKAGE,
            detail: check(draft).err().and_then(|failure| failure.detail),
        }),
        Err(code) => Err(CandidateSkinFailure::code(code)),
    }
}

#[cfg(test)]
mod tests;
