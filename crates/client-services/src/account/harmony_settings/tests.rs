//! 用例从鸿蒙的 `platforms/harmony/tests/keyboard-logic.test.ts` 里 `AccountPreferencePlan` 的那几组移植。TS 版拒绝整份文档、Rust 改成只跳过一个键的地方，断言按 Rust 写，差异记在 #8201 的 PR 正文。

use super::*;
use crate::account::{merge_account_preferences, AccountPreferenceField};
use crate::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings, Edition,
};
use crate::preferences::{
    BracketStyle, FrequencyMode, InputScheme, NumberKeypadOrder, QuoteStyle, ShuangpinProfile,
    TouchFourteenKeyStyle, TouchKeyboardLayout, TouchKeyboardSkinDesign, TouchNineKeyStyle,
    TouchTwentySixKeyStyle, TwentySixKeyNumberLayout, WubiProfile,
};

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
        haptic_strength: "light".into(),
    }
}

/// 字段表收录这个宿主导出的全部键，类型取导出值的类型，即追上了的服务端会发的那份。
fn full_schema() -> AccountPreferenceSchema {
    let mut schema = schema_for(&[]);
    for (key, value) in export_harmony_settings(&Preferences::default(), Some(&feedback())).unwrap()
    {
        schema.fields.insert(
            key,
            AccountPreferenceField {
                value_type: value.kind().to_owned(),
            },
        );
    }
    schema
}

/// 再收录只随五笔方案导出的 `input.wubi_schema`。
fn wubi_schema() -> AccountPreferenceSchema {
    let mut schema = full_schema();
    schema.fields.insert(
        "input.wubi_schema".into(),
        AccountPreferenceField {
            value_type: "string".into(),
        },
    );
    schema
}

fn string(value: &str) -> AccountPreferenceValue {
    AccountPreferenceValue::String(value.into())
}

fn cloud(settings: &[(&str, AccountPreferenceValue)]) -> AccountPreferences {
    AccountPreferences {
        revision: 3,
        settings: settings
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    }
}

fn apply(
    local: &Preferences,
    settings: &[(&str, AccountPreferenceValue)],
    schema: &AccountPreferenceSchema,
) -> HarmonySettingsApplied {
    apply_harmony_settings(local, &cloud(settings), schema, Some(feedback())).unwrap()
}

fn exported(preferences: &Preferences) -> BTreeMap<String, AccountPreferenceValue> {
    export_harmony_settings(preferences, Some(&feedback())).unwrap()
}

#[test]
fn export_maps_this_hosts_preferences_under_its_own_namespace() {
    let mut local = Preferences {
        scheme: InputScheme::Shuangpin,
        traditional_chinese_output: true,
        shuangpin_profile: ShuangpinProfile::Ziranma,
        learning: false,
        chinese_punctuation: false,
        touch_keyboard_layout: TouchKeyboardLayout::NineKey,
        global_theme: GlobalTheme::Night,
        touch_key_spacing_tenths: 40,
        ..Preferences::default()
    };
    local.frequency.mode = FrequencyMode::Linear;
    local.frequency.trigger_count = 3;
    local.frequency.linear_step = 2;
    local.custom_theme.base = GlobalTheme::Paper;
    local.custom_theme.candidate_skin = Some("harbour".into());
    local.custom_theme.candidate_skin_dark = Some("dusk".into());
    local.custom_theme.keyboard = Some(TouchKeyboardSkinDesign::default());
    let values = exported(&local);
    assert_eq!(values["input.schema"], string("shuangpin"));
    assert_eq!(values["input.character_set"], string("traditional"));
    assert_eq!(values["input.shuangpin_schema"], string("ziranma"));
    assert_eq!(
        values["input.learning"],
        AccountPreferenceValue::Boolean(false)
    );
    assert_eq!(
        values["input.frequency_trigger_count"],
        AccountPreferenceValue::Integer(3)
    );
    assert_eq!(values["platform.harmony.global_theme"], string("night"));
    assert_eq!(
        values["platform.harmony.custom_theme_base"],
        string("paper")
    );
    assert_eq!(
        values["platform.harmony.custom_candidate_skin"],
        string("harbour")
    );
    assert_eq!(
        values["platform.harmony.custom_candidate_skin_dark"],
        string("dusk")
    );
    assert_eq!(values[HARMONY_KEYBOARD_LAYOUT], string("nine_key"));
    assert_eq!(
        values["platform.harmony.touch_key_spacing_tenths"],
        AccountPreferenceValue::Integer(40)
    );
    // 自定义设计与其他宿主一样作为一个 JSON 字符串上传。
    assert_eq!(
        values["platform.harmony.custom_keyboard_skin"],
        string(&serde_json::to_string(&TouchKeyboardSkinDesign::default()).unwrap())
    );
    // 按键反馈来自宿主自己的文件。
    assert_eq!(values[HARMONY_FEEDBACK_KEYS[2]], string("light"));
    assert_eq!(
        values[HARMONY_FEEDBACK_KEYS[0]],
        AccountPreferenceValue::Boolean(true)
    );
    // 鸿蒙和 Android 是两台设备，各有键盘：从不写别的平台的键，也不写只有 Android 才有的那些。不带平台前缀的只有共享设置页在鸿蒙上能改的共享偏好。
    const SHARED: [&str; 4] = [
        "local_modes.symbol",
        "local_modes.date_time_menu",
        "second_third_candidate.enabled",
        "second_third_candidate.keys",
    ];
    assert!(
        values.keys().all(|key| key.starts_with("input.")
            || key.starts_with("platform.harmony.")
            || SHARED.contains(&key.as_str())),
        "{values:?}"
    );
    let platform: Vec<&str> = values
        .keys()
        .filter_map(|key| key.strip_prefix("platform.harmony."))
        .collect();
    assert_eq!(
        platform,
        [
            "custom_candidate_skin",
            "custom_candidate_skin_dark",
            "custom_keyboard_skin",
            "custom_theme_base",
            "fourteen_key_style",
            "global_theme",
            "haptic_strength",
            "haptics_enabled",
            "keyboard_height_adjustment",
            "keyboard_layout",
            "language_key_side",
            "nine_key_digit_symbols",
            "nine_key_style",
            "nine_key_swipe",
            "nine_key_symbols",
            "number_keypad_order",
            "number_pad_space",
            "number_row",
            "one_handed_side_panel",
            "quanpin_shift",
            "shuangpin_key_hints",
            "sound_enabled",
            "split_keyboard",
            "theme",
            "touch_key_spacing_tenths",
            "touch_row_spacing_tenths",
            "twenty_six_key_number_layout",
            "twenty_six_key_style",
            "twenty_six_symbol_first",
            "voice_shortcut",
        ]
    );
}

