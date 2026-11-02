//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;

#[test]
fn every_route_round_trips_through_its_argument() {
    for route in SurfaceRoute::ALL {
        assert_eq!(SurfaceRoute::parse(&route.as_arg()), Ok(route));
    }
    for category in SettingsCategory::ALL {
        let route = SurfaceRoute::Settings(Some(category));
        assert_eq!(SurfaceRoute::parse(&route.as_arg()), Ok(route));
    }
}

#[test]
fn settings_deep_link_names_a_category() {
    assert_eq!(
        SurfaceRoute::parse("settings:voice"),
        Ok(SurfaceRoute::Settings(Some(SettingsCategory::Voice)))
    );
    assert_eq!(
        SurfaceRoute::parse("settings:floating-toolbar"),
        Ok(SurfaceRoute::Settings(Some(
            SettingsCategory::FloatingToolbar
        )))
    );
    // The pages the redesigned navigation added are routable by their page ids.
    for (argument, category) in [
        ("settings:expression", SettingsCategory::Expression),
        ("settings:developer", SettingsCategory::Developer),
        ("settings:mcp", SettingsCategory::Mcp),
        ("settings:plugins", SettingsCategory::Plugins),
    ] {
        assert_eq!(
            SurfaceRoute::parse(argument),
            Ok(SurfaceRoute::Settings(Some(category)))
        );
    }
    assert_eq!(
        SurfaceRoute::parse("settings"),
        Ok(SurfaceRoute::Settings(None))
    );
}

#[test]
fn malformed_routes_are_rejected_rather_than_guessed() {
    assert_eq!(SurfaceRoute::parse(""), Err(RouteError::Empty));
    assert_eq!(
        SurfaceRoute::parse(&"a".repeat(MAX_ROUTE_BYTES + 1)),
        Err(RouteError::TooLong)
    );
    assert_eq!(
        SurfaceRoute::parse("Settings"),
        Err(RouteError::IllegalCharacter)
    );
    assert_eq!(
        SurfaceRoute::parse("emoji\n"),
        Err(RouteError::IllegalCharacter)
    );
    assert_eq!(
        SurfaceRoute::parse("emoji panel"),
        Err(RouteError::IllegalCharacter)
    );
    assert_eq!(
        SurfaceRoute::parse("settings:voice:extra"),
        Err(RouteError::Unknown)
    );
    assert_eq!(
        SurfaceRoute::parse("settings:unknown"),
        Err(RouteError::Unknown)
    );
    assert_eq!(SurfaceRoute::parse("emoji:input"), Err(RouteError::Unknown));
    assert_eq!(SurfaceRoute::parse("account"), Err(RouteError::Unknown));
}

#[test]
fn settings_routes_name_the_section_to_open() {
    assert_eq!(
        SurfaceRoute::parse("settings:about")
            .unwrap()
            .settings_category(),
        Some(SettingsCategory::About)
    );
    assert_eq!(
        SurfaceRoute::parse("settings:dictionary")
            .unwrap()
            .settings_category(),
        Some(SettingsCategory::Dictionary)
    );
    // A bare settings route keeps whichever page the shared UI defaults to.
    assert_eq!(SurfaceRoute::Settings(None).settings_category(), None);
    // A panel route never selects a settings section.
    assert_eq!(SurfaceRoute::Emoji.settings_category(), None);
    assert_eq!(SurfaceRoute::Clipboard.settings_category(), None);

    // Every category round-trips through the route a launcher would emit.
    for category in SettingsCategory::ALL {
        let argument = SurfaceRoute::Settings(Some(category)).as_arg();
        assert_eq!(
            SurfaceRoute::parse(&argument).unwrap().settings_category(),
            Some(category)
        );
    }
}

#[test]
fn panel_routes_keep_the_labels_the_hosts_already_use() {
    assert_eq!(SurfaceRoute::Settings(None).panel(), None);
    assert_eq!(
        SurfaceRoute::Settings(Some(SettingsCategory::Input)).panel(),
        None
    );
    let keyboard = SurfaceRoute::Keyboard.panel().expect("keyboard is a panel");
    assert_eq!(keyboard.label, "keyboard-panel");
    assert_eq!(keyboard.query, "keyboard");
    // 背单词面板与设置里的背单词页是两条路由：原生设置窗口的「开始复习」打开面板，`settings:vocabulary` 仍打开设置页。
    assert_eq!(
        SurfaceRoute::parse("vocabulary"),
        Ok(SurfaceRoute::Vocabulary)
    );
    let vocabulary = SurfaceRoute::Vocabulary
        .panel()
        .expect("vocabulary is a panel");
    assert_eq!(
        (vocabulary.label, vocabulary.width, vocabulary.height),
        ("vocabulary-panel", 560, 680)
    );
    assert_eq!(
        SurfaceRoute::parse("settings:vocabulary")
            .unwrap()
            .settings_category(),
        Some(SettingsCategory::Vocabulary)
    );
    for route in SurfaceRoute::ALL {
        let Some(panel) = route.panel() else { continue };
        // The window label is the query with the shared panel suffix.
        assert_eq!(panel.label, format!("{}-panel", panel.query));
        assert!(panel.width > 0 && panel.height > 0);
    }
}

#[test]
fn windows_panels_open_where_the_shipped_native_panels_did() {
    let emoji = SurfaceRoute::Emoji
        .panel_for(HostPlatform::Windows)
        .expect("emoji is a panel");
    assert_eq!(
        (emoji.width, emoji.height, emoji.placement),
        (550, 610, PanelPlacement::Center)
    );
    let handwriting = SurfaceRoute::Handwriting
        .panel_for(HostPlatform::Windows)
        .expect("handwriting is a panel");
    assert_eq!(
        (handwriting.width, handwriting.height, handwriting.placement),
        (980, 650, PanelPlacement::Center)
    );
    let keyboard = SurfaceRoute::Keyboard
        .panel_for(HostPlatform::Windows)
        .expect("keyboard is a panel");
    assert_eq!(
        (keyboard.width, keyboard.height, keyboard.placement),
        (1100, 400, PanelPlacement::BottomCenter)
    );
    for route in SurfaceRoute::ALL {
        // Labels and routes never change per host, only the geometry.
        let windows = route.panel_for(HostPlatform::Windows);
        assert_eq!(
            windows.map(|panel| (panel.label, panel.query, panel.title)),
            route
                .panel()
                .map(|panel| (panel.label, panel.query, panel.title))
        );
        // Every other host keeps the shared geometry.
        for platform in [HostPlatform::Macos, HostPlatform::Linux] {
            assert_eq!(route.panel_for(platform), route.panel());
        }
    }
}

