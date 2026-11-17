use crate::tests::read;
use crate::*;

fn options(root: &Path, shared: Option<&Path>) -> Value {
    let directory = |name: &str| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let resources = directory("resources");
    let dictionaries = directory("dictionaries");
    rusqlite::Connection::open(resources.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                        CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);",
        )
        .unwrap();
    std::fs::copy(
        resources.join("msime-pinyin.db"),
        dictionaries.join("msime-pinyin.db"),
    )
    .unwrap();
    let mut document = json!({
        "api_version": 1, "resources": resources, "dictionaries": dictionaries,
        "user_data": directory("user"), "cache": directory("cache"),
        "preferences_directory": directory("state"), "preferences": Preferences::default(),
        "keyboard_reranker": false,
    });
    if let Some(shared) = shared {
        document["dictionary_resource_root"] = json!(shared);
    }
    document
}

fn publish(root: &Path, pack: ResourcePack, name: &str) -> PathBuf {
    let directory = resource_packs::root(root).join(pack.id());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(name), b"synthetic resource").unwrap();
    std::fs::write(
        directory.join("msime-model.json"),
        serde_json::to_vec(&pack.manifest()).unwrap(),
    )
    .unwrap();
    directory.join(name)
}

#[test]
fn shared_packs_are_read_without_relocating_private_state() {
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("group");
    let document = options(root.path(), Some(&shared));
    let private = root.path().join("state");
    publish(&private, ResourcePack::Japanese, "msime-japanese.dat");
    publish(&private, ResourcePack::Wubi, "msime-wubi.db");
    let japanese = publish(&shared, ResourcePack::Japanese, "msime-japanese.dat");
    let wubi = publish(&shared, ResourcePack::Wubi, "msime-wubi.db");
    let parsed = HostOptions::from_document(document.clone()).unwrap();
    let expected_user = parsed.user_data.clone();
    let expected_cache = parsed.cache.clone();
    let engine = parsed.into_engine_options();
    assert_eq!(Path::new(&engine.japanese_dictionary), japanese);
    assert_eq!(Path::new(&engine.wubi_dictionary), wubi);
    assert_eq!(engine.user_data, expected_user);
    assert_eq!(engine.cache, expected_cache);
    // 未注入共享路径的旧宿主仍从自己的状态目录取包。
    let mut legacy = document;
    legacy
        .as_object_mut()
        .unwrap()
        .remove("dictionary_resource_root");
    let legacy = HostOptions::from_document(legacy)
        .unwrap()
        .into_engine_options();
    assert_eq!(
        Path::new(&legacy.japanese_dictionary),
        private.join("resource-packs/japanese/msime-japanese.dat")
    );
    assert_eq!(
        Path::new(&legacy.wubi_dictionary),
        private.join("resource-packs/wubi/msime-wubi.db")
    );
}

#[test]
fn missing_or_incomplete_shared_pack_does_not_resurrect_a_private_copy() {
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("group");
    let document = options(root.path(), Some(&shared));
    publish(
        &root.path().join("state"),
        ResourcePack::Japanese,
        "msime-japanese.dat",
    );
    let japanese = publish(&shared, ResourcePack::Japanese, "msime-japanese.dat");
    std::fs::remove_file(japanese.parent().unwrap().join("msime-model.json")).unwrap();
    let engine = HostOptions::from_document(document)
        .unwrap()
        .into_engine_options();
    assert!(engine.japanese_dictionary.is_empty());
    assert!(engine.wubi_dictionary.is_empty());
    assert!(!shared.join("preferences.json").exists());
    assert!(!shared.join("user").exists());
}

#[test]
fn focus_refreshes_shared_install_and_uninstall_without_private_copy() {
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("group");
    let document = options(root.path(), Some(&shared)).to_string();
    let created = read(unsafe { msime_client_create(document.as_ptr(), document.len()) });
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    let observed = || {
        SESSIONS.with(|sessions| {
            sessions.borrow()[&handle]
                .options
                .japanese_dictionary
                .clone()
        })
    };
    assert!(observed().is_empty());
    let japanese = publish(&shared, ResourcePack::Japanese, "msime-japanese.dat");
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    assert_eq!(Path::new(&observed()), japanese);
    assert!(!root.path().join("state/resource-packs/japanese").exists());
    std::fs::remove_dir_all(japanese.parent().unwrap()).unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    assert!(observed().is_empty());
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}

#[cfg(unix)]
#[test]
fn shared_pack_can_be_read_with_no_write_permission_and_rejects_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("group");
    let document = options(root.path(), Some(&shared));
    let japanese = publish(&shared, ResourcePack::Japanese, "msime-japanese.dat");
    let directory = japanese.parent().unwrap();
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o555)).unwrap();
    let engine = HostOptions::from_document(document.clone())
        .unwrap()
        .into_engine_options();
    assert_eq!(Path::new(&engine.japanese_dictionary), japanese);
    assert_eq!(
        std::fs::read_dir(directory).unwrap().count(),
        2,
        "只读查询不创建锁或缓存"
    );
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    let alias = root.path().join("alias");
    symlink(&shared, &alias).unwrap();
    let mut unsafe_document = document;
    unsafe_document["dictionary_resource_root"] = json!(alias);
    assert!(HostOptions::from_document(unsafe_document)
        .unwrap()
        .into_engine_options()
        .japanese_dictionary
        .is_empty());
}
