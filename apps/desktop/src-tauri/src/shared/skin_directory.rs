use std::path::{Path, PathBuf};

/// Create only the host-selected directory and pass its absolute path to
/// the platform opener. Neither a webview path nor a shell command is accepted.
fn prepare_and_open(
    root: &Path,
    launch: impl FnOnce(&Path) -> std::io::Result<()>,
) -> std::io::Result<PathBuf> {
    super::atomic_file::create_directory_and_check(root)?;
    // Do not canonicalize: Windows canonicalization adds a verbatim path prefix
    // which is intended for filesystem APIs, not shell directory navigation.
    let directory = std::path::absolute(root)?;
    launch(&directory)?;
    Ok(directory)
}

pub fn open(root: &Path) -> Result<(), &'static str> {
    prepare_and_open(root, launch_directory)
        .map(|_| ())
        .map_err(|_| "storage")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn launch_directory(directory: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        if crate::linux_process::run_status_path(
            "xdg-open",
            directory,
            std::time::Duration::from_secs(3),
        ) {
            Ok(())
        } else {
            Err(std::io::Error::other("directory opener failed"))
        }
    }

    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(directory)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("directory opener failed"))
        }
    }
}

#[cfg(target_os = "windows")]
fn launch_directory(directory: &Path) -> std::io::Result<()> {
    let directory = directory.to_path_buf();
    // A dedicated thread avoids inheriting a runtime worker's COM apartment;
    // the shell call itself lives in the Windows host layer.
    std::thread::spawn(move || {
        msime_host_windows::open_directory(&directory)
            .then_some(())
            .ok_or_else(|| std::io::Error::other("directory opener failed"))
    })
    .join()
    .map_err(|_| std::io::Error::other("directory opener failed"))?
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn launch_directory(_: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("unsupported platform"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_host_directory_before_launch_and_preserves_existing_files() {
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("synthetic state & spaces").join("skins");
        let opened = prepare_and_open(&root, |path| {
            assert!(path.is_absolute());
            assert!(path.is_dir());
            assert_eq!(path, std::path::absolute(&root).unwrap());
            Ok(())
        })
        .unwrap();
        std::fs::write(opened.join("marker"), b"synthetic").unwrap();
        prepare_and_open(&root, |_| Ok(())).unwrap();
        assert_eq!(std::fs::read(opened.join("marker")).unwrap(), b"synthetic");
    }

    #[test]
    fn file_collision_never_launches_or_overwrites() {
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::write(&root, b"synthetic").unwrap();
        assert!(prepare_and_open(&root, |_| panic!("must not launch")).is_err());
        assert_eq!(std::fs::read(root).unwrap(), b"synthetic");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_parent_without_creating_outside_it() {
        use msime_path_trust::untrusted_symlink as symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join("redirect");
        symlink(outside.path(), &linked).unwrap();
        let directory = linked.join("skins");

        assert!(prepare_and_open(&directory, |_| panic!("must not launch")).is_err());
        assert!(!outside.path().join("skins").exists());
    }

    #[test]
    fn launch_failure_is_reported_without_removing_directory() {
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        assert!(prepare_and_open(&root, |_| Err(std::io::Error::other("synthetic"))).is_err());
        assert!(root.is_dir());
    }
}
