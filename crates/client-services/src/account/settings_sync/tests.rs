use super::*;
use crate::account::AccountPreferenceField;
use crate::preferences::FrequencyPreferences;

fn schema_for(fields: &[(&str, &str)]) -> AccountPreferenceSchema {
    AccountPreferenceSchema {
        fields: fields
            .iter()
            .map(|(key, kind)| {
                (
                    (*key).to_owned(),
                    AccountPreferenceField {
                        value_type: (*kind).to_owned(),
                    },
                )
            })
            .collect(),
        maximum_bytes: 65_536,
        update_mode: "replace".into(),
        revision_required: true,
    }
}

fn feedback() -> HostKeyboardFeedback {
    HostKeyboardFeedback {
        sound_enabled: true,
        haptics_enabled: false,
        haptic_strength: "medium".into(),
    }
}

/// 字段表收录导出的全部键，类型取导出值的类型。
fn full_schema() -> AccountPreferenceSchema {
    let exported = export_android_settings(&Preferences::default(), Some(&feedback())).unwrap();
    let mut schema = schema_for(&[]);
    for (key, value) in exported {
        schema.fields.insert(
            key,
            AccountPreferenceField {
                value_type: value.kind().to_owned(),
            },
        );
    }
    // 只随五笔方案导出的键也要在字段表里。
    schema.fields.insert(
        "input.wubi_schema".into(),
        AccountPreferenceField {
            value_type: "string".into(),
        },
    );
    schema
}

fn document(settings: BTreeMap<String, AccountPreferenceValue>) -> AccountPreferences {
    AccountPreferences {
        revision: 3,
        settings,
    }
}

fn frequency_values(frequency: &FrequencyPreferences) -> BTreeMap<String, AccountPreferenceValue> {
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

fn frequency_schema() -> AccountPreferenceSchema {
    schema_for(&[
        ("input.frequency_mode", "string"),
        ("input.frequency_trigger_count", "integer"),
        ("input.frequency_linear_step", "integer"),
    ])
}

fn apply(
    local: &Preferences,
    settings: BTreeMap<String, AccountPreferenceValue>,
    schema: &AccountPreferenceSchema,
) -> AndroidSettingsApplied {
    apply_android_settings(local, &document(settings), schema, Some(feedback())).unwrap()
}

#[test]
fn settings_sync_export_is_exactly_the_shared_android_keys() {
    let mut preferences = Preferences {
        scheme: InputScheme::Wubi,
        ..Preferences::default()
    };
    preferences.voice_input.asr_token = "SENTINEL-asr".into();
    preferences.voice_input.polish_token = "SENTINEL-polish".into();
    preferences
        .voice_input
        .asr_tokens
        .insert("doubao".into(), "SENTINEL-asr-map".into());
    preferences.ai_assistant.token = "SENTINEL-ai".into();
    preferences.custom_translation.api_key = "SENTINEL-translation".into();
    preferences.tencent_tmt.secret_id = "SENTINEL-secret-id".into();
    preferences.tencent_tmt.secret_key = "SENTINEL-secret-key".into();
    // 云剪贴板中继是本机的配置，地址和令牌都不进同步文档。
    preferences.cloud_clipboard_relay.url = "https://SENTINEL-relay.example.test".into();
    preferences.cloud_clipboard_relay.token = "SENTINEL-relay-token".into();
    let exported = export_android_settings(&preferences, Some(&feedback())).unwrap();
    let keys: Vec<&str> = exported.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "input.bracket_style",
            "input.character_set",
            "input.chinese_punctuation",
            "input.enumeration_comma_backslash",
            "input.enumeration_comma_slash",
            "input.frequency_linear_step",
            "input.frequency_mode",
            "input.frequency_trigger_count",
            "input.learning",
            "input.paired_punctuation",
            "input.quote_style",
            "input.schema",
            "input.shuangpin_schema",
            "input.smart_punctuation",
            "input.traditional_variant",
            "input.wubi_auto_commit_unique",
            "input.wubi_code_hint",
            "input.wubi_fifth_code_top_commit",
            "input.wubi_schema",
            "platform.android.custom_candidate_skin",
            "platform.android.custom_candidate_skin_dark",
            "platform.android.custom_keyboard_skin",
            "platform.android.custom_theme_base",
            "platform.android.global_theme",
            "platform.android.haptic_strength",
            "platform.android.haptics_enabled",
            "platform.android.key_sound_pack",
            "platform.android.keyboard_height_adjustment",
            "platform.android.keyboard_layout",
            "platform.android.number_keypad_order",
            "platform.android.shuangpin_key_hints",
            "platform.android.sound_enabled",
            "platform.android.theme",
            "platform.android.toolbar_ai",
            "platform.android.toolbar_character_set",
            "platform.android.toolbar_clipboard",
            "platform.android.toolbar_emoji",
            "platform.android.toolbar_fullwidth",
            "platform.android.toolbar_layout",
            "platform.android.toolbar_punctuation",
            "platform.android.toolbar_skin",
            "platform.android.touch_key_spacing_tenths",
            "platform.android.touch_row_spacing_tenths",
            "platform.android.twenty_six_key_number_layout",
            "platform.android.voice_language",
            "platform.android.voice_shortcut",
        ]
    );
    // 设备本地、涉及隐私的设置和任何凭据都不导出，键名和取值里都不出现。
    for key in exported.keys() {
        for forbidden in [
            "incognito",
            "developer",
            "diagnostic",
            "contribute",
            "token",
            "secret",
            "password",
            "api_key",
            "helpcode_schema",
        ] {
            assert!(!key.contains(forbidden), "{key}");
        }
    }
    for value in exported.values() {
        if let AccountPreferenceValue::String(value) = value {
            assert!(!value.contains("SENTINEL"), "{value}");
        }
    }
    // 没有宿主按键反馈时不导出那三个键。
    let without = export_android_settings(&preferences, None).unwrap();
    assert!(ANDROID_FEEDBACK_KEYS
        .iter()
        .all(|key| !without.contains_key(*key)));
    assert_eq!(without.len(), exported.len() - 3);
}

