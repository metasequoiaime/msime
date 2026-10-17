//! `input.wubi_schema` 的应用：有就写进共享偏好，不管云端的方案是什么（#8201 的决定 10），与 Android、桌面和鸿蒙的映射相同。

use super::IosPreferencePlan;
use crate::account::{AccountError, AccountPreferenceValue, AccountPreferences};
use msime_client_core::ios_keyboard::IosKeyboardPreferences;
use msime_client_core::preferences::{Preferences, WubiProfile};
use std::collections::BTreeMap;

fn native(scheme: &str) -> IosKeyboardPreferences {
    IosKeyboardPreferences {
        input_scheme: scheme.into(),
        traditional_chinese_output: false,
        sound_enabled: true,
        haptics_enabled: true,
        haptic_strength: "medium".into(),
        english_suggestions: true,
        candidate_palette_follows_desktop: false,
        inline_preedit: false,
        haptics_available: true,
        tablet_full_keys: None,
        tablet_split_keyboard: None,
        dictionary_learning: true,
        global_theme: "system".into(),
        custom_keyboard_skin: None,
    }
}

fn document(schema: Option<&str>, wubi: Option<&str>) -> AccountPreferences {
    let mut settings = BTreeMap::new();
    if let Some(schema) = schema {
        settings.insert(
            "input.schema".to_owned(),
            AccountPreferenceValue::String(schema.into()),
        );
    }
    if let Some(wubi) = wubi {
        settings.insert(
            "input.wubi_schema".to_owned(),
            AccountPreferenceValue::String(wubi.into()),
        );
    }
    AccountPreferences {
        revision: 7,
        settings,
    }
}

/// 按 iOS 的两步应用一份文档：先算出要写的键盘设置，再按它应用到共享偏好。
fn apply(cloud: &AccountPreferences, current: &str, local: &Preferences) -> Preferences {
    let plan = IosPreferencePlan::from_cloud(cloud).unwrap();
    let requested = plan.requested_native(&native(current)).unwrap();
    let mut preferences = local.clone();
    plan.apply_shared(&requested, &mut preferences).unwrap();
    preferences
}

#[test]
fn the_account_wubi_profile_is_written_whatever_the_cloud_scheme_is() {
    let local = Preferences::default();
    assert_eq!(local.wubi_profile, WubiProfile::Wubi86);
    // 云端的方案是全拼、双拼、日文，没有方案，或者是 iOS 没有的粤拼：账号里别的设备选的 98 都写进本机。
    for schema in [
        Some("quanpin"),
        Some("shuangpin"),
        Some("japanese"),
        None,
        Some("cantonese"),
    ] {
        let applied = apply(&document(schema, Some("wubi98")), "quanpin", &local);
        assert_eq!(applied.wubi_profile, WubiProfile::Wubi98, "{schema:?}");
    }
    // 本机是账号装不下的方案（粤拼）时方案保留，五笔版本照样跟着账号。
    let applied = apply(
        &document(Some("quanpin"), Some("wubi98")),
        "cantonese",
        &local,
    );
    assert_eq!(applied.wubi_profile, WubiProfile::Wubi98);
    // 反过来也一样：本机是 98，账号里是 86。
    let wubi98 = Preferences {
        wubi_profile: WubiProfile::Wubi98,
        ..Preferences::default()
    };
    let applied = apply(
        &document(Some("quanpin"), Some("wubi86")),
        "quanpin",
        &wubi98,
    );
    assert_eq!(applied.wubi_profile, WubiProfile::Wubi86);
}

#[test]
fn a_document_without_the_wubi_profile_keeps_the_local_one() {
    let wubi98 = Preferences {
        wubi_profile: WubiProfile::Wubi98,
        ..Preferences::default()
    };
    for schema in [Some("quanpin"), Some("wubi"), None] {
        let applied = apply(&document(schema, None), "quanpin", &wubi98);
        assert_eq!(applied.wubi_profile, WubiProfile::Wubi98, "{schema:?}");
    }
}

#[test]
fn an_unknown_wubi_profile_rejects_the_document_whatever_the_scheme() {
    // 现在总是读这个键，就和 iOS 读的其余键一样：不认识的取值拒绝整份文档，不只在方案是五笔时。
    for schema in [Some("wubi"), Some("quanpin"), None] {
        assert_eq!(
            IosPreferencePlan::from_cloud(&document(schema, Some("wubi06"))),
            Err(AccountError::Invalid),
            "{schema:?}"
        );
    }
}