#[test]
fn capabilities_describe_each_host() {
    let linux = HostCapabilities::for_platform(HostPlatform::Linux);
    assert!(!linux.mobile_settings);
    assert!(linux.restart_input_method);
    assert!(linux.ime_mode_scope);
    assert!(linux.panel_windows);
    // 共享设置页经 Tauri 装入编辑好的候选窗皮肤。
    assert!(linux.candidate_skin_editor);
    assert!(linux.mode_switch_shortcuts);
    // Linux 在 X11 上画悬浮工具栏，缩放和字号与 macOS 一样生效；画不出来的会话只剩「工具栏」子菜单。
    assert!(linux.floating_toolbar);
    assert!(linux.floating_toolbar_appearance);
    assert!(linux.floating_toolbar_components);
    // Linux 的「工具栏」子菜单与 macOS 工具栏一样列出切换输入方案、手写和语音三项，设置页给它们开关。
    assert!(
        linux.floating_toolbar_handwriting
            && linux.floating_toolbar_voice
            && linux.floating_toolbar_input_scheme
    );
    // 自动隐藏、延时与阴影只有 macOS 的面板读：Linux 是输入法菜单，Windows 与
    // HarmonyOS 的面板还没接，所以共享设置页在它们上面不列这三行。
    assert!(!linux.floating_toolbar_idle_behavior);
    assert!(linux.panel_shortcuts);
    assert!(linux.number_row_selection);
    assert!(linux.voice_capture_devices);
    assert!(linux.candidate_font_controls);
    assert!(linux.candidate_window_scale);
    assert!(!linux.candidate_preedit_font);
    assert!(linux.candidate_row_colors);
    assert!(!linux.candidate_selection_appearance);
    // Fcitx5 经典界面的边框跟随主题和皮肤，宿主不读用户选的边框色。
    assert!(!linux.candidate_border_color);
    // IBus owns the candidate list's placement, so the host cannot pin it.
    assert!(!linux.candidate_follow_cursor);
    // The host chooses the preedit string the panel draws, so the raw keys
    // and the expanded pinyin are both reachable from the shared page.
    assert!(linux.shuangpin_preedit);
    // The AI token is the provider's, so the page neither asks for one nor
    // withholds the service controls for want of it. Every other host holds
    // the token itself and must keep asking.
    assert!(linux.ai_provider_credentials);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).ai_provider_credentials);
    }

    let windows = HostCapabilities::for_platform(HostPlatform::Windows);
    assert!(!windows.number_row_selection);
    assert!(windows.restart_input_method);
    // Windows keeps a cross-application CN/EN authority, so the scope
    // choice is real there.
    assert!(windows.ime_mode_scope);
    assert!(windows.panel_windows);
    // The source appends a helper code to a finished spelling; no gesture marks it, so the
    // explanation of the gesture would be describing something that does not happen here.
    assert!(!windows.helpcode_shift_entry);
    assert!(!windows.skin_directory_import);
    assert!(windows.candidate_skin_editor);
    // The TIP reads the CN/EN and 简繁 hotkeys from the shared preferences document, so the toggles take effect.
    assert!(windows.mode_switch_shortcuts);
    assert!(
        windows.floating_toolbar
            && windows.floating_toolbar_appearance
            && windows.floating_toolbar_components
    );
    // Windows 的工具栏也画手写、语音和切换输入方案三个按钮，设置页给它们开关。
    assert!(
        windows.floating_toolbar_handwriting
            && windows.floating_toolbar_voice
            && windows.floating_toolbar_input_scheme
    );
    // 自动隐藏、延时与阴影还没接进 Windows 的 FloatingToolbarWindow，设置页不列这三行。
    assert!(!windows.floating_toolbar_idle_behavior);
    assert!(windows.candidate_font_controls);
    assert!(windows.candidate_preedit_font);
    assert!(windows.candidate_row_colors);
    assert!(windows.candidate_selection_appearance);
    assert!(!windows.candidate_border_color);
    // Windows positions its own card, so pinning it is a real choice there.
    assert!(windows.candidate_follow_cursor);
    let macos = HostCapabilities::for_platform(HostPlatform::Macos);
    assert!(macos.restart_input_method);
    // InputMethodKit controllers identify the active application; the
    // native preference owner selects either that map or its global mode.
    assert!(macos.ime_mode_scope);
    assert!(macos.voice_capture_devices);
    assert!(macos.candidate_skin_editor);
    assert!(
        macos.floating_toolbar
            && macos.floating_toolbar_appearance
            && macos.floating_toolbar_components
    );
    // macOS 的面板按 FloatingToolbarBehavior.h 读自动隐藏、延时与阴影，所以只有它列这三行。
    assert!(macos.floating_toolbar_idle_behavior);
    assert!(macos.fuzzy_pinyin);
    assert!(macos.candidate_font_controls);
    assert!(macos.candidate_row_colors);
    assert!(macos.candidate_selection_appearance);
    // 桌面宿主的编号色和边框色固定跟随主题，宿主不读这两个槽位，设置页也不给取色器；桌面宿主上只有候选窗口还能单独设明暗，其余界面一律跟随颜色模式。
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Harmony,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert!(!host.candidate_number_color && !host.candidate_border_color);
        assert!(host.surface_themes_follow);
    }
    // Android 和 iOS 的键盘仍读这两个槽位和各界面的单独明暗。
    for platform in [HostPlatform::Android, HostPlatform::Ios] {
        let host = HostCapabilities::for_platform(platform);
        assert!(host.candidate_number_color && host.candidate_border_color);
        assert!(!host.surface_themes_follow);
    }
    assert!(macos.candidate_follow_cursor);
    assert!(macos.panel_shortcuts);
    assert!(macos.app_logo);
    assert!(windows.app_logo);
    assert!(linux.app_logo);
    // 只有向会话报告大写锁定的宿主提供「大写锁定时使用英文标点」，目前是 macOS 和 Linux；快捷短语编码进普通候选也是这两个。
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert!(!host.caps_lock_punctuation && !host.quick_phrase_candidates);
    }
    assert!(macos.caps_lock_punctuation && macos.quick_phrase_candidates);
    assert!(linux.caps_lock_punctuation && linux.quick_phrase_candidates);
    // 「移动光标后编辑光标后的部分」只在三个桌面宿主上提供，触屏宿主不列出它。
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Macos,
    ] {
        assert!(HostCapabilities::for_platform(platform).caret_edit_suffix);
    }
    for platform in [
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).caret_edit_suffix);
    }
    // 上屏后联想只在宿主接好显示和按键之后才提供，目前是 Android 和 Linux（IBus 与 Fcitx5）。
    for platform in [HostPlatform::Android, HostPlatform::Linux] {
        assert!(HostCapabilities::for_platform(platform).next_word_prediction);
    }
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).next_word_prediction);
    }
    // 「单按 Caps Lock 切换中英文」和「Shift+字母进入快捷模式」总开关只在三个桌面宿主上提供。
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Macos,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert!(host.caps_lock_language_switch && host.shift_letter_entry_switch);
    }
    for platform in [
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert!(!host.caps_lock_language_switch && !host.shift_letter_entry_switch);
    }
    // 「Ctrl+Shift+J 在中文和日语之间切换」只在 Windows 和 Linux 上提供：macOS 用系统输入模式「日」轮换，移动端还没有接上。
    for platform in [HostPlatform::Windows, HostPlatform::Linux] {
        assert!(HostCapabilities::for_platform(platform).japanese_toggle_shortcut);
    }
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).japanese_toggle_shortcut);
    }
    // 中日英循环目前只有 macOS 接上：Windows 的 TIP 和 Server 之间还要新开一条协议，Linux 的两个引擎各自处理切换。
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).trilingual_cycle);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).trilingual_cycle);
    }
    // 能在光标旁放徽标的宿主才声明它：触屏键盘在键面上就显示了模式。Windows 的 Server 自己画，Linux 交给 Fcitx5 面板。
    assert!(macos.input_mode_hud);
    assert!(windows.input_mode_hud);
    assert!(linux.input_mode_hud);
    // 应用例外只在认得前台应用、并按规则定起始模式的三个桌面宿主上提供。
    assert!(macos.app_input_mode_rules);
    assert!(windows.app_input_mode_rules);
    assert!(linux.app_input_mode_rules);
    assert!(!HostCapabilities::for_platform(HostPlatform::Android).app_input_mode_rules);
    assert!(!HostCapabilities::for_platform(HostPlatform::Harmony).app_input_mode_rules);
    assert!(!HostCapabilities::for_platform(HostPlatform::Ios).app_input_mode_rules);
    // Vim 模式的共享键先只给 macOS：Windows 仍读它自己在 TSF 本地的名单。
    assert!(macos.vim_mode_apps);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).vim_mode_apps);
    }
    assert!(!HostCapabilities::for_platform(HostPlatform::Android).input_mode_hud);
    assert!(!HostCapabilities::for_platform(HostPlatform::Ios).input_mode_hud);
    // 菜单栏状态项只有 macOS 画：Windows 有托盘语言栏，Linux 有输入法面板，触屏键盘的键面已显示模式。
    assert!(macos.input_mode_status_item);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).input_mode_status_item);
    }
    // 展开候选网格目前只有 macOS 的候选窗画（#7307），其他宿主不提供这个开关。
    assert!(macos.expand_candidates);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).expand_candidates);
    }
    // Mobile hosts draw no toolbar at all.
    let android = HostCapabilities::for_platform(HostPlatform::Android);
    assert!(android.mobile_settings);
    assert!(!android.candidate_skin_editor);
    // Android's InputMethodService keys the persisted mode by the focused
    // editor package when the shared scope is set to app.
    assert!(android.ime_mode_scope);
    assert!(
        !android.floating_toolbar
            && !android.floating_toolbar_appearance
            && !android.floating_toolbar_components
    );
    assert!(android.candidate_font_controls);
    assert!(android.candidate_row_colors);
    assert!(android.candidate_selection_appearance);
    assert!(android.candidate_border_color);
    let ios = HostCapabilities::for_platform(HostPlatform::Ios);
    assert!(ios.mobile_settings);
    assert!(ios.fuzzy_pinyin);
    assert!(ios.typing_statistics);
    assert!(!ios.panel_windows);
    // The candidate bar cascades the Latin face, the family and its fallbacks.
    assert!(ios.candidate_font_controls);
    assert!(ios.candidate_english_font);
    assert!(ios.candidate_preedit_font);
    assert!(ios.candidate_row_colors);
    assert!(ios.candidate_selection_appearance);
    assert!(ios.candidate_border_color);
    // The Engine expands shuangpin keys for the candidate bar's spelling, and Shift marks a helper code in a quanpin or shuangpin composition, so both rows describe something the keyboard does.
    assert!(ios.shuangpin_preedit);
    assert!(ios.helpcode_shift_entry);
    // Files cannot reach the App Group skin folder, so the skin button imports a picked folder.
    assert!(ios.skin_directory_import);
    // 手机不提供「制作皮肤」标签，皮肤页保持原样。
    assert!(!ios.candidate_skin_editor);
    // The strip pages in nines and is always a horizontal row, whatever the shared document says.
    assert_eq!(ios.fixed_candidate_page_size, Some(9));
    assert_eq!(ios.max_candidate_page_size, 9);
    assert_eq!(
        ios.fixed_candidate_layout,
        Some(crate::preferences::CandidateLayout::Horizontal)
    );
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Harmony,
    ] {
        let other = HostCapabilities::for_platform(platform);
        assert_eq!(other.fixed_candidate_page_size, None);
        // 第十个候选要靠 0 键选，只开给已经接好 0 键的宿主。
        assert_eq!(
            other.max_candidate_page_size,
            if matches!(
                platform,
                HostPlatform::Macos | HostPlatform::Linux | HostPlatform::Android
            ) {
                10
            } else {
                9
            }
        );
        assert_eq!(other.fixed_candidate_layout, None);
        // 只有 iOS 和 Android 的键盘工具栏按 `touch_toolbar` 选按钮。
        assert_eq!(
            other.touch_toolbar_components,
            platform == HostPlatform::Android
        );
        // 只有 HarmonyOS 的键盘按 `harmony_toolbar` 选工具和显示方式。
        assert_eq!(other.touch_toolbar_tools, platform == HostPlatform::Harmony);
    }
    assert!(ios.touch_toolbar_components);
    assert!(!ios.touch_toolbar_tools);
    // Windows handles Ctrl+Shift+Win+K on its maintenance hook, so the
    // panel shortcut row is real there now.
    assert!(windows.panel_shortcuts);
    // The TIP reads the CN/EN and 简繁 hotkeys from the shared preferences document, so the toggles take effect.
    assert!(windows.mode_switch_shortcuts);
    assert!(windows.system_fonts);

    let android = HostCapabilities::for_platform(HostPlatform::Android);
    assert!(!android.panel_windows);
    assert!(!android.window_chrome);
    // Typing statistics were previously gated on a user-agent match.
    assert!(android.typing_statistics);
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).typing_statistics);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).mode_switch_shortcuts);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).voice_capture_devices);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).typing_statistics);
    assert!(linux.fuzzy_pinyin);
    assert!(android.fuzzy_pinyin);
    assert!(android.mode_switch_shortcuts);
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).fuzzy_pinyin);
    // 背单词卡片的喇叭：macOS、Windows 的设置应用用网页的本机声音，移动三端用各自的系统语音；Linux 要看 spd-say 在不在，由设置应用填。
    for (platform, reads) in [
        (HostPlatform::Macos, true),
        (HostPlatform::Windows, true),
        (HostPlatform::Linux, false),
        (HostPlatform::Android, true),
        (HostPlatform::Ios, true),
        (HostPlatform::Harmony, true),
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).read_aloud,
            reads,
            "{platform:?}"
        );
    }
}