/// 账号既没有 `custom` 这个取值也装不下用户的键位表：选着自定义方案时不上传双拼方案，账号里的值保留；账号里出现 `custom`（例如更新的客户端写的）也不应用，本机的表不会被一个空表的 `custom` 顶掉。
#[test]
fn settings_sync_leaves_a_custom_shuangpin_profile_on_the_device() {
    let mut local = Preferences {
        shuangpin_profile: ShuangpinProfile::Custom,
        ..Preferences::default()
    };
    local
        .shuangpin_custom_profile
        .finals
        .insert("ing".into(), ";".into());
    let exported = export_android_settings(&local, None).unwrap();
    assert!(!exported.contains_key("input.shuangpin_schema"));
    assert!(exported.contains_key("input.character_set"));

    let mut values = BTreeMap::new();
    values.insert(
        "input.shuangpin_schema".into(),
        AccountPreferenceValue::String("custom".into()),
    );
    let applied = apply(
        &Preferences::default(),
        values,
        &schema_for(&[("input.shuangpin_schema", "string")]),
    );
    assert_eq!(
        applied.preferences.shuangpin_profile,
        ShuangpinProfile::Xiaohe
    );
    assert_eq!(applied.skipped, ["input.shuangpin_schema"]);

    // 反过来，本机选着自定义方案、账号里留着上次的内置方案：合并上传后应用回来时本机仍是自定义方案，表也还在。
    let mut values = BTreeMap::new();
    values.insert(
        "input.shuangpin_schema".into(),
        AccountPreferenceValue::String("xiaohe".into()),
    );
    let applied = apply(
        &local,
        values,
        &schema_for(&[("input.shuangpin_schema", "string")]),
    );
    assert_eq!(
        applied.preferences.shuangpin_profile,
        ShuangpinProfile::Custom
    );
    assert_eq!(
        applied.preferences.shuangpin_custom_profile,
        local.shuangpin_custom_profile
    );
    assert!(applied.skipped.is_empty());
}

#[test]
fn settings_sync_preserves_a_local_smart_abc_profile() {
    let local = Preferences {
        shuangpin_profile: ShuangpinProfile::SmartAbc,
        ..Preferences::default()
    };
    let exported = export_android_settings(&local, None).unwrap();
    assert!(!exported.contains_key("input.shuangpin_schema"));
    let values = BTreeMap::from([(
        "input.shuangpin_schema".into(),
        AccountPreferenceValue::String("microsoft".into()),
    )]);
    let applied = apply(
        &local,
        values,
        &schema_for(&[("input.shuangpin_schema", "string")]),
    );
    assert_eq!(
        applied.preferences.shuangpin_profile,
        ShuangpinProfile::SmartAbc
    );
    assert!(applied.skipped.is_empty());
}

#[test]
fn settings_sync_round_trips_every_exported_key() {
    let mut expected = Preferences {
        scheme: InputScheme::Wubi,
        wubi_profile: WubiProfile::Wubi98,
        shuangpin_profile: ShuangpinProfile::Ziranma,
        traditional_chinese_output: true,
        learning: false,
        chinese_punctuation: false,
        smart_punctuation: !Preferences::default().smart_punctuation,
        // 智能标点的值和本机不同，应用时两半跟着写成同一个值。
        smart_punctuation_direct_digit: !Preferences::default().smart_punctuation,
        smart_punctuation_direct_letter: !Preferences::default().smart_punctuation,
        paired_punctuation: !Preferences::default().paired_punctuation,
        quote_style: crate::preferences::QuoteStyle::CornerWhenTraditional,
        bracket_style: crate::preferences::BracketStyle::Hollow,
        wubi_code_hint: !Preferences::default().wubi_code_hint,
        wubi_auto_commit_unique: !Preferences::default().wubi_auto_commit_unique,
        wubi_fifth_code_top_commit: !Preferences::default().wubi_fifth_code_top_commit,
        touch_keyboard_layout: TouchKeyboardLayout::NineKey,
        theme: ThemeMode::Dark,
        touch_key_spacing_tenths: 40,
        touch_row_spacing_tenths: 60,
        touch_keyboard_height_adjustment: 10,
        touch_voice_shortcut: !Preferences::default().touch_voice_shortcut,
        touch_number_keypad_order: NumberKeypadOrder::Calculator,
        touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout::NineKey,
        touch_shuangpin_key_hints: !Preferences::default().touch_shuangpin_key_hints,
        ..Preferences::default()
    };
    expected.frequency = FrequencyPreferences {
        mode: FrequencyMode::Linear,
        trigger_count: 7,
        linear_step: 4,
    };
    expected.global_theme = GlobalTheme::Custom;
    expected.custom_theme.base = GlobalTheme::Night;
    expected.custom_theme.candidate_skin = Some("sakura".into());
    expected.custom_theme.candidate_skin_dark = Some("midnight".into());
    expected.validate().unwrap();
    let host = HostKeyboardFeedback {
        sound_enabled: false,
        haptics_enabled: true,
        haptic_strength: "strong".into(),
    };
    let exported = export_android_settings(&expected, Some(&host)).unwrap();
    let applied = apply(&Preferences::default(), exported, &full_schema());
    assert_eq!(applied.preferences, expected);
    assert_eq!(applied.feedback, Some(host));
    assert!(applied.skipped.is_empty());
}

