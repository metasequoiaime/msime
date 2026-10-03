#!/usr/bin/env python3
"""Check the Fcitx5 addon metadata without requiring a running desktop."""
from pathlib import Path
import configparser
import re
import sys

root = Path(__file__).resolve().parents[2]
addon = configparser.ConfigParser()
addon.read(root / "fcitx5/msime.conf")
assert addon["Addon"]["Name"] == "MSIME"
assert addon["Addon"]["Type"] == "SharedLibrary"
assert addon["Addon"]["Library"] == "libmsime-fcitx5"
entry = configparser.ConfigParser()
entry.read(root / "fcitx5/msime-inputmethod.conf")
assert entry["InputMethod"]["Addon"] == "msime"
assert entry["InputMethod"]["LangCode"] == "zh_CN"
cmake = (root / "CMakeLists.txt").read_text()
assert "MSIME_ENABLE_FCITX5" in cmake
# 默认值跟着环境走而不是跟着打包开关：装了 Fcitx5 开发包的机器就构建这个并列入口，
# 打包路径仍然无条件包含它。此前默认取自 MSIME_ENABLE_PACKAGING，也就是默认关闭。
assert "find_package(Fcitx5Core" in cmake
assert "MSIME_ENABLE_PACKAGING OR Fcitx5Core_FOUND" in cmake
assert cmake.index('option(MSIME_ENABLE_FCITX5') < cmake.index('include(cmake/packaging.cmake)')
packaging = (root / "cmake/packaging.cmake").read_text()
assert "if(MSIME_ENABLE_FCITX5)" in packaging
assert "fcitx5 (>= 5.0.20)" in packaging

cmake_fcitx5 = (root / "fcitx5/CMakeLists.txt").read_text()
# 徽章浮层用的是 wayland-scanner 生成的 C 代码。这个子工程声明 LANGUAGES CXX，不打开 C
# 的话生成的 .c 不参与编译，插件链接得过却在加载时报 undefined symbol，fcitx5 静默不加载
# ——表现是整个输入法没反应。同理 layer-shell 的协议引用了 xdg_popup，两份生成代码必须
# 一起编。这两条都发生过。
assert "enable_language(C)" in cmake_fcitx5
assert "xdg-shell.xml" in cmake_fcitx5
assert "wlr-layer-shell-unstable-v1.xml" in cmake_fcitx5