#[test]
fn a_fresh_document_exports_the_shared_defaults() {
    let sparse = exported(&Preferences::default());
    assert_eq!(sparse["input.schema"], string("quanpin"));
    assert_eq!(
        sparse["input.learning"],
        AccountPreferenceValue::Boolean(true)
    );
    assert_eq!(sparse["platform.harmony.global_theme"], string("system"));
    assert_eq!(
        sparse["platform.harmony.custom_theme_base"],
        string("system")
    );
    // 没有设计、没有皮肤包都上传成空串，清掉也能同步。
    assert_eq!(sparse["platform.harmony.custom_keyboard_skin"], string(""));
    assert_eq!(sparse["platform.harmony.custom_candidate_skin"], string(""));
    assert_eq!(
        sparse["platform.harmony.custom_candidate_skin_dark"],
        string("")
    );
    assert_eq!(sparse[HARMONY_KEYBOARD_LAYOUT], string("twenty_six_key"));
    assert_eq!(
        sparse["platform.harmony.number_keypad_order"],
        string("phone")
    );
    assert_eq!(
        sparse["platform.harmony.twenty_six_key_number_layout"],
        string("row")
    );
    assert_eq!(
        sparse["platform.harmony.shuangpin_key_hints"],
        AccountPreferenceValue::Boolean(false)
    );
    assert_eq!(sparse["input.quote_style"], string("curly"));
    assert_eq!(sparse["input.bracket_style"], string("solid"));
    // 五笔版本只随五笔方案上传。
    assert!(!sparse.contains_key("input.wubi_schema"));
    // 没有传按键反馈时不导出那三个键。
    let without = export_harmony_settings(&Preferences::default(), None).unwrap();
    assert!(HARMONY_FEEDBACK_KEYS
        .iter()
        .all(|key| !without.contains_key(*key)));
}

#[test]
fn a_fourteen_key_layout_is_left_out_so_the_cloud_keeps_its_own() {
    let local = Preferences {
        touch_keyboard_layout: TouchKeyboardLayout::FourteenKey,
        ..Preferences::default()
    };
    let omitted = exported(&local);
    assert!(!omitted.contains_key(HARMONY_KEYBOARD_LAYOUT));
    assert_eq!(omitted["input.schema"], string("quanpin"));
    let merged = merge_account_preferences(
        &cloud(&[(HARMONY_KEYBOARD_LAYOUT, string("nine_key"))]),
        &omitted,
        &full_schema(),
    )
    .unwrap();
    assert_eq!(merged.settings[HARMONY_KEYBOARD_LAYOUT], string("nine_key"));
    // 上传之后合并结果写回本机：云端的九键是本机切到 14 键之前的，本机仍是 14 键。
    let written_back =
        apply_harmony_settings(&local, &merged, &full_schema(), Some(feedback())).unwrap();
    assert_eq!(written_back.preferences, local);
    assert!(written_back.skipped.is_empty());
    // 应用方向照常认 14 键。
    let applied = apply(
        &Preferences {
            touch_keyboard_layout: TouchKeyboardLayout::NineKey,
            ..Preferences::default()
        },
        &[(HARMONY_KEYBOARD_LAYOUT, string("fourteen_key"))],
        &full_schema(),
    );
    assert_eq!(
        applied.preferences.touch_keyboard_layout,
        TouchKeyboardLayout::FourteenKey
    );
}

#[test]
fn a_theme_this_host_does_not_offer_uploads_as_system_and_is_skipped_on_apply() {
    let native = Preferences {
        global_theme: GlobalTheme::Native,
        ..Preferences::default()
    };
    assert_eq!(
        exported(&native)["platform.harmony.global_theme"],
        string("system")
    );
    let applied = apply(
        &Preferences {
            global_theme: GlobalTheme::Ink,
            ..Preferences::default()
        },
        &[("platform.harmony.global_theme", string("native"))],
        &full_schema(),
    );
    assert_eq!(applied.preferences.global_theme, GlobalTheme::Ink);
    assert_eq!(applied.skipped, ["platform.harmony.global_theme"]);
}

#[test]
fn an_unknown_haptic_strength_uploads_as_medium() {
    let odd = HostKeyboardFeedback {
        haptic_strength: "loud".into(),
        ..feedback()
    };
    let values = export_harmony_settings(&Preferences::default(), Some(&odd)).unwrap();
    assert_eq!(values[HARMONY_FEEDBACK_KEYS[2]], string("medium"));
    let system = HostKeyboardFeedback {
        haptic_strength: "system".into(),
        ..feedback()
    };
    let values = export_harmony_settings(&Preferences::default(), Some(&system)).unwrap();
    assert_eq!(values[HARMONY_FEEDBACK_KEYS[2]], string("system"));
}

