//! SQLite housekeeping shared by the database stages.

use std::path::Path;
use std::{fs::OpenOptions, io};

use anyhow::{bail, Result};
use rusqlite::{Connection, OpenFlags};

pub fn open(path: &Path) -> Result<Connection> {
    let path = no_follow_path(path)?;
    create_database_file(&path)?;
    Ok(Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?)
}

fn create_database_file(path: &Path) -> io::Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "database path is a symbolic link",
            ));
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn open_read_only(path: &Path) -> Result<Connection> {
    let path = no_follow_path(path)?;
    check_regular_file(&path)?;
    Ok(Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}

fn check_regular_file(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
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
            "database input is not a regular file",
        ));
    }
    Ok(())
}

fn no_follow_path(path: &Path) -> io::Result<std::path::PathBuf> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "database path has no parent")
    })?;
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "database path has no filename")
    })?;
    let resolved = std::fs::canonicalize(parent)?.join(name);
    if let Ok(metadata) = std::fs::symlink_metadata(&resolved) {
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "database path is a symbolic link",
            ));
        }
    }
    Ok(resolved)
}

pub fn integrity_check(connection: &Connection) -> Result<()> {
    let result: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result != "ok" {
        bail!("SQLite integrity check failed: {result}");
    }
    Ok(())
}

/// `ANALYZE` (and optionally `PRAGMA optimize`), keeping only `sqlite_stat1`. The bundled SQLite is compiled with STAT4, which the Python-built databases never carried; the runtime plans its queries from `sqlite_stat1` alone, so the extra table would only make the artifacts differ.
pub fn analyze(connection: &Connection, optimize: bool) -> Result<()> {
    connection.execute_batch("ANALYZE")?;
    if optimize {
        connection.execute_batch("PRAGMA optimize")?;
    }
    connection.execute_batch("DROP TABLE IF EXISTS sqlite_stat4")?;
    Ok(())
}

/// Ship a standalone database: no WAL header that needs sidecar files, and no free pages (dropping `sqlite_stat4` and replaced tables leaves some).
pub fn freeze(path: &Path) -> Result<()> {
    let connection = open(path)?;
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
    let mode: String = connection.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("delete") {
        bail!(
            "{}: cannot finalize a standalone database (journal mode {mode})",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn sqlite_opens_reject_symlinked_leaves() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real.db");
        open(&real).unwrap();
        let linked = directory.path().join("linked.db");
        symlink(&real, &linked).unwrap();

        assert!(open(&linked).is_err());
        assert!(open_read_only(&linked).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn sqlite_read_only_rejects_a_fifo_without_blocking() {
        use std::os::unix::fs::OpenOptionsExt;
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.db");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());

        let (done, result) = mpsc::channel();
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || {
            done.send(open_read_only(&worker_path).is_err()).unwrap();
        });
        let rejected_without_release = match result.recv_timeout(Duration::from_millis(100)) {
            Ok(rejected) => rejected,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let writer = std::fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&path)
                    .unwrap();
                drop(writer);
                result.recv_timeout(Duration::from_secs(1)).unwrap();
                false
            }
            Err(error) => panic!("SQLite reader failed to report: {error}"),
        };
        worker.join().unwrap();
        assert!(
            rejected_without_release,
            "FIFO SQLite input must be rejected without blocking"
        );
    }

    #[test]
    fn analyze_leaves_only_stat1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.db");
        let connection = open(&path).unwrap();
        connection
            .execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(a, b); CREATE INDEX i ON t(a); INSERT INTO t VALUES (1, 2), (1, 3), (2, 4);")
            .unwrap();
        analyze(&connection, true).unwrap();
        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE name LIKE 'sqlite_stat%' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(tables, ["sqlite_stat1"]);
        drop(connection);
        freeze(&path).unwrap();
        let frozen = open(&path).unwrap();
        let mode: String = frozen
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "delete");
        let free: i64 = frozen
            .query_row("PRAGMA freelist_count", [], |row| row.get(0))
            .unwrap();
        assert_eq!(free, 0);
        integrity_check(&open(&path).unwrap()).unwrap();
    }
}
