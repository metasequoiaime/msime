//! 候选行字段用的写时复制字符串。
//!
//! 九键一次刷新要把同一批词库行在行缓存、候选页、去重表之间来回复制，采样里约三到四成的时间花在 `String` 的分配、拷贝和释放上。`SharedString` 内部是 `Arc<String>`：克隆只加一次引用计数；读取经 `Deref` 得到 `String`，原来的 `as_str`、`&str` 传参和比较照常可用；改写经 `DerefMut` 走 `Arc::make_mut`，只有别处也持有同一份时才先复制，所以每处 `push_str`、`clear` 的结果与 `String` 完全相同。`Debug` 与 `String` 输出一致，golden 记录的字段格式不变。

use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// 没有缓冲区的空串记为 `None`，`Default`、`mem::take` 与 `String::default()` 一样不分配；行结构里大多数可选字段（`corrected_from`、未知的 `canonical_pinyin`）都是空的。
#[derive(Clone, Default)]
pub struct SharedString(Option<Arc<String>>);

static EMPTY: String = String::new();

impl SharedString {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取出内容：唯一持有时不复制。
    pub fn into_string(self) -> String {
        match self.0 {
            None => String::new(),
            Some(shared) => Arc::try_unwrap(shared).unwrap_or_else(|shared| (*shared).clone()),
        }
    }

    /// 遮住 `String::clear`：唯一持有时原地清空、留着缓冲区给接下来的 `push_str`；空串或与别处共享时直接放手，不像经 `DerefMut` 那样先建一个空串或把整串复制一份再清掉。
    pub fn clear(&mut self) {
        match self.0.as_mut().and_then(Arc::get_mut) {
            Some(owned) => owned.clear(),
            None => self.0 = None,
        }
    }

    fn shares_storage_with(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Deref for SharedString {
    type Target = String;

    fn deref(&self) -> &String {
        self.0.as_deref().unwrap_or(&EMPTY)
    }
}

impl DerefMut for SharedString {
    fn deref_mut(&mut self) -> &mut String {
        Arc::make_mut(self.0.get_or_insert_with(Default::default))
    }
}

impl fmt::Debug for SharedString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for SharedString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), formatter)
    }
}

impl PartialEq for SharedString {
    fn eq(&self, other: &Self) -> bool {
        self.shares_storage_with(other) || self.as_str() == other.as_str()
    }
}

impl Eq for SharedString {}

impl PartialOrd for SharedString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SharedString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for SharedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq<str> for SharedString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SharedString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for SharedString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<SharedString> for str {
    fn eq(&self, other: &SharedString) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<SharedString> for &str {
    fn eq(&self, other: &SharedString) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<SharedString> for String {
    fn eq(&self, other: &SharedString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for SharedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for SharedString {
    /// 带容量的空串照样包进去：调用方可能正把复用的缓冲区交回来。
    fn from(value: String) -> Self {
        if value.capacity() == 0 {
            Self(None)
        } else {
            Self(Some(Arc::new(value)))
        }
    }
}

impl From<&str> for SharedString {
    fn from(value: &str) -> Self {
        Self::from(value.to_owned())
    }
}

impl From<&String> for SharedString {
    fn from(value: &String) -> Self {
        Self::from(value.clone())
    }
}

impl From<SharedString> for String {
    fn from(value: SharedString) -> Self {
        value.into_string()
    }
}

impl serde::Serialize for SharedString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SharedString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

impl rusqlite::ToSql for SharedString {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.as_str().to_sql()
    }
}

impl PartialEq<&String> for SharedString {
    fn eq(&self, other: &&String) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_until_one_side_writes() {
        let mut first = SharedString::from("ni'hao");
        let second = first.clone();
        assert!(first.shares_storage_with(&second));
        first.push_str("'ma");
        assert_eq!(first, "ni'hao'ma");
        assert_eq!(second, "ni'hao");
        assert!(!first.shares_storage_with(&second));
    }

    #[test]
    fn clearing_keeps_owned_storage_and_releases_shared_storage() {
        let mut owned = SharedString::from("ni'hao");
        let buffer = owned.as_ptr();
        owned.clear();
        assert_eq!(owned, "");
        assert_eq!(owned.as_ptr(), buffer);
        let mut shared = SharedString::from("ni'hao");
        let other = shared.clone();
        shared.clear();
        assert!(shared.0.is_none());
        assert_eq!(other, "ni'hao");
        let mut empty = SharedString::default();
        empty.clear();
        assert!(empty.0.is_none());
    }

    #[test]
    fn empty_strings_hold_no_storage_until_written() {
        let mut empty = SharedString::default();
        assert!(empty.0.is_none());
        assert_eq!(empty, "");
        assert!(SharedString::from(String::new()).0.is_none());
        assert!(SharedString::from("").0.is_none());
        assert!(SharedString::from(String::with_capacity(8)).0.is_some());
        let mut written = SharedString::from("ni");
        let taken = std::mem::take(&mut written);
        assert!(written.0.is_none());
        assert_eq!(taken, "ni");
        empty.push_str("hao");
        assert_eq!(empty.into_string(), "hao");
        assert_eq!(SharedString::default().into_string(), "");
    }

    #[test]
    fn debug_and_comparisons_match_string() {
        let shared = SharedString::from("你好\"");
        let plain = String::from("你好\"");
        assert_eq!(format!("{shared:?}"), format!("{plain:?}"));
        assert_eq!(shared, plain);
        assert_eq!(plain, shared);
        assert_eq!(shared, "你好\"");
        assert_eq!(shared.as_str(), "你好\"");
        assert_eq!(shared.into_string(), plain);
    }

    #[test]
    fn hashing_matches_the_borrowed_str_for_map_lookups() {
        let mut set = std::collections::HashSet::new();
        set.insert(SharedString::from("ni"));
        assert!(set.contains("ni"));
    }
}