/// 全拼 14 键和九键双拼的布局按 `fourteen_key`、`nine_key_shuangpin` 上传，读回来还是原来的布局，不会落成 26 键。九键双拼用哪张表是本机的设置，不上传。
#[test]
fn settings_sync_round_trips_the_fourteen_key_layout() {
    for (layout, id) in [
        (TouchKeyboardLayout::FourteenKey, "fourteen_key"),
        (TouchKeyboardLayout::NineKeyShuangpin, "nine_key_shuangpin"),
    ] {
        let expected = Preferences {
            touch_keyboard_layout: layout,
            ..Preferences::default()
        };
        let exported = export_android_settings(&expected, None).unwrap();
        assert_eq!(
            exported.get("platform.android.keyboard_layout"),
            Some(&AccountPreferenceValue::String(id.into()))
        );
        assert!(!exported
            .keys()
            .any(|key| key.contains("nine_key_shuangpin")));
        let applied = apply(&Preferences::default(), exported, &full_schema());
        assert_eq!(applied.preferences.touch_keyboard_layout, layout);
        assert!(applied.skipped.is_empty());
    }
}

#[test]
fn settings_sync_round_trips_the_eighteen_key_layout() {
    let expected = Preferences {
        touch_keyboard_layout: TouchKeyboardLayout::EighteenKey,
        ..Preferences::default()
    };
    let exported = export_android_settings(&expected, None).unwrap();
    assert_eq!(
        exported.get("platform.android.keyboard_layout"),
        Some(&AccountPreferenceValue::String("eighteen_key".into()))
    );
    let applied = apply(&Preferences::default(), exported, &full_schema());
    assert_eq!(
        applied.preferences.touch_keyboard_layout,
        TouchKeyboardLayout::EighteenKey
    );
    assert!(applied.skipped.is_empty());
}

/// 本机选着账号装不下的方案时不导出 `input.schema`，账号里留着上次的方案。把合并结果应用回来（上传之后写回本机）、或者应用别的设备上传的文档时，Android 和桌面都保留本机的方案，文档里其余的键照常应用。
#[test]
fn settings_sync_keeps_a_scheme_the_account_cannot_hold() {
    for scheme in [
        InputScheme::Cantonese,
        InputScheme::Zhuyin,
        InputScheme::Vietnamese,
        InputScheme::Tibetan,
        InputScheme::Stroke,
    ] {
        let local = Preferences {
            scheme,
            ..Preferences::default()
        };
        let exported = export_android_settings(&local, None).unwrap();
        assert!(!exported.contains_key("input.schema"), "{scheme:?}");
        let mut cloud = document(BTreeMap::from([(
            "input.schema".into(),
            AccountPreferenceValue::String("quanpin".into()),
        )]));
        cloud.settings.insert(
            "input.learning".into(),
            AccountPreferenceValue::Boolean(!local.learning),
        );
        let merged =
            crate::account::merge_account_preferences(&cloud, &exported, &full_schema()).unwrap();
        assert_eq!(
            merged.settings["input.schema"],
            AccountPreferenceValue::String("quanpin".into()),
            "{scheme:?}"
        );
        let applied = apply(&local, merged.settings, &full_schema());
        assert_eq!(applied.preferences.scheme, scheme, "{scheme:?}");
        assert!(applied.skipped.is_empty(), "{scheme:?}");

        // 别的设备上传的文档：方案保留，学习开关照常应用。
        let applied = apply_desktop_settings(&local, &cloud, &full_schema()).unwrap();
        assert_eq!(applied.preferences.scheme, scheme, "{scheme:?}");
        assert_eq!(applied.preferences.learning, !local.learning, "{scheme:?}");
        assert!(applied.skipped.is_empty(), "{scheme:?}");
    }
}

/// Android 上传 14 键，所以本机是 14 键时云端的布局照常应用；跳过 14 键只是鸿蒙的规则。
#[test]
fn settings_sync_android_applies_the_cloud_layout_over_fourteen_key() {
    let local = Preferences {
        touch_keyboard_layout: TouchKeyboardLayout::FourteenKey,
        ..Preferences::default()
    };
    let applied = apply(
        &local,
        BTreeMap::from([(
            "platform.android.keyboard_layout".into(),
            AccountPreferenceValue::String("nine_key".into()),
        )]),
        &full_schema(),
    );
    assert_eq!(
        applied.preferences.touch_keyboard_layout,
        TouchKeyboardLayout::NineKey
    );
}

#[test]
fn settings_sync_frequency_round_trips_and_unsupported_fields_are_ignored() {
    let expected = FrequencyPreferences {
        mode: FrequencyMode::Linear,
        trigger_count: 7,
        linear_step: 4,
    };
    let values = frequency_values(&expected);
    let applied = apply(&Preferences::default(), values.clone(), &frequency_schema());
    assert_eq!(applied.preferences.frequency, expected);

    // 字段表里没有的键不动本地设置，也不算跳过。
    let applied = apply(&Preferences::default(), values, &schema_for(&[]));
    assert_eq!(
        applied.preferences.frequency,
        FrequencyPreferences::default()
    );
    assert!(applied.skipped.is_empty());
}

