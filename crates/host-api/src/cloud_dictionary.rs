use msime_client_core::{has_disallowed_control_with_options, is_bounded_text, is_bounded_utf16};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CloudDictionaryRequest {
    SnapshotPreview,
    SnapshotExport,
    SnapshotRestorePreview {
        text: String,
    },
    SnapshotRestore {
        text: String,
        expected_sha256: String,
        revision: i64,
    },
    SnapshotRestoreNative {
        token: String,
    },
    SnapshotRestoreCancel,
    SnapshotChooseRestore,
    SnapshotEnqueue {
        token: String,
    },
    SnapshotStatus,
    SnapshotCancel,
    List {
        kind: String,
        offset: usize,
        search: String,
    },
    Catalog {
        kind: String,
        code: String,
        offset: usize,
        scheme: String,
        profile: String,
    },
    Changes {
        after: i64,
        limit: usize,
    },
    Add {
        kind: String,
        code: String,
        word: String,
        weight: i64,
    },
    Update {
        kind: String,
        id: String,
        code: String,
        word: String,
        weight: i64,
        revision: i64,
    },
    EditCatalog {
        kind: String,
        code: String,
        word: String,
        revision: i64,
        replacement: Option<CloudDictionaryValue>,
    },
    Candidates {
        text: String,
        kind: String,
        scheme: String,
        profile: String,
        limit: usize,
    },
    Rank {
        text: String,
        kind: String,
        scheme: String,
        profile: String,
        limit: usize,
        code: String,
        word: String,
        revision: i64,
        mode: String,
        linear_step: i64,
        trigger_count: i64,
        force_top: bool,
    },
    RemoveCandidate {
        text: String,
        kind: String,
        scheme: String,
        profile: String,
        limit: usize,
        code: String,
        word: String,
        revision: i64,
    },
    FixedPositions {
        context: String,
        offset: usize,
    },
    SetFixedPosition {
        context: String,
        code: String,
        word: String,
        position: Option<i64>,
        revision: i64,
    },
    Delete {
        kind: String,
        id: String,
        revision: i64,
    },
    Import {
        kind: String,
        format: String,
        text: String,
    },
    Export {
        kind: String,
        format: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CloudDictionaryValue {
    pub code: String,
    pub word: String,
    pub weight: i64,
}

pub fn validate_cloud_request(request: &CloudDictionaryRequest) -> Result<(), &'static str> {
    let valid_kind =
        |kind: &str| matches!(kind, "pinyin" | "wubi" | "wubi98" | "quick" | "english");
    let valid_token = |token: &str| msime_client_core::is_bounded_ascii_identifier(token, 96);
    let valid_value = |kind: &str, code: &str, word: &str, weight: i64| {
        let max_code_bytes = match kind {
            "wubi" | "wubi98" => 4,
            "quick" => 32,
            "english" => 64,
            _ => 256,
        };
        let code_alphabet_ok = match kind {
            "quick" => {
                msime_client_core::dictionary::quick_phrase_transport_code_is_well_formed(code)
            }
            "wubi" | "wubi98" => msime_client_core::dictionary::wubi_code_is_well_formed(code),
            "english" => msime_client_core::is_ascii_alphabetic(code),
            _ => msime_client_core::dictionary::pinyin_code_is_well_formed(code, true),
        };
        code_alphabet_ok
            && !code.is_empty()
            && is_bounded_text(code, max_code_bytes)
            && !word.is_empty()
            && is_bounded_text(word, 1024)
            && weight >= 0
            && (kind != "quick"
                || is_bounded_utf16(
                    word,
                    msime_client_core::dictionary::import::MAX_QUICK_PHRASE_UTF16,
                ))
    };
    let valid_format = |kind: &str, format: &str| {
        matches!(format, "standard" | "windows") || (kind == "pinyin" && format == "hans")
    };
    match request {
        CloudDictionaryRequest::SnapshotPreview
        | CloudDictionaryRequest::SnapshotExport
        | CloudDictionaryRequest::SnapshotStatus
        | CloudDictionaryRequest::SnapshotRestoreCancel
        | CloudDictionaryRequest::SnapshotChooseRestore
        | CloudDictionaryRequest::SnapshotCancel => Ok(()),
        CloudDictionaryRequest::SnapshotRestorePreview { text } => {
            if valid_snapshot_text(text) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::SnapshotRestore {
            text,
            expected_sha256,
            revision,
        } => {
            if valid_snapshot_text(text) && crate::valid_sha256(expected_sha256) && *revision >= 0 {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::SnapshotRestoreNative { token } => {
            if valid_token(token) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::SnapshotEnqueue { token } => {
            if valid_token(token) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::List {
            kind,
            offset,
            search,
        } => {
            if valid_kind(kind) && *offset <= 1_000_000 && is_bounded_text(search, 1024) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Catalog {
            kind,
            code,
            offset,
            scheme,
            profile,
        } => {
            if valid_kind(kind)
                && *offset <= 1_000_000
                && code.len() <= 256
                && !code.contains('\0')
                && !scheme.is_empty()
                && !profile.is_empty()
                && is_bounded_text(scheme, 64)
                && is_bounded_text(profile, 64)
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Changes { after, limit } => {
            if *after >= 0 && (1..=100).contains(limit) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Add {
            kind,
            code,
            word,
            weight,
        } => {
            if valid_kind(kind) && valid_value(kind, code, word, *weight) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Update {
            kind,
            id,
            code,
            word,
            weight,
            revision,
        } => {
            if valid_kind(kind)
                && msime_client_core::is_lower_hex(id, 64)
                && valid_value(kind, code, word, *weight)
                && *revision > 0
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Delete { kind, id, revision } => {
            if valid_kind(kind) && msime_client_core::is_lower_hex(id, 64) && *revision > 0 {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::EditCatalog {
            kind,
            code,
            word,
            revision,
            replacement,
        } => {
            let identity_ok = valid_kind(kind)
                && !code.is_empty()
                && is_bounded_text(code, 256)
                && !word.is_empty()
                && is_bounded_text(word, 1024)
                && *revision >= 0;
            let replacement_ok = replacement
                .as_ref()
                .is_none_or(|value| valid_value(kind, &value.code, &value.word, value.weight));
            if identity_ok && replacement_ok {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Candidates {
            text,
            kind,
            scheme,
            profile,
            limit,
        } => {
            if msime_client_core::cloud::dictionary::valid_candidate_query(
                text, kind, scheme, profile, *limit,
            ) {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Rank {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
            mode,
            linear_step,
            trigger_count,
            ..
        } => {
            if msime_client_core::cloud::dictionary::valid_candidate_query(
                text, kind, scheme, profile, *limit,
            ) && msime_client_core::cloud::dictionary::valid_candidate_value(code, word)
                && *revision >= 0
                && kind != "quick"
                && matches!(
                    mode.as_str(),
                    "disabled" | "pin" | "halve" | "linear" | "promote"
                )
                && (1..=100).contains(linear_step)
                && (1..=10).contains(trigger_count)
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::RemoveCandidate {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
        } => {
            if msime_client_core::cloud::dictionary::valid_candidate_query(
                text, kind, scheme, profile, *limit,
            ) && msime_client_core::cloud::dictionary::valid_candidate_value(code, word)
                && *revision >= 0
                && kind != "quick"
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::FixedPositions { context, offset } => {
            if is_bounded_text(context, 1024) && *offset <= 1_000_000 {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::SetFixedPosition {
            context,
            code,
            word,
            position,
            revision,
        } => {
            if is_bounded_text(context, 1024)
                && is_bounded_text(code, 256)
                && is_bounded_text(word, 1024)
                && !code.is_empty()
                && !word.is_empty()
                && *revision >= 0
                && position.is_none_or(|value| (1..=5).contains(&value))
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Import { kind, format, text } => {
            if valid_kind(kind)
                && valid_format(kind, format)
                && !text.is_empty()
                && text.len() <= msime_client_core::cloud::dictionary::MAX_IMPORT_BYTES
                && !has_disallowed_control_with_options(text, true)
            {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
        CloudDictionaryRequest::Export { kind, format } => {
            if valid_kind(kind) && matches!(format.as_str(), "standard" | "windows") {
                Ok(())
            } else {
                Err("invalid cloud dictionary request")
            }
        }
    }
}

fn valid_snapshot_text(value: &str) -> bool {
    const MAX_SNAPSHOT_BYTES: usize = 512 * 1024 * 1024;
    !value.is_empty()
        && value.len() <= MAX_SNAPSHOT_BYTES
        && !has_disallowed_control_with_options(value, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_snapshot_restore_cancel_operation() {
        let request: CloudDictionaryRequest = serde_json::from_value(serde_json::json!({
            "operation": "snapshot_restore_cancel"
        }))
        .expect("snapshot restore cancellation is part of the host protocol");
        assert!(matches!(
            request,
            CloudDictionaryRequest::SnapshotRestoreCancel
        ));
    }

    #[test]
    fn validates_dictionary_values_and_entry_identity() {
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotPreview).is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotStatus).is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotRestoreCancel).is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotChooseRestore).is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotCancel).is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::SnapshotExport).is_ok());
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotRestorePreview {
                text: "{\"type\":\"header\"}\n".into(),
            })
            .is_ok()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotRestore {
                text: "{\"type\":\"header\"}\n".into(),
                expected_sha256: "a".repeat(64),
                revision: 0,
            })
            .is_ok()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotRestore {
                text: "bad\u{0007}".into(),
                expected_sha256: "a".repeat(64),
                revision: 0,
            })
            .is_err()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotRestoreNative {
                token: "native-preview-token".into(),
            })
            .is_ok()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotEnqueue {
                token: "a-token".into(),
            })
            .is_ok()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SnapshotEnqueue {
                token: "bad token".into(),
            })
            .is_err()
        );
        assert!(validate_cloud_request(&CloudDictionaryRequest::Add {
            kind: "pinyin".into(),
            code: "ni".into(),
            word: "你".into(),
            weight: 100,
        })
        .is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Add {
            kind: "pinyin".into(),
            code: "".into(),
            word: "你".into(),
            weight: 100,
        })
        .is_err());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Delete {
            kind: "pinyin".into(),
            id: "a".repeat(64),
            revision: 2,
        })
        .is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Delete {
            kind: "pinyin".into(),
            id: "bad".into(),
            revision: 2,
        })
        .is_err());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Delete {
            kind: "pinyin".into(),
            id: "A".repeat(64),
            revision: 2,
        })
        .is_err());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Catalog {
            kind: "pinyin".into(),
            code: "nihc".into(),
            offset: 0,
            scheme: "shuangpin".into(),
            profile: "xiaohe".into(),
        })
        .is_ok());
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::EditCatalog {
                kind: "pinyin".into(),
                code: "ni".into(),
                word: "你".into(),
                revision: 42,
                replacement: None,
            })
            .is_ok()
        );
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::EditCatalog {
                kind: "pinyin".into(),
                code: "ni".into(),
                word: "你".into(),
                revision: 42,
                replacement: Some(CloudDictionaryValue {
                    code: "ni".into(),
                    word: "你".into(),
                    weight: 1,
                }),
            })
            .is_ok()
        );
        assert!(validate_cloud_request(&CloudDictionaryRequest::Candidates {
            text: "nihc".into(),
            kind: "pinyin".into(),
            scheme: "shuangpin".into(),
            profile: "xiaohe".into(),
            limit: 100,
        })
        .is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Rank {
            text: "nihc".into(),
            kind: "pinyin".into(),
            scheme: "shuangpin".into(),
            profile: "xiaohe".into(),
            limit: 100,
            code: "ni'hao".into(),
            word: "你好".into(),
            revision: 42,
            mode: "pin".into(),
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
        })
        .is_ok());
        assert!(
            validate_cloud_request(&CloudDictionaryRequest::SetFixedPosition {
                context: "server:context".into(),
                code: "ni'hao".into(),
                word: "你好".into(),
                position: None,
                revision: 42,
            })
            .is_ok()
        );
        assert!(validate_cloud_request(&CloudDictionaryRequest::Rank {
            text: "nihc".into(),
            kind: "quick".into(),
            scheme: "pinyin".into(),
            profile: "xiaohe".into(),
            limit: 100,
            code: "k".into(),
            word: "短语".into(),
            revision: 42,
            mode: "pin".into(),
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
        })
        .is_err());
    }

    #[test]
    fn enforces_quick_phrase_utf16_limit() {
        let valid = "界".repeat(199);
        let invalid = "界".repeat(200);
        let request = |word| CloudDictionaryRequest::Add {
            kind: "quick".into(),
            code: "k".into(),
            word,
            weight: 1,
        };
        assert!(validate_cloud_request(&request(valid)).is_ok());
        assert!(validate_cloud_request(&request(invalid)).is_err());
    }

    #[test]
    fn rejects_invalid_quick_phrase_code_alphabet() {
        let request = |code| CloudDictionaryRequest::Add {
            kind: "quick".into(),
            code,
            word: "短语".into(),
            weight: 1,
        };
        assert!(validate_cloud_request(&request("k2".into())).is_ok());
        assert!(validate_cloud_request(&request("K2".into())).is_err());
        assert!(validate_cloud_request(&request("k-2".into())).is_err());
    }

    #[test]
    fn validates_import_and_export_formats() {
        assert!(validate_cloud_request(&CloudDictionaryRequest::Import {
            kind: "pinyin".into(),
            format: "hans".into(),
            text: "你好".into(),
        })
        .is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Import {
            kind: "wubi".into(),
            format: "hans".into(),
            text: "你好".into(),
        })
        .is_err());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Import {
            kind: "pinyin".into(),
            format: "standard".into(),
            text: "bad\u{0007}".into(),
        })
        .is_err());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Export {
            kind: "english".into(),
            format: "windows".into(),
        })
        .is_ok());
        assert!(validate_cloud_request(&CloudDictionaryRequest::Export {
            kind: "english".into(),
            format: "hans".into(),
        })
        .is_err());
    }
}