source = (root / "fcitx5/FcitxEngine.cpp").read_text()
ibus_source = (root / "src/core/ClientEngine.cpp").read_text()
# Commit statistics also run on detached workers. They must be included in the same
# pending-write barrier as key-count batches, or an addon unload can execute this
# translation unit after its shared library has already been unmapped.
record_statistics = source[source.index("void recordTypingStatistics("):source.index("  // `typingStatistics`", source.index("void recordTypingStatistics("))]
assert "fcitx_key_press_writes.begin();" in record_statistics
assert "fcitx_key_press_writes.end();" in record_statistics
# Provider callbacks cross the C ABI from a worker thread. The streaming text
# callback must contain allocation/UI queue failures so no C++ exception can
# escape through the Rust provider boundary and terminate the input method.
stream_update = ibus_source[ibus_source.index("extern \"C\" void voice_provider_stream_update"):ibus_source.index("// English mode", ibus_source.index("extern \"C\" void voice_provider_stream_update"))]
assert "try" in stream_update and "catch (...)" in stream_update
# 中英文切换提示：面板那个弹出物必须排在面板更新之后，先弹再刷会被 clearPanel()/render()
# 收掉，表现为提示时有时无。
assert "showCustomInputMethodInformation" in source
toggle = source[source.index("bool toggleInputMode()"):]
toggle = toggle[:toggle.index("bool toggleWordCharacter()")]
assert toggle.index("clearPanel();") < toggle.index("showInputModeHud();")
assert toggle.index("render();") < toggle.index("showInputModeHud();")
assert "input_mode_hud" in source
# #2589: moving into another text field shows the current mode too, after the panel is drawn for the same reason as above, and only for a focus change (switching to this input method gets Fcitx5's own popup).
activate = source[source.index("void activate(const fcitx::InputMethodEntry &"):]
activate = activate[:activate.index("void deactivate(const fcitx::InputMethodEntry &")]
assert "event.type() == fcitx::EventType::InputContextFocusIn) state->showInputModeHud();" in activate
assert activate.index("state->render();") < activate.index("state->showInputModeHud();")
assert 'tsf_preedit_style' in source
assert 'candidate_preedit_style' in source
assert 'FcitxSchemeBooleanAction' in source
assert '英文输入模式' in source
assert 'msime-shuangpin-preedit' in source
assert 'msime-wubi-code-hint' in source
assert 'msime-shuangpin-profile' in source
assert 'cycleShuangpinProfile' in source
# 五笔在输入方案菜单和状态文字里都按共享偏好 `wubi_profile` 标出 86 或 98。
assert source.count('wubi_scheme_label(') >= 2
assert source.count('value("wubi_profile", std::string("wubi86"))') >= 2
assert '"输入方案：五笔"' not in source
assert 'cycleFrequencyMode' in source
assert 'msime-frequency' in source
assert 'msime-frequency-trigger' in source
assert 'msime-frequency-step' in source
assert 'msime-candidate-theme' in source
assert 'cycleCandidateTheme' in source
assert 'msime-global-theme' in source
assert 'setThemeChoice' in source
assert 'msime_client_resolve_theme' in source
# The classic UI draws the ‹ › page buttons only when the theme names both images; a theme without them drops the buttons the stock theme has.
candidate_theme = (root / "src/candidates/CandidateFcitxTheme.h").read_text()
assert '"[InputPanel/PrevPage]\\n"' in candidate_theme
assert '"[InputPanel/NextPage]\\n"' in candidate_theme
assert 'msime_client_builtin_skins' not in source
assert 'candidate_skin_catalog' in source
assert 'msime_client_load_preferences' in source
assert 'applyContextOverrides' in source
assert 'effectiveContextSnapshot' in source
assert 'msime_client_update_preferences' in source
assert 'applyPreferenceSnapshot' in source
assert 'saveStringPreference("character_width"' in source
assert 'snapshot["preferences"]["character_width"]' in source
assert 'saveBooleanPreference("traditional_chinese_output"' in source
assert 'preferences_["traditional_chinese_output"]' in source
assert 'waitForPreferenceSave' in source
assert 'if (fixedPosition > 0) actions.push_back(make(20, "取消固定"));' in source
# Both hosts label the Engine's local mode through the shared table; comparing against hand-written names is how the Fcitx5 footer once missed every mode ("phrase", "abbreviation", "english", "japanese" are not names the Engine emits).
for host_source in (source, ibus_source):
    assert 'candidate_local_mode_label(mode)' in host_source
    for stale in ('"phrase"', '"abbreviation"', '"english"', '"japanese"'):
        assert f'mode == {stale}' not in host_source
# Every name the Engine emits in local_mode, except "none", has a label in the shared table.
engine_types = (root.parents[1] / "crates/engine/src/types.rs").read_text()
mode_names = engine_types[engine_types.index("impl LocalInputMode {"):]
mode_names = mode_names[:mode_names.index("\n}\n")]
emitted = re.findall(r'=> "([a-z_]+)",', mode_names)
assert "none" in emitted and len(emitted) > 1
labels = (root / "src/candidates/CandidateLocalModeLabels.h").read_text()
for name in emitted:
    if name != "none":
        assert f'{{"{name}", "' in labels, name
# Plugin input modes and sounds are wired the same way in both hosts. A key the local mode or the scheme spells with (View.spelling_symbols) is sent to the Engine before any host binding, digits gate on the listed symbols rather than on a mode name, generated commits stay out of typing statistics, and sounds only post requests to the Host API.
for host_source in (source, ibus_source):
    assert 'msime::linux_host::engine_spelling(' in host_source
    assert 'msime::linux_host::spelling_space(' in host_source
    assert 'msime::linux_host::spelling_digits(' in host_source
    assert 'context.value("typing_statistics", true)' in host_source
    assert 'msime_client_key_sound(' in host_source
    assert 'msime_client_commit_sound(' in host_source
    assert 'msime_client_music_set_active' in host_source
    assert 'local_mode_enabled_by_default(' in host_source
    for mode in ('"expression"', '"command"', '"mention"'):
        assert mode in host_source, mode
