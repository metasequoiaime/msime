//! Moving the Linux data directory with the input hosts held off it.
//!
//! The lease and `QuiescedHosts`, which keeps it up across the requests of one settings-page action, are shared with macOS and the MCP server (`msime_client_core::dictionary::quiesce`; `is_lease_file` is re-exported here). What stays Linux-only is the long hold: moving the data directory keeps the IBus and Fcitx5 hosts off the user directory for the whole copy, with the lease up and the exclusive dictionary lock held together (`hold_hosts_off`).

use msime_client_core::dictionary::access::DictionaryAccess;
pub(crate) use msime_client_core::dictionary::quiesce::is_lease_file;
use msime_client_core::dictionary::quiesce::{Lease, RETRY_BUDGET, RETRY_INTERVAL};
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

/// Keep both hosts off `user_data` for as long as the returned guard lives, for work that replaces the directory rather than editing through the Engine, such as moving the data root. Unlike `msime_client_core::dictionary::quiesce::QuiescedHosts` the lease goes up first and stays up until the guard is dropped. A host that is not running holds no lock, so it cannot keep this busy. `Ok(None)` when `user_data` does not exist: no session can be open on it.
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
    use msime_client_core::dictionary::quiesce::LEASE_NAME;

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
