use std::io::Read;

const INITIAL_READ_CAPACITY: usize = 8 * 1024;

pub(crate) enum ReadError {
    TooLarge,
    Io,
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