#[test]
fn capabilities_round_trip_and_reject_unknown_keys() {
    let capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
    let text = serde_json::to_string(&capabilities).expect("serializes");
    assert_eq!(
        serde_json::from_str::<HostCapabilities>(&text).expect("deserializes"),
        capabilities
    );
    let mut document: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    document["unexpected"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<HostCapabilities>(document).is_err());
}

#[test]
fn every_host_offers_cantonese_zhuyin_vietnamese_tibetan_and_stroke() {
    use crate::preferences::InputScheme;
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let schemes = HostCapabilities::for_platform(platform).input_schemes;
        assert_eq!(schemes.len(), 10, "{platform:?}");
        for scheme in [
            InputScheme::Cantonese,
            InputScheme::Zhuyin,
            InputScheme::Vietnamese,
            InputScheme::Tibetan,
            InputScheme::Stroke,
        ] {
            assert!(schemes.contains(&scheme), "{platform:?} {scheme:?}");
        }
    }
}

#[test]
fn a_build_compiles_the_schemes_its_platform_offers() {
    // 每个目标平台都编译全部十个方案。
    let platform = if cfg!(target_os = "macos") {
        HostPlatform::Macos
    } else if cfg!(target_os = "android") {
        HostPlatform::Android
    } else if cfg!(target_os = "ios") {
        HostPlatform::Ios
    } else if cfg!(target_env = "ohos") {
        HostPlatform::Harmony
    } else if cfg!(target_os = "linux") {
        HostPlatform::Linux
    } else {
        HostPlatform::Windows
    };
    assert_eq!(
        compiled_input_schemes(),
        HostCapabilities::for_platform(platform)
            .input_schemes
            .as_slice()
    );
}