#[test]
fn settings_sync_names_only_the_schemes_the_account_knows() {
    for (scheme, schema) in [
        (InputScheme::Quanpin, Some("quanpin")),
        (InputScheme::Shuangpin, Some("shuangpin")),
        (InputScheme::Wubi, Some("wubi")),
        (InputScheme::Japanese, Some("japanese")),
        (InputScheme::Korean, Some("korean")),
        (InputScheme::Cantonese, None),
        (InputScheme::Zhuyin, None),
        (InputScheme::Vietnamese, None),
        (InputScheme::Tibetan, None),
        (InputScheme::Stroke, None),
    ] {
        assert_eq!(account_input_schema(scheme), schema, "{scheme:?}");
        let preferences = Preferences {
            scheme,
            ..Preferences::default()
        };
        let exported = export_android_settings(&preferences, None).unwrap();
        assert_eq!(
            exported.get("input.schema"),
            schema
                .map(|value| AccountPreferenceValue::String(value.into()))
                .as_ref()
        );
        // 五笔版本只随五笔方案导出。
        assert_eq!(
            exported.contains_key("input.wubi_schema"),
            scheme == InputScheme::Wubi
        );
    }
}

#[test]
fn settings_sync_unknown_enum_values_skip_only_their_key() {
    let mut schema = full_schema();
    schema.fields.insert(
        "input.schema".into(),
        AccountPreferenceField {
            value_type: "string".into(),
        },
    );
    for unknown in [
        "cantonese",
        "zhuyin",
        "vietnamese",
        "tibetan",
        "stroke",
        "esperanto",
    ] {
        let mut values = frequency_values(&FrequencyPreferences {
            mode: FrequencyMode::Linear,
            trigger_count: 7,
            linear_step: 4,
        });
        values.insert(
            "input.schema".into(),
            AccountPreferenceValue::String(unknown.into()),
        );
        let local = Preferences {
            scheme: InputScheme::Wubi,
            ..Preferences::default()
        };
        let applied = apply(&local, values, &schema);
        assert_eq!(applied.preferences.scheme, InputScheme::Wubi, "{unknown}");
        assert_eq!(applied.preferences.frequency.mode, FrequencyMode::Linear);
        assert_eq!(applied.skipped, ["input.schema"]);
    }

    let unknown_values = [
        ("input.character_set", "cyrillic"),
        ("input.shuangpin_schema", "sogou"),
        ("input.wubi_schema", "wubi06"),
        ("input.frequency_mode", "random"),
        ("input.quote_style", "semantic"),
        ("input.traditional_variant", "s2hk"),
        ("input.bracket_style", "round"),
        ("platform.android.keyboard_layout", "future_layout"),
        ("platform.android.theme", "sepia"),
        ("platform.android.global_theme", "aurora"),
        ("platform.android.custom_theme_base", "custom"),
        ("platform.android.custom_keyboard_skin", "{not json"),
        ("platform.android.custom_candidate_skin", "../escape"),
        ("platform.android.custom_candidate_skin_dark", "../escape"),
        ("platform.android.haptic_strength", "max"),
    ];
    let mut values: BTreeMap<String, AccountPreferenceValue> = unknown_values
        .iter()
        .map(|(key, value)| {
            (
                (*key).to_owned(),
                AccountPreferenceValue::String((*value).into()),
            )
        })
        .collect();
    values.insert(
        "input.learning".into(),
        AccountPreferenceValue::Boolean(false),
    );
    let applied = apply(&Preferences::default(), values, &schema);
    let mut expected: Vec<&str> = unknown_values.iter().map(|(key, _)| *key).collect();
    expected.sort();
    assert_eq!(applied.skipped, expected);
    // 能用的键照常应用，跳过的键保持本地值。
    assert!(!applied.preferences.learning);
    assert_eq!(
        Preferences {
            learning: true,
            ..applied.preferences
        },
        Preferences::default()
    );
    assert_eq!(applied.feedback, Some(feedback()));
}

/// 「原生」只在 iOS 提供：Android 收到它时与不认识的主题 id 一样只跳过这个键，主题和自定义主题的底都保持本机的值。
#[test]
fn settings_sync_skips_the_ios_only_native_theme() {
    let local = Preferences {
        global_theme: GlobalTheme::Night,
        ..Preferences::default()
    };
    let values = BTreeMap::from([
        (
            GLOBAL_THEME.to_owned(),
            AccountPreferenceValue::String("native".into()),
        ),
        (
            CUSTOM_THEME_BASE.to_owned(),
            AccountPreferenceValue::String("native".into()),
        ),
        (
            "input.learning".to_owned(),
            AccountPreferenceValue::Boolean(false),
        ),
    ]);
    let applied = apply(&local, values, &full_schema());
    assert_eq!(applied.skipped, [CUSTOM_THEME_BASE, GLOBAL_THEME]);
    assert_eq!(applied.preferences.global_theme, GlobalTheme::Night);
    assert_eq!(
        applied.preferences.custom_theme.base,
        local.custom_theme.base
    );
    assert!(!applied.preferences.learning);
}