#[test]
fn uploading_keeps_what_other_devices_wrote() {
    let schema = full_schema();
    let base = AccountPreferences {
        revision: 7,
        settings: BTreeMap::from([
            ("platform.ios.keyboard_skin".into(), string("rose")),
            ("input.schema".into(), string("quanpin")),
        ]),
    };
    let merged = merge_account_preferences(
        &base,
        &BTreeMap::from([("input.schema".into(), string("wubi"))]),
        &schema,
    )
    .unwrap();
    assert_eq!(merged.settings["input.schema"], string("wubi"));
    assert_eq!(
        merged.settings["platform.ios.keyboard_skin"],
        string("rose")
    );
    assert_eq!(merged.revision, 7);
    // 写字段表里没有的键、或者键对类型不对，都拒绝而不是悄悄丢掉。
    assert_eq!(
        merge_account_preferences(
            &base,
            &BTreeMap::from([("input.unknown_field".into(), string("x"))]),
            &schema,
        ),
        Err(AccountError::Invalid)
    );
    assert_eq!(
        merge_account_preferences(
            &base,
            &BTreeMap::from([("input.learning".into(), string("yes"))]),
            &schema,
        ),
        Err(AccountError::Invalid)
    );
    // 照片那么大的设计放得进 1 MiB 的字段表，放不进旧的 64 KiB。
    let photo = "A".repeat(4 * (512_000 + 2) / 3);
    let design = BTreeMap::from([(
        "platform.harmony.custom_keyboard_skin".into(),
        string(&format!("{{\"photo\":\"{photo}\"}}")),
    )]);
    let mut photo_schema = schema_for(&[("platform.harmony.custom_keyboard_skin", "string")]);
    photo_schema.maximum_bytes = 1024 * 1024;
    let empty = AccountPreferences {
        revision: 1,
        settings: BTreeMap::new(),
    };
    let with_photo = merge_account_preferences(&empty, &design, &photo_schema).unwrap();
    assert_eq!(with_photo.settings, design);
    photo_schema.maximum_bytes = 64 * 1024;
    assert_eq!(
        merge_account_preferences(&empty, &design, &photo_schema),
        Err(AccountError::Invalid)
    );
}

#[test]
fn the_digit_order_and_follow_system_haptics_travel() {
    let local = Preferences {
        touch_number_keypad_order: NumberKeypadOrder::Calculator,
        ..Preferences::default()
    };
    assert_eq!(
        exported(&local)["platform.harmony.number_keypad_order"],
        string("calculator")
    );
    let applied = apply(
        &Preferences::default(),
        &[
            ("platform.harmony.number_keypad_order", string("calculator")),
            (HARMONY_FEEDBACK_KEYS[2], string("system")),
        ],
        &full_schema(),
    );
    assert_eq!(
        applied.preferences.touch_number_keypad_order,
        NumberKeypadOrder::Calculator
    );
    assert_eq!(applied.feedback.unwrap().haptic_strength, "system");
}

#[test]
fn the_twenty_six_key_digit_layer_travels() {
    let local = Preferences {
        touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout::NineKey,
        ..Preferences::default()
    };
    assert_eq!(
        exported(&local)["platform.harmony.twenty_six_key_number_layout"],
        string("nine_key")
    );
    let applied = apply(
        &Preferences::default(),
        &[(
            "platform.harmony.twenty_six_key_number_layout",
            string("nine_key"),
        )],
        &full_schema(),
    );
    assert_eq!(
        applied.preferences.touch_twenty_six_key_number_layout,
        TwentySixKeyNumberLayout::NineKey
    );
    // 文档里没有按键反馈的键：反馈文件不动。
    assert_eq!(applied.feedback, None);
}

#[test]
fn the_shuangpin_key_hint_switch_travels() {
    let local = Preferences {
        touch_shuangpin_key_hints: true,
        ..Preferences::default()
    };
    assert_eq!(
        exported(&local)["platform.harmony.shuangpin_key_hints"],
        AccountPreferenceValue::Boolean(true)
    );
    let applied = apply(
        &local,
        &[(
            "platform.harmony.shuangpin_key_hints",
            AccountPreferenceValue::Boolean(false),
        )],
        &full_schema(),
    );
    assert!(!applied.preferences.touch_shuangpin_key_hints);
}