#[test]
fn platform_names_round_trip() {
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert_eq!(HostPlatform::parse(platform.as_str()), Ok(platform));
    }
    assert_eq!(HostPlatform::parse(""), Err(RouteError::Empty));
    assert_eq!(HostPlatform::parse("bsd"), Err(RouteError::Unknown));
}

/// A newly named host must not claim a surface nobody has written. Every
/// capability here stays false until a HarmonyOS host actually consumes it,
/// so the shared UI never renders a control that saves and does nothing.
#[test]
fn harmony_groups_with_mobile_hosts_and_claims_nothing_unwritten() {
    assert!(!HostPlatform::Harmony.is_desktop());
    let harmony = HostCapabilities::for_platform(HostPlatform::Harmony);
    assert!(harmony.mobile_settings);
    assert!(!harmony.panel_windows);
    assert!(!harmony.window_chrome);
    // ArkUI enumerates the installed families, so the page can offer them.
    assert!(harmony.system_fonts);
    // ArkUI resolves a family list per glyph, which is how a separate Latin family is honoured.
    assert!(harmony.candidate_english_font);
    // The completions are drawn from the packaged dictionary on the candidate strip, and the
    // switch that governs them is the shared preference rather than a native store.
    assert!(harmony.english_suggestions);
    // The composition row draws the Engine's editing text, so raw versus expanded is visible.
    assert!(harmony.shuangpin_preedit);
    // One commit path, so there is nothing to choose between and no control for it.
    assert!(!harmony.voice_commit_mode);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).shuangpin_preedit);
    // 候选窗的预编辑行和 TIP 的行内组字都取 Engine 的 preedit，原始按键与展开拼音的差别在 Windows 上看得见。
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).shuangpin_preedit);
    // 双拼键位图由 macOS、Windows 和 Linux（X11 客户端）画。
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).shuangpin_keymap_hint);
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).shuangpin_keymap_hint);
    assert!(HostCapabilities::for_platform(HostPlatform::Linux).shuangpin_keymap_hint);
    for platform in [
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).shuangpin_keymap_hint);
    }
    assert!(!HostCapabilities::for_platform(HostPlatform::Ios).english_suggestions);
    assert!(!HostCapabilities::for_platform(HostPlatform::Windows).english_suggestions);
    // Pango resolves the panel's family list per glyph, so the Linux hosts name the English family first.
    assert!(HostCapabilities::for_platform(HostPlatform::Linux).candidate_english_font);
    // 全角输入 was withheld from every touch platform, which was a statement about form
    // factor rather than about who can honour it. What decides is whether the host tells the
    // runtime a width: all six do, from a toolbar, a menu or the keyboard's own tools.
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Harmony,
        HostPlatform::Ios,
    ] {
        assert!(HostCapabilities::for_platform(platform).character_width);
    }
    // Android and iOS run the configured transcription provider, streaming one included, but draw no interim text, so they are the hosts without that switch.
    for platform in [HostPlatform::Android, HostPlatform::Ios] {
        assert!(!HostCapabilities::for_platform(platform).voice_stream_preedit);
        assert!(HostCapabilities::for_platform(platform).voice_provider_settings);
    }
    // The two chords exist wherever a keyboard can send them and the host routes them.
    assert!(HostCapabilities::for_platform(HostPlatform::Android).maintenance_shortcuts);
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).maintenance_shortcuts);
    assert!(!HostCapabilities::for_platform(HostPlatform::Ios).maintenance_shortcuts);
    assert!(HostCapabilities::for_platform(HostPlatform::Android).fullwidth_chord);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).fullwidth_chord);
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).fullwidth_chord);
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).fullwidth_chord);
    }
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Harmony,
    ] {
        assert!(HostCapabilities::for_platform(platform).voice_stream_preedit);
    }
    // Drawn from the input method's status-bar panel, which scales itself by the shared
    // scale and font size, hides the buttons the user turned off, and opens the emoji panel
    // and the screen keyboard in the window its candidates otherwise occupy.
    assert!(
        harmony.floating_toolbar
            && harmony.floating_toolbar_appearance
            && harmony.floating_toolbar_components
    );
    assert!(!harmony.restart_input_method);
    // The editor attribute carries the client's bundle name, so a per-application map is real.
    assert!(harmony.ime_mode_scope);
    // Harmony's Engine consumes the shared fuzzy-pinyin rules on every prepared session, so
    // the settings page may expose the same rule picker as the other mobile hosts.
    assert!(harmony.fuzzy_pinyin);
    // Consumed: the hardware key router reads all four bindings, so the page may offer them.
    assert!(harmony.mode_switch_shortcuts);
    // Consumed since the panel chord was bound: the extension sees Ctrl+Shift+Super+K while it
    // is attached to an editor, which is the only state in which a panel that inserts into that
    // editor is useful anyway.
    assert!(harmony.panel_shortcuts);
    // Shift marks a helper code here exactly as it does on Android; the page explains that
    // gesture and would otherwise have explained it to nobody on this host.
    assert!(harmony.helpcode_shift_entry);
    // Its skin folder is inside the sandbox, so the page asks the user to point at a skin
    // rather than offering to open a folder that nothing can browse.
    assert!(harmony.skin_directory_import);
    // 2in1 的设置投影按设备形态再打开它，平台默认按手机处理。
    assert!(!harmony.candidate_skin_editor);
    assert!(harmony.number_row_selection);
    // The two network providers create their own capturer, so a chosen microphone is routable.
    assert!(harmony.voice_capture_devices);
    assert!(harmony.candidate_font_controls);
    assert!(harmony.candidate_row_colors);
    assert!(harmony.candidate_selection_appearance);
    assert!(!harmony.candidate_border_color);
    assert!(harmony.candidate_follow_cursor);
    // The 2in1 status-bar badge is the only mode readout a machine with a hardware keyboard
    // gets when the toolbar is off, so the switch that governs it belongs on this host too.
    assert!(harmony.input_mode_hud);
    // Typing statistics are unconditional across every host.
    assert!(harmony.typing_statistics);
}

