use super::{ClipboardHistoryEntry, ClipboardHistoryStore};
use std::io::{self, ErrorKind};
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_CATEGORIES: usize = 5;
pub const MAX_CATEGORY_NAME_CHARACTERS: usize = 12;
const MAX_CATALOG_BYTES: u64 = 8192;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClipboardCategory {
    pub id: String,
    pub name: String,
}

fn invalid() -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, "invalid clipboard category")
}

fn normalized_name(name: &str) -> io::Result<String> {
    let name = name.trim();
    if name.is_empty()
        || ["本机", "云端", "全部记录", "未分类"].contains(&name)
        || name.len() > 1024
        || name.chars().any(char::is_control)
        || name.graphemes(true).count() > MAX_CATEGORY_NAME_CHARACTERS
    {
        return Err(invalid());
    }
    Ok(name.to_owned())
}

pub(super) fn retain_category(entry: &mut ClipboardHistoryEntry, categories: &[ClipboardCategory]) {
    if entry
        .category_id
        .as_ref()
        .is_some_and(|id| !categories.iter().any(|category| &category.id == id))
    {
        entry.category_id = None;
    }
}

impl ClipboardHistoryStore {
    fn categories_path(&self) -> std::path::PathBuf {
        let mut path = self.path.as_os_str().to_owned();
        path.push(".categories.json");
        path.into()
    }

