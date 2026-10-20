//! Linux 的 IBus 和 Fcitx5 宿主只从运行时选项文档（`runtime-options.json`）的 `candidate_skin_catalog` 知道装了哪些皮肤、每款画什么颜色和尺寸，它们自己不扫皮肤目录。所以每个改皮肤目录或改皮肤选择的写入方都要把扫描结果发布进这份文档：设置应用在保存偏好和重扫皮肤时，MCP 服务在写皮肤和改偏好时。两边都经这里算出要写的文档，16 KiB 上限、裁剪顺序和「没变就不写」的判断只有这一份。
//!
//! 文件的读写留给调用方：设置应用和 MCP 服务各有自己的私有文件读写原语和错误类型，这里只处理文档本身。

use super::catalog::{host_candidate_catalog, SkinCatalog};
use serde_json::Value;
use std::path::Path;

/// `runtime-options.json` 最多能有多长。IBus 启动器、Fcitx5 插件和 `msime_host_api::refresh_host_options` 都只读 16 KiB，更长的文档 IBus 保留旧配置，Fcitx5 和升级刷新直接失败。
pub const LINUX_RUNTIME_OPTIONS_LIMIT: usize = 16384;

/// 皮肤目录最多能把文档撑到多长：留出 `LINUX_RUNTIME_OPTIONS_LIMIT` 的最后 1 KiB 给升级刷新，它改写资源和词库路径、补一个换行，但不知道皮肤目录的存在。
pub const LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET: usize = LINUX_RUNTIME_OPTIONS_LIMIT - 1024;

/// 文档比宿主读的上限还长，没有写：替换掉一份能用的文件会让两个宿主都停下来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooLarge;

/// 按宿主读取的方式序列化 `document`，超过宿主上限时拒绝。
pub fn runtime_options_bytes(document: &Value) -> Result<Vec<u8>, TooLarge> {
    let bytes = serialize(document);
    if bytes.len() > LINUX_RUNTIME_OPTIONS_LIMIT {
        return Err(TooLarge);
    }
    Ok(bytes)
}

/// 把 `document` 连同从 `root` 扫描到的已安装皮肤一起序列化，皮肤目录写在 `candidate_skin_catalog` 里；放不进 `LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET` 时从末尾逐个丢掉皮肤包，直到放得下。
///
/// 自定义主题选中的皮肤（浅色、深色两个槽位）最后才丢，因为屏幕上画的是它们的颜色。连空目录都放不下时整个键不写，免得宿主原本能读的文档因为皮肤目录读不了；去掉目录后仍超出宿主上限的文档直接拒绝。
pub fn runtime_options_with_skin_catalog(
    document: &mut Value,
    root: &Path,
    catalog: &SkinCatalog,
) -> Result<Vec<u8>, TooLarge> {
    let custom_theme = &document["preferences"]["custom_theme"];
    let selected: Vec<String> = ["candidate_skin", "candidate_skin_dark"]
        .into_iter()
        .filter_map(|slot| custom_theme[slot].as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    let selected: Vec<&str> = selected.iter().map(String::as_str).collect();
    let mut published = host_candidate_catalog(catalog, root, &selected);
    loop {
        document["candidate_skin_catalog"] = published.clone();
        let bytes = serialize(document);
        if bytes.len() <= LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET {
            return Ok(bytes);
        }
        let Some(packages) = published["packages"].as_array_mut() else {
            break;
        };
        if packages.is_empty() {
            break;
        }
        let dropped = packages
            .iter()
            .rposition(|package| {
                !package["id"]
                    .as_str()
                    .is_some_and(|id| selected.contains(&id))
            })
            .unwrap_or(packages.len() - 1);
        packages.remove(dropped);
    }
    if let Some(object) = document.as_object_mut() {
        object.remove("candidate_skin_catalog");
    }
    runtime_options_bytes(document)
}

/// 只换掉 `document` 里的皮肤目录，其余键原样保留。返回要写回的字节；新目录与文档里已有的一样时返回 `None`，调用方不写文件，监视这个文件的宿主也就不会白白重载一次。
///
/// 同名替换一款皮肤（MCP 的 `create_candidate_skin` 带 `replace`、皮肤编辑器、图库重装）时，宿主比较的就是这份条目：它含配色、装饰带尺寸和装饰图路径、卡片圆角，正是非 canvas 包在 Linux 上画的全部内容。条目变了文件就变，IBus 按文档内容、Fcitx5 按主题请求（含所画包的条目）发现它；条目没变说明宿主从它画的东西都没变，装饰图本身和 canvas 包的文件由 Fcitx5 按文件戳另行发现。
pub fn republish_skin_catalog(
    document: &mut Value,
    root: &Path,
    catalog: &SkinCatalog,
) -> Result<Option<Vec<u8>>, TooLarge> {
    let unchanged = document.get("candidate_skin_catalog").cloned();
    let bytes = runtime_options_with_skin_catalog(document, root, catalog)?;
    Ok((document.get("candidate_skin_catalog") != unchanged.as_ref()).then_some(bytes))
}

fn serialize(document: &Value) -> Vec<u8> {
    // `Value` 的键都是字符串、数字都是有限值，序列化不会失败。
    serde_json::to_vec_pretty(document).expect("a JSON value serializes")
}

#[cfg(test)]
mod tests;