#[test]
fn linux_native_canvas_renderer_reads_the_fcitx5_report() {
    // platforms/linux 的 candidate_panel_status_document("fcitx5", None, true) 逐字节写出的就是这一行。
    let native = "{\"host\":\"fcitx5\",\"limit\":null,\"renderer\":\"native_canvas\"}\n";
    assert!(linux_native_canvas_renderer(native));
    assert_eq!(CandidatePanelLimit::from_host_status(native), None);
    for document in [
        r#"{"host":"fcitx5","limit":null}"#,
        r#"{"host":"fcitx5","limit":null,"renderer":"classicui"}"#,
        r#"{"host":"ibus","limit":"ibus_panel","renderer":7}"#,
        "not json",
        "",
    ] {
        assert!(!linux_native_canvas_renderer(document), "{document}");
    }
    let mut linux = HostCapabilities::for_platform(HostPlatform::Linux);
    assert!(
        !linux.expand_candidates
            && !linux.candidate_preedit_font
            && !linux.candidate_selection_appearance
    );
    linux.apply_linux_native_canvas();
    assert!(
        linux.expand_candidates
            && linux.candidate_preedit_font
            && linux.candidate_selection_appearance
    );
    let mut windows = HostCapabilities::for_platform(HostPlatform::Windows);
    let before = windows.clone();
    windows.apply_linux_native_canvas();
    assert_eq!(windows, before);
}

