//! Shared bounded reads for response bodies.

use std::io::{self, Read};

#[derive(Debug)]
pub(crate) enum BoundedReadError {
    TooLarge,
    Read(io::Error),
}

/// Read at most one byte past `maximum` so streams without a trustworthy
/// `Content-Length` cannot grow the settings process without bound.
pub(crate) fn read_bounded(reader: impl Read, maximum: usize) -> Result<Vec<u8>, BoundedReadError> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(BoundedReadError::Read)?;
    if bytes.len() > maximum {
        return Err(BoundedReadError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{BoundedReadError, read_bounded};
    use std::io;

    const MAX_BODY_BYTES: usize = 64 * 1024;

    #[test]
    fn accepts_small_and_exact_limit_bodies() {
        for size in [0, 2, MAX_BODY_BYTES] {
            let body = vec![b' '; size];
            assert_eq!(read_bounded(body.as_slice(), MAX_BODY_BYTES).unwrap(), body);
        }
    }

    #[test]
    fn rejects_oversize_without_consuming_the_rest() {
        let body = vec![b' '; MAX_BODY_BYTES * 4];
        let mut cursor = io::Cursor::new(body);
        assert!(matches!(
            read_bounded(&mut cursor, MAX_BODY_BYTES),
            Err(BoundedReadError::TooLarge)
        ));
        assert_eq!(cursor.position(), (MAX_BODY_BYTES + 1) as u64);
    }

    #[test]
    fn propagates_transport_failure_instead_of_parsing_partial_json() {
        struct Broken;
        impl io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::TimedOut))
            }
        }
        let Err(BoundedReadError::Read(error)) = read_bounded(Broken, MAX_BODY_BYTES) else {
            panic!("expected the transport error to be preserved");
        };
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
