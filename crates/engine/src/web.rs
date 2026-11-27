//! 浏览器没有文件系统（仅 wasm32-unknown-unknown）：宿主把 SQLite 词库的字节交给这里，导入 sqlite-wasm-rs 默认的内存 VFS，路径就是 `RuntimePaths` 解析出的那个绝对路径，所以 `Connection::open` 照常能打开。句子模型从字节加载，n-gram 不随网页发布，因此这里不保存非数据库文件。
//!
//! 这是网页引擎（`msime-engine-wasm`）的产品胶水，不在引擎文档化的 API 里。

use rusqlite::ffi::vfs::memvfs::MemVfsUtil;
use rusqlite::ffi::vfs::transfer::DbTransfer;
use rusqlite::ffi::vfs::VfsFilesManager;

/// The memory VFS that sqlite-wasm-rs installs as the default from `sqlite3_os_init`; looking it up runs SQLite's automatic initialization first.
fn vfs() -> Result<MemVfsUtil, String> {
    // SAFETY: wasm32-unknown-unknown here is single-threaded (SQLite is built with SQLITE_THREADSAFE=0 and the engine runs in one Worker), so the lookup and every later SQLite call stay on the thread that installed the memory VFS, and nothing else registers VFSes concurrently.
    #[allow(unsafe_code)]
    let vfs = unsafe { MemVfsUtil::get() };
    vfs.map_err(|error| error.to_string())
}

/// 已存在同名库时先删掉再导入
pub fn import_database(path: &str, bytes: &[u8]) -> Result<(), String> {
    let vfs = vfs()?;
    let Ok(_) = vfs.remove(path);
    vfs.import_db(path, bytes)
        .map_err(|error| error.to_string())
}

/// 删除内存 VFS 里的词库；调用前须关闭所有打开它的连接。库不存在时什么也不做。
pub fn delete_database(path: &str) -> Result<(), String> {
    let Ok(_) = vfs()?.remove(path);
    Ok(())
}