#[test]
fn candidate_panel_limit_reads_only_what_the_host_reported() {
    assert_eq!(
        CandidatePanelLimit::from_host_status(r#"{"host":"ibus","limit":"gnome_shell"}"#),
        Some(CandidatePanelLimit::GnomeShell)
    );
    assert_eq!(
        CandidatePanelLimit::from_host_status(r#"{"host":"fcitx5","limit":"fcitx_theme"}"#),
        Some(CandidatePanelLimit::FcitxTheme)
    );
    assert_eq!(
        CandidatePanelLimit::from_host_status(r#"{"host":"fcitx5","limit":"kimpanel"}"#),
        Some(CandidatePanelLimit::Kimpanel)
    );
    // platforms/linux 的 candidate_panel_status_document("ibus", IbusPanel) 逐字节写出的就是这一行。
    assert_eq!(
        CandidatePanelLimit::from_host_status("{\"host\":\"ibus\",\"limit\":\"ibus_panel\"}\n"),
        Some(CandidatePanelLimit::IbusPanel)
    );
    for document in [
        r#"{"host":"fcitx5","limit":null}"#,
        r#"{"host":"ibus"}"#,
        r#"{"host":"ibus","limit":"something_newer"}"#,
        "not json",
        "",
    ] {
        assert_eq!(
            CandidatePanelLimit::from_host_status(document),
            None,
            "{document}"
        );
    }
    assert_eq!(
        CandidatePanelLimit::status_file(Some(std::ffi::OsStr::new("/run/user/1000"))),
        Some(std::path::PathBuf::from(
            "/run/user/1000/msime-client/candidate-panel.json"
        ))
    );
    assert_eq!(
        CandidatePanelLimit::status_file(Some(std::ffi::OsStr::new("relative"))),
        None
    );
    assert_eq!(CandidatePanelLimit::status_file(None), None);
    let mut linux = HostCapabilities::for_platform(HostPlatform::Linux);
    linux.candidate_panel_limit = Some(CandidatePanelLimit::GnomeShell);
    let encoded = serde_json::to_value(&linux).unwrap();
    assert_eq!(encoded["candidate_panel_limit"], "gnome_shell");
    assert_eq!(
        serde_json::from_value::<HostCapabilities>(encoded).unwrap(),
        linux
    );
    linux.candidate_panel_limit = Some(CandidatePanelLimit::IbusPanel);
    let encoded = serde_json::to_value(&linux).unwrap();
    assert_eq!(encoded["candidate_panel_limit"], "ibus_panel");
    assert_eq!(
        serde_json::from_value::<HostCapabilities>(encoded).unwrap(),
        linux
    );
    assert!(
        serde_json::to_value(HostCapabilities::for_platform(HostPlatform::Linux))
            .unwrap()
            .get("candidate_panel_limit")
            .is_none()
    );
}

/// The commit strategy is offered only where the host acts on it. Windows and macOS each have more than one way to put a result into the editor; the Linux hosts commit through IBus or Fcitx5 and ignore the stored mode, so a selector there would save a choice nothing reads.
#[test]
fn voice_commit_mode_is_offered_only_where_a_host_chooses_between_paths() {
    assert!(HostCapabilities::for_platform(HostPlatform::Windows).voice_commit_mode);
    assert!(HostCapabilities::for_platform(HostPlatform::Macos).voice_commit_mode);
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert!(!HostCapabilities::for_platform(platform).voice_commit_mode);
    }
}

/// 鸿蒙版在拿到 AI 生成类资质之前不提供 AI 对话、润色、回复、AI 联想、AI 翻译和 AI 皮肤；其余宿主照旧提供。鸿蒙的社区皮肤按这一项排除 AI 生成的作品，在各端画出「AI 生成」标识之前，不要把鸿蒙改成真。
#[test]
fn only_harmony_withholds_ai_generation() {
    assert!(!HostCapabilities::for_platform(HostPlatform::Harmony).ai_generation);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        assert!(HostCapabilities::for_platform(platform).ai_generation);
    }
}

/// 鸿蒙版在实名认证到位之前不提供社区发布、评分和举报；其余宿主照旧提供。
#[test]
fn only_harmony_withholds_community_contribution() {
    assert!(!HostCapabilities::for_platform(HostPlatform::Harmony).community_contribution);
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        assert!(HostCapabilities::for_platform(platform).community_contribution);
    }
}

/// 空格锁定和开始、结束提示音的细分只有 Android、iOS 还读存下的值；录音后端只有 Linux 有得选。macOS、Windows、HarmonyOS 只给一个「麦克风」下拉。
#[test]
fn only_mobile_reads_space_lock_and_cue_split_and_only_linux_picks_capture_backend() {
    for (platform, space_lock, cues, backends) in [
        (HostPlatform::Macos, false, false, false),
        (HostPlatform::Windows, false, false, false),
        (HostPlatform::Linux, false, false, true),
        (HostPlatform::Harmony, false, false, false),
        (HostPlatform::Android, true, true, true),
        (HostPlatform::Ios, true, true, true),
    ] {
        let capabilities = HostCapabilities::for_platform(platform);
        assert_eq!(
            capabilities.voice_space_lock_switch, space_lock,
            "{platform:?} space lock"
        );
        assert_eq!(capabilities.voice_cue_switches, cues, "{platform:?} cues");
        assert_eq!(
            capabilities.voice_capture_backends, backends,
            "{platform:?} capture backends"
        );
    }
}

/// Only the hosts that wire them claim sound packs, the `/` and `@` modes, music and typing effects.
#[test]
fn plugin_surfaces_are_claimed_only_by_the_hosts_that_wire_them() {
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
    ] {
        let capabilities = HostCapabilities::for_platform(platform);
        assert!(
            capabilities.key_sound
                && capabilities.plugin_triggers
                && capabilities.music
                && capabilities.typing_effects,
            "{platform:?}"
        );
    }
    // HarmonyOS claims its 2in1 sound and trigger surfaces in its own form-factor projection; iOS wires none. Android 的 IME 进程播放按键音，其余仍未接入。
    for platform in [
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let capabilities = HostCapabilities::for_platform(platform);
        assert_eq!(
            capabilities.key_sound,
            platform == HostPlatform::Android,
            "{platform:?}"
        );
        assert!(
            !capabilities.plugin_triggers && !capabilities.music,
            "{platform:?}"
        );
        // The HarmonyOS KeyboardView draws the flash and the combo badge itself.
        assert_eq!(
            capabilities.typing_effects,
            platform == HostPlatform::Harmony,
            "{platform:?}"
        );
    }
    // 只有 Android 列出可选的内置辅助码方案，没有带授权码表的郑码不在其中；其他宿主不写这一项。
    let android = HostCapabilities::for_platform(HostPlatform::Android);
    assert!(android
        .helpcode_schemas
        .iter()
        .any(|schema| schema == "ziranma"));
    assert!(!android
        .helpcode_schemas
        .iter()
        .any(|schema| schema == "zhengma"));
    // 列出的每一项都是共享偏好认得的方案 id，包括五笔 86。
    assert!(android
        .helpcode_schemas
        .iter()
        .any(|schema| schema == "wubi86"));
    for schema in &android.helpcode_schemas {
        let parsed: crate::preferences::HelpcodeSchema =
            serde_json::from_value(serde_json::Value::String(schema.clone())).unwrap();
        assert_eq!(parsed.as_str(), schema);
    }
    let windows =
        serde_json::to_value(HostCapabilities::for_platform(HostPlatform::Windows)).unwrap();
    assert!(windows.get("helpcode_schemas").is_none());
    let mut claimed = HostCapabilities::for_platform(HostPlatform::Windows);
    claimed.key_sound = true;
    let text = serde_json::to_string(&claimed).unwrap();
    assert!(
        serde_json::from_str::<HostCapabilities>(&text)
            .unwrap()
            .key_sound
    );
}