assert 'const bool unicodeMode' not in source
assert 'unicode_digit' not in ibus_source
# The key sound is asked for from the key event, after ensure(): never for a restricted or private context, and the session must be released (music told to stop) before it is destroyed.
assert source.index('music_.release(session_') < source.index('msime_client_destroy(session_)')
assert 'state->playKeySound(event);' in source
assert 'play_key_sound(engine, key, flags);' in ibus_source
# The typing effect is asked for at the same two points as the sounds, and Linux shows only its combo count: IBus at the end of the candidate aux line, Fcitx5 in the aux line below the page, never in setAuxUp, which the voice, emoji search and configuration notices own.
for host in (source, ibus_source):
    assert host.count('msime_client_typing_effect(') == 2
    assert 'kTypingEffectCommit' in host and 'kTypingEffectRepeat' in host
    assert 'typing_combo_label(' in host
assert ibus_source.index('msime_client_key_sound(s.session, key_class)') < ibus_source.index('kTypingEffectRepeat')
assert source.index('msime_client_key_sound(session_, keyClass)') < source.index('kTypingEffectRepeat')
assert 'setAuxDown(fcitx::Text(candidateAux()))' in source
assert 'setAuxUp(fcitx::Text(candidateAux()))' not in source
# /fy's translation goes to the selected service whatever the gloss switches say, and never to an offline gloss.
for host in (source, ibus_source):
    assert 'command_translation_query(' in host
