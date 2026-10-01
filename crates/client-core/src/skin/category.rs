//! 社区皮肤（键盘皮肤与候选窗皮肤）共用的发布分类。

use serde::{Deserialize, Serialize};

/// 每个返回社区皮肤条目的请求都带上这个查询参数，服务端才会在条目里加上 `category`。早于分类功能发布的客户端以 `deny_unknown_fields` 读取条目，所以服务端只对显式请求的客户端返回该字段。
pub(crate) const INCLUDE_CATEGORY: &str = "include=category";

/// 社区皮肤的发布分类，键盘皮肤和候选窗皮肤用同一套 id。分类只是发布元数据，不属于皮肤内容本身，也不计入任何请求摘要。服务端将来新增的分类 id 一律读作 [`SkinCategory::Other`]，这样旧客户端不会因新分类而读取失败。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinCategory {
    Nature,
    Guofeng,
    Acg,
    Cute,
    Food,
    Tech,
    Minimal,
    #[default]
    #[serde(other)]
    Other,
}

impl SkinCategory {
    /// 全部分类，顺序即界面上筛选按钮的顺序。
    pub const ALL: [Self; 8] = [
        Self::Nature,
        Self::Guofeng,
        Self::Acg,
        Self::Cute,
        Self::Food,
        Self::Tech,
        Self::Minimal,
        Self::Other,
    ];

    /// 服务端使用的分类 id。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nature => "nature",
            Self::Guofeng => "guofeng",
            Self::Acg => "acg",
            Self::Cute => "cute",
            Self::Food => "food",
            Self::Tech => "tech",
            Self::Minimal => "minimal",
            Self::Other => "other",
        }
    }
}