/// 候选窗样式控件跟着各宿主画的界面走：两个桌面窗口和 Fcitx5 主题三项都支持，鸿蒙 2in1 卡片支持圆角和底色透明度，Android 和 iOS 的键盘候选条只支持整体缩放（候选栏和工具栏一起缩放）。
#[test]
fn candidate_window_style_is_offered_where_the_host_draws_the_card() {
    let flags = |platform| {
        let capabilities = HostCapabilities::for_platform(platform);
        (
            capabilities.candidate_window_scale,
            capabilities.candidate_window_opacity,
            capabilities.candidate_corner_radius,
        )
    };
    assert_eq!(flags(HostPlatform::Windows), (true, true, true));
    assert_eq!(flags(HostPlatform::Macos), (true, true, true));
    assert_eq!(flags(HostPlatform::Linux), (true, true, true));
    assert_eq!(flags(HostPlatform::Harmony), (false, true, true));
    assert_eq!(flags(HostPlatform::Android), (true, false, false));
    assert_eq!(flags(HostPlatform::Ios), (true, false, false));
}

/// macOS、Windows 和 HarmonyOS 把 Tab、PageUp/PageDown 两组翻页键固定为开（关掉只让键失效，不会还给应用），Linux 和 Android、iOS 照旧读偏好并显示开关。三个桌面宿主把中英混输的触发字符数固定为默认值，触屏宿主照旧读。
#[test]
fn hosts_fix_the_tab_page_paging_keys_and_the_mixed_input_prefix() {
    // (宿主, 读 Tab/PageUp 开关, 读触发字符数)
    for (platform, reads_paging, reads_prefix) in [
        (HostPlatform::Windows, false, false),
        (HostPlatform::Macos, false, false),
        (HostPlatform::Linux, true, false),
        (HostPlatform::Harmony, false, true),
        (HostPlatform::Android, true, true),
        (HostPlatform::Ios, true, true),
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert_eq!(host.tab_page_paging_switches, reads_paging, "{platform:?}");
        assert_eq!(
            host.mixed_input_minimum_prefix, reads_prefix,
            "{platform:?}"
        );
        assert_eq!(
            platform.reads_mixed_input_minimum_prefix(),
            reads_prefix,
            "{platform:?}"
        );
    }
    assert_eq!(
        crate::preferences::MixedInputPreferences::default().minimum_prefix,
        crate::preferences::DEFAULT_MIXED_INPUT_MINIMUM_PREFIX
    );
}

/// 卡片内边距目前只有 macOS 候选窗和 Linux 的 Fcitx5 主题读，其他宿主不显示这个滑块（#7316）。
#[test]
fn only_macos_and_linux_offer_the_candidate_padding() {
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Harmony,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).candidate_padding,
            matches!(platform, HostPlatform::Macos | HostPlatform::Linux),
            "{platform:?}"
        );
    }
}

#[test]
fn only_macos_offers_candidate_item_geometry() {
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).candidate_item_geometry,
            platform == HostPlatform::Macos
        );
    }
}

/// 「键盘高度」只给真正按它改变键盘高度的宿主：macOS 和 Linux 的键盘窗口高度固定。间距只在 macOS 上固定成默认值，其余宿主照旧可调。
#[test]
fn touch_keyboard_geometry_is_offered_where_the_keyboard_applies_it() {
    for platform in [
        HostPlatform::Windows,
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Harmony,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        let capabilities = HostCapabilities::for_platform(platform);
        assert_eq!(
            capabilities.touch_keyboard_height,
            !matches!(platform, HostPlatform::Macos | HostPlatform::Linux),
            "{platform:?}"
        );
        assert_eq!(
            capabilities.touch_keyboard_spacing,
            platform != HostPlatform::Macos,
            "{platform:?}"
        );
    }
}

#[test]
fn full_offers_every_compiled_scheme_and_leaves_the_capabilities_alone() {
    let full = Edition::full();
    assert_eq!(offered_input_schemes(full), compiled_input_schemes());
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let before = HostCapabilities::for_platform(platform);
        let mut after = before.clone();
        after.narrow_to_edition(full);
        assert_eq!(after, before);
        // full 的能力文档里没有 `edition` 键，与引入版本之前逐字节相同。
        assert!(serde_json::to_value(&after)
            .unwrap()
            .get("edition")
            .is_none());
    }
}

