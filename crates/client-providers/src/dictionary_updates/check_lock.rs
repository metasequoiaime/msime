//! 一次检查（含整段下载）、回退，以及「还有没有检查在进行」的探测共用的互斥。
//!
//! iOS 上状态目录在 App Group 共享容器里，应用被挂起时还持着那里的文件锁，系统会以 `0xdead10cc` 把它杀掉（#11973），而一次检查要等好几分钟的网络。iOS 上发起检查、回退和查状态的只有主应用（键盘扩展不发这些请求），所以那里改用进程内的互斥，不在共享容器里留文件锁。其他平台没有这个限制，照旧用 `dictionary-updates/update.lock` 的跨进程文件锁。

use std::io;
use std::path::Path;

#[cfg(not(target_os = "ios"))]
const LOCK: &str = "update.lock";

/// 持有期间没有别的检查或回退会开始；随丢弃释放。
pub(super) struct CheckLock {
    #[cfg(not(target_os = "ios"))]
    _file: crate::file_lock::ScopedFileLock,
    #[cfg(target_os = "ios")]
    _held: process::Held,
}

impl CheckLock {
    /// 已有检查在进行时返回 `None`，不等待。
    pub(super) fn try_acquire(folder: &Path) -> io::Result<Option<Self>> {
        #[cfg(not(target_os = "ios"))]
        {
            Ok(crate::file_lock::ScopedFileLock::try_exclusive(
                crate::file_lock::open_private_lock_file(folder.join(LOCK))?,
            )?
            .map(|file| Self { _file: file }))
        }
        #[cfg(target_os = "ios")]
        {
            Ok(process::try_hold(folder).map(|held| Self { _held: held }))
        }
    }

    /// 等到正在进行的检查结束再取得。
    pub(super) fn acquire(folder: &Path) -> io::Result<Self> {
        #[cfg(not(target_os = "ios"))]
        {
            Ok(Self {
                _file: crate::file_lock::ScopedFileLock::exclusive(
                    crate::file_lock::open_private_lock_file(folder.join(LOCK))?,
                )?,
            })
        }
        #[cfg(target_os = "ios")]
        {
            Ok(Self {
                _held: process::hold(folder),
            })
        }
    }

    /// 确认此刻没有检查在进行。取不到锁或打不开锁文件时都当作还在进行。
    pub(super) fn idle(folder: &Path) -> bool {
        matches!(Self::try_acquire(folder), Ok(Some(_)))
    }
}

/// 进程内按状态目录互斥：iOS 用它代替文件锁，其他平台只在测试里编译。
#[cfg(any(target_os = "ios", test))]
mod process {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};

    struct Registry {
        held: Mutex<HashSet<PathBuf>>,
        released: Condvar,
    }

    fn registry() -> &'static Registry {
        static REGISTRY: OnceLock<Registry> = OnceLock::new();
        REGISTRY.get_or_init(|| Registry {
            held: Mutex::default(),
            released: Condvar::new(),
        })
    }

    fn held() -> MutexGuard<'static, HashSet<PathBuf>> {
        registry()
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) struct Held(PathBuf);

    impl Drop for Held {
        fn drop(&mut self) {
            held().remove(&self.0);
            registry().released.notify_all();
        }
    }

    pub(super) fn try_hold(folder: &Path) -> Option<Held> {
        held()
            .insert(folder.to_path_buf())
            .then(|| Held(folder.to_path_buf()))
    }

    pub(super) fn hold(folder: &Path) -> Held {
        let mut held = held();
        while held.contains(folder) {
            held = registry()
                .released
                .wait(held)
                .unwrap_or_else(PoisonError::into_inner);
        }
        held.insert(folder.to_path_buf());
        Held(folder.to_path_buf())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::mpsc;
        use std::time::Duration;

        #[test]
        fn one_holder_per_state_directory_and_waiters_resume_after_release() {
            let directory = tempfile::tempdir().unwrap();
            let folder = directory.path().join("dictionary-updates");
            let other = directory.path().join("other");
            let first = try_hold(&folder).unwrap();
            assert!(try_hold(&folder).is_none());
            // 不同的状态目录互不影响。
            drop(try_hold(&other).unwrap());

            let (sender, receiver) = mpsc::channel();
            let waiting = folder.clone();
            let waiter = std::thread::spawn(move || {
                let held = hold(&waiting);
                sender.send(()).unwrap();
                drop(held);
            });
            assert!(receiver.recv_timeout(Duration::from_millis(200)).is_err());
            drop(first);
            receiver.recv_timeout(Duration::from_secs(10)).unwrap();
            waiter.join().unwrap();
            drop(try_hold(&folder).unwrap());
        }
    }
}
