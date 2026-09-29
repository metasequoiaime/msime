use std::io::Read;

pub(crate) enum ReadError {
    TooLarge,
    Io,
}

pub(crate) fn read(reader: impl Read, maximum: u64) -> Result<Vec<u8>, ReadError> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ReadError::Io)?;
    if bytes.len() as u64 > maximum {
        return Err(ReadError::TooLarge);
    }
    Ok(bytes)
}
