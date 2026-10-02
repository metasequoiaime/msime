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
        ("settings:download", SettingsCategory::Download),
        ("settings:developer", SettingsCategory::Developer),
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
    assert!(linux.mode_switch_shortcuts);
    // Linux stands the toolbar up as an IBus property menu: the switch and
    // component visibility work, but scale and icon size have no surface.
    assert!(linux.floating_toolbar);
    assert!(!linux.floating_toolbar_appearance);
    assert!(linux.floating_toolbar_components);
    assert!(linux.panel_shortcuts);
    assert!(linux.number_row_selection);
    assert!(linux.voice_capture_devices);
    assert!(linux.candidate_font_controls);
    assert!(!linux.candidate_preedit_font);
    assert!(linux.candidate_row_colors);
    assert!(!linux.candidate_selection_appearance);
    // The Fcitx5 classic UI theme draws the border even though neither Linux panel has a hover state.
    assert!(linux.candidate_border_color);
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
    // The TIP reads the CN/EN and 简繁 hotkeys from the shared preferences document, so the toggles take effect.
    assert!(windows.mode_switch_shortcuts);
    assert!(
        windows.floating_toolbar
            && windows.floating_toolbar_appearance
            && windows.floating_toolbar_components
    );
    assert!(windows.candidate_font_controls);
    assert!(windows.candidate_preedit_font);
    assert!(windows.candidate_row_colors);
    assert!(windows.candidate_selection_appearance);
    assert!(windows.candidate_border_color);
    // Windows positions its own card, so pinning it is a real choice there.
    assert!(windows.candidate_follow_cursor);
    let macos = HostCapabilities::for_platform(HostPlatform::Macos);
    assert!(macos.restart_input_method);
    // InputMethodKit controllers identify the active application; the
    // native preference owner selects either that map or its global mode.
    assert!(macos.ime_mode_scope);
    assert!(macos.voice_capture_devices);
    assert!(
        macos.floating_toolbar
            && macos.floating_toolbar_appearance
            && macos.floating_toolbar_components
    );
    assert!(macos.fuzzy_pinyin);
    assert!(macos.candidate_font_controls);
    assert!(macos.candidate_row_colors);
    assert!(macos.candidate_selection_appearance);
    assert!(macos.candidate_border_color);
    assert!(macos.candidate_follow_cursor);
    assert!(macos.panel_shortcuts);
    // Only the two hosts that can put a badge beside the caret claim it; a touch keyboard says
    // the mode on its own key faces, and Windows/Linux draw nothing of the kind.
    assert!(macos.input_mode_hud);
    assert!(!windows.input_mode_hud);
    assert!(linux.input_mode_hud);
    assert!(!HostCapabilities::for_platform(HostPlatform::Android).input_mode_hud);
    assert!(!HostCapabilities::for_platform(HostPlatform::Ios).input_mode_hud);
    // Mobile hosts draw no toolbar at all.
    let android = HostCapabilities::for_platform(HostPlatform::Android);
    assert!(android.mobile_settings);
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
    // The strip pages in nines and is always a horizontal row, whatever the shared document says.
    assert_eq!(ios.fixed_candidate_page_size, Some(9));
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
        assert_eq!(other.fixed_candidate_layout, None);
        assert!(!other.touch_toolbar_components);
    }
    assert!(ios.touch_toolbar_components);
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
fn every_host_offers_cantonese_zhuyin_and_vietnamese() {
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
        assert_eq!(schemes.len(), 8, "{platform:?}");
        for scheme in [
            InputScheme::Cantonese,
            InputScheme::Zhuyin,
            InputScheme::Vietnamese,
        ] {
            assert!(schemes.contains(&scheme), "{platform:?} {scheme:?}");
        }
    }
}

#[test]
fn a_build_compiles_the_schemes_its_platform_offers() {
    // Every target compiles all eight.
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
    assert!(!HostCapabilities::for_platform(HostPlatform::Windows).shuangpin_preedit);
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
    for platform in [
        HostPlatform::Windows,
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
    assert!(harmony.number_row_selection);
    // The two network providers create their own capturer, so a chosen microphone is routable.
    assert!(harmony.voice_capture_devices);
    assert!(harmony.candidate_font_controls);
    assert!(harmony.candidate_row_colors);
    assert!(harmony.candidate_selection_appearance);
    assert!(harmony.candidate_border_color);
    assert!(harmony.candidate_follow_cursor);
    // The 2in1 status-bar badge is the only mode readout a machine with a hardware keyboard
    // gets when the toolbar is off, so the switch that governs it belongs on this host too.
    assert!(harmony.input_mode_hud);
    // Typing statistics are unconditional across every host.
    assert!(harmony.typing_statistics);
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
    // HarmonyOS claims its 2in1 sound and trigger surfaces in its own form-factor projection; Android and iOS wire none.
    for platform in [
        HostPlatform::Android,
        HostPlatform::Ios,
        HostPlatform::Harmony,
    ] {
        let capabilities = HostCapabilities::for_platform(platform);
        assert!(
            !capabilities.key_sound && !capabilities.plugin_triggers && !capabilities.music,
            "{platform:?}"
        );
        // The HarmonyOS KeyboardView draws the flash and the combo badge itself.
        assert_eq!(
            capabilities.typing_effects,
            platform == HostPlatform::Harmony,
            "{platform:?}"
        );
    }
    let mut claimed = HostCapabilities::for_platform(HostPlatform::Windows);
    claimed.key_sound = true;
    let text = serde_json::to_string(&claimed).unwrap();
    assert!(
        serde_json::from_str::<HostCapabilities>(&text)
            .unwrap()
            .key_sound
    );
}

/// The candidate window style controls follow the surface each host draws: both desktop windows take all three, the Fcitx5 theme only a radius, the HarmonyOS 2in1 card a radius and a fill alpha, and the keyboard strips none.
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
    assert_eq!(flags(HostPlatform::Linux), (false, false, true));
    assert_eq!(flags(HostPlatform::Harmony), (false, true, true));
    assert_eq!(flags(HostPlatform::Android), (false, false, false));
    assert_eq!(flags(HostPlatform::Ios), (false, false, false));
}
