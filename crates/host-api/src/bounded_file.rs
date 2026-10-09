use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

/// 只读打开，不跟随最后一级符号链接。多链接文件只在所在目录只有 root 或当前用户能写时放行，与 `msime_client_core::file_lock::open_private_file` 相同（#6386）。
pub(crate) fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A user-controlled FIFO must not block the host thread while it is opened.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || !(msime_client_core::file_lock::has_single_link(&file)?
            || msime_path_trust::multi_link_is_trusted(&file, path)?)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file with a trusted link count",
        ));
    }
    Ok(file)
}

/// Read a file while enforcing a byte ceiling before and during the read.
pub(crate) fn read(file: File, maximum: u64) -> io::Result<Vec<u8>> {
    let length = file.metadata()?.len();
    if length > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds byte limit",
        ));
    }
    let capacity = usize::try_from(length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "file is too large for this platform",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds byte limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::{open_private, read};

    #[cfg(unix)]
    #[test]
    fn open_private_rejects_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-host-data").unwrap();
        let linked = root.path().join("private.json");
        symlink(&target, &linked).unwrap();

        assert!(open_private(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-host-data");
    }

    #[cfg(unix)]
    #[test]
    fn open_private_rejects_a_directory() {
        let root = tempfile::tempdir().unwrap();

        assert!(open_private(root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn open_private_rejects_a_hard_linked_file() {
        use std::os::unix::fs::MetadataExt;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-host-data").unwrap();
        let linked = root.path().join("private.json");
        std::fs::hard_link(&target, &linked).unwrap();

        assert_eq!(std::fs::metadata(&linked).unwrap().nlink(), 2);
        msime_path_trust::open_to_other_users(root.path()).unwrap();
        assert!(open_private(&linked).is_err());
    }

    /// 内置按键音在 Nix 的 store 去重后是多链接文件；目录只有属主能写时照常读（#6386）。
    #[cfg(unix)]
    #[test]
    fn open_private_reads_a_hard_link_in_a_closed_directory() {
        let root = tempfile::tempdir().unwrap();
        msime_path_trust::close_to_other_users(root.path()).unwrap();
        let original = root.path().join("tap.wav");
        std::fs::write(&original, b"synthetic-sound").unwrap();
        std::fs::hard_link(&original, root.path().join("deduplicated.wav")).unwrap();

        assert_eq!(
            read(open_private(&original).unwrap(), 64).unwrap(),
            b"synthetic-sound"
        );
    }
}
