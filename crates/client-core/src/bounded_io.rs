//! Shared bounded file reads for local client-core documents.

use std::fs::File;
use std::io::{self, Read};

pub(crate) fn read_bounded(mut reader: impl Read, maximum: u64) -> io::Result<Vec<u8>> {
    read_bounded_with_capacity(&mut reader, maximum, 0)
}

fn read_bounded_with_capacity(
    mut reader: impl Read,
    maximum: u64,
    capacity: usize,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reader exceeded byte limit",
        ));
    }
    Ok(bytes)
}

/// Read at most `maximum` bytes, reporting an oversized document through the caller's domain
/// error while preserving ordinary I/O failures.
pub(crate) fn read_bounded_file<E>(
    file: File,
    maximum: u64,
    too_large: impl FnOnce() -> E,
) -> Result<Vec<u8>, E>
where
    E: From<io::Error>,
{
    read_bounded_file_with(file, maximum, too_large, E::from)
}

pub(crate) fn read_bounded_file_with<E>(
    file: File,
    maximum: u64,
    too_large: impl FnOnce() -> E,
    io_error: impl FnOnce(io::Error) -> E + Copy,
) -> Result<Vec<u8>, E> {
    let file_size = file.metadata().map_err(io_error)?.len();
    if file_size > maximum {
        return Err(too_large());
    }
    let capacity = usize::try_from(file_size).unwrap_or(0);
    match read_bounded_with_capacity(file, maximum, capacity) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => Err(too_large()),
        Err(error) => Err(io_error(error)),
    }
}
