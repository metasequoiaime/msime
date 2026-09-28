//! Android std::fs::File::lock is unsupported; use rustix's safe flock on that target.
//! The owning File keeps the lock alive and releases it when closed.
//!
//! This module is public so that no caller anywhere in the workspace has to re-derive which
//! locking API works on which target. `host-api` had its own `File::lock` call, and on Android it
//! failed on the first line of every shared clipboard operation - which made the keyboard's own
//! `onCreateInputView` throw and the input method die before it could draw a single key.
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

fn lock_file_options() -> OpenOptions {
    let mut options = File::options();
    options.read(true).write(true).create(true).truncate(false);
    options
}

pub(crate) fn open_lock_file(path: impl AsRef<Path>) -> io::Result<File> {
    lock_file_options().open(path)
}

/// Open a lock file with owner-only permissions on Unix hosts.
pub fn open_private_lock_file(path: impl AsRef<Path>) -> io::Result<File> {
    let mut options = lock_file_options();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(crate) fn try_shared(file: &File) -> io::Result<bool> {
    #[cfg(not(target_os = "android"))]
    {
        match file.try_lock_shared() {
            Ok(()) => Ok(true),
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
    #[cfg(target_os = "android")]
    {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockShared) {
            Ok(()) => Ok(true),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

/// False is contention, not an I/O failure. Never waits for another lock owner.
pub(crate) fn try_exclusive(file: &File) -> io::Result<bool> {
    #[cfg(not(target_os = "android"))]
    {
        match file.try_lock() {
            Ok(()) => Ok(true),
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
    #[cfg(target_os = "android")]
    {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(true),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

/// `try_exclusive`，但对「刚被释放却还没真正放开」的那一瞬给一点宽限。
///
/// flock 的锁属于打开文件描述，而不是进程。任何一次 fork 都会让子进程在 exec 之前持有
/// 父进程全部描述符的副本，父进程此刻关掉文件也不会立即解锁——锁要等最后一个副本消失。
/// 这个窗口只有毫秒级，但它足以让另一条路径拿到一个转瞬即逝的「被占用」，而调用方会把
/// 它当成真的有人在用。本进程自己就会拉起子进程（provider 入口、桌面面板），所以这不是
/// 只在测试里才发生的事：仓库的测试套件正是因为剪贴板那组测试会启动四个子进程，才让快照
/// 队列的测试随机变红。
///
/// 因此在报告占用之前短暂重试。总时长有上限，仍然不会无限等待——真正被别的进程长期持有
/// 的锁还是会如实返回 false。
pub(crate) fn try_exclusive_with_grace(file: &File) -> io::Result<bool> {
    const ATTEMPTS: u32 = 10;
    const INTERVAL: std::time::Duration = std::time::Duration::from_millis(5);
    for attempt in 0..ATTEMPTS {
        if try_exclusive(file)? {
            return Ok(true);
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(INTERVAL);
        }
    }
    Ok(false)
}

pub fn exclusive(file: &File) -> io::Result<()> {
    #[cfg(not(target_os = "android"))]
    {
        file.lock()
    }
    #[cfg(target_os = "android")]
    {
        loop {
            match rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive) {
                Ok(()) => return Ok(()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // 宽限期内释放的锁应当被拿到，长期被持有的锁仍要如实报告占用——后者是产品语义，
    // 不能因为加了重试就变成无限等待。
    #[test]
    fn grace_waits_for_a_lock_that_is_about_to_be_released_and_still_reports_real_contention() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lease");
        let holder = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        assert!(try_exclusive(&holder).unwrap());

        // 另一条路径此刻看到的是占用；宽限期结束后仍然是 false，而不是永远等下去。
        let waiter = File::options().read(true).write(true).open(&path).unwrap();
        let started = Instant::now();
        assert!(!try_exclusive_with_grace(&waiter).unwrap());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "宽限期必须有上限"
        );

        // 持有者在宽限期内放手，等待方应当拿到锁，而不是报告一个转瞬即逝的占用。
        let releasing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(15));
            drop(holder);
        });
        assert!(try_exclusive_with_grace(&waiter).unwrap());
        releasing.join().unwrap();
    }
}
