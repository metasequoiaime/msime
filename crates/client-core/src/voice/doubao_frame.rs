use flate2::{write::GzEncoder, Compression};
use std::io::{Read, Write};

// A small compressed response must not expand without bound in host-api.
const MAX_RESPONSE_FRAME_BYTES: usize = 1_048_576;
const MAX_RESPONSE_PAYLOAD_BYTES: usize = 1_048_576;
const INITIAL_RESPONSE_CAPACITY: usize = 8 * 1024;

/// Build the initial Doubao ASR request used by the Windows client.
pub fn start_frame(
    enable_itn: bool,
    enable_punc: bool,
    enable_ddc: bool,
    boosting_table_id: &str,
) -> Vec<u8> {
    let mut request = serde_json::json!({
        "user": {"uid": "metasequoia-ime"},
        "audio": {"format": "pcm", "codec": "raw", "rate": 16000, "bits": 16, "channel": 1},
        "request": {
            "model_name": "bigmodel",
            "enable_itn": enable_itn,
            "enable_punc": enable_punc,
            "enable_ddc": enable_ddc,
            "show_utterances": false,
            "result_type": "full"
        }
    });
    if !boosting_table_id.is_empty() {
        request["request"]["corpus"] = serde_json::json!({"boosting_table_id": boosting_table_id});
    }
    encode_json_frame(0x01, 0x01, 1, request.to_string().as_bytes())
}

/// Build one PCM audio packet. The final packet uses the negative sequence
/// and final flag required by the Doubao protocol.
pub fn audio_frame(sequence: i32, pcm: &[u8], final_chunk: bool) -> Vec<u8> {
    let sequence = if final_chunk {
        -sequence.abs()
    } else {
        sequence.abs()
    };
    encode_json_frame(0x02, if final_chunk { 0x03 } else { 0x01 }, sequence, pcm)
}

pub fn encode_json_frame(message_type: u8, flags: u8, sequence: i32, payload: &[u8]) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::with_capacity(payload.len()), Compression::default());
    gzip.write_all(payload).expect("gzip write to memory");
    let compressed = gzip.finish().expect("gzip finish");
    let mut frame = Vec::with_capacity(12 + compressed.len());
    frame.extend_from_slice(&[0x11, (message_type << 4) | (flags & 0x0f), 0x11, 0]);
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(&(compressed.len() as i32).to_be_bytes());
    frame.extend_from_slice(&compressed);
    frame
}

pub fn decode_json_frame(frame: &[u8]) -> Option<(bool, i32, Vec<u8>)> {
    if frame.len() < 8
        || frame.len() > MAX_RESPONSE_FRAME_BYTES
        || frame[0] != 0x11
        || (frame[1] >> 4) != 0x09
        || frame[2] != 0x11
    {
        return None;
    }
    let flags = frame[1] & 0x0f;
    let mut offset = 4usize;
    if flags & 0x01 != 0 {
        offset += 4;
    }
    if flags & 0x04 != 0 {
        offset += 4;
    }
    if offset + 4 > frame.len() {
        return None;
    }
    let size = u32::from_be_bytes(frame[offset..offset + 4].try_into().ok()?) as usize;
    offset += 4;
    // Compare by subtraction: hostile length fields must never overflow an
    // addition, and one WebSocket message must contain exactly one frame.
    if size != frame.len() - offset {
        return None;
    }
    // bufread preserves unread bytes, allowing us to reject concatenated gzip
    // members or junk inside the declared compressed payload as well.
    let mut decoder = flate2::bufread::GzDecoder::new(&frame[offset..]);
    let mut payload = Vec::with_capacity(frame.len().min(INITIAL_RESPONSE_CAPACITY));
    (&mut decoder)
        .take((MAX_RESPONSE_PAYLOAD_BYTES + 1) as u64)
        .read_to_end(&mut payload)
        .ok()?;
    if payload.len() > MAX_RESPONSE_PAYLOAD_BYTES || !decoder.into_inner().is_empty() {
        return None;
    }
    Some(((flags & 0x02) != 0, 0, payload))
}