#[test]
fn applying_writes_only_what_the_schema_declares() {
    let schema = full_schema();
    let mut local = Preferences::default();
    local.frequency.mode = FrequencyMode::Promote;
    let settings = [
        ("input.schema", string("wubi")),
        ("input.learning", AccountPreferenceValue::Boolean(false)),
        (
            "input.frequency_trigger_count",
            AccountPreferenceValue::Integer(5),
        ),
        ("platform.harmony.global_theme", string("paper")),
        ("platform.harmony.custom_candidate_skin", string("harbour")),
        (
            "platform.harmony.custom_candidate_skin_dark",
            string("dusk"),
        ),
    ];
    let applied = apply(&local, &settings, &schema);
    assert_eq!(applied.preferences.scheme, InputScheme::Wubi);
    assert!(!applied.preferences.learning);
    assert_eq!(applied.preferences.frequency.trigger_count, 5);
    // 云端没说的成员保持原样。
    assert_eq!(applied.preferences.frequency.mode, FrequencyMode::Promote);
    assert_eq!(applied.preferences.global_theme, GlobalTheme::Paper);
    assert_eq!(
        applied.preferences.custom_theme.candidate_skin.as_deref(),
        Some("harbour")
    );
    assert_eq!(
        applied
            .preferences
            .custom_theme
            .candidate_skin_dark
            .as_deref(),
        Some("dusk")
    );
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    // 三个按键反馈键都没出现：反馈文件不重写。
    assert_eq!(applied.feedback, None);

    // `system` 底色、空串的皮肤包和设计把它们清掉，账号不带的取色器保持原样。
    let mut themed = Preferences::default();
    themed.custom_theme.base = GlobalTheme::Ink;
    themed.custom_theme.candidate_skin = Some("harbour".into());
    themed.custom_theme.candidate_skin_dark = Some("dusk".into());
    themed.custom_theme.keyboard = Some(TouchKeyboardSkinDesign::default());
    themed.custom_theme.candidate_colors.text = Some("#112233".into());
    let cleared = apply(
        &themed,
        &[
            ("platform.harmony.custom_theme_base", string("system")),
            ("platform.harmony.custom_candidate_skin", string("")),
            ("platform.harmony.custom_candidate_skin_dark", string("")),
            ("platform.harmony.custom_keyboard_skin", string("")),
        ],
        &schema,
    );
    let theme = &cleared.preferences.custom_theme;
    assert_eq!(theme.base, GlobalTheme::System);
    assert_eq!(theme.candidate_skin, None);
    assert_eq!(theme.candidate_skin_dark, None);
    assert_eq!(theme.keyboard, None);
    assert_eq!(theme.candidate_colors.text.as_deref(), Some("#112233"));

    // 只带浅色槽位的文档（上传它的设备还不认识深色槽位）不动本机的深色槽位。
    let light_only = apply(
        &themed,
        &[("platform.harmony.custom_candidate_skin", string("sakura"))],
        &schema,
    );
    assert_eq!(
        light_only
            .preferences
            .custom_theme
            .candidate_skin
            .as_deref(),
        Some("sakura")
    );
    assert_eq!(
        light_only
            .preferences
            .custom_theme
            .candidate_skin_dark
            .as_deref(),
        Some("dusk")
    );

    // 服务端还没声明的键不读：这是所有 `platform.harmony.*` 在服务端收录之前的状态，必须是安静的那种情况。
    let bare = schema_for(&[("input.schema", "string")]);
    let partial = apply(&local, &settings, &bare);
    assert_eq!(partial.preferences.scheme, InputScheme::Wubi);
    assert!(partial.preferences.learning);
    assert_eq!(partial.preferences.global_theme, GlobalTheme::System);
    assert!(partial.skipped.is_empty());

    // 只有反馈键改了的成员变，其余保留本机的值。
    let with_feedback = apply(
        &local,
        &[(
            HARMONY_FEEDBACK_KEYS[0],
            AccountPreferenceValue::Boolean(false),
        )],
        &schema,
    );
    assert_eq!(
        with_feedback.feedback,
        Some(HostKeyboardFeedback {
            sound_enabled: false,
            ..feedback()
        })
    );
}

#[test]
fn a_value_this_host_has_no_meaning_for_skips_only_its_key() {
    let schema = wubi_schema();
    let local = Preferences {
        global_theme: GlobalTheme::Ink,
        ..Preferences::default()
    };
    for (key, value) in [
        ("platform.harmony.global_theme", string("midnight")),
        ("platform.harmony.custom_theme_base", string("custom")),
        ("platform.harmony.custom_candidate_skin", string("ink")),
        (
            "platform.harmony.custom_candidate_skin",
            string("../escape"),
        ),
        (
            "platform.harmony.custom_candidate_skin_dark",
            string("night"),
        ),
        (
            "platform.harmony.custom_candidate_skin_dark",
            string("../escape"),
        ),
        ("platform.harmony.custom_keyboard_skin", string("{not json")),
        ("platform.harmony.number_keypad_order", string("abacus")),
        (
            "platform.harmony.twenty_six_key_number_layout",
            string("grid"),
        ),
        ("platform.harmony.theme", string("sepia")),
        (
            "platform.harmony.twenty_six_key_style",
            string("split_space"),
        ),
        (
            "platform.harmony.fourteen_key_style",
            string("comma_period"),
        ),
        ("platform.harmony.nine_key_style", string("long_space")),
        ("platform.harmony.number_pad_space", string("space_in_grid")),
        ("platform.harmony.language_key_side", string("before_space")),
        ("platform.harmony.nine_key_swipe", string("both_ways")),
        (HARMONY_KEYBOARD_LAYOUT, string("future_layout")),
        (HARMONY_FEEDBACK_KEYS[2], string("loud")),
        (
            "platform.harmony.touch_key_spacing_tenths",
            AccountPreferenceValue::Integer(200),
        ),
        (
            "platform.harmony.keyboard_height_adjustment",
            AccountPreferenceValue::Integer(-100),
        ),
        ("input.character_set", string("esperanto")),
        ("input.wubi_schema", string("wubi06")),
        ("input.quote_style", string("semantic")),
    ] {
        let applied = apply(
            &local,
            &[
                (key, value.clone()),
                ("input.learning", AccountPreferenceValue::Boolean(false)),
            ],
            &schema,
        );
        assert_eq!(applied.skipped, [key], "{key} = {value:?}");
        // 其余的键照常应用，跳过的那个键保持本机的值。
        assert!(!applied.preferences.learning, "{key}");
        let mut expected = local.clone();
        expected.learning = false;
        assert_eq!(applied.preferences, expected, "{key} = {value:?}");
    }
}

#[test]
fn an_unknown_scheme_keeps_the_local_one_and_the_rest_still_applies() {
    let local = Preferences {
        scheme: InputScheme::Wubi,
        ..Preferences::default()
    };
    for unknown in [
        "cantonese",
        "zhuyin",
        "vietnamese",
        "tibetan",
        "stroke",
        "esperanto",
    ] {
        let kept = apply(
            &local,
            &[
                ("input.schema", string(unknown)),
                ("input.learning", AccountPreferenceValue::Boolean(false)),
            ],
            &full_schema(),
        );
        assert_eq!(kept.preferences.scheme, InputScheme::Wubi, "{unknown}");
        assert!(!kept.preferences.learning, "{unknown}");
    }
}