#[test]
fn settings_sync_out_of_range_values_skip_only_their_key() {
    let schema = full_schema();
    let values = BTreeMap::from([
        (
            "platform.android.touch_key_spacing_tenths".to_owned(),
            AccountPreferenceValue::Integer(500),
        ),
        (
            "platform.android.touch_row_spacing_tenths".to_owned(),
            AccountPreferenceValue::Integer(1),
        ),
        (
            "platform.android.keyboard_height_adjustment".to_owned(),
            AccountPreferenceValue::Integer(i64::from(i8::MAX) + 1),
        ),
        (
            "input.frequency_trigger_count".to_owned(),
            AccountPreferenceValue::Integer(-1),
        ),
        (
            "input.frequency_linear_step".to_owned(),
            AccountPreferenceValue::Integer(3),
        ),
        (
            "input.chinese_punctuation".to_owned(),
            AccountPreferenceValue::Boolean(false),
        ),
    ]);
    let applied = apply(&Preferences::default(), values, &schema);
    assert_eq!(
        applied.skipped,
        [
            "input.frequency_trigger_count",
            "platform.android.keyboard_height_adjustment",
            "platform.android.touch_key_spacing_tenths",
            "platform.android.touch_row_spacing_tenths",
        ]
    );
    assert_eq!(applied.preferences.frequency.linear_step, 3);
    assert!(!applied.preferences.chinese_punctuation);
    let defaults = Preferences::default();
    assert_eq!(
        applied.preferences.touch_key_spacing_tenths,
        defaults.touch_key_spacing_tenths
    );
    assert_eq!(
        applied.preferences.touch_keyboard_height_adjustment,
        defaults.touch_keyboard_height_adjustment
    );
    applied.preferences.validate().unwrap();

    // `number` 字段里带小数的值本机表示不了，同样只跳过这一个键。
    let mut schema = schema.clone();
    schema.fields.insert(
        "platform.android.touch_key_spacing_tenths".into(),
        AccountPreferenceField {
            value_type: "number".into(),
        },
    );
    let applied = apply(
        &Preferences::default(),
        BTreeMap::from([(
            "platform.android.touch_key_spacing_tenths".to_owned(),
            AccountPreferenceValue::Number(45.5),
        )]),
        &schema,
    );
    assert_eq!(
        applied.skipped,
        ["platform.android.touch_key_spacing_tenths"]
    );
}

#[test]
fn settings_sync_wubi_profile_round_trips_through_the_account_wubi_schema() {
    let schema = schema_for(&[("input.wubi_schema", "string")]);
    for profile in [WubiProfile::Wubi86, WubiProfile::Wubi98] {
        let values = BTreeMap::from([(
            "input.wubi_schema".to_owned(),
            AccountPreferenceValue::String(account_wubi_schema(profile).into()),
        )]);
        let local = Preferences {
            wubi_profile: if profile == WubiProfile::Wubi86 {
                WubiProfile::Wubi98
            } else {
                WubiProfile::Wubi86
            },
            ..Preferences::default()
        };
        assert_eq!(
            apply(&local, values, &schema).preferences.wubi_profile,
            profile
        );
    }
    // 账号的字段表里没有它时不动本地设置。
    let values = BTreeMap::from([(
        "input.wubi_schema".to_owned(),
        AccountPreferenceValue::String("wubi98".into()),
    )]);
    let applied = apply(&Preferences::default(), values, &frequency_schema());
    assert_eq!(applied.preferences.wubi_profile, WubiProfile::Wubi86);
}

#[test]
fn settings_sync_malformed_documents_are_still_refused_whole() {
    // 值的类型与字段表不符。
    let schema = full_schema();
    let values = BTreeMap::from([(
        "input.learning".to_owned(),
        AccountPreferenceValue::String("yes".into()),
    )]);
    assert_eq!(
        apply_android_settings(&Preferences::default(), &document(values), &schema, None),
        Err(AccountError::Invalid)
    );
}

#[test]
fn settings_sync_host_feedback_is_read_only_when_the_document_has_it() {
    let schema = full_schema();
    let without = BTreeMap::from([(
        "input.learning".to_owned(),
        AccountPreferenceValue::Boolean(false),
    )]);
    assert!(!needs_host_feedback(&document(without.clone()), &schema));
    let applied =
        apply_android_settings(&Preferences::default(), &document(without), &schema, None).unwrap();
    assert_eq!(applied.feedback, None);

    let with = BTreeMap::from([(
        "platform.android.haptics_enabled".to_owned(),
        AccountPreferenceValue::Boolean(true),
    )]);
    assert!(needs_host_feedback(&document(with.clone()), &schema));
    assert!(!needs_host_feedback(
        &document(with.clone()),
        &schema_for(&[])
    ));
    assert_eq!(
        apply_android_settings(
            &Preferences::default(),
            &document(with.clone()),
            &schema,
            None
        ),
        Err(AccountError::Storage)
    );
    let applied = apply(&Preferences::default(), with, &schema);
    assert_eq!(
        applied.feedback,
        Some(HostKeyboardFeedback {
            haptics_enabled: true,
            ..feedback()
        })
    );
    assert!(valid_haptic_strength("light") && !valid_haptic_strength("off"));
    assert!(valid_haptic_strength("system"), "跟随系统");
}

/// 深色槽位与浅色槽位各自同步：空串清掉本机的深色皮肤，字段表没收录这个键（旧服务端）时本机的深色皮肤保持不变。
#[test]
fn settings_sync_dark_candidate_skin_slot_syncs_on_its_own() {
    let mut local = Preferences::default();
    local.custom_theme.candidate_skin = Some("sakura".into());
    local.custom_theme.candidate_skin_dark = Some("midnight".into());
    local.validate().unwrap();
    let exported = export_android_settings(&local, None).unwrap();
    assert_eq!(
        exported.get("platform.android.custom_candidate_skin_dark"),
        Some(&AccountPreferenceValue::String("midnight".into()))
    );

    let cleared = BTreeMap::from([(
        "platform.android.custom_candidate_skin_dark".to_owned(),
        AccountPreferenceValue::String(String::new()),
    )]);
    let applied = apply(&local, cleared.clone(), &full_schema());
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    assert_eq!(
        applied.preferences.custom_theme.candidate_skin.as_deref(),
        Some("sakura")
    );
    assert_eq!(applied.preferences.custom_theme.candidate_skin_dark, None);

    let mut legacy = full_schema();
    legacy
        .fields
        .remove("platform.android.custom_candidate_skin_dark");
    let applied = apply(&local, cleared, &legacy);
    assert_eq!(
        applied
            .preferences
            .custom_theme
            .candidate_skin_dark
            .as_deref(),
        Some("midnight")
    );
}