/// Decode the numeric error code from a Doubao error frame (message type 0xF).
pub fn decode_error_code(frame: &[u8]) -> Option<i32> {
    if frame.len() < 12 || frame[0] != 0x11 || (frame[1] >> 4) != 0x0f || frame[2] != 0x11 {
        return None;
    }
    let flags = frame[1] & 0x0f;
    let mut offset = 4usize;
    if flags & 0x01 != 0 {
        offset += 4;
    }
    if flags & 0x04 != 0 {
        offset += 4;
    }
    if offset + 4 > frame.len() {
        return None;
    }
    Some(i32::from_be_bytes(
        frame[offset..offset + 4].try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_decoder_bounds_expansion_and_accepts_exact_limit() {
        let payload = vec![b'a'; MAX_RESPONSE_PAYLOAD_BYTES];
        let frame = encode_json_frame(9, 1, 1, &payload);
        assert_eq!(decode_json_frame(&frame).unwrap().2, payload);
        let frame = encode_json_frame(9, 1, 1, &vec![b'a'; MAX_RESPONSE_PAYLOAD_BYTES + 1]);
        assert!(frame.len() < 4096);
        assert!(decode_json_frame(&frame).is_none());
        assert!(decode_json_frame(&vec![0; MAX_RESPONSE_FRAME_BYTES + 1]).is_none());
    }

    #[test]
    fn response_decoder_rejects_truncation_hostile_sizes_and_wrong_version() {
        let frame = encode_json_frame(9, 1, 1, b"{}");
        for length in 0..frame.len() {
            assert!(decode_json_frame(&frame[..length]).is_none());
        }
        for size in [0_u32, 1, u32::MAX, i32::MAX as u32] {
            let mut invalid = frame.clone();
            invalid[8..12].copy_from_slice(&size.to_be_bytes());
            assert!(decode_json_frame(&invalid).is_none());
        }
        let mut invalid = frame;
        invalid[0] = 0x21;
        assert!(decode_json_frame(&invalid).is_none());
    }

    #[test]
    fn response_decoder_rejects_trailing_bytes_and_corrupt_gzip() {
        let frame = encode_json_frame(9, 1, 1, b"{}");
        let mut invalid = frame.clone();
        invalid.push(0);
        assert!(decode_json_frame(&invalid).is_none());
        // Even if the outer size includes junk, gzip must consume it all.
        let size = (invalid.len() - 12) as u32;
        invalid[8..12].copy_from_slice(&size.to_be_bytes());
        assert!(decode_json_frame(&invalid).is_none());
        let mut concatenated = frame.clone();
        concatenated.extend_from_slice(&frame[12..]);
        let size = (concatenated.len() - 12) as u32;
        concatenated[8..12].copy_from_slice(&size.to_be_bytes());
        assert!(decode_json_frame(&concatenated).is_none());
        let mut corrupt = frame;
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(decode_json_frame(&corrupt).is_none());
    }

    #[test]
    fn error_decoder_rejects_a_different_protocol_version() {
        let mut error = [0x11, 0xf0, 0x11, 0, 0, 0, 0, 7, 0, 0, 0, 42];
        assert_eq!(decode_error_code(&error), Some(7));
        error[0] = 0x21;
        assert_eq!(decode_error_code(&error), None);
        error[0] = 0x11;
        error[2] = 0x21;
        assert_eq!(decode_error_code(&error), None);
    }

    #[test]
    fn builds_windows_compatible_start_and_final_audio_frames() {
        let start = start_frame(true, false, true, "table");
        assert_eq!(&start[..4], &[0x11, 0x11, 0x11, 0]);
        assert_eq!(&start[4..8], &[0, 0, 0, 1]);

        let audio = audio_frame(2, &[0, 1, 2, 3], true);
        assert_eq!(&audio[..4], &[0x11, 0x23, 0x11, 0]);
        assert_eq!(&audio[4..8], &(-2i32).to_be_bytes());
    }

    #[test]
    fn encodes_protocol_header_and_gzip_payload() {
        let error = [0x11, 0xf0, 0x11, 0, 0, 0, 0, 7, 0, 0, 0, 42];
        assert_eq!(decode_error_code(&error), Some(7));
        let error = [0x11, 0xf0, 0x11, 0, 0, 0, 0, 7, 0, 0, 0, 42];
        assert_eq!(decode_error_code(&error), Some(7));
        let frame = encode_json_frame(9, 0, 1, b"{}");
        assert_eq!(&frame[..4], &[0x11, 0x90, 0x11, 0]);
        let mut response = frame[..4].to_vec();
        response.extend_from_slice(&frame[8..12]);
        response.extend_from_slice(&frame[12..]);
        let (last, sequence, payload) = decode_json_frame(&response).unwrap();
        assert!(!last && sequence == 0 && payload == b"{}");
        let mut final_response = response.clone();
        final_response[1] |= 2;
        assert!(decode_json_frame(&final_response).unwrap().0);
        let mut invalid = response;
        invalid[2] = 0x10;
        assert!(decode_json_frame(&invalid).is_none());
    }
}
