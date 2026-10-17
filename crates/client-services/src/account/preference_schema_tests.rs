//! 偏好字段表的两种键名（#8201 的 P5）：服务端的 snake_case 和 `preferences_schema` 交给宿主的 camelCase 都能读回同一份字段表。

use super::page::PreferenceSchemaResponse;
use super::*;
use serde_json::json;

fn schema() -> AccountPreferenceSchema {
    AccountPreferenceSchema {
        fields: BTreeMap::from([
            (
                "input.schema".to_owned(),
                AccountPreferenceField {
                    value_type: "string".into(),
                },
            ),
            (
                "input.learning".to_owned(),
                AccountPreferenceField {
                    value_type: "boolean".into(),
                },
            ),
        ]),
        maximum_bytes: 1_048_576,
        update_mode: "replace".into(),
        revision_required: true,
    }
}

#[test]
fn what_the_settings_page_reads_is_read_back_as_the_same_schema() {
    let page = serde_json::to_value(PreferenceSchemaResponse::from(schema())).unwrap();
    assert_eq!(page["maximumBytes"], 1_048_576);
    let read: AccountPreferenceSchema = serde_json::from_value(page).unwrap();
    assert_eq!(read, schema());
}

#[test]
fn the_server_shape_still_reads_and_is_still_written() {
    let server = json!({
        "fields": { "input.schema": { "type": "string", "maxLength": 32 }, "input.learning": { "type": "boolean" } },
        "maximum_bytes": 1_048_576,
        "update_mode": "replace",
        "revision_required": true,
    });
    let read: AccountPreferenceSchema = serde_json::from_value(server).unwrap();
    assert_eq!(read, schema());
    let written = serde_json::to_value(&read).unwrap();
    assert_eq!(written["maximum_bytes"], 1_048_576);
    assert!(written.get("maximumBytes").is_none());
}

#[test]
fn one_key_given_in_both_spellings_is_refused() {
    let both = json!({
        "fields": {},
        "maximum_bytes": 1_048_576,
        "maximumBytes": 1_048_576,
        "update_mode": "replace",
        "revision_required": true,
    });
    assert!(serde_json::from_value::<AccountPreferenceSchema>(both).is_err());
}