#[test]
fn settings_sync_new_android_keys_round_trip() {
    let mut expected = Preferences::default();
    expected.touch_toolbar.ai = !expected.touch_toolbar.ai;
    expected.plugins.key_sound.pack = "msime-woodblock".into();
    expected.voice_input.language = "en-US".into();
    expected.validate().unwrap();
    let exported = export_android_settings(&expected, None).unwrap();
    let applied = apply(&Preferences::default(), exported, &full_schema());
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    assert_eq!(applied.preferences, expected);
}

#[test]
fn settings_sync_keeps_android_text_trackpad_on_this_device() {
    let mut local = Preferences::default();
    local.android_text_trackpad.enabled = true;
    local.android_text_trackpad.hold_to_open = true;
    local.android_text_trackpad.horizontal_dp = 48;
    local.android_text_trackpad.vertical_dp = 8;
    local.android_text_trackpad.tap_selection = false;
    local.android_text_trackpad.haptics = false;
    local.android_text_trackpad.trail = false;
    local.android_text_trackpad.magnifier = false;
    assert_eq!(
        export_android_settings(&local, None).unwrap(),
        export_android_settings(&Preferences::default(), None).unwrap()
    );
    let applied = apply(
        &local,
        export_android_settings(&Preferences::default(), None).unwrap(),
        &full_schema(),
    );
    assert_eq!(
        applied.preferences.android_text_trackpad,
        local.android_text_trackpad
    );
}

/// 本地设置的全部同步键，取一组非默认值。
fn local_values() -> BTreeMap<String, AccountPreferenceValue> {
    use AccountPreferenceValue::{Boolean, Integer, String};
    BTreeMap::from([
        ("general.app_theme".to_owned(), String("qiushan".into())),
        (
            "platform.android.one_handed".to_owned(),
            String("left".into()),
        ),
        ("platform.android.split_keyboard".to_owned(), Boolean(true)),
        ("platform.android.key_popup".to_owned(), Boolean(false)),
        (
            "platform.android.swipe_down_symbols".to_owned(),
            Boolean(false),
        ),
        (
            "platform.android.swipe_symbols_direction".to_owned(),
            String("up".into()),
        ),
        ("platform.android.space_cursor".to_owned(), Boolean(false)),
        ("platform.android.space_voice".to_owned(), Boolean(false)),
        (
            "platform.android.key_animation".to_owned(),
            String("ripple".into()),
        ),
        ("platform.android.toolbar_phrase".to_owned(), Boolean(false)),
        ("platform.android.toolbar_scheme".to_owned(), Boolean(false)),
        ("platform.android.toolbar_hidden".to_owned(), Boolean(true)),
        (
            "platform.android.handwriting_mode".to_owned(),
            String("line".into()),
        ),
        (
            "platform.android.handwriting_delay_ms".to_owned(),
            Integer(900),
        ),
        (
            "platform.android.handwriting_show_pinyin".to_owned(),
            Boolean(false),
        ),
        (
            "platform.android.handwriting_stroke_color".to_owned(),
            String("blue".into()),
        ),
        (
            "platform.android.handwriting_stroke_width".to_owned(),
            Integer(6),
        ),
        (
            "platform.android.voice_offline_fallback".to_owned(),
            Boolean(true),
        ),
    ])
}

fn local_schema() -> AccountPreferenceSchema {
    let mut schema = full_schema();
    for (key, value) in local_values() {
        schema.fields.insert(
            key,
            AccountPreferenceField {
                value_type: value.kind().to_owned(),
            },
        );
    }
    schema
}

#[test]
fn settings_sync_android_local_settings_round_trip() {
    let local = local_values();
    assert_eq!(local.len(), ANDROID_LOCAL_SETTINGS.len());
    let mut settings = export_android_settings(&Preferences::default(), None).unwrap();
    assert!(local.keys().all(|key| !settings.contains_key(key)));
    assert!(insert_android_local_settings(&mut settings, &local).is_empty());
    let cloud = document(settings);
    let (accepted, skipped) = android_local_settings(&cloud, &local_schema()).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    assert_eq!(accepted, local);
    // 本地设置不改共享偏好。
    let applied = apply(&Preferences::default(), cloud.settings, &local_schema());
    assert_eq!(applied.preferences, Preferences::default());
}

#[test]
fn settings_sync_android_local_settings_skip_unknown_keys_and_bad_values() {
    use AccountPreferenceValue::{Boolean, Integer, Number, String};
    let mut settings = BTreeMap::new();
    let rejected = insert_android_local_settings(
        &mut settings,
        &BTreeMap::from([
            ("platform.android.incognito".to_owned(), Boolean(true)),
            (
                "platform.android.developer_options".to_owned(),
                Boolean(true),
            ),
            (
                "platform.android.voice_contribute_audio".to_owned(),
                Boolean(true),
            ),
            (
                "platform.android.handwriting_delay_ms".to_owned(),
                Integer(650),
            ),
            (
                "platform.android.handwriting_stroke_width".to_owned(),
                Number(4.0),
            ),
        ]),
    );
    assert_eq!(
        rejected,
        [
            "platform.android.developer_options",
            "platform.android.handwriting_delay_ms",
            "platform.android.incognito",
            "platform.android.voice_contribute_audio",
        ]
    );
    assert_eq!(
        settings,
        BTreeMap::from([(
            "platform.android.handwriting_stroke_width".to_owned(),
            Integer(4)
        )])
    );
    let cloud = document(BTreeMap::from([
        ("general.app_theme".to_owned(), String("winter".into())),
        (
            "platform.android.one_handed".to_owned(),
            String("middle".into()),
        ),
        (
            "platform.android.handwriting_delay_ms".to_owned(),
            Integer(5000),
        ),
        (
            "platform.android.handwriting_stroke_width".to_owned(),
            Integer(0),
        ),
        ("platform.android.key_popup".to_owned(), Boolean(false)),
    ]));
    let (accepted, skipped) = android_local_settings(&cloud, &local_schema()).unwrap();
    assert_eq!(
        skipped,
        [
            "general.app_theme",
            "platform.android.one_handed",
            "platform.android.handwriting_delay_ms",
            "platform.android.handwriting_stroke_width",
        ]
    );
    assert_eq!(
        accepted,
        BTreeMap::from([("platform.android.key_popup".to_owned(), Boolean(false))])
    );
    // 字段表没收录的键不交给宿主，类型冲突拒绝整份文档。
    assert!(android_local_settings(&cloud, &full_schema())
        .unwrap()
        .0
        .is_empty());
    let conflicting = schema_for(&[("platform.android.key_popup", "string")]);
    assert!(android_local_settings(&cloud, &conflicting).is_err());
}