#[test]
fn a_narrower_edition_drops_its_missing_schemes_and_says_which_edition_it_is() {
    use crate::preferences::InputScheme;
    let wubi = Edition::by_id("wubi").unwrap();
    assert_eq!(offered_input_schemes(wubi), [InputScheme::Wubi]);
    let mut capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
    capabilities.narrow_to_edition(wubi);
    assert_eq!(capabilities.input_schemes, [InputScheme::Wubi]);
    assert_eq!(
        capabilities.edition,
        Some(EditionInfo {
            id: "wubi".into(),
            display_name: "水杉五笔".into(),
            input_schemes: vec![InputScheme::Wubi],
            default_scheme: InputScheme::Wubi,
            temporary_japanese: false,
            neural_keyboard: false,
            offline_glosses: true,
            handwriting: true,
            wubi_mixed_pinyin_default: true,
        })
    );
    let document = serde_json::to_value(&capabilities).unwrap();
    assert_eq!(
        document["edition"],
        serde_json::json!({
            "id": "wubi",
            "display_name": "水杉五笔",
            "input_schemes": ["wubi"],
            "default_scheme": "wubi",
            "temporary_japanese": false,
            "neural_keyboard": false,
            "offline_glosses": true,
            "handwriting": true,
            "wubi_mixed_pinyin_default": true,
        })
    );
    assert_eq!(
        serde_json::from_value::<HostCapabilities>(document).unwrap(),
        capabilities
    );

    // 宿主先在运行时去掉了某个方案（这里是双拼），版本再收窄时不会把它加回来；`edition` 里仍列出本版本的全部方案。
    let pinyin = Edition::by_id("pinyin").unwrap();
    let mut capabilities = HostCapabilities::for_platform(HostPlatform::Linux);
    capabilities
        .input_schemes
        .retain(|scheme| *scheme != InputScheme::Shuangpin);
    capabilities.narrow_to_edition(pinyin);
    assert_eq!(capabilities.input_schemes, [InputScheme::Quanpin]);
    let edition = capabilities.edition.unwrap();
    assert_eq!(
        edition.input_schemes,
        [InputScheme::Quanpin, InputScheme::Shuangpin]
    );
    assert!(edition.temporary_japanese);
    assert!(!edition.wubi_mixed_pinyin_default);
}

/// 不提供中文方案的版本（日文、越南文和藏文版）不带离线释义也不提供手写：设置页据此藏起手写页，macOS 和 Windows 悬浮工具栏的手写按钮开关也随之消失。提供中文方案的版本保持原样。
#[test]
fn editions_without_a_chinese_scheme_offer_no_handwriting_or_offline_glosses() {
    for id in ["japanese", "vietnamese", "tibetan"] {
        let mut capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
        assert!(capabilities.floating_toolbar_handwriting);
        capabilities.narrow_to_edition(Edition::by_id(id).unwrap());
        assert!(!capabilities.floating_toolbar_handwriting, "{id}");
        // Windows 的工具栏同样按版本去掉手写按钮的开关，语音和切换输入方案不受影响。
        let mut windows = HostCapabilities::for_platform(HostPlatform::Windows);
        assert!(windows.floating_toolbar_handwriting);
        windows.narrow_to_edition(Edition::by_id(id).unwrap());
        assert!(!windows.floating_toolbar_handwriting, "{id}");
        assert!(
            windows.floating_toolbar_voice && windows.floating_toolbar_input_scheme,
            "{id}"
        );
        let edition = capabilities.edition.unwrap();
        assert!(!edition.handwriting, "{id}");
        assert!(!edition.offline_glosses, "{id}");
    }
    for id in ["pinyin", "wubi"] {
        let mut capabilities = HostCapabilities::for_platform(HostPlatform::Macos);
        capabilities.narrow_to_edition(Edition::by_id(id).unwrap());
        assert!(capabilities.floating_toolbar_handwriting, "{id}");
        let edition = capabilities.edition.unwrap();
        assert!(edition.handwriting, "{id}");
        assert!(edition.offline_glosses, "{id}");
    }
}

#[test]
fn fixed_ai_candidate_limit_and_effect_intensity_sit_at_their_defaults() {
    // 不给「候选数量」「效果强度」的宿主固定用默认值；固定的值必须正好是新档案的默认值，否则没改过的人升级后也会变。
    let limit = crate::preferences::AiAssistantPreferences::default().candidate_limit;
    let intensity = crate::preferences::PluginPreferences::default().effect_intensity;
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Linux,
        HostPlatform::Harmony,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert_eq!(host.fixed_ai_candidate_limit, Some(limit), "{platform:?}");
        let json = serde_json::to_value(&host).unwrap();
        assert_eq!(json["fixed_ai_candidate_limit"], 3, "{platform:?}");
    }
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Harmony,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert_eq!(host.fixed_effect_intensity, Some(intensity), "{platform:?}");
        let json = serde_json::to_value(&host).unwrap();
        assert_eq!(json["fixed_effect_intensity"], 50, "{platform:?}");
    }
    // Android、iOS 仍按偏好决定 AI 候选数量；Linux 不画效果样式，没有强度，Android、iOS 也按偏好。按偏好的宿主不写这些键，旧版本设置页读到的能力文档不变。
    for platform in [HostPlatform::Android, HostPlatform::Ios] {
        let host = HostCapabilities::for_platform(platform);
        assert_eq!(host.fixed_ai_candidate_limit, None, "{platform:?}");
        assert_eq!(platform.fixed_ai_candidate_limit(), None, "{platform:?}");
        let json = serde_json::to_value(&host).unwrap();
        assert!(
            json.get("fixed_ai_candidate_limit").is_none(),
            "{platform:?}"
        );
    }
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Android,
        HostPlatform::Ios,
    ] {
        let host = HostCapabilities::for_platform(platform);
        assert_eq!(host.fixed_effect_intensity, None, "{platform:?}");
        assert_eq!(platform.fixed_effect_intensity(), None, "{platform:?}");
        let json = serde_json::to_value(&host).unwrap();
        assert!(json.get("fixed_effect_intensity").is_none(), "{platform:?}");
    }
}

#[test]
fn english_online_capability_only_names_a_host_with_the_input_path() {
    for platform in [
        HostPlatform::Macos,
        HostPlatform::Linux,
        HostPlatform::Windows,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).english_online_candidates,
            platform == HostPlatform::Macos
        );
    }
}

#[test]
fn custom_helpcode_trigger_is_only_offered_by_linux_and_harmony() {
    for platform in [
        HostPlatform::Linux,
        HostPlatform::Macos,
        HostPlatform::Windows,
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        assert_eq!(
            HostCapabilities::for_platform(platform).helpcode_trigger_key,
            matches!(platform, HostPlatform::Linux | HostPlatform::Harmony)
        );
    }
}
