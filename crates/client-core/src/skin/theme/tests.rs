use super::*;
use crate::preferences::CustomCandidateColors;

/// The design's `THEMES` table (dc.html L1484-1491), in its own order: id, bg, panel, accent, text, kb.bg, kb.key, kb.spec, kb.fg, kb.sub.
const DESIGN: [(&str, [&str; 9]); 5] = [
    (
        "shuishan",
        [
            "#1E1F1C", "#2A2B27", "#7FE08E", "#FFFFFF", "#1E1F1C", "#2F302C", "#23241F", "#FFFFFF",
            "#9FB5A3",
        ],
    ),
    (
        "light",
        [
            "#E9E9E9", "#FFFFFF", "#005FB8", "#1A1A1A", "#E4E6EA", "#FFFFFF", "#C8CCD3", "#1A1A1A",
            "#6A6F76",
        ],
    ),
    (
        "paper",
        [
            "#E8E4DA", "#F7F5F0", "#2C7A4B", "#1A1E1B", "#E6E1D5", "#FBF9F4", "#D3CCBC", "#1A1E1B",
            "#6E6A5E",
        ],
    ),
    (
        "night",
        [
            "#0F1B22", "#16262F", "#4FD1C5", "#E6F1F4", "#0F1B22", "#1D3340", "#15252E", "#E6F1F4",
            "#86A6B0",
        ],
    ),
    (
        "ink",
        [
            "#0B0B0B", "#1A1A1A", "#FFFFFF", "#9A9A9A", "#111111", "#2A2A2A", "#1C1C1C", "#F2F2F2",
            "#9A9A9A",
        ],
    ),
];

#[test]
fn ids_round_trip_in_picker_order() {
    let ids: Vec<_> = GlobalTheme::ALL.iter().map(|theme| theme.id()).collect();
    assert_eq!(
        ids,
        ["system", "native", "shuishan", "light", "paper", "night", "ink", "custom"]
    );
    for theme in GlobalTheme::ALL {
        assert_eq!(GlobalTheme::from_id(theme.id()), Some(theme));
        assert_eq!(
            serde_json::to_value(theme).unwrap(),
            serde_json::Value::String(theme.id().to_owned())
        );
        assert_eq!(
            serde_json::from_value::<GlobalTheme>(theme.id().into()).unwrap(),
            theme
        );
    }
    assert_eq!(GlobalTheme::default(), GlobalTheme::System);
}

#[test]
fn unknown_and_retired_ids_are_refused() {
    for id in [
        "",
        "Night",
        " night",
        "willow_green",
        "fluent",
        "forest",
        "custom ",
        "水杉",
    ] {
        assert_eq!(GlobalTheme::from_id(id), None, "{id}");
        assert!(
            serde_json::from_value::<GlobalTheme>(id.into()).is_err(),
            "{id}"
        );
    }
    assert_eq!(GlobalTheme::from_id("ink"), Some(GlobalTheme::Ink));
    for theme in GlobalTheme::ALL {
        assert_eq!(
            theme.is_base(),
            theme == GlobalTheme::System || theme.builtin().is_some(),
            "{}",
            theme.id()
        );
    }
    assert!(!GlobalTheme::Custom.is_base());
    assert!(!GlobalTheme::Native.is_base());
}

#[test]
fn native_is_offered_only_on_ios() {
    use crate::host_surface::HostPlatform;
    assert_eq!(GlobalTheme::Native.title(), "原生");
    assert_eq!(
        GlobalTheme::Native.platforms(),
        Some(&[HostPlatform::Ios][..])
    );
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let ids: Vec<_> = catalog_for(platform)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        let expected: Vec<_> = GlobalTheme::ALL
            .into_iter()
            .filter(|theme| *theme != GlobalTheme::Native || platform == HostPlatform::Ios)
            .collect();
        assert_eq!(ids, expected, "{}", platform.as_str());
    }
    let native = catalog()
        .into_iter()
        .find(|entry| entry.id == GlobalTheme::Native)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap()["platforms"],
        serde_json::json!(["ios"])
    );
    let system = &catalog()[0];
    assert_eq!(
        serde_json::to_value(system).unwrap()["platforms"],
        serde_json::Value::Null
    );
}

