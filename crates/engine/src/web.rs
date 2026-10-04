//! 浏览器没有文件系统（仅 wasm32-unknown-unknown）：宿主把 SQLite 词库的字节交给这里，导入 sqlite-wasm-rs 默认的内存 VFS，路径就是 `RuntimePaths` 解析出的那个绝对路径，所以 `Connection::open` 照常能打开。句子模型从字节加载，n-gram 不随网页发布，因此这里不保存非数据库文件。

use rusqlite::ffi::{MemVfsUtil, WasmOsCallback};

fn vfs() -> MemVfsUtil<WasmOsCallback> {
    MemVfsUtil::<WasmOsCallback>::new()
}

/// 已存在同名库时先 delete_db 再 import_db
pub fn import_database(path: &str, bytes: &[u8]) -> Result<(), String> {
    let vfs = vfs();
    if vfs.exists(path) {
        vfs.delete_db(path);
    }
    vfs.import_db(path, bytes)
        .map_err(|error| error.to_string())
}

/// 删除内存 VFS 里的词库；调用前须关闭所有打开它的连接。库不存在时什么也不做。
pub fn delete_database(path: &str) -> Result<(), String> {
    vfs().delete_db(path);
    Ok(())
}
