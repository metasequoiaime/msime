use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

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
    if !metadata.is_file() || !has_single_link(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a single-link regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}

#[cfg(windows)]
fn has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.number_of_links() == 1
}

#[cfg(not(any(unix, windows)))]
fn has_single_link(_: &std::fs::Metadata) -> bool {
    true
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
    use super::open_private;

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
        assert!(open_private(&linked).is_err());
    }
}