#[test]
fn catalog_is_complete_and_copies_the_design() {
    let catalog = catalog();
    assert_eq!(catalog.len(), 8);
    assert_eq!(
        catalog.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        GlobalTheme::ALL
    );
    for entry in &catalog {
        assert!(!entry.title.is_empty());
    }
    for edge in [&catalog[0], &catalog[1], &catalog[7]] {
        assert!(edge.appearance.is_none());
        assert!(edge.preview.is_none());
        assert!(edge.candidate.is_none());
        assert!(edge.keyboard.is_none());
    }
    assert_eq!(BUILTIN_THEMES.len(), DESIGN.len());
    for (id, [bg, panel, accent, text, kb_bg, key, spec, fg, sub]) in DESIGN {
        let entry = catalog
            .iter()
            .find(|entry| entry.id.id() == id)
            .unwrap_or_else(|| panic!("{id} missing"));
        let preview = entry.preview.as_ref().unwrap();
        assert_eq!(
            (
                preview.background.as_str(),
                preview.panel.as_str(),
                preview.accent.as_str(),
                preview.text.as_str()
            ),
            (bg, panel, accent, text),
            "{id}"
        );
        let keyboard = entry.keyboard.as_ref().unwrap();
        assert_eq!(
            [
                keyboard.background.as_str(),
                &keyboard.key,
                &keyboard.function_key,
                &keyboard.text,
                &keyboard.secondary,
                &keyboard.accent,
            ],
            [kb_bg, key, spec, fg, sub, accent],
            "{id}"
        );
        // Readable on an accent fill by the same luminance rule the AI generator uses: dark text on the bright accents, white on the deep ones.
        let expected_on_accent = match id {
            "light" | "paper" => "#FFFFFF",
            _ => "#000000",
        };
        assert_eq!(keyboard.on_accent, expected_on_accent, "{id}");
        let candidate = entry.candidate.as_ref().unwrap();
        assert_eq!(
            candidate,
            &CandidateThemePalette {
                surface: Some(panel.into()),
                border: Some("#0000001F".into()),
                text: Some(text.into()),
                number: Some(sub.into()),
                secondary: Some(sub.into()),
                accent: Some(accent.into()),
                selected: Some(format!("{accent}24")),
                selected_text: Some(accent.into()),
                selected_number: Some(sub.into()),
                hover: Some(format!("{text}0F")),
                show_selected_bar: None,
            },
            "{id}"
        );
        let appearance = if matches!(id, "light" | "paper") {
            ThemeAppearance::Light
        } else {
            ThemeAppearance::Dark
        };
        assert_eq!(entry.appearance, Some(appearance), "{id}");
    }
}

#[test]
fn every_emitted_colour_is_hex6_or_hex8() {
    let valid = |value: &str| {
        value.strip_prefix('#').is_some_and(|hex| {
            matches!(hex.len(), 6 | 8)
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        })
    };
    for entry in catalog() {
        let value = serde_json::to_value(&entry).unwrap();
        for section in ["preview", "candidate", "keyboard"] {
            if let Some(object) = value[section].as_object() {
                for (key, color) in object {
                    if let Some(color) = color.as_str() {
                        assert!(valid(color), "{} {section}.{key} = {color}", entry.id.id());
                    }
                }
            }
        }
    }
}

#[test]
fn system_resolves_to_platform_tokens() {
    let resolved = resolve(
        GlobalTheme::System,
        &CustomTheme::default(),
        true,
        CandidateLayout::Vertical,
        None,
    );
    assert_eq!(resolved.source, ThemeSource::System);
    assert_eq!(resolved.appearance, None);
    assert_eq!(resolved.candidate, None);
    assert_eq!(resolved.keyboard, None);
    let value = serde_json::to_value(&resolved).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "id": "system", "source": "system", "appearance": null,
            "candidate": null, "keyboard": null, "candidate_skin": null
        })
    );
}

/// `native` 与 `system` 一样不带调色板，忽略自定义主题，只把自己的 id 带回去，宿主据此分辨两者。
#[test]
fn native_resolves_to_platform_tokens_under_its_own_id() {
    let resolved = resolve(
        GlobalTheme::Native,
        &sakura(),
        false,
        CandidateLayout::Horizontal,
        None,
    );
    assert_eq!(
        serde_json::to_value(&resolved).unwrap(),
        serde_json::json!({
            "id": "native", "source": "system", "appearance": null,
            "candidate": null, "keyboard": null, "candidate_skin": null
        })
    );
}

#[test]
fn builtin_themes_ignore_custom_data_and_mode() {
    let custom = CustomTheme {
        candidate_skin: Some("sakura".into()),
        candidate_colors: CustomCandidateColors {
            accent: Some("#FF0000".into()),
            ..CustomCandidateColors::default()
        },
        ..CustomTheme::default()
    };
    for theme in BUILTIN_THEMES {
        for dark in [false, true] {
            let resolved = resolve(theme.id, &custom, dark, CandidateLayout::Vertical, None);
            assert_eq!(resolved.source, ThemeSource::Builtin);
            assert_eq!(resolved.appearance, Some(theme.appearance));
            assert_eq!(resolved.candidate, Some(theme.candidate()));
            assert_eq!(resolved.keyboard, Some(theme.keyboard()));
            assert_eq!(resolved.candidate_skin, None);
        }
    }
}

/// A package whose light palette has a pink surface and whose dark palette has a navy one, so a test can tell which mode was applied.
fn package(base: GlobalTheme, light: bool, dark: bool) -> ThemePackage {
    let palette = |surface: &str| CandidatePalette {
        surface: Some(surface.into()),
        accent: Some("#ff69b4".into()),
        border: Some("rgba(0, 0, 0, 0.5)".into()),
        text: Some("not a colour".into()),
        show_selected_bar: Some(true),
        ..CandidatePalette::default()
    };
    ThemePackage {
        id: "sakura".into(),
        base,
        layouts: vec![CandidateLayout::Vertical],
        light: light.then(|| palette("#fff0f5")),
        dark: dark.then(|| palette("#102030")),
    }
}