#[test]
fn settings_sync_app_theme_ids_follow_the_app_theme_picker() {
    let ids: Vec<&str> = crate::skin::app_theme::AppTheme::ALL
        .iter()
        .map(|theme| theme.id())
        .collect();
    assert_eq!(ids, APP_THEME_IDS);
}

#[test]
fn settings_sync_custom_keyboard_skin_library_is_bounded_and_an_array() {
    let mut settings = BTreeMap::new();
    assert!(insert_custom_keyboard_skins(
        &mut settings,
        "[{\"id\":\"a\"}]"
    ));
    assert!(!insert_custom_keyboard_skins(&mut settings, "{}"));
    assert!(!insert_custom_keyboard_skins(
        &mut settings,
        &format!("[\"{}\"]", "a".repeat(MAX_CUSTOM_KEYBOARD_SKINS_BYTES))
    ));
    let schema = schema_for(&[(CUSTOM_KEYBOARD_SKINS, "string")]);
    let cloud = document(settings.clone());
    assert_eq!(
        custom_keyboard_skins(&cloud, &schema).unwrap(),
        Some(Ok("[{\"id\":\"a\"}]".to_owned()))
    );
    let bad = document(BTreeMap::from([(
        CUSTOM_KEYBOARD_SKINS.to_owned(),
        AccountPreferenceValue::String("nope".into()),
    )]));
    assert_eq!(custom_keyboard_skins(&bad, &schema).unwrap(), Some(Err(())));
    assert_eq!(
        custom_keyboard_skins(&cloud, &schema_for(&[])).unwrap(),
        None
    );
}

#[test]
fn settings_sync_desktop_export_is_only_the_shared_input_keys() {
    let mut preferences = Preferences {
        scheme: InputScheme::Wubi,
        traditional_chinese_output: true,
        ..Preferences::default()
    };
    preferences.cloud_clipboard_relay.url = "https://SENTINEL-relay.example.test".into();
    preferences.cloud_clipboard_relay.token = "SENTINEL-relay-token".into();
    let exported = export_desktop_settings(&preferences);
    assert!(exported.keys().all(|key| key.starts_with("input.")));
    assert!(exported.values().all(|value| !matches!(
        value,
        AccountPreferenceValue::String(text) if text.contains("SENTINEL")
    )));
    let android = export_android_settings(&preferences, None).unwrap();
    let shared: BTreeMap<_, _> = android
        .into_iter()
        .filter(|(key, _)| key.starts_with("input."))
        .collect();
    assert_eq!(exported, shared);
    assert_eq!(
        exported.get("input.character_set"),
        Some(&AccountPreferenceValue::String("traditional".into()))
    );
    assert_eq!(
        exported.get("input.wubi_schema"),
        Some(&AccountPreferenceValue::String("wubi86".into()))
    );
}

#[test]
fn settings_sync_desktop_apply_changes_only_the_shared_input_settings() {
    let local = Preferences::default();
    let mut settings = export_desktop_settings(&Preferences {
        scheme: InputScheme::Shuangpin,
        learning: !local.learning,
        ..Preferences::default()
    });
    // 别的平台的设置和本机不认识的方案都不应改动桌面偏好。
    settings.insert(
        "platform.android.theme".into(),
        AccountPreferenceValue::String("dark".into()),
    );
    let mut schema = full_schema();
    schema.fields.insert(
        "input.schema".into(),
        AccountPreferenceField {
            value_type: "string".into(),
        },
    );
    let applied = apply_desktop_settings(&local, &document(settings.clone()), &schema).unwrap();
    assert_eq!(applied.preferences.scheme, InputScheme::Shuangpin);
    assert_eq!(applied.preferences.learning, !local.learning);
    assert_eq!(applied.preferences.theme, local.theme);
    assert!(applied.skipped.is_empty());

    settings.insert(
        "input.schema".into(),
        AccountPreferenceValue::String("future-scheme".into()),
    );
    let applied = apply_desktop_settings(&local, &document(settings), &schema).unwrap();
    assert_eq!(applied.preferences.scheme, local.scheme);
    assert_eq!(applied.skipped, ["input.schema"]);
}