assert ibus_source.index('music.release(session') < ibus_source.index('msime_client_destroy(session)')
# Both hosts name the installed built-in sound packs, which the parent project installs.
assert 'MSIME_SOUND_PACKS="${CMAKE_INSTALL_FULL_DATADIR}/msime-client/sound-packs"' in cmake_fcitx5
assert 'installed_sound_pack_directory(' in ibus_source
assert 'DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/sound-packs")' in cmake
# The IBus candidate property menu uses the shared Windows wording (置顶, 固定到第 N 位, 取消固定) and takes the slot from the CandidateFixN action name.
assert 'msime::linux_host::candidate_pin_label' in ibus_source
assert 'candidate_fix_label(fix[12] - \'0\')' in ibus_source
assert 'ibus_text_new_from_static_string("取消固定")' in ibus_source
assert '"固定候选"' not in ibus_source
assert '"固定到 1"' not in ibus_source
assert 'std::string("取消固定 ")' not in ibus_source
# Only main-lexicon candidates of the user dictionary sources carry candidate actions; the shared policy decides it for both hosts.
assert 'candidate_dictionary_actions_available(state_.view_.value("scheme", 0u), item->source())' in source
assert 'candidate_dictionary_actions_available(scheme, item->source())' in source
assert 'source_(candidate.value("source", 0u))' in source
assert 'text_(candidate.at("text").get<std::string>())' in source
assert 'fixed_position_(candidate.value("fixed_position", 0u))' in source
assert 'item->text()))' in source
assert 'item->fixedPosition()' in source
assert 'state_.session_ != item->session()' in source
assert '!state_.ic_.hasFocus() || !state_.input_enabled_' in source
assert 'state_.privateInput() || state_.session_ != item->session()' in source
assert 'voice_cancelled_' in source
assert 'msime_voice_stream_inline_enabled(' in source
assert 'voice_preedit_' in source and 'voice_transcript_' in source
assert 'msime_voice_result_or_transcript(' in source
assert 'clipboard_generation_' in source
assert 'result.value("_path", std::string{}) == clipboard_path_' in source
assert 'result.value("_generation", uint64_t{}) == clipboard_generation_' in source
assert 'const auto generation = clipboard_generation_' in source
assert 'cloud_clipboard_generation_' in source
assert 'cloud_clipboard_enabled_' in source
assert 'help_action_{&factory_, "help", "帮助"}' in source
assert 'feedback_action_{&factory_, "feedback", "反馈"}' in source
assert '(std::strcmp(panel_, "voice") == 0 && !state->voice_enabled_)' in source
assert 'if (!state || state->restricted() ||' in source
assert '!state->ensure()) return;' not in source[source.index('class FcitxDesktopPanelAction'):source.index('// The shared `floating_toolbar`')]
assert 'state->restricted() || state->privateInput()' not in source[source.index('class FcitxDesktopPanelAction'):source.index('// The shared `floating_toolbar`')]
assert 'toggleFloatingToolbar' in source
assert 'saveNestedBooleanPreference("floating_toolbar", "enabled", enabled)' in source
assert 'toolbar_enabled_action_' in source
assert 'toggleVoiceEnabled' in source
assert 'saveNestedBooleanPreference("voice_input", "enabled", enabled)' in source
assert 'voice_enabled_action_' in source
assert 'voice_enabled_);' in source
assert 'voice_action_);' in source
assert 'reloadFcitxService' in source
assert 'reload_service_action_' in source
assert 'maintenance_reload_held_' in source
# Fcitx5's ReloadConfig never reaches addons, so the reset runs in process: the controller's ReloadAddonConfig, the chord and the status-menu action all end in resetSessions.
assert 'void reloadConfig() override { resetSessions(); }' in source
assert 'engine_->resetSessions();' in source
assert 'return reloadFcitxService();' not in source
assert 'fcitx5-remote' in source
assert 'ClientInputModeMemory.h' in source
assert 'fcitx_app_input_modes' in source
assert 'fcitx_global_input_mode' in source
assert 'ic_.program()' in source
assert 'restoreInputMode' in source
assert 'result.value("_socket", std::string{}) == cloud_clipboard_socket_' in source
assert 'result.value("_generation", uint64_t{}) == cloud_clipboard_generation_' in source
assert 'cloud_clipboard_enabled_ = result.value("enabled", true)' in source
# The provider relays the account API page as is, so the list is `items` ([{id,text,updated_at}]) like the shared panel reads it; `entries` is the cloud dictionary's shape.
assert 'result.value("items", Json::array())' in source
assert 'result.value("entries", Json::array())' not in source[source.index('void refreshCloudClipboard()'):source.index('bool pasteCloudClipboard(')]
assert 'if (!cloud_clipboard_enabled_) return false;' in source
assert 'emoji_generation_' in source
assert 'result.value("_generation", uint64_t{}) == emoji_generation_' in source
assert 'result["_generation"] = generation' in source
assert 'item.value("source", 255u) == source' in source
assert 'Json candidates = Json::array();' in source
assert 'std::string panelPreview(const std::string &text)' in source
assert 'fcitx::utf8::nextNChar(text.begin(), 40)' in source
assert 'text.substr(0, 40)' not in source
assert 'if (voice_job_.valid()) {' in source
assert 'voice cancellation does not wait for the provider future' in (root / 'fcitx5/tests/native.cpp').read_text()
assert 'msime-helpcode-schema' in source
assert 'cycleHelpcodeSchema' in source
assert 'toggleLocalMode' in source
assert 'msime-local-unicode' in source
assert 'msime-local-temporary-japanese' in source
# 五个可配置模式快捷键及各自的宿主状态都要保留，避免设置能保存却不生效。
for name in ("mode_shift_enabled_", "mode_ctrl_enabled_", "mode_ctrl_space_enabled_",
             "mode_ctrl_alt_space_enabled_", "character_set_shortcut_enabled_"):
    assert name in source, name
for key in ("switch_language_shift", "switch_language_ctrl", "switch_language_ctrl_space",
            "switch_language_ctrl_alt_space", "toggle_character_set_ctrl_shift_f"):
    assert key in source, key
