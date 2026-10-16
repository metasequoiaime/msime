//! 搬移 Linux 数据目录，期间让输入宿主不碰它。
//!
//! 租约和 `QuiescedHosts`（它让租约在一次设置页操作的多个请求之间一直挂着）与 macOS 和 MCP 服务端共用（`msime_client_services::dictionary::quiesce`；`is_lease_file` 在这里重新导出）。只属于 Linux 的是长时间让出：搬移数据目录时，IBus 和 Fcitx5 宿主在整个拷贝期间都不碰用户目录，租约挂着，独占词库锁也同时持有（`hold_hosts_off`）。

use msime_client_services::dictionary::access::DictionaryAccess;
pub(crate) use msime_client_services::dictionary::quiesce::is_lease_file;
use msime_client_services::dictionary::quiesce::{Lease, RETRY_BUDGET, RETRY_INTERVAL};
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HoldError {
    /// A session still held the dictionaries when the budget ran out.
    Busy,
    /// The lease could not be written or the lock files could not be opened.
    Unavailable,
}

/// Both hosts kept off the user directory: the lease that asked them to close their sessions, and the exclusive dictionary lock that proves they did and keeps any session from opening while it is held, even after the lease has expired. Dropping it releases the lock first and then removes the lease.
pub(crate) struct HostsHeldOff<Access = DictionaryAccess> {
    _access: Access,
    _lease: Lease,
}

/// 在返回的守卫存活期间让两个宿主都不碰 `user_data`，用于替换整个目录而不是经 Engine 编辑的工作，比如搬移数据根目录。与 `msime_client_services::dictionary::quiesce::QuiescedHosts` 不同，这里先挂上租约，并一直挂到守卫被丢弃。没有运行的宿主不持有锁，所以不会让这里一直忙。`user_data` 不存在时返回 `Ok(None)`：不可能有会话开在它上面。
pub(crate) fn hold_hosts_off(
    user_data: &Path,
    dictionaries: &Path,
) -> Result<Option<HostsHeldOff>, HoldError> {
    hold_with(user_data, RETRY_BUDGET, || {
        DictionaryAccess::try_maintenance(user_data, dictionaries)
    })
}

fn hold_with<Access>(
    user_data: &Path,
    budget: Duration,
    mut try_exclusive: impl FnMut() -> std::io::Result<Option<Access>>,
) -> Result<Option<HostsHeldOff<Access>>, HoldError> {
    if !user_data.is_absolute() {
        return Err(HoldError::Unavailable);
    }
    match std::fs::symlink_metadata(user_data) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(HoldError::Unavailable),
        Ok(_) => {}
    }
    let lease = Lease::acquire(user_data).map_err(|_| HoldError::Unavailable)?;
    let deadline = Instant::now() + budget;
    loop {
        match try_exclusive() {
            Ok(Some(access)) => {
                return Ok(Some(HostsHeldOff {
                    _access: access,
                    _lease: lease,
                }))
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(RETRY_INTERVAL),
            Ok(None) => return Err(HoldError::Busy),
            Err(_) => return Err(HoldError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::dictionary::quiesce::LEASE_NAME;

    fn lease_expiry(directory: &Path) -> Option<u128> {
        std::fs::read_to_string(directory.join(LEASE_NAME))
            .ok()
            .and_then(|text| text.lines().next()?.parse().ok())
    }

    /// How long the test waits for something that normally happens within milliseconds before calling it stuck. It only guards against a hang: on a loaded machine (a CI runner, or several builds at once) writing the lease has taken longer than two seconds, which failed the test without anything being wrong.
    const PATIENCE: Duration = Duration::from_secs(30);

    #[test]
    fn a_hold_waits_for_the_session_to_close_and_keeps_the_lease_until_dropped() {
        let directory = tempfile::tempdir().unwrap();
        let user = directory.path().to_path_buf();
        // A host with a session open on the directory, releasing it once it sees the lease, as both hosts do on their timers.
        let session = DictionaryAccess::try_session(&user, &user)
            .unwrap()
            .unwrap();
        let host = {
            let user = user.clone();
            std::thread::spawn(move || {
                let deadline = Instant::now() + PATIENCE;
                while !user.join(LEASE_NAME).exists() {
                    assert!(Instant::now() < deadline, "the lease never appeared");
                    std::thread::sleep(Duration::from_millis(5));
                }
                drop(session);
            })
        };
        let held = hold_with(&user, PATIENCE, || {
            DictionaryAccess::try_maintenance(&user, &user)
        })
        .unwrap()
        .unwrap();
        host.join().unwrap();
        // While held, the lease is up and no session can open, whether or not a host still honours the lease.
        assert!(lease_expiry(&user).is_some());
        assert!(DictionaryAccess::try_session(&user, &user)
            .unwrap()
            .is_none());
        drop(held);
        assert!(!user.join(LEASE_NAME).exists());
        let deadline = Instant::now() + PATIENCE;
        while DictionaryAccess::try_session(&user, &user)
            .unwrap()
            .is_none()
        {
            assert!(Instant::now() < deadline, "the lock stayed held");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_session_that_never_closes_makes_the_hold_busy_and_takes_the_lease_down() {
        let directory = tempfile::tempdir().unwrap();
        let _session = DictionaryAccess::try_session(directory.path(), directory.path())
            .unwrap()
            .unwrap();
        let result = hold_with(directory.path(), Duration::from_millis(120), || {
            DictionaryAccess::try_maintenance(directory.path(), directory.path())
        });
        assert_eq!(result.err(), Some(HoldError::Busy));
        assert!(!directory.path().join(LEASE_NAME).exists());
    }

    #[test]
    fn a_missing_user_directory_needs_no_hold_and_a_relative_one_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("user");
        let result = hold_with(
            &missing,
            Duration::from_secs(2),
            || -> std::io::Result<Option<()>> { unreachable!("nothing to lock") },
        );
        assert!(matches!(result, Ok(None)));
        assert!(!missing.exists());
        let relative = hold_with(Path::new("relative/user"), Duration::from_secs(2), || {
            Ok(Some(()))
        });
        assert_eq!(relative.err(), Some(HoldError::Unavailable));
    }
}
