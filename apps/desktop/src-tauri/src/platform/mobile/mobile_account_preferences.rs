use msime_client_core::account::AccountPreferenceValue;
use msime_client_core::preferences::FrequencyPreferences;
use std::collections::BTreeMap;

pub(crate) fn insert_string(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    key: &str,
    value: &str,
) {
    settings.insert(
        key.to_owned(),
        AccountPreferenceValue::String(value.to_owned()),
    );
}

pub(crate) fn insert_bool(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    key: &str,
    value: bool,
) {
    settings.insert(key.to_owned(), AccountPreferenceValue::Boolean(value));
}

pub(crate) fn insert_integer(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    key: &str,
    value: i64,
) {
    settings.insert(key.to_owned(), AccountPreferenceValue::Integer(value));
}

pub(crate) fn frequency_account_preferences(
    frequency: &FrequencyPreferences,
) -> BTreeMap<String, AccountPreferenceValue> {
    BTreeMap::from([
        (
            "input.frequency_mode".into(),
            AccountPreferenceValue::String(frequency.mode.as_str().into()),
        ),
        (
            "input.frequency_trigger_count".into(),
            AccountPreferenceValue::Integer(i64::from(frequency.trigger_count)),
        ),
        (
            "input.frequency_linear_step".into(),
            AccountPreferenceValue::Integer(i64::from(frequency.linear_step)),
        ),
    ])
}