fn sakura() -> CustomTheme {
    CustomTheme {
        candidate_skin: Some("sakura".into()),
        ..CustomTheme::default()
    }
}

#[test]
fn custom_over_system_without_package_or_pickers_draws_the_platform() {
    let resolved = resolve(
        GlobalTheme::Custom,
        &CustomTheme::default(),
        false,
        CandidateLayout::Vertical,
        None,
    );
    assert_eq!(resolved.source, ThemeSource::Custom);
    assert_eq!(resolved.appearance, None);
    assert_eq!(resolved.candidate, None);
    assert_eq!(resolved.keyboard, None);

    let designed = CustomTheme {
        keyboard: Some(TouchKeyboardSkinDesign::default()),
        ..CustomTheme::default()
    };
    assert_eq!(
        resolve(
            GlobalTheme::Custom,
            &designed,
            false,
            CandidateLayout::Vertical,
            None
        )
        .keyboard,
        Some(KeyboardThemePalette {
            background: "#E8F0EB".into(),
            key: "#FFFFFF".into(),
            function_key: "#185C47".into(),
            text: "#17251D".into(),
            secondary: "#17251D99".into(),
            accent: "#185C47".into(),
            on_accent: "#FFFFFF".into(),
        })
    );
}

#[test]
fn custom_over_a_builtin_base_without_package_keeps_the_base() {
    let paper = GlobalTheme::Paper.builtin().unwrap();
    let custom = CustomTheme {
        base: GlobalTheme::Paper,
        ..CustomTheme::default()
    };
    for dark in [false, true] {
        let resolved = resolve(
            GlobalTheme::Custom,
            &custom,
            dark,
            CandidateLayout::Vertical,
            None,
        );
        assert_eq!(resolved.source, ThemeSource::Custom);
        assert_eq!(resolved.appearance, Some(ThemeAppearance::Light));
        assert_eq!(resolved.candidate, Some(paper.candidate()));
        assert_eq!(resolved.keyboard, Some(paper.keyboard()));
        assert_eq!(resolved.candidate_skin, None);
    }

    let designed = CustomTheme {
        keyboard: Some(TouchKeyboardSkinDesign {
            accent: 0xF0F0F0,
            ..TouchKeyboardSkinDesign::default()
        }),
        ..custom
    };
    let keyboard = resolve(
        GlobalTheme::Custom,
        &designed,
        false,
        CandidateLayout::Vertical,
        None,
    )
    .keyboard
    .unwrap();
    assert_eq!(keyboard.accent, "#F0F0F0");
    assert_eq!(keyboard.on_accent, "#000000");
}

#[test]
fn custom_layers_base_package_and_pickers() {
    let custom = CustomTheme {
        candidate_colors: CustomCandidateColors {
            accent: Some("#112233".into()),
            hover: Some("#445566".into()),
            ..CustomCandidateColors::default()
        },
        ..sakura()
    };
    let night = GlobalTheme::Night.builtin().unwrap();
    // 深色模式下画 night 底的皮肤包，用它的深色配色。
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Night, true, true)),
    );
    assert_eq!(resolved.candidate_skin.as_deref(), Some("sakura"));
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Dark));
    let candidate = resolved.candidate.unwrap();
    assert_eq!(candidate.surface.as_deref(), Some("#102030"));
    assert_eq!(candidate.border.as_deref(), Some("#00000080"));
    // An unparseable package colour keeps the base theme's slot.
    assert_eq!(candidate.text.as_deref(), Some(night.text));
    assert_eq!(candidate.accent.as_deref(), Some("#112233"));
    assert_eq!(candidate.selected.as_deref(), Some("#11223324"));
    assert_eq!(candidate.selected_text.as_deref(), Some("#112233"));
    assert_eq!(candidate.hover.as_deref(), Some("#445566"));
    assert_eq!(candidate.number.as_deref(), Some(night.keyboard_secondary));
    assert_eq!(
        candidate.secondary.as_deref(),
        Some(night.keyboard_secondary)
    );
    assert_eq!(candidate.show_selected_bar, Some(true));
    assert_eq!(resolved.keyboard, Some(night.keyboard()));
}

#[test]
fn a_package_is_drawn_only_in_the_mode_of_its_base() {
    let paper = GlobalTheme::Paper.builtin().unwrap();
    // 浅色模式下画 paper 底的皮肤包，用它的浅色配色。
    let resolved = resolve(
        GlobalTheme::Custom,
        &sakura(),
        false,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Paper, true, false)),
    );
    assert_eq!(resolved.candidate_skin.as_deref(), Some("sakura"));
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Light));
    let candidate = resolved.candidate.unwrap();
    assert_eq!(candidate.surface.as_deref(), Some("#FFF0F5"));
    assert_eq!(candidate.text.as_deref(), Some(paper.text));
    assert_eq!(candidate.selected.as_deref(), Some("#FF69B424"));
    assert_eq!(candidate.selected_text.as_deref(), Some("#FF69B4"));

    // 同一个浅色皮肤到了深色模式不画；`custom.base` 是 system，于是画平台自己的。
    let resolved = resolve(
        GlobalTheme::Custom,
        &sakura(),
        true,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Paper, true, false)),
    );
    assert_eq!(resolved.candidate_skin, None);
    assert_eq!(resolved.appearance, None);
    assert_eq!(resolved.candidate, None);

    // 深色底、只声明浅色的包在深色模式下：包属于这个模式但没有这个模式的配色，底原样画。
    let night = GlobalTheme::Night.builtin().unwrap();
    let resolved = resolve(
        GlobalTheme::Custom,
        &sakura(),
        true,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Night, true, false)),
    );
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Dark));
    assert_eq!(resolved.candidate, Some(night.candidate()));

    // system 底的包两种模式都画，按模式取配色。
    for (dark, surface) in [(false, "#FFF0F5"), (true, "#102030")] {
        let resolved = resolve(
            GlobalTheme::Custom,
            &sakura(),
            dark,
            CandidateLayout::Vertical,
            Some(&package(GlobalTheme::System, true, true)),
        );
        assert_eq!(resolved.candidate_skin.as_deref(), Some("sakura"));
        assert_eq!(
            resolved.candidate.unwrap().surface.as_deref(),
            Some(surface)
        );
    }
}