#[test]
fn a_document_that_contradicts_the_schema_is_refused_whole() {
    let schema = full_schema();
    // 字段表声明了的键带着别的类型来。
    assert_eq!(
        apply_harmony_settings(
            &Preferences::default(),
            &cloud(&[("input.learning", AccountPreferenceValue::Integer(1))]),
            &schema,
            Some(feedback()),
        ),
        Err(AccountError::Invalid)
    );
    // 关掉修订号检查的字段表不可信。
    let mut unchecked = schema;
    unchecked.revision_required = false;
    assert_eq!(
        apply_harmony_settings(
            &Preferences::default(),
            &cloud(&[("input.learning", AccountPreferenceValue::Boolean(false))]),
            &unchecked,
            Some(feedback()),
        ),
        Err(AccountError::Unavailable)
    );
}

#[test]
fn the_host_feedback_is_needed_only_when_the_document_has_it() {
    let schema = full_schema();
    let plain = cloud(&[("input.learning", AccountPreferenceValue::Boolean(false))]);
    assert!(!needs_harmony_feedback(&plain, &schema));
    assert!(apply_harmony_settings(&Preferences::default(), &plain, &schema, None).is_ok());
    let with_sound = cloud(&[(
        HARMONY_FEEDBACK_KEYS[0],
        AccountPreferenceValue::Boolean(false),
    )]);
    assert!(needs_harmony_feedback(&with_sound, &schema));
    assert_eq!(
        apply_harmony_settings(&Preferences::default(), &with_sound, &schema, None),
        Err(AccountError::Storage)
    );
    // Android 的反馈键不算。
    let android = cloud(&[(
        "platform.android.sound_enabled",
        AccountPreferenceValue::Boolean(false),
    )]);
    assert!(!needs_harmony_feedback(
        &android,
        &schema_for(&[("platform.android.sound_enabled", "boolean")])
    ));
}

#[test]
fn another_platforms_keys_never_reach_this_host() {
    let schema = schema_for(&[
        ("platform.android.keyboard_layout", "string"),
        ("platform.android.global_theme", "string"),
        ("platform.android.touch_key_spacing_tenths", "integer"),
        ("platform.android.sound_enabled", "boolean"),
        ("platform.ios.keyboard_layout", "string"),
    ]);
    let local = Preferences::default();
    let applied = apply(
        &local,
        &[
            ("platform.android.keyboard_layout", string("nine_key")),
            ("platform.android.global_theme", string("ink")),
            (
                "platform.android.touch_key_spacing_tenths",
                AccountPreferenceValue::Integer(40),
            ),
            (
                "platform.android.sound_enabled",
                AccountPreferenceValue::Boolean(false),
            ),
            ("platform.ios.keyboard_layout", string("handwriting")),
        ],
        &schema,
    );
    assert_eq!(applied.preferences, local);
    assert_eq!(applied.feedback, None);
    assert!(applied.skipped.is_empty());
}

#[test]
fn the_account_scheme_follows_the_edition_both_ways() {
    let schema = wubi_schema();
    let local = Preferences {
        scheme: InputScheme::Wubi,
        shuangpin_profile: ShuangpinProfile::Ziranma,
        wubi_profile: WubiProfile::Wubi98,
        ..Preferences::default()
    };
    let mut full = exported(&local);
    filter_uploaded_account_settings(Some(Edition::full()), &mut full);
    assert_eq!(full["input.schema"], string("wubi"));
    assert_eq!(full["input.shuangpin_schema"], string("ziranma"));
    assert_eq!(full["input.wubi_schema"], string("wubi98"));
    assert_eq!(full[HARMONY_KEYBOARD_LAYOUT], string("twenty_six_key"));
    // 单方案版本不上传方案和随它的布局，也不上传它不提供的双拼方案，自己的五笔版本照常上传。
    let mut wubi = exported(&local);
    filter_uploaded_account_settings(Edition::by_id("wubi"), &mut wubi);
    assert!(!wubi.contains_key("input.schema"));
    assert!(!wubi.contains_key(HARMONY_KEYBOARD_LAYOUT));
    assert!(!wubi.contains_key("input.shuangpin_schema"));
    assert_eq!(wubi["input.wubi_schema"], string("wubi98"));
    // 多方案版本上传自己的方案，从不上传五笔版本。
    let mut pinyin = exported(&Preferences {
        scheme: InputScheme::Shuangpin,
        shuangpin_profile: ShuangpinProfile::Ziranma,
        ..Preferences::default()
    });
    filter_uploaded_account_settings(Edition::by_id("pinyin"), &mut pinyin);
    assert_eq!(pinyin["input.schema"], string("shuangpin"));
    assert!(!pinyin.contains_key("input.wubi_schema"));

    let downloaded = |edition: Option<&Edition>, local: &Preferences| {
        let mut document = cloud(&[
            ("input.schema", string("wubi")),
            ("input.character_set", string("traditional")),
            (HARMONY_KEYBOARD_LAYOUT, string("nine_key")),
        ]);
        filter_downloaded_account_settings(edition, &mut document.settings);
        apply_harmony_settings(local, &document, &schema, Some(feedback())).unwrap()
    };
    // 本版本没有的方案当作没有这一项，随它的布局也不动，其余照常应用。
    let on_pinyin = downloaded(Edition::by_id("pinyin"), &Preferences::default());
    assert_eq!(on_pinyin.preferences.scheme, InputScheme::Quanpin);
    assert_eq!(
        on_pinyin.preferences.touch_keyboard_layout,
        TouchKeyboardLayout::TwentySixKey
    );
    assert!(on_pinyin.preferences.traditional_chinese_output);
    let on_full = downloaded(Some(Edition::full()), &Preferences::default());
    assert_eq!(on_full.preferences.scheme, InputScheme::Wubi);
    assert_eq!(
        on_full.preferences.touch_keyboard_layout,
        TouchKeyboardLayout::NineKey
    );
    // 单方案版本从不采用账号里的方案。
    let mut quanpin = cloud(&[("input.schema", string("quanpin"))]);
    filter_downloaded_account_settings(Edition::by_id("wubi"), &mut quanpin.settings);
    let on_wubi = apply_harmony_settings(
        &Preferences {
            scheme: InputScheme::Wubi,
            ..Preferences::default()
        },
        &quanpin,
        &schema,
        None,
    )
    .unwrap();
    assert_eq!(on_wubi.preferences.scheme, InputScheme::Wubi);
}

