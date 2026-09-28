//! Shared bounded file reads for local client-core documents.

use std::fs::File;
use std::io::{self, Read};

pub(crate) fn read_bounded(mut reader: impl Read, maximum: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
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
    if file.metadata().map_err(io_error)?.len() > maximum {
        return Err(too_large());
    }
    match read_bounded(file, maximum) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => Err(too_large()),
        Err(error) => Err(io_error(error)),
    }
}
