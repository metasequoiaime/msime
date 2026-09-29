//! Validation primitives shared by native and mobile dictionary snapshot readers.

use serde::de::{DeserializeSeed, MapAccess, Visitor};
use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum SnapshotValidationError {
    #[error("invalid snapshot object")]
    Invalid,
}

/// Parse one snapshot record while rejecting JSON constructs the snapshot
/// format does not permit: arrays, floating point values, duplicate keys and
/// objects nested more than one level deep.
pub fn parse_strict_object(bytes: &[u8]) -> Result<Map<String, Value>, SnapshotValidationError> {
    struct StrictValue {
        depth: usize,
    }

    impl<'de> DeserializeSeed<'de> for StrictValue {
        type Value = Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct ValueVisitor {
                depth: usize,
            }

            impl<'de> Visitor<'de> for ValueVisitor {
                type Value = Value;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a strict JSON object value")
                }

                fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                    Ok(Value::Bool(value))
                }

                fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                    Ok(Value::Number(value.into()))
                }

                fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                    Ok(Value::Number(value.into()))
                }

                fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Err(E::custom("floating point values are not allowed"))
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                    Ok(Value::String(value.to_owned()))
                }

                fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                    Ok(Value::String(value))
                }

                fn visit_none<E>(self) -> Result<Self::Value, E> {
                    Ok(Value::Null)
                }

                fn visit_unit<E>(self) -> Result<Self::Value, E> {
                    Ok(Value::Null)
                }

                fn visit_seq<A>(self, _sequence: A) -> Result<Self::Value, A::Error>
                where
                    A: serde::de::SeqAccess<'de>,
                {
                    Err(serde::de::Error::custom("arrays are not allowed"))
                }

                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: MapAccess<'de>,
                {
                    if self.depth > 1 {
                        return Err(serde::de::Error::custom("nested objects are not allowed"));
                    }
                    let mut object = Map::new();
                    while let Some(key) = map.next_key::<String>()? {
                        if object.contains_key(&key) {
                            return Err(serde::de::Error::custom("duplicate JSON key"));
                        }
                        let value = map.next_value_seed(StrictValue {
                            depth: self.depth + 1,
                        })?;
                        object.insert(key, value);
                    }
                    Ok(Value::Object(object))
                }
            }

            deserializer.deserialize_any(ValueVisitor { depth: self.depth })
        }
    }

    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|_| SnapshotValidationError::Invalid)?;
    deserializer
        .end()
        .map_err(|_| SnapshotValidationError::Invalid)?;
    value
        .as_object()
        .cloned()
        .ok_or(SnapshotValidationError::Invalid)
}

pub fn has_keys(map: &Map<String, Value>, keys: &[&str]) -> bool {
    map.len() == keys.len() && keys.iter().all(|key| map.contains_key(*key))
}

pub fn required_text<'a, E>(
    map: &'a Map<String, Value>,
    key: &str,
    maximum_bytes: usize,
    error: E,
) -> Result<&'a str, E> {
    map.get(key)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= maximum_bytes
                && !value
                    .bytes()
                    .any(|byte| matches!(byte, 0 | b'\t' | b'\n' | b'\r'))
        })
        .ok_or(error)
}

pub fn required_integer<E>(map: &Map<String, Value>, key: &str, error: E) -> Result<i64, E> {
    map.get(key).and_then(Value::as_i64).ok_or(error)
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
    use super::{parse_strict_object, valid_timestamp};

    #[test]
    fn strict_snapshot_objects_reject_ambiguous_json() {
        assert!(parse_strict_object(br#"{"type":"header"}"#).is_ok());
        assert!(parse_strict_object(br#"{"type":1.5}"#).is_err());
        assert!(parse_strict_object(br#"{"type":[]}"#).is_err());
        assert!(parse_strict_object(br#"{"type":1,"type":2}"#).is_err());
        assert!(parse_strict_object(br#"{"data":{"nested":{"too_deep":true}}}"#).is_err());
    }

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