#[test]
fn the_dark_slot_is_used_in_dark_mode() {
    let mut dusk = package(GlobalTheme::Night, false, true);
    dusk.id = "dusk".into();
    let custom = CustomTheme {
        candidate_skin_dark: Some("dusk".into()),
        ..sakura()
    };
    assert_eq!(custom.candidate_skin_for(false), Some("sakura"));
    assert_eq!(custom.candidate_skin_for(true), Some("dusk"));
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        Some(&dusk),
    );
    assert_eq!(resolved.candidate_skin.as_deref(), Some("dusk"));
    // 浅色模式取的是浅色槽位，交来深色槽位的包被当作 id 对不上而忽略。
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        Some(&dusk),
    );
    assert_eq!(resolved.candidate_skin, None);
    // 没设深色槽位的旧文档：深色模式也取 `candidate_skin`。
    assert_eq!(sakura().candidate_skin_for(true), Some("sakura"));
}

#[test]
fn a_skin_base_left_from_another_mode_follows_the_host() {
    // 应用深色皮肤时 `custom.base` 被写成 night；浅色模式下这款皮肤不画，底也不留在 night。
    let custom = CustomTheme {
        base: GlobalTheme::Night,
        ..sakura()
    };
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Night, false, true)),
    );
    assert_eq!(resolved.appearance, None);
    assert_eq!(resolved.candidate, None);
    assert_eq!(resolved.keyboard, None);
    // 皮肤包没装时也一样。
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        None,
    );
    assert_eq!(resolved.appearance, None);
    // 深色模式下 night 底照常。
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        None,
    );
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Dark));
    // 完全没设皮肤的自定义主题，底是用户自己选的，不受明暗规则影响。
    let resolved = resolve(
        GlobalTheme::Custom,
        &CustomTheme {
            base: GlobalTheme::Night,
            ..CustomTheme::default()
        },
        false,
        CandidateLayout::Vertical,
        None,
    );
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Dark));
}

#[test]
fn the_package_base_wins_over_the_custom_base() {
    let custom = CustomTheme {
        base: GlobalTheme::Light,
        ..sakura()
    };
    let resolved = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Ink, false, true)),
    );
    assert_eq!(resolved.appearance, Some(ThemeAppearance::Dark));
    assert_eq!(
        resolved.keyboard,
        Some(GlobalTheme::Ink.builtin().unwrap().keyboard())
    );
    assert_eq!(
        resolved.candidate.unwrap().surface.as_deref(),
        Some("#102030")
    );
}

#[test]
fn a_text_picker_carries_the_numbers_unless_they_are_picked() {
    let custom = CustomTheme {
        base: GlobalTheme::Light,
        candidate_colors: CustomCandidateColors {
            text: Some("#112233".into()),
            ..CustomCandidateColors::default()
        },
        ..CustomTheme::default()
    };
    let candidate = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        None,
    )
    .candidate
    .unwrap();
    assert_eq!(candidate.text.as_deref(), Some("#112233"));
    assert_eq!(candidate.number.as_deref(), Some("#1122339D"));
    assert_eq!(candidate.secondary.as_deref(), Some("#1122339D"));
    assert_eq!(candidate.selected_number.as_deref(), Some("#1122339D"));
    assert_eq!(candidate.hover.as_deref(), Some("#1122330F"));

    let custom = CustomTheme {
        candidate_colors: CustomCandidateColors {
            text: Some("#112233".into()),
            number: Some("#445566".into()),
            ..CustomCandidateColors::default()
        },
        ..custom
    };
    let candidate = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        None,
    )
    .candidate
    .unwrap();
    assert_eq!(candidate.number.as_deref(), Some("#445566"));
    assert_eq!(candidate.secondary.as_deref(), Some("#445566"));

    // Over system only the picked slots are drawn; the selection stays with the platform.
    let custom = CustomTheme {
        base: GlobalTheme::System,
        ..custom
    };
    let candidate = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        None,
    )
    .candidate
    .unwrap();
    assert_eq!(candidate.text.as_deref(), Some("#112233"));
    assert_eq!(candidate.selected, None);
    assert_eq!(candidate.selected_number, None);
    assert_eq!(candidate.hover, None);
}

