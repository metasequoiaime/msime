//! Shared bounded file reads for local client-core documents.

use std::fs::File;
use std::io::{self, Read};

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
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > maximum {
        return Err(too_large());
    }
    Ok(bytes)
}
