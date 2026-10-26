//! 脚本插件的持久存储（能力 `persist_data`）在磁盘上的一侧：每个插件一个文件 `<plugins root>/script-data/<id>.bin`，最多 `limits::MAX_STORAGE_BYTES` 字节，0600 权限，整份原子替换。只在桌面编译。
//!
//! - **谁写**：输入法进程把存储缓冲（`msime_script_runtime::Storage`）在失焦、会话结束和后台定时刷新时交给后台线程，由那里调 [`write`]。按键路径上不碰文件。
//! - **写之前在锁里核对**：持有插件目录的写锁（`import::lock_plugin_root`，跨进程，和授权、导入、删包同一把），授权文件里这个插件还授予着 `persist_data`、设置文件里的存储代次还是缓冲读进来时的那个，才写；否则拒绝（[`Written::Refused`]），由调用方把缓冲清掉。清除数据、卸载、撤销 `persist_data` 都在同一把锁里删文件、把代次加 1（[`clear_locked`]），所以输入法进程不会把已经清掉的数据写回去。
//! - **读**不加锁：写入方整份原子替换。文件不在、读不出、是链接或超过上限时按空的处理：截断会交给插件一段它没写过的字节。
//! - 跨进程（IBus 和 Fcitx5 同时在跑）以最后写盘的为准。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use msime_script_runtime::{limits, Capability};

use super::import::{lock_plugin_root, RootWrites};
use super::script_grants::{self, ScriptGrantError};
use super::script_settings;
use crate::skin::catalog::safe_id;

/// 存放各插件存储文件的子目录，在插件目录下。
pub const DATA_DIRECTORY: &str = "script-data";

/// 插件 `id` 的存储文件。
pub fn path(root: &Path, id: &str) -> PathBuf {
    root.join(DATA_DIRECTORY).join(format!("{id}.bin"))
}

/// 存储现在的内容。文件不在、读不出、是链接或超过上限时为空。
pub fn read(root: &Path, id: &str) -> Vec<u8> {
    if !safe_id(id) {
        return Vec::new();
    }
    let path = path(root, id);
    if crate::storage::reject_symlink(&path).is_err()
        || !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file())
    {
        return Vec::new();
    }
    crate::storage::open_private_file_in(&path)
        .and_then(|file| {
            crate::bounded_io::read_bounded_file(file, limits::MAX_STORAGE_BYTES as u64, || {
                io::Error::other("script storage is too large")
            })
        })
        .unwrap_or_default()
}

/// 存储文件占的字节数，给设置页显示「已用」。没有文件、是链接或超过上限（输入法会当作空的）时为 0。
pub fn used_bytes(root: &Path, id: &str) -> u64 {
    if !safe_id(id) {
        return 0;
    }
    let path = path(root, id);
    if crate::storage::reject_symlink(&path).is_err() {
        return 0;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata)
            if metadata.is_file() && metadata.len() <= limits::MAX_STORAGE_BYTES as u64 =>
        {
            metadata.len()
        }
        _ => 0,
    }
}

/// [`write`] 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// 写好了（内容为空时是删掉了文件）。
    Saved,
    /// 没写：这个插件已经不再授予 `persist_data`、存储被清过（代次变了）、插件目录不在，或者内容超过上限。调用方应当丢掉手上的缓冲。
    Refused,
}

/// 把插件 `id` 的存储整份换成 `bytes`。`epoch` 是缓冲从磁盘读进来时设置文件里的存储代次。核对和写入都在插件目录的写锁里，见模块说明。内容为空时删掉文件。
pub fn write(root: &Path, id: &str, epoch: u64, bytes: &[u8]) -> io::Result<Written> {
    if !safe_id(id) || bytes.len() > limits::MAX_STORAGE_BYTES {
        return Ok(Written::Refused);
    }
    crate::storage::reject_symlink(root)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Ok(Written::Refused);
    }
    let _writes = lock_plugin_root(root)?;
    let granted = script_grants::read(root)
        .get(id)
        .is_some_and(|grant| grant.granted.contains(Capability::PersistData));
    if !granted || script_settings::read(root).storage_epoch(id) != epoch {
        return Ok(Written::Refused);
    }
    let path = path(root, id);
    if bytes.is_empty() {
        remove(&path)?;
        return Ok(Written::Saved);
    }
    let directory = root.join(DATA_DIRECTORY);
    if !crate::storage::create_directory_and_check(&directory)? {
        return Err(io::Error::other(
            "script storage directory is not a directory",
        ));
    }
    crate::storage::replace_private_file(&path, bytes)?;
    Ok(Written::Saved)
}

/// 设置页的「清除数据」：删掉插件 `id` 的存储文件，存储代次加 1，返回新的代次。插件目录不在时什么都不做。
pub fn clear(root: &Path, id: &str) -> Result<u64, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    crate::storage::reject_symlink(root).map_err(|_| ScriptGrantError::Storage)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Ok(0);
    }
    let writes = lock_plugin_root(root)?;
    Ok(clear_locked(root, &writes, id, true)?)
}

/// 删掉存储文件并把存储代次加 1，持有插件目录的写锁。`keep_values` 为假时（卸载）设置值一并清掉。
pub(super) fn clear_locked(
    root: &Path,
    writes: &RootWrites,
    id: &str,
    keep_values: bool,
) -> io::Result<u64> {
    remove(&path(root, id))?;
    script_settings::bump_storage_epoch(root, writes, id, keep_values)
}

/// 存储文件在不在。授权写入时用它决定撤销 `persist_data` 要不要清。
pub(super) fn exists(root: &Path, id: &str) -> bool {
    fs::symlink_metadata(path(root, id)).is_ok()
}

fn remove(path: &Path) -> io::Result<()> {
    match crate::storage::remove_private_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
