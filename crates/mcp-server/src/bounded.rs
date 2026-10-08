use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

const INITIAL_READ_CAPACITY: usize = 8 * 1024;

pub(crate) enum ReadError {
    TooLarge,
    Io,
}

/// Open a host-owned document without following a replaced leaf symlink.
pub(crate) fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A user-controlled FIFO must not block the MCP process while it is opened.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    Ok(file)
}

pub(crate) fn read(reader: impl Read, maximum: u64) -> Result<Vec<u8>, ReadError> {
    let mut bytes = Vec::with_capacity(maximum.min(INITIAL_READ_CAPACITY as u64) as usize);
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ReadError::Io)?;
    if bytes.len() as u64 > maximum {
        return Err(ReadError::TooLarge);
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
        std::fs::write(&target, b"synthetic-mcp-data").unwrap();
        let linked = root.path().join("private.json");
        symlink(&target, &linked).unwrap();

        assert!(open_private(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-mcp-data");
    }

    #[cfg(unix)]
    #[test]
    fn open_private_rejects_a_directory() {
        let root = tempfile::tempdir().unwrap();

        assert!(open_private(root.path()).is_err());
    }
}