#[test]
fn a_selected_picker_draws_readable_text_on_itself() {
    let resolve_selected = |base: GlobalTheme, selected: &str, package: Option<&ThemePackage>| {
        let custom = CustomTheme {
            base,
            candidate_skin: package.map(|package| package.id.clone()),
            candidate_colors: CustomCandidateColors {
                selected: Some(selected.into()),
                accent: Some("#112233".into()),
                text: Some("#445566".into()),
                ..CustomCandidateColors::default()
            },
            ..CustomTheme::default()
        };
        resolve(
            GlobalTheme::Custom,
            &custom,
            false,
            CandidateLayout::Vertical,
            package,
        )
        .candidate
        .unwrap()
    };

    // A light selection takes black text, whatever the accent says.
    let light = resolve_selected(GlobalTheme::Light, "#ffe680", None);
    assert_eq!(light.selected.as_deref(), Some("#FFE680"));
    assert_eq!(light.selected_text.as_deref(), Some("#000000"));
    assert_eq!(light.selected_number.as_deref(), Some("#0000009D"));
    // Unselected rows keep the picked colours.
    assert_eq!(light.text.as_deref(), Some("#445566"));
    assert_eq!(light.accent.as_deref(), Some("#112233"));

    // A dark selection takes white text.
    let dark = resolve_selected(GlobalTheme::Light, "#1a3d7c", None);
    assert_eq!(dark.selected_text.as_deref(), Some("#FFFFFF"));
    assert_eq!(dark.selected_number.as_deref(), Some("#FFFFFF9D"));

    // Over system, where nothing is derived from the accent, the picked selection still brings its text.
    let system = resolve_selected(GlobalTheme::System, "#1a3d7c", None);
    assert_eq!(system.selected_text.as_deref(), Some("#FFFFFF"));
    assert_eq!(system.selected_number.as_deref(), Some("#FFFFFF9D"));

    // A package selection is not a picker: the accent still colours the text on it.
    let custom = CustomTheme {
        base: GlobalTheme::Light,
        candidate_colors: CustomCandidateColors {
            accent: Some("#112233".into()),
            ..CustomCandidateColors::default()
        },
        ..sakura()
    };
    let mut packaged = package(GlobalTheme::Light, true, false);
    packaged.light.as_mut().unwrap().selected = Some("#ffe680".into());
    let candidate = resolve(
        GlobalTheme::Custom,
        &custom,
        false,
        CandidateLayout::Vertical,
        Some(&packaged),
    )
    .candidate
    .unwrap();
    assert_eq!(candidate.selected.as_deref(), Some("#FFE680"));
    assert_eq!(candidate.selected_text.as_deref(), Some("#112233"));

    // The picker wins over the package selection and answers for its own colour.
    let picked = resolve_selected(GlobalTheme::Light, "#000000", Some(&packaged));
    assert_eq!(picked.selected.as_deref(), Some("#000000"));
    assert_eq!(picked.selected_text.as_deref(), Some("#FFFFFF"));
}

#[test]
fn custom_package_is_used_only_for_declared_modes_and_its_own_id() {
    let dark = resolve(
        GlobalTheme::Custom,
        &sakura(),
        true,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::System, true, false)),
    );
    // No dark palette and a system base: nothing to draw but platform tokens, and the package is not drawn here, so a host draws none of its decoration either.
    assert_eq!(dark.candidate, None);
    assert_eq!(dark.candidate_skin, None);

    let other = CustomTheme {
        candidate_skin: Some("other".into()),
        ..CustomTheme::default()
    };
    let mismatched = resolve(
        GlobalTheme::Custom,
        &other,
        false,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::Paper, true, true)),
    );
    assert_eq!(mismatched.candidate, None);
    assert_eq!(mismatched.candidate_skin, None);
    assert_eq!(mismatched.appearance, None);

    let light = resolve(
        GlobalTheme::Custom,
        &sakura(),
        false,
        CandidateLayout::Vertical,
        Some(&package(GlobalTheme::System, true, false)),
    );
    assert_eq!(light.appearance, None);
    let candidate = light.candidate.unwrap();
    assert_eq!(candidate.accent.as_deref(), Some("#FF69B4"));
    // With a system base the derived selection slots stay with the platform.
    assert_eq!(candidate.selected_text, None);
    assert_eq!(candidate.selected, None);
}

#[test]
fn package_colours_normalize_to_hex() {
    for (input, expected) in [
        ("#abc", Some("#AABBCC")),
        ("#a1b2c3", Some("#A1B2C3")),
        ("#a1b2c3d4", Some("#A1B2C3D4")),
        ("transparent", Some("#00000000")),
        ("rgb(255, 0, 16)", Some("#FF0010")),
        ("rgba(0,0,0,0.12)", Some("#0000001F")),
        ("rgba(0,0,0,1.5)", None),
        ("rgb(256,0,0)", None),
        ("#12345", None),
        ("#ggg", None),
        ("red", None),
        ("", None),
    ] {
        assert_eq!(normalized_color(input).as_deref(), expected, "{input}");
    }
}

