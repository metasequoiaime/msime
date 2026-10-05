//! The explicit, bounded local library shared with the Android IME process.

use crate::community::resource::{validate_resource, CommunityResource, CommunityResourceKind};
use crate::file_lock;
use serde_json::from_slice;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const MAXIMUM_BYTES: u64 = 4_000_000;
const MAXIMUM_ITEMS: usize = 50;

fn is_valid_reply(item: &CommunityResource) -> bool {
    item.kind == CommunityResourceKind::Reply && validate_resource(item).is_ok()
}

#[derive(Debug, Error)]
pub enum CommunityResourceLibraryError {
    #[error("community resource library storage failed")]
    Io(#[from] std::io::Error),
    #[error("community resource library is malformed")]
    Json(#[from] serde_json::Error),
    #[error("community resource library is invalid")]
    Invalid,
}

#[derive(Clone, Debug)]
pub struct CommunityResourceLibraryStore {
    file: PathBuf,
}

impl CommunityResourceLibraryStore {
    pub fn new(file: impl AsRef<Path>) -> Self {
        Self {
            file: file.as_ref().to_path_buf(),
        }
    }

    pub fn load(&self) -> Result<Vec<CommunityResource>, CommunityResourceLibraryError> {
        let _lock = self.lock()?;
        self.read_locked()
    }

    pub fn save_reply(&self, item: CommunityResource) -> Result<(), CommunityResourceLibraryError> {
        if !is_valid_reply(&item) {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let _lock = self.lock()?;
        let mut items = self.read_locked()?;
        if let Some(existing) = items.iter_mut().find(|value| value.id == item.id) {
            *existing = item;
        } else {
            if items.len() >= MAXIMUM_ITEMS {
                return Err(CommunityResourceLibraryError::Invalid);
            }
            items.push(item);
        }
        self.write_locked(&items)
    }

    pub fn remove(&self, id: Uuid) -> Result<(), CommunityResourceLibraryError> {
        let _lock = self.lock()?;
        let mut items = self.read_locked()?;
        items.retain(|item| item.id != id);
        self.write_locked(&items)
    }

    fn lock(&self) -> Result<File, CommunityResourceLibraryError> {
        let Some(parent) = self.file.parent() else {
            return Err(CommunityResourceLibraryError::Invalid);
        };
        if !crate::storage::create_directory_and_check(parent)? {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let lock_path = self.file.with_extension("json.lock");
        let lock = file_lock::open_lock_file(lock_path)?;
        file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn read_locked(&self) -> Result<Vec<CommunityResource>, CommunityResourceLibraryError> {
        let metadata = match fs::symlink_metadata(&self.file) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() || metadata.len() > MAXIMUM_BYTES {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let bytes =
            crate::bounded_io::read_bounded_file(File::open(&self.file)?, MAXIMUM_BYTES, || {
                CommunityResourceLibraryError::Invalid
            })?;
        let items: Vec<CommunityResource> = from_slice(&bytes)?;
        if items.len() > MAXIMUM_ITEMS || items.iter().any(|item| !is_valid_reply(item)) {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let mut ids = std::collections::BTreeSet::new();
        if items.iter().any(|item| !ids.insert(item.id)) {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        Ok(items)
    }

    fn write_locked(
        &self,
        items: &[CommunityResource],
    ) -> Result<(), CommunityResourceLibraryError> {
        let Some(parent) = self.file.parent() else {
            return Err(CommunityResourceLibraryError::Invalid);
        };
        if !crate::storage::create_directory_and_check(parent)? {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let bytes = serde_json::to_vec_pretty(items)?;
        if bytes.len() as u64 > MAXIMUM_BYTES {
            return Err(CommunityResourceLibraryError::Invalid);
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.file).map_err(|error| error.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::resource::{CommunityResourceContent, SharedWord};

    fn reply() -> CommunityResource {
        CommunityResource {
            id: Uuid::parse_str("10000000-0000-4000-8000-000000000001").unwrap(),
            kind: CommunityResourceKind::Reply,
            name: "礼貌回复".into(),
            description: "公开说明".into(),
            author: "示例作者".into(),
            content: CommunityResourceContent {
                entries: Vec::new(),
                prompt: Some("请礼貌回复。".into()),
            },
            revision: 1,
            saves: 0,
            saved: true,
            owned: false,
            rating_count: 0,
            rating_average: 0.0,
            my_rating: 0,
            moderation: None,
        }
    }

    #[test]
    fn stores_only_reply_resources_and_replaces_by_publication_id() {
        let root = tempfile::tempdir().unwrap();
        let store =
            CommunityResourceLibraryStore::new(root.path().join("files/CommunityLibrary.json"));
        store.save_reply(reply()).unwrap();
        let mut updated = reply();
        updated.revision = 2;
        updated.content.prompt = Some("请更简洁地回复。".into());
        store.save_reply(updated).unwrap();
        let values = store.load().unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].revision, 2);
        assert!(store
            .save_reply(CommunityResource {
                content: CommunityResourceContent {
                    entries: vec![SharedWord {
                        kind: crate::cloud::dictionary::DictionaryKind::Quick,
                        code: "x".into(),
                        word: "y".into(),
                        weight: 1
                    }],
                    prompt: None,
                },
                ..reply()
            })
            .is_err());

        let corrupt = CommunityResource {
            id: Uuid::nil(),
            ..reply()
        };
        std::fs::write(
            root.path().join("files/CommunityLibrary.json"),
            serde_json::to_vec(&[corrupt]).unwrap(),
        )
        .unwrap();
        assert!(store.load().is_err(), "nil publication IDs are not usable");
    }

    #[test]
    fn rejects_reply_metadata_and_prompt_outside_community_contract() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("files/CommunityLibrary.json");
        let store = CommunityResourceLibraryStore::new(&path);
        let mut cases = Vec::new();

        let mut long_prompt = reply();
        long_prompt.content.prompt = Some("字".repeat(2_001));
        cases.push(long_prompt);

        let mut blank_name = reply();
        blank_name.name = " ".into();
        cases.push(blank_name);

        let mut zero_revision = reply();
        zero_revision.revision = 0;
        cases.push(zero_revision);

        let mut invalid_rating = reply();
        invalid_rating.rating_average = 5.0;
        cases.push(invalid_rating);

        for item in cases {
            assert!(matches!(
                store.save_reply(item.clone()),
                Err(CommunityResourceLibraryError::Invalid)
            ));
            assert!(!path.exists());

            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, serde_json::to_vec(&[item]).unwrap()).unwrap();
            assert!(matches!(
                store.load(),
                Err(CommunityResourceLibraryError::Invalid)
            ));
            std::fs::remove_file(&path).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_ancestor_before_creating_library_storage() {
        use crate::storage::untrusted_symlink as symlink;

        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked = parent.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let file = linked.join("missing").join("CommunityLibrary.json");
        let store = CommunityResourceLibraryStore::new(&file);

        assert!(matches!(
            store.save_reply(reply()),
            Err(CommunityResourceLibraryError::Io(_))
        ));
        assert!(!outside.path().join("missing").exists());
    }
}
