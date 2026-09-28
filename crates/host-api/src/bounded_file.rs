use std::fs::File;
use std::io::{self, Read};

/// Read a file while enforcing a byte ceiling before and during the read.
pub(crate) fn read(file: File, maximum: u64) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds byte limit",
        ));
    }
    let mut bytes = Vec::new();
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