#[test]
fn summary_packages_keep_only_declared_modes() {
    let summary = SkinSummary {
        id: "sakura".into(),
        name: "樱花".into(),
        version: "1".into(),
        base: GlobalTheme::Paper,
        author: None,
        description: None,
        layouts: vec!["vertical".into()],
        themes: vec!["dark".into()],
        min_width_dip: 0.0,
        corner_radius_dip: None,
        decoration_top_dip: 0.0,
        decoration_width_dip: 0.0,
        decoration_image: None,
        decoration_align: Default::default(),
        background: None,
        toolbar: Default::default(),
        toolbar_stylesheet: None,
        preview: None,
        candidate: Default::default(),
        license: None,
    };
    let package = ThemePackage::from(&summary);
    assert_eq!(package.base, GlobalTheme::Paper);
    assert_eq!(package.layouts, vec![CandidateLayout::Vertical]);
    assert!(package.light.is_none());
    assert!(package.dark.is_some());
}

#[test]
fn a_package_is_not_drawn_in_a_layout_it_does_not_declare() {
    let custom = CustomTheme {
        candidate_colors: CustomCandidateColors {
            hover: Some("#445566".into()),
            ..CustomCandidateColors::default()
        },
        ..sakura()
    };
    let night = GlobalTheme::Night.builtin().unwrap();
    let vertical_only = package(GlobalTheme::Night, true, true);
    let horizontal = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Horizontal,
        Some(&vertical_only),
    );
    // The package's base and the pickers still draw; its own colours and its decoration do not.
    assert_eq!(horizontal.candidate_skin, None);
    assert_eq!(horizontal.appearance, Some(ThemeAppearance::Dark));
    assert_eq!(
        horizontal.candidate,
        Some(CandidateThemePalette {
            hover: Some("#445566".into()),
            ..night.candidate()
        })
    );
    assert_eq!(horizontal.keyboard, Some(night.keyboard()));

    let vertical = resolve(
        GlobalTheme::Custom,
        &custom,
        true,
        CandidateLayout::Vertical,
        Some(&vertical_only),
    );
    assert_eq!(vertical.candidate_skin.as_deref(), Some("sakura"));
    assert_eq!(
        vertical.candidate.unwrap().surface.as_deref(),
        Some("#102030")
    );
}

#[test]
fn host_catalog_entries_read_as_packages() {
    let package = ThemePackage::from_host_catalog_entry(serde_json::json!({
        "id": "sakura",
        "title": "樱花",
        "base": "night",
        "layouts": ["horizontal", "vertical"],
        "candidate": { "dark": { "accent": "#ff0000", "border": "transparent", "show_selected_bar": false } },
        "decoration_top_dip": 12.0,
        "decoration_width_dip": 40.0,
        "decoration_image": "/skins/sakura/preview.png",
        "decoration_align": "center",
        "corner_radius_dip": 12.0
    }))
    .unwrap();
    assert_eq!(package.id, "sakura");
    assert_eq!(package.base, GlobalTheme::Night);
    assert!(package.light.is_none());
    assert_eq!(
        package.layouts,
        vec![CandidateLayout::Horizontal, CandidateLayout::Vertical]
    );
    assert_eq!(
        package.dark.as_ref().unwrap().accent.as_deref(),
        Some("#ff0000")
    );
    assert_eq!(
        package.dark.as_ref().unwrap().show_selected_bar,
        Some(false)
    );
    let resolved = resolve(
        GlobalTheme::Custom,
        &CustomTheme {
            candidate_skin: Some("sakura".into()),
            ..Default::default()
        },
        true,
        CandidateLayout::Vertical,
        Some(&package),
    );
    let candidate = resolved.candidate.unwrap();
    assert_eq!(candidate.accent.as_deref(), Some("#FF0000"));
    assert_eq!(candidate.selected_text.as_deref(), Some("#FF0000"));
    assert_eq!(candidate.border.as_deref(), Some("#00000000"));

    let package = ThemePackage::from_host_catalog_entry(
        serde_json::json!({ "id": "sakura", "title": "樱花", "base": "system", "layouts": [] }),
    )
    .unwrap();
    assert_eq!(package.base, GlobalTheme::System);
    assert!(package.light.is_none() && package.dark.is_none());
    let entry = |extra: serde_json::Value| {
        let mut entry = serde_json::json!({ "id": "sakura", "title": "樱花", "base": "night", "layouts": ["vertical"] });
        for (key, value) in extra.as_object().unwrap() {
            if value.is_null() {
                entry.as_object_mut().unwrap().remove(key);
            } else {
                entry[key] = value.clone();
            }
        }
        entry
    };
    assert!(ThemePackage::from_host_catalog_entry(entry(serde_json::json!({}))).is_ok());
    for (extra, why) in [
        (serde_json::json!({ "base": "fluent" }), "retired base"),
        (serde_json::json!({ "base": "custom" }), "custom base"),
        (serde_json::json!({ "id": "night" }), "global theme id"),
        (serde_json::json!({ "id": "../x" }), "unsafe id"),
        (serde_json::json!({ "base": null }), "no base"),
        (serde_json::json!({ "title": null }), "no title"),
        (serde_json::json!({ "layouts": null }), "no layouts"),
        (
            serde_json::json!({ "layouts": ["diagonal"] }),
            "unknown layout",
        ),
        (
            serde_json::json!({ "candidate": { "sepia": {} } }),
            "unknown mode",
        ),
        (
            serde_json::json!({ "candidate": { "dark": { "showSelectedBar": false } } }),
            "camelCase palette key",
        ),
        (
            serde_json::json!({ "themes": ["dark"], "minWidthDip": 0.0 }),
            "SkinSummary keys",
        ),
    ] {
        assert!(
            ThemePackage::from_host_catalog_entry(entry(extra)).is_err(),
            "{why}"
        );
    }
}