# A bare modifier is measured on its release, and only when nothing else was typed
# while it was held; the press half only arms it.
assert "pure_shift_candidate_" in source and "pure_ctrl_candidate_" in source
assert "modifier_toggle_deadline_" in source
assert "event.isRelease()" in source
# Ctrl+Space and Ctrl+Alt+Space must be able to return from English passthrough.
# The old gate sat above the chord and made the switch one-way.
# English mode keeps fullwidth and the Chinese punctuation lock below that gate, so the gate is found by its comment rather than by a bare return.
assert source.index("if (!toggleInputMode()) return false;") < source.index(
    "English mode still honours fullwidth output"
)
# The startup mode belongs to the input context, not to each Engine session, or
# refocusing would put the default back over the mode the user chose.
assert "ime_mode_chosen_" in source
assert 'preferences_.value("default_ime_mode", "chinese")' in source
assert 'voicePreferences.value("hotkey_hold_space_lock", voice_hotkey_hold_space_lock_)' in source
assert 'voice_options_.value("asr_provider", std::string{"doubao"})' in source
assert 'fcitx_system_dark_theme()' in source
# The portal read is a blocking D-Bus round trip: it runs once per addon on a worker, never from each context's loop tick.
assert source.count('fcitx_system_dark_theme()') == 1
theme_step = source[source.index('uint64_t stepSystemTheme()'):source.index('void applySystemTheme(bool dark)')]
assert 'system_theme_job_ = detachedJob(' in theme_step
assert theme_step.index('detachedJob(') < theme_step.index('fcitx_system_dark_theme()')
assert '~FcitxEngine() override' in source and 'system_theme_job_.wait_for(' in source[source.index('~FcitxEngine() override'):]
assert 'refreshSystemTheme' not in source
assert 'system_theme_probe_due_' not in source
assert 'setNextInterval(stepSystemTheme())' in source
assert 'pkg_check_modules(GIO REQUIRED IMPORTED_TARGET gio-2.0)' in (root / "fcitx5/CMakeLists.txt").read_text()
assert 'fcitx_system_dark_theme' in (root / "fcitx5/SystemTheme.cpp").read_text()

# Runtime option reload must follow the same clipboard path precedence as
# initial session setup and fence reads started against the old history file.
assert 'nextClipboard = options.value("preferences_directory", std::string{})' in source
assert 'options.value("clipboard_history_path", std::string{})' in source
assert '++clipboard_generation_' in source
assert 'clipboard_loading_ = false' in source
# Explicit clipboard_history_path has precedence over the derived preferences
# directory in both initial setup and runtime reload.
assert 'auto clipboard_path = options.value("clipboard_history_path", std::string());' in source
assert 'auto nextClipboard = options.value("clipboard_history_path", std::string{});' in source
# The shared option names the history file, while the Host API clipboard ABI
# receives its containing preferences directory.  Fcitx5 must normalize the
# explicit default filename before loading or mutating history.
assert 'std::filesystem::path(clipboard_path).parent_path().string()' in source
assert 'std::filesystem::path(nextClipboard).parent_path().string()' in source

# Cloud clipboard follows the provider socket contract: explicit options,
# environment override, then the per-user runtime socket, and hot reload must
# fence requests started against an old endpoint.
assert 'cloudClipboardSocket' in source
assert 'cloud-clipboard.sock' in source
assert 'MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET' in source
assert 'nextCloudClipboard' in source
assert '++cloud_clipboard_generation_' in source
assert 'cloud_clipboard_enabled_ = true;' in source
assert 'result.value("_socket", std::string{}) == translation_socket_' in source
assert '{"_socket", socket}' in source
assert 'FcitxSentenceTranslationAction' in source
assert 'msime-translate-sentence' in source
assert 'void translateSentence()' in source
assert 'translation_manual_sentence_' in source
assert '"TranslateSentence"' in ibus_source
assert 'void translate_sentence(IBusEngine *engine)' in ibus_source