#[test]
fn the_korean_scheme_travels() {
    let values = exported(&Preferences {
        scheme: InputScheme::Korean,
        ..Preferences::default()
    });
    assert_eq!(values["input.schema"], string("korean"));
    let applied = apply(
        &Preferences::default(),
        &[("input.schema", string("korean"))],
        &full_schema(),
    );
    assert_eq!(applied.preferences.scheme, InputScheme::Korean);
}

#[test]
fn the_wubi_profile_travels_as_the_account_wubi_schema() {
    let schema = wubi_schema();
    let wubi98 = exported(&Preferences {
        scheme: InputScheme::Wubi,
        wubi_profile: WubiProfile::Wubi98,
        ..Preferences::default()
    });
    assert_eq!(wubi98["input.schema"], string("wubi"));
    assert_eq!(wubi98["input.wubi_schema"], string("wubi98"));
    let sparse = exported(&Preferences {
        scheme: InputScheme::Wubi,
        ..Preferences::default()
    });
    assert_eq!(sparse["input.wubi_schema"], string("wubi86"));
    // 不在五笔上时不上传版本，免得盖掉账号里别的设备选的。
    let quanpin = exported(&Preferences {
        wubi_profile: WubiProfile::Wubi98,
        ..Preferences::default()
    });
    assert!(!quanpin.contains_key("input.wubi_schema"));

    let applied = apply(
        &Preferences::default(),
        &[
            ("input.schema", string("wubi")),
            ("input.wubi_schema", string("wubi98")),
        ],
        &schema,
    );
    assert_eq!(applied.preferences.scheme, InputScheme::Wubi);
    assert_eq!(applied.preferences.wubi_profile, WubiProfile::Wubi98);
    let back = apply(
        &applied.preferences,
        &[
            ("input.schema", string("wubi")),
            ("input.wubi_schema", string("wubi86")),
        ],
        &schema,
    );
    assert_eq!(back.preferences.wubi_profile, WubiProfile::Wubi86);
    // 还不认识 98 的设备写的文档没有这个键，保留本机版本。
    let older = apply(
        &Preferences {
            wubi_profile: WubiProfile::Wubi98,
            ..Preferences::default()
        },
        &[("input.schema", string("wubi"))],
        &schema,
    );
    assert_eq!(older.preferences.wubi_profile, WubiProfile::Wubi98);
    // 服务端没声明这个键时不读它。
    let undeclared = apply(
        &Preferences {
            wubi_profile: WubiProfile::Wubi98,
            ..Preferences::default()
        },
        &[
            ("input.schema", string("wubi")),
            ("input.wubi_schema", string("wubi86")),
        ],
        &full_schema(),
    );
    assert_eq!(undeclared.preferences.wubi_profile, WubiProfile::Wubi98);
}

#[test]
fn schemes_the_account_cannot_hold_leave_its_scheme_alone() {
    for scheme in [
        InputScheme::Cantonese,
        InputScheme::Zhuyin,
        InputScheme::Vietnamese,
        InputScheme::Tibetan,
        InputScheme::Stroke,
    ] {
        let values = exported(&Preferences {
            scheme,
            ..Preferences::default()
        });
        assert!(!values.contains_key("input.schema"), "{scheme:?}");
        let merged = merge_account_preferences(
            &cloud(&[("input.schema", string("wubi"))]),
            &values,
            &full_schema(),
        )
        .unwrap();
        assert_eq!(
            merged.settings["input.schema"],
            string("wubi"),
            "{scheme:?}"
        );
        // 上传之后合并结果写回本机：本机的方案不被云端留着的五笔盖掉。
        let local = Preferences {
            scheme,
            ..Preferences::default()
        };
        let written_back =
            apply_harmony_settings(&local, &merged, &full_schema(), Some(feedback())).unwrap();
        assert_eq!(written_back.preferences, local, "{scheme:?}");
        assert!(written_back.skipped.is_empty(), "{scheme:?}");
    }
}

/// 账号还没有收录的双拼方案（智能 ABC、自定义表）不上传 `input.shuangpin_schema`，账号里留着上次的内置方案；合并上传后写回本机、或者下载别的设备的设置时都不拿它盖掉本机的方案，与 Android 相同。鸿蒙 TS 按缺省值 `xiaohe` 上传、应用时照写（差异第 88 条）。
#[test]
fn shuangpin_profiles_the_account_cannot_hold_stay_on_the_device() {
    for profile in [ShuangpinProfile::SmartAbc, ShuangpinProfile::Custom] {
        let local = Preferences {
            scheme: InputScheme::Shuangpin,
            shuangpin_profile: profile,
            ..Preferences::default()
        };
        let values = exported(&local);
        assert!(
            !values.contains_key("input.shuangpin_schema"),
            "{profile:?}"
        );
        let merged = merge_account_preferences(
            &cloud(&[("input.shuangpin_schema", string("microsoft"))]),
            &values,
            &full_schema(),
        )
        .unwrap();
        assert_eq!(
            merged.settings["input.shuangpin_schema"],
            string("microsoft"),
            "{profile:?}"
        );
        let written_back =
            apply_harmony_settings(&local, &merged, &full_schema(), Some(feedback())).unwrap();
        assert_eq!(written_back.preferences, local, "{profile:?}");
        assert!(written_back.skipped.is_empty(), "{profile:?}");
    }
}