/// 别的设备改了智能标点时，本机的数字后、字母后直出两半跟着写成同一个值，否则智能标点开着却不起作用。
#[test]
fn settings_sync_smart_punctuation_change_writes_both_halves() {
    for enabled in [true, false] {
        let local = Preferences {
            smart_punctuation: !enabled,
            smart_punctuation_direct_digit: !enabled,
            smart_punctuation_direct_letter: !enabled,
            ..Preferences::default()
        };
        let exported = export_desktop_settings(&Preferences {
            smart_punctuation: enabled,
            ..local.clone()
        });
        let applied = apply_desktop_settings(&local, &document(exported), &full_schema()).unwrap();
        assert_eq!(applied.preferences.smart_punctuation, enabled);
        assert_eq!(applied.preferences.smart_punctuation_direct_digit, enabled);
        assert_eq!(applied.preferences.smart_punctuation_direct_letter, enabled);
        assert!(applied.skipped.is_empty());
    }
}

/// 智能标点默认开的一次性迁移和设置同步：迁移后的本机上传的是迁移后的值（macOS、Windows 上旧文档里的 `false` 已经读成开）；用户手动应用一份迁移前上传的云端快照时，快照里的「关」照常生效，迁移标记留在本机，之后再读不会把它改回开。
#[test]
fn settings_sync_after_the_smart_punctuation_migration() {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::preferences::PreferencesStore::new(dir.path());
    let mut legacy =
        serde_json::to_value(crate::preferences::PreferencesSnapshot::default()).unwrap();
    let preferences = legacy["preferences"].as_object_mut().unwrap();
    for key in [
        "smart_punctuation",
        "smart_punctuation_repeat",
        "smart_punctuation_direct_digit",
        "smart_punctuation_direct_letter",
    ] {
        preferences.insert(key.into(), false.into());
    }
    preferences.remove("smart_punctuation_migrated").unwrap();
    std::fs::write(
        dir.path().join("preferences.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    let flipped = cfg!(any(windows, target_os = "macos"));

    let local = store.load().unwrap();
    assert_eq!(
        export_desktop_settings(&local.preferences).get("input.smart_punctuation"),
        Some(&AccountPreferenceValue::Boolean(flipped))
    );

    let stale = export_desktop_settings(&Preferences {
        smart_punctuation: false,
        ..Preferences::default()
    });
    let applied =
        apply_desktop_settings(&local.preferences, &document(stale), &full_schema()).unwrap();
    assert!(!applied.preferences.smart_punctuation);
    assert!(applied.preferences.smart_punctuation_migrated);
    store.save(local.revision, applied.preferences).unwrap();
    let reloaded = store.load().unwrap().preferences;
    assert!(!reloaded.smart_punctuation);
    assert!(!reloaded.smart_punctuation_direct_digit);
    assert!(reloaded.smart_punctuation_migrated);
}

/// 云端的智能标点和本机相同时，本机单独调过的一半保持原样，同步不会每次都把它冲掉。
#[test]
fn settings_sync_unchanged_smart_punctuation_leaves_the_halves_alone() {
    let local = Preferences {
        smart_punctuation: true,
        smart_punctuation_direct_digit: true,
        smart_punctuation_direct_letter: false,
        ..Preferences::default()
    };
    let exported = export_desktop_settings(&local);
    let applied = apply_desktop_settings(&local, &document(exported), &full_schema()).unwrap();
    assert_eq!(applied.preferences, local);
    assert!(applied.skipped.is_empty());
}

/// 引号和方头括号样式按偏好文档里的写法上传，三种引号样式都能读回来。
#[test]
fn settings_sync_punctuation_styles_round_trip() {
    use crate::preferences::{BracketStyle, QuoteStyle};
    for (quote_style, quote_value) in [
        (QuoteStyle::Curly, "curly"),
        (QuoteStyle::Corner, "corner"),
        (QuoteStyle::CornerWhenTraditional, "corner_when_traditional"),
    ] {
        for (bracket_style, bracket_value) in [
            (BracketStyle::Solid, "solid"),
            (BracketStyle::Hollow, "hollow"),
        ] {
            let expected = Preferences {
                quote_style,
                bracket_style,
                ..Preferences::default()
            };
            let exported = export_desktop_settings(&expected);
            assert_eq!(
                exported.get("input.quote_style"),
                Some(&AccountPreferenceValue::String(quote_value.into()))
            );
            assert_eq!(
                exported.get("input.bracket_style"),
                Some(&AccountPreferenceValue::String(bracket_value.into()))
            );
            let applied = apply_desktop_settings(
                &Preferences::default(),
                &document(exported),
                &full_schema(),
            )
            .unwrap();
            assert_eq!(applied.preferences, expected);
            assert!(applied.skipped.is_empty());
        }
    }
    for (variant, value) in [
        (TraditionalVariant::Standard, "s2t"),
        (TraditionalVariant::Taiwan, "s2tw"),
        (TraditionalVariant::TaiwanPhrases, "s2twp"),
    ] {
        let expected = Preferences {
            traditional_chinese_output: true,
            traditional_chinese_variant: variant,
            ..Preferences::default()
        };
        let exported = export_desktop_settings(&expected);
        assert_eq!(
            exported.get("input.traditional_variant"),
            Some(&AccountPreferenceValue::String(value.into()))
        );
        let applied =
            apply_desktop_settings(&Preferences::default(), &document(exported), &full_schema())
                .unwrap();
        assert_eq!(applied.preferences, expected);
        assert!(applied.skipped.is_empty());
    }
    // 字段表没有这两个键时不应用，也不算跳过。
    let mut exported = export_desktop_settings(&Preferences {
        quote_style: QuoteStyle::Corner,
        ..Preferences::default()
    });
    exported.retain(|key, _| key == "input.quote_style");
    let applied = apply_desktop_settings(
        &Preferences::default(),
        &document(exported),
        &schema_for(&[]),
    )
    .unwrap();
    assert_eq!(applied.preferences.quote_style, QuoteStyle::Curly);
    assert!(applied.skipped.is_empty());
}