# Voice provider discovery must match IBus and the standalone Linux provider
# contract, including the runtime socket fallback during hot reload.
assert 'providerSocket(options, "voice_provider_socket"' in source
assert '"MSIME_VOICE_PROVIDER_SOCKET", "voice.sock"' in source
assert 'preferences_.value("clipboard_history", false)' in source
assert 'if (!preferences_.value("clipboard_history", false))' in source
assert 'translationSocket' in source
assert '"MSIME_TRANSLATION_PROVIDER_SOCKET", "translation.sock"' in source
assert '"MSIME_TRANSLATION_PROVIDER_SOCKET"' in ibus_source
assert '"translation.sock"' in ibus_source

# Moving preferences_directory while focused must invalidate the old async
# store read and retry state before loading from the new directory.
assert 'nextPreferencesDirectory' in source
assert 'preferences_job_session_ = 0' in source
assert 'preferences_snapshot_ = Json()' in source
assert 'preferences_save_retry_.reset()' in source

# Fcitx5 menu preference writes retain the last failed field and expose a retry
# action. A failed revision comparison must not leave the user with a silent
# diagnostic-only failure as the old async save path did.
assert 'struct PendingPreferenceSave' in source
assert 'startPreferenceSave' in source
assert 'retryPreferenceSave' in source
assert 'preferences_save_retry_' in source
assert 'FcitxPreferenceSaveRetryAction' in source
assert '重试保存设置' in source

# Native Fcitx5 sessions use the same non-focus-stealing X11/Wayland voice
# surface as IBus when one is available, while the auxiliary panel remains the
# explicit fallback for headless or unsupported desktops.
for marker in ("WaveOverlayModel", "WaveOverlaySurface", "WaveOverlayX11Surface",
               "WaveOverlayWaylandSurface", "create_fcitx_wave_overlay_surface",
               "MSIME_WAVE_OVERLAY_BACKEND", "updateVoiceOverlay", "hideVoiceOverlay",
               "wave_overlay_.set_input_level", "wave_overlay_.actions_visible = true"):
    assert marker in source, marker
assert "WaveOverlayX11Surface.cpp" in (root / "fcitx5/CMakeLists.txt").read_text()
assert "WaveOverlayWaylandSurface.cpp" in (root / "fcitx5/CMakeLists.txt").read_text()

# The floating toolbar's eight component switches decide what the "工具栏" submenu
# contains. They decided nothing here before, while the settings page showed all of
# them for this platform, so each switch is pinned to the entry it governs.
assert "msime-toolbar" in source
assert "rebuildToolbarMenu" in source and "refreshToolbar" in source
for key, default in (("english_mode", "true"), ("fullwidth", "true"),
                     ("punctuation", "true"), ("character_set", "true"),
                     ("emoji", "true"), ("screen_keyboard", "false"),
                     ("settings", "true")):
    assert f'toolbar.value("{key}", {default})' in source, key
# The entries are the existing actions, not copies: a toolbar entry that behaved
# differently from the status-area action beside it would be a second
# implementation of the same toggle.
for action in ("input_mode_action_", "english_action_", "width_action_",
               "chinese_punctuation_action_", "traditional_action_",
               "desktop_emoji_action_", "keyboard_action_", "settings_action_"):
    assert f"append(" in source and action in source, action
# A rebuild removes exactly what it added; the menu is shared across contexts.
assert "toolbar_entries_" in source

# 还没做首次配置时，激活和按键两条路径都要给出具体提示，而不是笼统的「请检查运行配置」；但只有激活会打开设置窗口，打字途中弹出的窗口可能抢走键盘焦点。引导走与 IBus 启动器同一个脚本，每个登录会话只弹一次的限制因此两边共用。
assert source.count("catch (const OptionsNotConfigured &) { notConfigured(*state, true); }") == 1
# Key events and the in-process reset show the hint without the guide.
assert source.count("catch (const OptionsNotConfigured &) { notConfigured(*state, false); }") == 2
resetSessions = source[source.index("void resetSessions() {"):]
resetSessions = resetSessions[:resetSessions.index("\n  }\n")]
assert "notConfigured(*state, false)" in resetSessions and "notConfigured(*state, true)" not in resetSessions
keyEvent = source[source.index("void keyEvent(const fcitx::InputMethodEntry &"):]
keyEvent = keyEvent[:keyEvent.index("\n  }\n")]
assert "notConfigured(*state, false)" in keyEvent and "notConfigured(*state, true)" not in keyEvent
assert "kFirstRunHint" in source and "kFirstRunGuideProgram" in source
assert 'fcitx::startProcess({guide, "--host", "fcitx5"})' in source
assert 'MSIME_BINDIR="${CMAKE_INSTALL_FULL_BINDIR}"' in cmake_fcitx5
assert "scripts/msime-linux-first-run-guide" in cmake

