//! Validation primitives shared by native and mobile dictionary snapshot readers.

use serde_json::{Map, Value};

pub fn has_keys(map: &Map<String, Value>, keys: &[&str]) -> bool {
    map.len() == keys.len() && keys.iter().all(|key| map.contains_key(*key))
}

pub fn valid_timestamp(value: &str) -> bool {
    fn digits(bytes: &[u8], start: usize, end: usize) -> Option<u32> {
        (end <= bytes.len() && bytes[start..end].iter().all(u8::is_ascii_digit)).then(|| {
            bytes[start..end]
                .iter()
                .fold(0, |value, byte| value * 10 + u32::from(byte - b'0'))
        })
    }

    let bytes = value.as_bytes();
    if bytes.len() < 20
        || digits(bytes, 0, 4).is_none()
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return false;
    }
    let year = digits(bytes, 0, 4).unwrap();
    let month = match digits(bytes, 5, 7) {
        Some(value) => value,
        None => return false,
    };
    let day = match digits(bytes, 8, 10) {
        Some(value) => value,
        None => return false,
    };
    let hour = match digits(bytes, 11, 13) {
        Some(value) => value,
        None => return false,
    };
    let minute = match digits(bytes, 14, 16) {
        Some(value) => value,
        None => return false,
    };
    let second = match digits(bytes, 17, 19) {
        Some(value) => value,
        None => return false,
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days[month as usize - 1]
        || hour >= 24
        || minute >= 60
        || second >= 60
    {
        return false;
    }
    let mut offset = 19;
    if matches!(bytes.get(offset), Some(b'.' | b',')) {
        offset += 1;
        let start = offset;
        while bytes.get(offset).is_some_and(u8::is_ascii_digit) {
            offset += 1;
        }
        if offset == start {
            return false;
        }
    }
    let zone = &bytes[offset..];
    if zone == b"Z" {
        return true;
    }
    if zone.len() != 6
        || !matches!(zone[0], b'+' | b'-')
        || !zone[1].is_ascii_digit()
        || !zone[2].is_ascii_digit()
        || zone[3] != b':'
        || !zone[4].is_ascii_digit()
        || !zone[5].is_ascii_digit()
    {
        return false;
    }
    let zone_hour = u32::from(zone[1] - b'0') * 10 + u32::from(zone[2] - b'0');
    let zone_minute = u32::from(zone[4] - b'0') * 10 + u32::from(zone[5] - b'0');
    zone_hour < 24 && zone_minute < 60
}

#[cfg(test)]
mod tests {
    use super::valid_timestamp;

    #[test]
    fn accepts_bounded_iso_timestamps() {
        assert!(valid_timestamp("2026-09-28T12:34:56Z"));
        assert!(valid_timestamp("2024-02-29T12:34:56.123+09:00"));
    }

    #[test]
    fn rejects_invalid_dates_and_zones() {
        assert!(!valid_timestamp("2023-02-29T12:34:56Z"));
        assert!(!valid_timestamp("2026-09-28T12:34:56+24:00"));
    }
}