    /// 分类目录与正文共用写锁；目录缺失表示尚未设置分类，坏文件不能覆盖。
    pub fn categories(&self) -> io::Result<Vec<ClipboardCategory>> {
        let path = self.categories_path();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            crate::storage::reject_symlink(parent)?;
        }
        crate::storage::reject_symlink(&path)?;
        let file = match crate::storage::open_private_file_in(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_CATALOG_BYTES, || {
            io::Error::new(
                ErrorKind::InvalidData,
                "clipboard categories exceed size limit",
            )
        })?;
        let categories: Vec<ClipboardCategory> = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidData,
                "invalid clipboard categories document",
            )
        })?;
        if categories.len() > MAX_CATEGORIES
            || categories.iter().enumerate().any(|(index, category)| {
                uuid::Uuid::parse_str(&category.id).is_err()
                    || normalized_name(&category.name).ok().as_ref() != Some(&category.name)
                    || categories[..index].iter().any(|previous| {
                        previous.id == category.id || previous.name == category.name
                    })
            })
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "invalid clipboard categories document",
            ));
        }
        Ok(categories)
    }

    fn persist_categories(&self, categories: &[ClipboardCategory]) -> io::Result<()> {
        let path = self.categories_path();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let bytes = serde_json::to_vec(categories).expect("clipboard categories are serializable");
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(parent)?;
            crate::storage::write_private_file_at(
                &directory,
                path.file_name().ok_or_else(invalid)?,
                &bytes,
            )
        }
        #[cfg(not(unix))]
        {
            use std::io::Write;
            let mut temp = tempfile::NamedTempFile::new_in(parent)?;
            temp.write_all(&bytes)?;
            temp.as_file().sync_all()?;
            temp.persist(&path).map_err(|error| error.error)?;
            Ok(())
        }
    }

    pub fn add_category(&self, name: &str) -> io::Result<ClipboardCategory> {
        let name = normalized_name(name)?;
        let _lock = self.lock_writer()?;
        let mut categories = self.categories()?;
        if categories.len() >= MAX_CATEGORIES || categories.iter().any(|c| c.name == name) {
            return Err(invalid());
        }
        let category = ClipboardCategory {
            id: uuid::Uuid::new_v4().to_string(),
            name,
        };
        categories.push(category.clone());
        self.persist_categories(&categories)?;
        Ok(category)
    }

    pub fn rename_category(&self, id: &str, name: &str) -> io::Result<bool> {
        let name = normalized_name(name)?;
        let _lock = self.lock_writer()?;
        let mut categories = self.categories()?;
        if categories.iter().any(|c| c.id != id && c.name == name) {
            return Err(invalid());
        }
        let Some(category) = categories.iter_mut().find(|c| c.id == id) else {
            return Ok(false);
        };
        category.name = name;
        self.persist_categories(&categories)?;
        Ok(true)
    }

    /// 只移除目录；失效 ID 在读取或下次写正文时归未分类，避免跨文件事务和正文丢失。
    pub fn remove_category(&self, id: &str) -> io::Result<bool> {
        let _lock = self.lock_writer()?;
        let mut categories = self.categories()?;
        let count = categories.len();
        categories.retain(|category| category.id != id);
        if count == categories.len() {
            return Ok(false);
        }
        self.persist_categories(&categories)?;
        Ok(true)
    }

    /// 按原文移动条目，不信任宿主可能已过时的索引或分类目录。
    pub fn set_category(&mut self, text: &str, id: Option<&str>) -> io::Result<bool> {
        let _lock = self.lock_writer()?;
        let mut latest = Self::open_with_limit(&self.path, self.limit);
        latest.load()?;
        let categories = latest.categories()?;
        if id.is_some_and(|id| !categories.iter().any(|category| category.id == id)) {
            return Err(invalid());
        }
        let Some(entry) = latest.entries.iter_mut().find(|entry| entry.text == text) else {
            self.entries = latest.entries;
            return Ok(false);
        };
        entry.category_id = id.map(str::to_owned);
        self.persist(&latest.entries)?;
        self.entries = latest.entries;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store(path: &std::path::Path) -> ClipboardHistoryStore {
        ClipboardHistoryStore::open(path.join("history.json"))
    }

    #[test]
    fn names_are_bounded_unique_and_unicode_aware() {
        let directory = tempfile::tempdir().unwrap();
        let history = store(directory.path());
        for name in [
            "",
            "  ",
            "bad\nname",
            "十三个汉字十三个汉字十三个汉字",
            &"a".repeat(1025),
        ] {
            assert!(history.add_category(name).is_err());
        }
        let category = history.add_category("  合成分类  ").unwrap();
        assert_eq!(category.name, "合成分类");
        assert!(history.add_category("合成分类").is_err());
        history.add_category(&"👩‍💻".repeat(12)).unwrap();
        assert!(history.add_category(&"👩‍💻".repeat(13)).is_err());
        for index in 0..3 {
            history.add_category(&format!("分类{index}")).unwrap();
        }
        assert!(history.add_category("第六个").is_err());
        assert_eq!(history.categories().unwrap().len(), 5);
    }

    #[test]
    fn migration_capture_edit_pin_and_undo_preserve_membership() {
        let directory = tempfile::tempdir().unwrap();
        let mut history = store(directory.path());
        std::fs::write(&history.path, br#"["synthetic legacy"]"#).unwrap();
        history.load().unwrap();
        assert_eq!(history.entries()[0].category_id, None);
        let category = history.add_category("合成分类").unwrap();
        history
            .set_category("synthetic legacy", Some(&category.id))
            .unwrap();
        history.push_mobile("synthetic legacy".into()).unwrap();
        history.set_pinned("synthetic legacy", true).unwrap();
        history.push_mobile("synthetic duplicate".into()).unwrap();
        history
            .replace("synthetic legacy", "synthetic duplicate".into())
            .unwrap();
        assert_eq!(history.entries().len(), 1);
        assert_eq!(
            history.entries()[0].category_id.as_deref(),
            Some(category.id.as_str())
        );
        let removed = history
            .remove_many(&["synthetic duplicate".into()])
            .unwrap();
        history.restore_many(&removed).unwrap();
        assert_eq!(history.entries(), removed);
        assert!(history.rename_category(&category.id, "改名").unwrap());
        history.load().unwrap();
        assert_eq!(
            history.entries()[0].category_id.as_deref(),
            Some(category.id.as_str())
        );
        history.remove_category(&category.id).unwrap();
        history.load().unwrap();
        assert!(history.entries()[0].category_id.is_none());
        assert_eq!(history.entries()[0].text, "synthetic duplicate");
        assert!(history.entries()[0].pinned);
        let next = history.add_category("改名").unwrap();
        assert_ne!(next.id, category.id);
        history
            .remove_many(&["synthetic duplicate".into()])
            .unwrap();
        history.restore_many(&removed).unwrap();
        assert!(history.entries()[0].category_id.is_none());
        history.clear().unwrap();
        assert_eq!(history.categories().unwrap(), vec![next]);
    }

    #[test]
    fn stale_writers_reload_membership_and_do_not_revive_deleted_categories() {
        let directory = tempfile::tempdir().unwrap();
        let mut first = store(directory.path());
        let mut second = store(directory.path());
        first.push_mobile("synthetic a".into()).unwrap();
        second.load().unwrap();
        let category = first.add_category("合成").unwrap();
        first
            .set_category("synthetic a", Some(&category.id))
            .unwrap();
        second.push_mobile("synthetic b".into()).unwrap();
        assert!(second
            .entries()
            .iter()
            .any(|e| e.category_id.as_deref() == Some(&category.id)));
        first.remove_category(&category.id).unwrap();
        second.set_pinned("synthetic a", true).unwrap();
        assert!(second.entries().iter().all(|e| e.category_id.is_none()));
        assert!(second
            .set_category("synthetic a", Some(&category.id))
            .is_err());
        assert!(!second.set_category("absent", None).unwrap());
        assert_eq!(second.entries().len(), 2);
    }

    #[test]
    fn concurrent_category_creators_enforce_global_capacity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_owned();
        let workers: Vec<_> = (0..12)
            .map(|index| {
                let path = path.clone();
                std::thread::spawn(move || {
                    store(&path).add_category(&format!("合成{index}")).is_ok()
                })
            })
            .collect();
        let accepted = workers
            .into_iter()
            .filter_map(|worker| worker.join().unwrap().then_some(()))
            .count();
        assert_eq!(accepted, MAX_CATEGORIES);
        assert_eq!(store(&path).categories().unwrap().len(), MAX_CATEGORIES);
    }

    #[test]
    fn malformed_catalog_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let history = store(directory.path());
        std::fs::write(history.categories_path(), b"not json").unwrap();
        assert!(history.add_category("合成").is_err());
        assert_eq!(
            std::fs::read(history.categories_path()).unwrap(),
            b"not json"
        );
    }

    #[cfg(unix)]
    #[test]
    fn catalog_symlink_is_rejected_and_files_are_private() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let history = store(directory.path());
        let category = history.add_category("合成").unwrap();
        assert_eq!(
            std::fs::metadata(history.categories_path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let outside = directory.path().join("outside.json");
        std::fs::write(&outside, b"[]").unwrap();
        std::fs::remove_file(history.categories_path()).unwrap();
        symlink(&outside, history.categories_path()).unwrap();
        assert!(history.remove_category(&category.id).is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"[]");
    }
}
