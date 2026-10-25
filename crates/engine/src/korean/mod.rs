//! 韩语两套式与可选罗马字布局共用字母组合规则，稳定音节逐步上屏。
//! 空格、回车、标点和光标键结束组字；只有主动打开的单音节汉字列表提供候选，不混入云候选或学习。

pub mod dubeolsik;
pub mod hanja;
pub mod romaja;
pub mod scheme;

pub use scheme::KoreanScheme;

/// 韩语方案内的键位选择；宿主选项的编码保持 0 为现有两套式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KoreanLayout {
    #[default]
    Dubeolsik,
    Romaja,
}

impl KoreanLayout {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Dubeolsik),
            1 => Some(Self::Romaja),
            _ => None,
        }
    }
}