/// 数字后、字母后直出两半不上云，智能标点要其中之一为真才起作用：别的设备打开或关掉了智能标点时，两半跟着写成同一个值，与 Android 的 `apply_input_settings`、鸿蒙 TS 的 `applyAccountPreferences` 相同。
#[test]
fn a_smart_punctuation_change_from_another_device_writes_both_halves() {
    for enabled in [true, false] {
        let local = Preferences {
            smart_punctuation: !enabled,
            smart_punctuation_direct_digit: !enabled,
            smart_punctuation_direct_letter: !enabled,
            ..Preferences::default()
        };
        let applied = apply(
            &local,
            &[(
                "input.smart_punctuation",
                AccountPreferenceValue::Boolean(enabled),
            )],
            &full_schema(),
        );
        assert_eq!(applied.preferences.smart_punctuation, enabled);
        assert_eq!(applied.preferences.smart_punctuation_direct_digit, enabled);
        assert_eq!(applied.preferences.smart_punctuation_direct_letter, enabled);
        assert!(applied.skipped.is_empty());
    }
}

/// 云端的智能标点和本机相同时（包括这台设备自己上传后合并结果写回本机），本机单独调过的一半保持原样。
#[test]
fn an_unchanged_smart_punctuation_leaves_the_halves_alone() {
    let local = Preferences {
        smart_punctuation: true,
        smart_punctuation_direct_digit: true,
        smart_punctuation_direct_letter: false,
        ..Preferences::default()
    };
    let document = AccountPreferences {
        revision: 1,
        settings: exported(&local),
    };
    let applied =
        apply_harmony_settings(&local, &document, &full_schema(), Some(feedback())).unwrap();
    assert_eq!(applied.preferences, local);
    assert!(applied.skipped.is_empty());
}

#[test]
fn the_punctuation_styles_travel() {
    let values = exported(&Preferences {
        quote_style: QuoteStyle::CornerWhenTraditional,
        bracket_style: BracketStyle::Hollow,
        ..Preferences::default()
    });
    assert_eq!(
        values["input.quote_style"],
        string("corner_when_traditional")
    );
    assert_eq!(values["input.bracket_style"], string("hollow"));
    let applied = apply(
        &Preferences::default(),
        &[
            ("input.quote_style", string("corner")),
            ("input.bracket_style", string("hollow")),
        ],
        &full_schema(),
    );
    assert_eq!(applied.preferences.quote_style, QuoteStyle::Corner);
    assert_eq!(applied.preferences.bracket_style, BracketStyle::Hollow);
}

#[test]
fn every_exported_key_round_trips() {
    let mut local = Preferences {
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
        quote_style: QuoteStyle::CornerWhenTraditional,
        bracket_style: BracketStyle::Hollow,
        wubi_code_hint: !Preferences::default().wubi_code_hint,
        wubi_auto_commit_unique: !Preferences::default().wubi_auto_commit_unique,
        wubi_fifth_code_top_commit: !Preferences::default().wubi_fifth_code_top_commit,
        touch_keyboard_layout: TouchKeyboardLayout::NineKey,
        theme: crate::preferences::ThemeMode::Dark,
        touch_key_spacing_tenths: 40,
        touch_row_spacing_tenths: 60,
        touch_keyboard_height_adjustment: 10,
        touch_voice_shortcut: !Preferences::default().touch_voice_shortcut,
        touch_number_keypad_order: NumberKeypadOrder::Calculator,
        touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout::NineKey,
        touch_shuangpin_key_hints: !Preferences::default().touch_shuangpin_key_hints,
        touch_twenty_six_key_style: TouchTwentySixKeyStyle::Emoji,
        touch_fourteen_key_style: TouchFourteenKeyStyle::LongSpace,
        touch_nine_key_style: TouchNineKeyStyle::Emoji,
        touch_number_row: !Preferences::default().touch_number_row,
        touch_twenty_six_symbol_first: !Preferences::default().touch_twenty_six_symbol_first,
        touch_number_pad_space: crate::preferences::TouchNumberPadSpace::AfterZero,
        touch_nine_key_sidebar_symbols: vec!["，".into(), "……".into(), ":-)".into()],
        touch_nine_key_digit_sidebar_symbols: vec!["+".into(), "（）".into()],
        touch_one_handed_side_panel: crate::preferences::TouchOneHandedSidePanel::NumberPad,
        touch_language_key_side: crate::preferences::TouchLanguageKeySide::Left,
        touch_nine_key_swipe_digits: crate::preferences::TouchNineKeySwipeDigits::Down,
        global_theme: GlobalTheme::Custom,
        ..Preferences::default()
    };
    local.frequency.mode = FrequencyMode::Linear;
    local.frequency.trigger_count = 7;
    local.frequency.linear_step = 4;
    local.custom_theme.base = GlobalTheme::Night;
    local.custom_theme.candidate_skin = Some("sakura".into());
    local.custom_theme.candidate_skin_dark = Some("midnight".into());
    local.custom_theme.keyboard = Some(TouchKeyboardSkinDesign::default());
    local.validate().unwrap();
    let host = HostKeyboardFeedback {
        sound_enabled: false,
        haptics_enabled: true,
        haptic_strength: "strong".into(),
    };
    let document = AccountPreferences {
        revision: 1,
        settings: export_harmony_settings(&local, Some(&host)).unwrap(),
    };
    let applied = apply_harmony_settings(
        &Preferences::default(),
        &document,
        &wubi_schema(),
        Some(feedback()),
    )
    .unwrap();
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    assert_eq!(applied.preferences, local);
    assert_eq!(applied.feedback, Some(host));
}