# The Korean Hanja keys (Hangul_Hanja and a bare F9), which also open the Zhuyin list, and the open lists are read through the shared core/KoreanHanja.h and core/InputSchemes.h in both hosts. The keys are decided before the rules that would finish the composition and hand the key to the application, so a trigger the Engine leaves unhandled (a lone jamo) never writes the syllable out and then leaks the key. Down is the second key that opens the Zhuyin list, so each host sends the command twice.
fcitx_convert = 'command(MSIME_OPEN_CANDIDATE_LIST)'
ibus_convert = 'msime_client_command(s.session, MSIME_OPEN_CANDIDATE_LIST)'
assert source.count(fcitx_convert) == 2 and ibus_source.count(ibus_convert) == 2
for host in (source, ibus_source):
    assert 'msime::linux_host::korean_hanja_key(' in host
    assert 'msime::linux_host::candidate_list_composition(' in host
    assert 'msime::linux_host::zhuyin_list_down_key(' in host
    assert 'msime::linux_host::korean_hanja_list_open(' in host
    assert 'msime::linux_host::opened_candidate_list(' in host
assert 'msime::linux_host::korean_hanja_punctuation_key(' in ibus_source
fcitx_key = source[source.index("bool FcitxState::key(fcitx::KeyEvent &event) {"):]
fcitx_key = fcitx_key[:fcitx_key.index("\n}\n")]
assert fcitx_key.index(fcitx_convert) < fcitx_key.index("if (composing && commitsOnBlur() && !openedList)")
assert fcitx_key.index(fcitx_convert) < fcitx_key.rindex("if (composing) command(MSIME_FINISH_COMPOSITION);")
ibus_key = ibus_source[ibus_source.index("gboolean process_key(IBusEngine *engine, guint key, guint keycode, guint flags) {"):]
ibus_key = ibus_key[:ibus_key.index("\n}\n")]
assert ibus_key.index(ibus_convert) < ibus_key.index("commits_on_blur && has_composition)")
# With a list open Return chooses the highlighted Hanja or Zhuyin candidate in both hosts rather than writing the composition out and breaking the line.
assert "command(openedList ? MSIME_COMMIT_CANDIDATE : MSIME_COMMIT_RAW)" in fcitx_key
ibus_return = ibus_key[ibus_key.index("case IBUS_Return:"):]
assert ibus_return.index("if (opened_list) {") < ibus_return.index("command = MSIME_COMMIT_RAW;")

# Stroke (scheme 8) is offered in both menus only with stroke.db: Fcitx5 registers its action and rebuilds the menu when stroke.db alone appears, and IBus lists Scheme/Stroke, lets PropertyActivate through for it and maps it to the "stroke" id. Missing either IBus half makes choosing 笔画 silently do nothing.
assert '{&scheme_stroke_action_, "msime-scheme-stroke"}' in source
assert 'FcitxSchemeItemAction scheme_stroke_action_{&factory_, 8, "笔画"};' in source
assert 'schemeAvailable("stroke")' in source
assert 'std::tuple{"stroke", "Scheme/Stroke", "笔画"}' in ibus_source
assert 'property_name != "Scheme/Stroke"' in ibus_source
assert 'property_name == "Scheme/Stroke" ? std::string("stroke")' in ibus_source

print("Fcitx5 addon metadata passed")
