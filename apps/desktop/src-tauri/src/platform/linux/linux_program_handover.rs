//! 升级后把设置应用交给新程序。
//!
//! dpkg 升级时把新文件改名覆盖旧文件，正在运行的设置应用继续执行已被替换的旧程序。关闭设置窗口只是把它隐藏起来、进程再留十分钟，所以升级后再打开设置，单实例把启动参数转给这个旧进程，用户看到的仍是旧界面。IBus 宿主用同样的 `/proc/self/exe` 判断在焦点变化时换到新程序（见 platforms/linux/README.md 的升级一段）；设置应用在收到下一次启动时做同样的事：程序已被替换就把这次启动交给新程序，自己退出。程序被删除而不是替换（`apt remove`）时没有可以启动的新程序，照旧由旧进程处理。

use std::path::{Path, PathBuf};
use std::time::Duration;

/// 内核在已被删除的可执行文件的 `/proc/self/exe` 链接文本后面加的后缀。
const DELETED_SUFFIX: &str = " (deleted)";

/// 隐藏窗口之后、退出之前等页面写完待保存编辑的时间。隐藏会触发页面的 `visibilitychange`，`useFlushOnWindowLeave` 立刻开始保存，不等自动保存 400ms 的倒计时；写一份偏好文件远用不了一秒。
pub(crate) const FLUSH_GRACE: Duration = Duration::from_secs(1);

/// `/proc/self/exe` 的链接文本 `link` 所说的程序已被替换时，返回原路径上的新程序；没有被删除、被删除后原路径上没有新文件（卸载）、或路径不是绝对路径时为 `None`。`is_file` 是对原路径的检查，测试里替换成假的文件系统。
pub(crate) fn replaced_program(link: &Path, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let original = PathBuf::from(link.to_str()?.strip_suffix(DELETED_SUFFIX)?);
    (original.is_absolute() && is_file(&original)).then_some(original)
}

/// 本进程正在执行的程序是否已被新文件替换，是的话返回新程序的路径。
pub(crate) fn running_program_replaced() -> Option<PathBuf> {
    let link = std::fs::read_link("/proc/self/exe").ok()?;
    replaced_program(&link, Path::is_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_deleted_program_with_a_new_file_in_its_place_is_replaced() {
        let present = |path: &Path| path == Path::new("/usr/bin/msime-linux-desktop");
        assert_eq!(
            replaced_program(Path::new("/usr/bin/msime-linux-desktop (deleted)"), present),
            Some(PathBuf::from("/usr/bin/msime-linux-desktop"))
        );
        // 还是安装时的那个文件：没有升级。
        assert_eq!(
            replaced_program(Path::new("/usr/bin/msime-linux-desktop"), present),
            None
        );
        // 被删除且原路径上没有新文件：卸载，没有可以交接的程序。
        assert_eq!(
            replaced_program(Path::new("/usr/bin/msime-linux-desktop (deleted)"), |_| {
                false
            }),
            None
        );
        assert_eq!(
            replaced_program(Path::new("msime-linux-desktop (deleted)"), |_| true),
            None
        );
    }

    /// 用真实内核核对后缀：改名覆盖一个正在被引用的文件后，原 inode 的 `/proc/<pid>/fd` 链接带上 ` (deleted)`，原路径上是新文件。
    #[test]
    fn the_kernel_marks_a_renamed_over_file_as_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("program");
        std::fs::write(&program, b"old").unwrap();
        let old = std::fs::File::open(&program).unwrap();
        let staged = directory.path().join("program.dpkg-new");
        std::fs::write(&staged, b"new").unwrap();
        std::fs::rename(&staged, &program).unwrap();

        use std::os::fd::AsRawFd;
        let link = std::fs::read_link(format!("/proc/self/fd/{}", old.as_raw_fd())).unwrap();
        assert_eq!(
            replaced_program(&link, Path::is_file),
            Some(program.clone())
        );

        std::fs::remove_file(&program).unwrap();
        assert_eq!(replaced_program(&link, Path::is_file), None);
    }
}