/// 分离式键盘的主开关与 Android 的 `platform.android.split_keyboard` 一样同步，放在鸿蒙自己的键下；启用宽度、中间空隙和竖屏选项只留本机，不导出也不应用。
#[test]
fn split_keyboard_switch_syncs_under_the_harmony_key_and_its_bounds_stay_local() {
    let remote = Preferences {
        touch_keyboard_split: true,
        touch_keyboard_split_threshold: 720,
        touch_keyboard_split_gap: 35,
        touch_keyboard_split_portrait: true,
        ..Preferences::default()
    };
    let settings = exported(&remote);
    assert_eq!(
        settings["platform.harmony.split_keyboard"],
        AccountPreferenceValue::Boolean(true)
    );
    assert_eq!(
        exported(&Preferences::default())["platform.harmony.split_keyboard"],
        AccountPreferenceValue::Boolean(false)
    );
    assert!(!settings.contains_key("platform.android.split_keyboard"));
    assert!(settings.keys().all(|key| !key.contains("threshold")
        && !key.contains("gap")
        && !key.contains("portrait")));

    let local = Preferences::default();
    let applied = apply(
        &local,
        &[(
            "platform.harmony.split_keyboard",
            AccountPreferenceValue::Boolean(true),
        )],
        &full_schema(),
    );
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    assert!(applied.preferences.touch_keyboard_split);
    assert_eq!(applied.preferences.touch_keyboard_split_threshold, 600);
    assert_eq!(applied.preferences.touch_keyboard_split_gap, 25);

    // Android 的同名开关是另一台设备的选择，鸿蒙不读它。
    let mut schema = full_schema();
    schema.fields.insert(
        "platform.android.split_keyboard".into(),
        AccountPreferenceField {
            value_type: "boolean".into(),
        },
    );
    let applied = apply(
        &local,
        &[(
            "platform.android.split_keyboard",
            AccountPreferenceValue::Boolean(true),
        )],
        &schema,
    );
    assert!(!applied.preferences.touch_keyboard_split);

    // 字段表还没收录这个键时，应用不读它。
    let mut schema = full_schema();
    schema.fields.remove("platform.harmony.split_keyboard");
    let applied = apply(
        &local,
        &[(
            "platform.harmony.split_keyboard",
            AccountPreferenceValue::Boolean(true),
        )],
        &schema,
    );
    assert!(!applied.preferences.touch_keyboard_split);
}

/// 鸿蒙只导出共享设置页在它上面能改的两组新增共享偏好：符号模式、日期菜单和第二三候选；快捷短语混输、语法模型、AI 候选翻译、思考强度和桌面快捷键都不导出，应用时也不改本机。
#[test]
fn only_the_shared_settings_this_host_can_change_sync() {
    let mut remote = Preferences::default();
    remote.local_modes.symbol = !remote.local_modes.symbol;
    remote.local_modes.date_time_menu = !remote.local_modes.date_time_menu;
    remote.second_third_candidate.enabled = !remote.second_third_candidate.enabled;
    remote.mixed_input.quick_phrase = !remote.mixed_input.quick_phrase;
    remote.translation_ai = !remote.translation_ai;
    remote.ai_assistant.thinking = "high".into();
    let settings = exported(&remote);
    assert_eq!(
        settings["local_modes.symbol"],
        AccountPreferenceValue::Boolean(remote.local_modes.symbol)
    );
    assert_eq!(
        settings["local_modes.date_time_menu"],
        AccountPreferenceValue::Boolean(remote.local_modes.date_time_menu)
    );
    assert_eq!(
        settings["second_third_candidate.enabled"],
        AccountPreferenceValue::Boolean(remote.second_third_candidate.enabled)
    );
    assert_eq!(
        settings["second_third_candidate.keys"],
        string("semicolon_quote")
    );
    for key in [
        "mixed_input.quick_phrase",
        "mixed_input.quick_phrase_frequency",
        "sentence_association.grammar_model",
        "translation.ai",
        "ai_assistant.thinking",
        "voice_input.polish_thinking",
        "keybindings.toggle_english_candidates",
        "keybindings.toggle_full_width",
        "keybindings.toggle_punctuation",
    ] {
        assert!(!settings.contains_key(key), "{key}");
    }

    let local = Preferences::default();
    let mut schema = full_schema();
    for key in ["mixed_input.quick_phrase", "translation.ai"] {
        schema.fields.insert(
            key.into(),
            AccountPreferenceField {
                value_type: "boolean".into(),
            },
        );
    }
    let applied = apply(
        &local,
        &[
            ("local_modes.symbol", settings["local_modes.symbol"].clone()),
            (
                "local_modes.date_time_menu",
                settings["local_modes.date_time_menu"].clone(),
            ),
            (
                "second_third_candidate.enabled",
                settings["second_third_candidate.enabled"].clone(),
            ),
            ("second_third_candidate.keys", string("semicolon_quote")),
            (
                "mixed_input.quick_phrase",
                AccountPreferenceValue::Boolean(remote.mixed_input.quick_phrase),
            ),
            (
                "translation.ai",
                AccountPreferenceValue::Boolean(remote.translation_ai),
            ),
        ],
        &schema,
    );
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    assert_eq!(
        applied.preferences.local_modes.symbol,
        remote.local_modes.symbol
    );
    assert_eq!(
        applied.preferences.local_modes.date_time_menu,
        remote.local_modes.date_time_menu
    );
    assert_eq!(
        applied.preferences.second_third_candidate,
        remote.second_third_candidate
    );
    assert_eq!(
        applied.preferences.mixed_input.quick_phrase,
        local.mixed_input.quick_phrase
    );
    assert_eq!(applied.preferences.translation_ai, local.translation_ai);
}

/// 本地识别的停顿时间和判停灵敏度（#10310）只留在本机：改了它们，鸿蒙导出的账号键值一个也不变，应用云端设置时也保留本机的值。
#[test]
fn voice_pause_settings_never_reach_the_account() {
    let mut local = Preferences::default();
    local.voice_input.pause_millis = 4000;
    local.voice_input.stop_sensitivity = 20;
    assert_eq!(exported(&local), exported(&Preferences::default()));

    let applied = apply(
        &local,
        &[("input.schema", string("shuangpin"))],
        &full_schema(),
    );
    assert_eq!(applied.preferences.voice_input.pause_millis, 4000);
    assert_eq!(applied.preferences.voice_input.stop_sensitivity, 20);
}