/// The settings page draws the theme picker and its previews synchronously on every host, including hosts whose bridge has no theme call, so it reads a checked-in copy of `catalog()`. This keeps that copy equal to the table above; set `MSIME_WRITE_THEME_CATALOG=1` to rewrite it after changing a palette.
#[test]
fn web_catalog_copy_matches_the_catalog() {
    let catalog = serde_json::to_value(catalog()).expect("catalog serializes");
    if std::env::var_os("MSIME_WRITE_THEME_CATALOG").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/ui/src/theme/theme-catalog.json"
        );
        let text = serde_json::to_string_pretty(&catalog).expect("catalog prints") + "\n";
        std::fs::write(path, text).expect("web catalog copy is writable");
    }
    let copy: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../packages/ui/src/theme/theme-catalog.json"
    ))
    .expect("web catalog copy is JSON");
    assert_eq!(
        copy, catalog,
        "packages/ui/src/theme/theme-catalog.json is stale; rerun this test with MSIME_WRITE_THEME_CATALOG=1"
    );
}

/// The settings page previews a custom theme with `customCandidatePalette` (packages/ui/src/theme/global-theme.ts) on hosts without a theme call, and until the host answers on the rest. These cases pin that mirror to `resolve()`: `apps/desktop/tests/candidate/custom-theme-parity.test.ts` runs the mirror over the same file. Each case names the base (the package's manifest base when there is a package), the host mode, the package's declared modes and raw manifest palettes in the shape the skin scan gives them, and the pickers; `expected` is what `resolve()` draws. Set `MSIME_WRITE_THEME_CATALOG=1` to rewrite the file after changing `resolve()` or a case.
#[test]
fn web_custom_theme_mirror_cases_match_resolve() {
    use serde_json::{json, Value};
    let cases = json!([
        {
            "name": "a text picker beats the package text",
            "base": "paper", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"text": "#112233"}, "dark": {}}},
            "colors": {"text": "#ff0000"},
        },
        {
            "name": "a package accent derives the selected row over a built-in base",
            "base": "paper", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"accent": "#AA0000"}, "dark": {}}},
            "colors": {},
        },
        {
            "name": "a package translation colour is the secondary text",
            "base": "paper", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"number": "#123456", "translation": "rgba(1, 2, 3, .5)"}, "dark": {}}},
            "colors": {"text": "#abcdef"},
        },
        {
            "name": "the package number and converted notations are drawn",
            "base": "night", "dark": true,
            "package": {"themes": ["dark"], "candidate": {"light": {}, "dark": {"number": "#123456", "surface": "rgb(1, 2, 3)"}}},
            "colors": {},
        },
        {
            "name": "a package over system draws only its own mode, with the pickers on top",
            "base": "system", "dark": true,
            "package": {"themes": ["dark", "light"], "candidate": {
                "dark": {"surface": "#abc", "border": "rgba(0, 0, 0, 0.5)", "text": "transparent", "hover": "not a colour", "showSelectedBar": false},
                "light": {"surface": "#ffffff", "accent": "#00ff00"},
            }},
            "colors": {"hover": "#010203"},
        },
        {
            "name": "an undeclared mode draws the base and the pickers without the package",
            "base": "system", "dark": true,
            "package": {"themes": ["light"], "candidate": {"light": {"surface": "#FFF0F5"}, "dark": {"surface": "#102030"}}},
            "colors": {"accent": "#2c7a4b"},
        },
        {
            "name": "a package that does not declare its base's mode leaves the base unchanged",
            "base": "ink", "dark": true,
            "package": {"themes": ["light"], "candidate": {"light": {"surface": "#FFF0F5"}, "dark": {"surface": "#102030"}}},
            "colors": {},
        },
        {
            "name": "a text picker sets the numbers over the package number",
            "base": "light", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"number": "#010101", "selected": "#11111180"}, "dark": {}}},
            "colors": {"text": "#202020"},
        },
        {
            "name": "unparseable package colours are left out",
            "base": "system", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"accent": "RGBA(255,0,0,1)", "selected": "rgb(256, 0, 0)", "border": "#12345", "surface": "rgba(1, 2, 3, 1.5)", "showSelectedBar": true}, "dark": {}}},
            "colors": {},
        },
        {
            "name": "pickers over a built-in base without a package",
            "base": "ink", "dark": false, "package": null,
            "colors": {"text": "#abcdef", "number": "#123456", "surface": "#000000"},
        },
        {
            "name": "custom over system with nothing set draws the platform",
            "base": "system", "dark": false, "package": null, "colors": {},
        },
        {
            "name": "a light selected picker over a built-in base reads in black",
            "base": "paper", "dark": false, "package": null,
            "colors": {"selected": "#FFE680"},
        },
        {
            "name": "a dark selected picker over a built-in base reads in white",
            "base": "ink", "dark": false, "package": null,
            "colors": {"selected": "#1A3D7C"},
        },
        {
            "name": "a selected picker over system derives its own text",
            "base": "system", "dark": true, "package": null,
            "colors": {"selected": "#FFE680"},
        },
        {
            "name": "a selected picker beats the package selected row and derives its text",
            "base": "paper", "dark": false,
            "package": {"themes": ["light"], "candidate": {"light": {"selected": "#11111180", "accent": "#AA0000"}, "dark": {}}},
            "colors": {"selected": "#1A3D7C"},
        },
        {
            "name": "a dark package is not drawn in light mode and the base follows the host",
            "base": "night", "dark": false,
            "package": {"themes": ["dark"], "candidate": {"light": {}, "dark": {"surface": "#102030", "accent": "#FF69B4"}}},
            "colors": {"hover": "#010203"},
        },
        {
            "name": "a light package is not drawn in dark mode",
            "base": "paper", "dark": true,
            "package": {"themes": ["light"], "candidate": {"light": {"surface": "#FFF0F5"}, "dark": {}}},
            "colors": {},
        },
    ]);
    let palette = |value: &Value| CandidatePalette {
        surface: value["surface"].as_str().map(Into::into),
        border: value["border"].as_str().map(Into::into),
        text: value["text"].as_str().map(Into::into),
        number: value["number"].as_str().map(Into::into),
        accent: value["accent"].as_str().map(Into::into),
        selected: value["selected"].as_str().map(Into::into),
        hover: value["hover"].as_str().map(Into::into),
        translation: value["translation"].as_str().map(Into::into),
        show_selected_bar: value["showSelectedBar"].as_bool(),
    };
    let mut expected = Vec::new();
    for case in cases.as_array().expect("cases") {
        let base: GlobalTheme = serde_json::from_value(case["base"].clone()).expect("base");
        let declares = |mode: &str| {
            case["package"]["themes"]
                .as_array()
                .is_some_and(|themes| themes.iter().any(|theme| theme == mode))
        };
        let package = (!case["package"].is_null()).then(|| ThemePackage {
            id: "sample".into(),
            base,
            layouts: vec![CandidateLayout::Vertical],
            light: declares("light").then(|| palette(&case["package"]["candidate"]["light"])),
            dark: declares("dark").then(|| palette(&case["package"]["candidate"]["dark"])),
        });
        let custom = CustomTheme {
            base,
            candidate_skin: package.as_ref().map(|package| package.id.clone()),
            candidate_colors: serde_json::from_value(case["colors"].clone()).expect("colors"),
            ..CustomTheme::default()
        };
        let resolved = resolve(
            GlobalTheme::Custom,
            &custom,
            case["dark"].as_bool().expect("dark"),
            CandidateLayout::Vertical,
            package.as_ref(),
        );
        let mut case = case.clone();
        case["expected"] = serde_json::to_value(resolved.candidate).expect("palette serializes");
        expected.push(case);
    }
    let expected = Value::Array(expected);
    if std::env::var_os("MSIME_WRITE_THEME_CATALOG").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/desktop/tests/candidate/custom-theme-parity.json"
        );
        let text = serde_json::to_string_pretty(&expected).expect("cases print") + "\n";
        std::fs::write(path, text).expect("parity cases are writable");
    }
    let copy: Value = serde_json::from_str(include_str!(
        "../../../../../apps/desktop/tests/candidate/custom-theme-parity.json"
    ))
    .expect("parity cases are JSON");
    assert_eq!(
        copy, expected,
        "apps/desktop/tests/candidate/custom-theme-parity.json is stale; rerun this test with MSIME_WRITE_THEME_CATALOG=1"
    );
}

#[test]
fn a_package_translation_colour_is_the_secondary_text() {
    let custom = CustomTheme {
        candidate_skin: Some("sakura".into()),
        ..Default::default()
    };
    let package = |translation: Option<&str>| ThemePackage {
        id: "sakura".into(),
        base: GlobalTheme::Paper,
        layouts: vec![CandidateLayout::Vertical],
        light: Some(CandidatePalette {
            number: Some("#123456".into()),
            translation: translation.map(Into::into),
            ..Default::default()
        }),
        dark: None,
    };
    let secondary = |package: &ThemePackage| {
        resolve(
            GlobalTheme::Custom,
            &custom,
            false,
            CandidateLayout::Vertical,
            Some(package),
        )
        .candidate
        .unwrap()
        .secondary
    };
    assert_eq!(
        secondary(&package(Some("rgba(1, 2, 3, 0.5)"))).as_deref(),
        Some("#01020380")
    );
    // Without one, or with one no host could read, it follows the numbers as before.
    assert_eq!(secondary(&package(None)).as_deref(), Some("#123456"));
    assert_eq!(
        secondary(&package(Some("blue"))).as_deref(),
        Some("#123456")
    );
}
