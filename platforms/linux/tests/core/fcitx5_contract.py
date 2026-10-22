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
# .deb 只推荐 Fcitx5，Ubuntu 22.04 的 5.0.14 也会和插件同时存在；靠这条依赖，旧版 Fcitx5 在检查依赖时就拒绝加载插件，不会去 dlopen 一个链接了新符号的库。
assert addon["Addon/Dependencies"]["0"] == "core:5.0.20"
entry = configparser.ConfigParser()
entry.read(root / "fcitx5/msime-inputmethod.conf")
assert entry["InputMethod"]["Addon"] == "msime"
assert entry["InputMethod"]["LangCode"] == "zh_CN"
# 不写 LayoutHint：Fcitx5 按输入法组的默认布局（或用户给本输入法单独选的布局）翻译按键，Dvorak、Colemak 用户在 fcitx5-configtool 里选好布局即可（#6365）。
assert "LayoutHint" not in entry["InputMethod"]
cmake = (root / "CMakeLists.txt").read_text()
assert "MSIME_ENABLE_FCITX5" in cmake
# 默认值跟着环境走而不是跟着打包开关：装了 Fcitx5 开发包的机器就构建这个并列入口，
# 打包路径仍然无条件包含它。此前默认取自 MSIME_ENABLE_PACKAGING，也就是默认关闭。
assert "find_package(Fcitx5Core" in cmake
assert "MSIME_ENABLE_PACKAGING OR Fcitx5Core_FOUND" in cmake
assert cmake.index("option(MSIME_ENABLE_FCITX5") < cmake.index(
    "include(cmake/packaging.cmake)"
)
packaging = (root / "cmake/packaging.cmake").read_text()
assert "if(MSIME_ENABLE_FCITX5)" in packaging
assert "fcitx5 (>= 5.0.20)" in packaging
# IBus 和 Fcitx5 二选一（#6401）：Fcitx5 只能作为 IBus 的备选出现在 Depends 里。单独写进 Depends 会让 Ubuntu 22.04 整包装不上（#6305），写进 Recommends 会把 Fcitx5 拉进 IBus 系统；IBus 写成硬依赖则把 IBus 拉进 deepin 这类 Fcitx5 系统。dpkg-shlibdeps 从插件推出的 libfcitx5* 由 package-container.sh 重新打包时去掉。
assert 'set(MSIME_DEBIAN_INPUT_FRAMEWORK "ibus (>= ${MSIME_IBUS_MIN_VERSION}) | fcitx5 (>= 5.0.20)")' in packaging
assert 'set(CPACK_DEBIAN_PACKAGE_DEPENDS "${MSIME_DEBIAN_INPUT_FRAMEWORK}, ' in packaging
assert not re.search(r"CPACK_DEBIAN_PACKAGE_RECOMMENDS[^\n]*fcitx5", packaging)
assert '"(ibus >= ${MSIME_IBUS_MIN_VERSION} or fcitx5 >= 5.0.20), ' in packaging
assert "onnxruntime|Fcitx5[A-Za-z]+)" in packaging
package_container = (root / "package-container.sh").read_text()
assert 'sed -E -i "/^Depends:/s/, libfcitx5' in package_container
assert 'grep -E "^Depends:.*libfcitx5"' in package_container
# PPA 与 OBS 的源码包不经过 package-container.sh，由 debian/rules 在 dh_shlibdeps 之后同样去掉 libfcitx5*。
debian_rules = (root / "packaging/debian/rules").read_text()
assert "sed -E -i '/^shlibs:Depends=/s/, libfcitx5" in debian_rules
assert "! grep -E '^shlibs:Depends=.*libfcitx5' debian/msime.substvars" in debian_rules

cmake_fcitx5 = (root / "fcitx5/CMakeLists.txt").read_text()
# 徽章浮层用的是 wayland-scanner 生成的 C 代码。这个子工程声明 LANGUAGES CXX，不打开 C
# 的话生成的 .c 不参与编译，插件链接得过却在加载时报 undefined symbol，fcitx5 静默不加载
# ——表现是整个输入法没反应。同理 layer-shell 的协议引用了 xdg_popup，两份生成代码必须
# 一起编。这两条都发生过。
assert "enable_language(C)" in cmake_fcitx5
assert "xdg-shell.xml" in cmake_fcitx5
assert "wlr-layer-shell-unstable-v1.xml" in cmake_fcitx5

source = (root / "fcitx5/FcitxEngine.cpp").read_text()
# 动作名写成 MSIME_EDITION_FCITX5_ADDON "-<名字>"，按版本带上插件名（src/core/LinuxEdition.h）；下面按 full 展开后的名字（msime-<名字>）检查。
assert 'registerAction("msime-' not in source
source = source.replace('MSIME_EDITION_FCITX5_ADDON "-', '"msime-')
ibus_source = (root / "src/core/ClientEngine.cpp").read_text()
# Commit statistics also run on detached workers. They must be included in the same
# pending-write barrier as key-count batches, or an addon unload can execute this
# translation unit after its shared library has already been unmapped.
record_statistics = source[
    source.index("void recordTypingStatistics(") : source.index(
        "  // `typingStatistics`", source.index("void recordTypingStatistics(")
    )
]
assert "fcitx_key_press_writes.begin();" in record_statistics
assert "fcitx_key_press_writes.end();" in record_statistics
# Provider callbacks cross the C ABI from a worker thread. The streaming text
# callback must contain allocation/UI queue failures so no C++ exception can
# escape through the Rust provider boundary and terminate the input method.
stream_update = ibus_source[
    ibus_source.index(
        'extern "C" void voice_provider_stream_update'
    ) : ibus_source.index(
        "// English mode",
        ibus_source.index('extern "C" void voice_provider_stream_update'),
    )
]
assert "try" in stream_update and "catch (...)" in stream_update
# 中英文切换提示：面板那个弹出物必须排在面板更新之后，先弹再刷会被 clearPanel()/render()
# 收掉，表现为提示时有时无。
assert "showCustomInputMethodInformation" in source
toggle = source[source.index("bool toggleInputMode()") :]
toggle = toggle[: toggle.index("bool toggleWordCharacter()")]
assert toggle.index("clearPanel();") < toggle.index("showInputModeHud();")
assert toggle.index("render();") < toggle.index("showInputModeHud();")
assert "input_mode_hud" in source
# #2589: moving into another text field shows the current mode too, after the panel is drawn for the same reason as above, and only for a focus change (switching to this input method gets Fcitx5's own popup).
activate = source[source.index("void activate(const fcitx::InputMethodEntry &") :]
activate = activate[: activate.index("void deactivate(const fcitx::InputMethodEntry &")]
focus_hint = "state->showInputModeHud(FcitxState::ModeHudTrigger::FocusEntered);"
assert "event.type() == fcitx::EventType::InputContextFocusIn)\n          " + focus_hint in activate
assert activate.index("state->render();") < activate.index(focus_hint)
# 获得焦点时的光标徽章要等输入框补报光标（GTK、Qt 在获得焦点之后的空闲回调里才报），整个插件只用一块徽章表面，新的提示替换屏幕上那块。
assert "InputContextCursorRectChanged" in source and "settleFocusInputModeHud();" in source
assert "mode_badge_focus_hint(hint, caret_rect)" in source
assert "engine_->modeBadge()" in source and "engine_->scheduleModeBadgeHide(ic_);" in source
# 全 0 的光标报告不算（还没有光标），接着等；宽高为 0 的点状光标（XIM 客户端）是真实位置。
cursor_watch = source[source.index("mode_hud_cursor_watch_ = instance->watchEvent(") :]
cursor_watch = cursor_watch[: cursor_watch.index("});")]
assert cursor_watch.index("mode_badge_caret_usable(") < cursor_watch.index("settleFocusInputModeHud();")
# 失焦或换输入法时立刻收起这个上下文的徽章（macOS 的 deactivateServer:），开始打字就作废还在等光标的获得焦点提示。
deactivate = source[source.index("void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override") :]
deactivate = deactivate[: deactivate.index("void holdMenuContext(")]
assert "state->cancelFocusInputModeHud();" in deactivate and "dismissModeBadge(event.inputContext());" in deactivate
key_event_body = source[source.index("void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override") :]
key_event_body = key_event_body[: key_event_body.index("std::string subModeLabelImpl(")]
assert key_event_body.index("if (!event.isRelease()) state->cancelFocusInputModeHud();") < key_event_body.index("state->keyDown(event);")
# Linux 应用普遍把 Ctrl+Shift+P 用作命令面板或隐私窗口；两个 Linux 宿主都只从工具栏菜单插入宣传文字。
assert "FcitxKey_P ? FcitxKey_p" not in source
assert "toggle_chord_held_ = FcitxKey_p" not in source
assert "promotion_shortcut" not in ibus_source
assert "Ctrl+Shift+P" not in ibus_source
assert "tsf_preedit_style" in source
assert "candidate_preedit_style" in source
assert "FcitxSchemeBooleanAction" in source
assert "menu_label::kEnglishCandidateMode" in source
assert "msime-shuangpin-preedit" in source
assert "msime-wubi-code-hint" in source
# 四码唯一自动上屏与剩余编码同属五笔方案组，两处断言守住它的入口和偏好键名。
assert "msime-wubi-auto-commit-unique" in source
assert "wubi_auto_commit_unique" in source
assert "msime-shuangpin-profile" in source
assert "cycleShuangpinProfile" in source
# 五笔在输入方案子菜单和「输入方案（…）」一行里都按共享偏好 `wubi_profile` 标出 86 或 98：两处都经 schemeMenuTitle 取 MenuLabels.h 的 scheme_menu_title，它对五笔调 wubi_scheme_label。
menu_labels = (root / "src/core/MenuLabels.h").read_text()
assert source.count("schemeMenuTitle(") >= 3
assert 'value("wubi_profile", std::string("wubi86"))' in source
assert 'if (scheme == "wubi") return wubi_scheme_label(wubi_profile);' in menu_labels
assert '"输入方案：五笔"' not in source
assert "cycleFrequencyMode" in source
assert "msime-frequency" in source
assert "msime-frequency-trigger" in source
assert "msime-frequency-step" in source
assert "msime-candidate-theme" in source
assert "cycleCandidateTheme" in source
assert "msime-global-theme" in source
assert "setThemeChoice" in source
assert "msime_client_resolve_theme" in source
# The classic UI draws the ‹ › page buttons only when the theme names both images; a theme without them drops the buttons the stock theme has.
candidate_theme = (root / "src/candidates/CandidateFcitxTheme.h").read_text()
assert '"[InputPanel/PrevPage]\\n"' in candidate_theme
assert '"[InputPanel/NextPage]\\n"' in candidate_theme
assert "msime_client_builtin_skins" not in source
assert "candidate_skin_catalog" in source
assert "msime_client_load_preferences" in source
assert "applyContextOverrides" in source
assert "effectiveContextSnapshot" in source
assert "msime_client_update_preferences" in source
assert "applyPreferenceSnapshot" in source
assert 'saveStringPreference("character_width"' in source
assert 'snapshot["preferences"]["character_width"]' in source
assert 'saveBooleanPreference("traditional_chinese_output"' in source
assert 'preferences_["traditional_chinese_output"]' in source
assert "waitForPreferenceSave" in source
# Both hosts label the Engine's local mode through the shared table; comparing against hand-written names is how the Fcitx5 footer once missed every mode ("phrase", "abbreviation", "english", "japanese" are not names the Engine emits).
for host_source in (source, ibus_source):
    assert "candidate_local_mode_label(mode)" in host_source
    for stale in ('"phrase"', '"abbreviation"', '"english"', '"japanese"'):
        assert f"mode == {stale}" not in host_source
# Every name the Engine emits in local_mode, except "none", has a label in the shared table.
engine_types = (root.parents[1] / "crates/engine/src/types.rs").read_text()
mode_names = engine_types[engine_types.index("impl LocalInputMode {") :]
mode_names = mode_names[: mode_names.index("\n}\n")]
emitted = re.findall(r'=> "([a-z_]+)",', mode_names)
assert "none" in emitted and len(emitted) > 1
labels = (root / "src/candidates/CandidateLocalModeLabels.h").read_text()
for name in emitted:
    if name != "none":
        assert f'{{"{name}", "' in labels, name
# Plugin input modes and sounds are wired the same way in both hosts. A key the local mode or the scheme spells with (View.spelling_symbols) is sent to the Engine before any host binding, digits gate on the listed symbols rather than on a mode name, generated commits stay out of typing statistics, and sounds only post requests to the Host API.
# 声音、打字特效和背景音乐由生命周期核心驱动（MSIME_LIFECYCLE_FEEDBACK）：两个宿主不再调旧的 key_sound、commit_sound、typing_effect 和 music_set_active，它们在 FEEDBACK 下是空操作，留着就是一个永远不响的调用。
for host_source in (source, ibus_source):
    assert "msime::linux_host::engine_spelling(" in host_source
    assert "msime::linux_host::spelling_space(" in host_source
    assert "msime::linux_host::spelling_digits(" in host_source
    assert 'context.value("typing_statistics", true)' in host_source
    assert "msime_client_set_lifecycle(" in host_source
    assert "MSIME_LIFECYCLE_FEEDBACK" in host_source
    assert "msime_client_key(" in host_source
    assert "msime_client_set_field_traits" in host_source
    assert "msime_client_set_active" in host_source
    for retired in (
        "msime_client_key_sound(",
        "msime_client_commit_sound(",
        "msime_client_music_set_active",
        "msime_client_typing_effect(",
    ):
        assert retired not in host_source, retired
    assert "local_mode_enabled_by_default(" in host_source
    for mode in ('"expression"', '"command"', '"mention"', '"chaizi"'):
        assert mode in host_source, mode
assert "const bool unicodeMode" not in source
assert "unicode_digit" not in ibus_source
# 会话销毁之前先释放（告诉核心本会话停用，音乐随之停下）。
assert source.index("music_.release(session_") < source.index(
    "msime_client_destroy(session_)"
)


def body(text, signature):
    """返回从 signature 开始、到与它配对的右花括号为止的函数体。"""
    start = text.index(signature)
    depth = 0
    for position in range(text.index("{", start), len(text)):
        if text[position] == "{":
            depth += 1
        elif text[position] == "}":
            depth -= 1
            if depth == 0:
                return text[start : position + 1]
    raise AssertionError(signature)


# 原生画布与框架菜单共用操作列表；只有已固定的候选才提供取消固定。顺序照 macOS 的「候选操作」菜单：置顶、固定排位、取消固定、删除。
menu_actions = body(source, "std::vector<std::pair<int, std::string>> candidateMenuActions(")
assert "if (item.fixedPosition() > 0) actions.emplace_back(20, msime::linux_host::candidate_clear_fix_label);" in menu_actions
assert (
    menu_actions.index("candidate_pin_label")
    < menu_actions.index("candidate_fix_label(slot)")
    < menu_actions.index("candidate_clear_fix_label")
    < menu_actions.index("candidate_remove_label")
)
# 关掉页码时主题不生成 ‹ ›：两节只在 page_arrows 为真时写出。
assert "if (style.page_arrows)" in candidate_theme
framework_actions = body(source, "candidateActions(const fcitx::CandidateWord &candidate) const override")
native_canvas = body(source, "bool FcitxState::renderNativeCanvas()")
assert "candidateMenuActions(state_, *item)" in framework_actions
assert "candidateMenuActions(*this, *item)" in native_canvas


# 每个会话一建好就选择加入 FEEDBACK 并报告字段属性，排在任何一个键之前。
for host, create, opt_in, report in (
    (source, "msime_client_create(", "msime_client_set_lifecycle(session_, MSIME_LIFECYCLE_FEEDBACK);", "syncFieldTraits();"),
    (ibus_source, "msime_client_create(", "msime_client_set_lifecycle(session, MSIME_LIFECYCLE_FEEDBACK);", "report_field_traits();"),
):
    created = host.index(create)
    assert created < host.index(opt_in) < host.index(report, host.index(opt_in))
    assert host.count("msime_client_set_lifecycle(") == 1
# 顺序契约：msime_client_key 在引擎处理这个键之前，按键音因此排在这个键带来的上屏音前面；字段属性在 msime_client_key 之前比较和报告。IBus 原来在 process_key 之后才出按键音，上屏音反而先响。
key_event = body(source, "void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override")
assert key_event.index("state->countKeyPress(event);") < key_event.index("state->keyDown(event);")
assert key_event.index("state->keyDown(event);") < key_event.index("state->key(event)")
assert key_event.index("state->key(event)") < key_event.index("state->keySettled(comboBefore);")
ibus_key_event = body(ibus_source, "gboolean process_key_and_count(")
assert ibus_key_event.index("key_down(engine, key, keycode, flags);") < ibus_key_event.index(
    "process_key(engine, key, keycode, flags)"
)
assert ibus_key_event.index("process_key(engine, key, keycode, flags)") < ibus_key_event.index(
    "key_settled(engine, combo_before);"
)
for key_down in (body(source, "void keyDown(const fcitx::KeyEvent &event)"), body(ibus_source, "void key_down(IBusEngine *engine, guint key, guint keycode, guint flags)")):
    traits = "syncFieldTraits();" if "syncFieldTraits" in key_down else "s.report_field_traits();"
    assert key_down.index(traits) < key_down.index("msime_client_key(")
    # 自动重复与 macOS 一样不出声、不画：按硬件键码认出重复，报告字段属性之后只用 lifecycle_silent_turn 开新的一轮就返回，脚本 fuel 因此仍按每次按下给。
    assert "lifecycle_key(msime::linux_host::key_sound_class(" in key_down
    repeat = "key_repeat_.press(keycode)" if "key_repeat_" in key_down else "s.key_repeat.press(keycode)"
    release = "key_repeat_.release(keycode)" if "key_repeat_" in key_down else "s.key_repeat.release(keycode)"
    assert release in key_down
    silent = key_down.index("lifecycle_silent_turn()")
    assert key_down.index(repeat) < key_down.index(traits) < silent < key_down.index("return;", silent) < key_down.index("lifecycle_key(")
    assert "key_press_sounds(" in key_down
    # 隐私字段不由宿主拦着：核心按字段属性决定静音，这样只带 NoSpellCheck 的字段才能照常出声。
    assert "privateInput()" not in key_down and "private_input" not in key_down
# 字段属性逐个标志映射，不复用 privateInput() / private_input 那个把隐私和不做拼写检查并在一起的布尔值。
fcitx_traits = body(source, "std::uint32_t fieldTraits() const")
assert "flags.test(fcitx::CapabilityFlag::Sensitive)" in fcitx_traits
assert "flags.test(fcitx::CapabilityFlag::NoSpellCheck)" in fcitx_traits
assert "privateInput()" not in fcitx_traits
assert "field_traits_.sync(session_, fieldTraits(), msime_client_set_field_traits);" in body(source, "void syncFieldTraits()")
ibus_traits = body(ibus_source, "uint32_t current_field_traits() const")
assert "(content_hints & IBUS_INPUT_HINT_PRIVATE) != 0" in ibus_traits
assert "(content_hints & IBUS_INPUT_HINT_NO_SPELLCHECK) != 0" in ibus_traits
assert "private_input" not in ibus_traits
assert "field_traits.sync(session, current_field_traits(), msime_client_set_field_traits);" in body(ibus_source, "void report_field_traits()")
# 每次 msime_client_focus(true) 之前都不经去重报告一次字段属性：脚本运行时在每次获得焦点之后要先看到属性确认（focus 之前的一次 set_field_traits，值相同也算）才往脚本投递事件。两个宿主各只有一处调用 msime_client_focus，就在这个辅助函数里，排在 confirm 之后。
for host, helper, confirm in (
    (source, "char *focusSession(bool focused)", "if (focused) field_traits_.confirm(session_, fieldTraits(), msime_client_set_field_traits);"),
    (ibus_source, "char *focus_session(bool focused)", "field_traits.confirm(session, current_field_traits(), msime_client_set_field_traits);"),
):
    assert host.count("msime_client_focus(") == 1
    focus = body(host, helper)
    assert focus.index(confirm) < focus.index("msime_client_focus(")
lifecycle_header = (root / "src/system/LifecycleFeedback.h").read_text()
assert "MSIME_TRAIT_KNOWN | (private_field ? MSIME_TRAIT_SECURE | MSIME_TRAIT_PRIVATE : 0u) |" in lifecycle_header
assert "(no_spellcheck ? MSIME_TRAIT_NO_SPELLCHECK : 0u)" in lifecycle_header
# 输入框属性变了但会话不重建（隐私与不做拼写检查之间的切换）时也立刻报告，音乐不等下一个键。
ibus_content_type = body(ibus_source, "void content_type(IBusEngine *engine, guint purpose, guint hints)")
assert "s.content_hints = hints;" in ibus_content_type
unchanged = ibus_content_type.index("if (blocked == s.blocked && private_input == s.private_input) {")
assert unchanged < ibus_content_type.index("s.report_field_traits();", unchanged) < ibus_content_type.index("return;", unchanged) < ibus_content_type.index("s.close();")
assert "state->syncFieldTraits();" in source[source.index("InputContextCapabilityChanged") :]
# IBus 的会话由 process_key 惰性创建：key_down 在判断会话之前先打开它（同 Fcitx5 在 keyDown 之前跑的 ensure()），词库维护或出错关掉会话后的第一个键才有按键音，它的派发也才属于它自己这一轮。
ibus_key_down = body(ibus_source, "void key_down(IBusEngine *engine, guint key, guint keycode, guint flags)")
assert ibus_key_down.index("key_press_sounds(") < ibus_key_down.index("s.open();") < ibus_key_down.index("if (!s.session ||") < ibus_key_down.index("msime_client_key(")
# 设置里的声音（plugins）变了而本会话正激活着时，两个宿主都再声明一次激活：set_active 对选择加入的会话总是返回 true，MusicActivity 不会重试，不重新声明的话刚打开的背景音乐要等下一次换焦点才响。
ibus_live = body(ibus_source, "void apply_live_preferences(IBusEngine *engine, Json snapshot) {")
ibus_plugins = ibus_live.index('value("plugins", Json(nullptr)) !=')
assert ibus_live.index("s.music.told()") < ibus_plugins < ibus_live.index("msime_client_set_active(s.session, true);", ibus_plugins)
fcitx_plugins = source.index('preferences_.value("plugins", Json(nullptr)) !=')
assert source.rindex("music_.told()", 0, fcitx_plugins) > fcitx_plugins - 200
assert fcitx_plugins < source.index("msime_client_set_active(session_, true);", fcitx_plugins) < fcitx_plugins + 300
# 背景音乐只说激活与否，SECURE 由核心按字段属性处理。
for music in (body(source, "void syncMusic()"), body(ibus_source, "void sync_music(IBusEngine *engine)")):
    assert "msime_client_set_active" in music
    assert "privateInput()" not in music and "private_input" not in music
# 上屏音不再由宿主出，连击数取自派发回复的 feedback（解析器容忍并读取这个新键）。
for host in (source, ibus_source):
    assert host.count("reply_typing_effect(result)") == 1
    assert "typing_combo_label(" in host
# 指针点选和翻页先开启新的一轮（MSIME_KEY_POINTER）。
assert "msime_client_key(session_, MSIME_KEY_POINTER);" in source
assert "state->pointerTurn();" in body(source, "void select(fcitx::InputContext *ic) const override")
assert "state->pointerTurn();" in body(source, "void move(uint32_t command)")
assert "msime_client_key(s.session, MSIME_KEY_POINTER);" in ibus_source
assert "pointer_turn(engine);" in body(ibus_source, "void candidate_clicked(IBusEngine *engine, guint index, guint button,")
assert ibus_source.count("pointer_turn(e);") == 4
# IBus 的点击只带槽位号：candidate_clicked 先核对面板快照的会话和代次（与滚轮、翻页同一道），左键、滚轮和译文页都在它之后；轮次只开给真正派发的点击，每次 pointer_turn 之后、下一个 return 之前必有一次派发。译文页的序号取自快照里那一行的 id，不按点击时的页码重算。
ibus_clicked = body(ibus_source, "void candidate_clicked(IBusEngine *engine, guint index, guint button,")
ibus_click_fence = ibus_clicked.index('s.rendered_view.value("generation", uint64_t{0}) !=')
assert ibus_click_fence < ibus_clicked.index("if (s.translation_candidates_active) {") < ibus_clicked.index("pointer_turn(engine);")
assert ibus_click_fence < ibus_clicked.index("msime_client_select(s.session, generation, global_index)")
ibus_click_dispatches = (
    "apply(engine, msime_client_select(",
    "apply(engine, msime_client_command(",
    "commit_translation_candidate(engine,",
    "render_translation_candidates(engine);",
)
ibus_turn = ibus_clicked.find("pointer_turn(engine);")
assert ibus_turn >= 0
while ibus_turn >= 0:
    ibus_turn_end = ibus_clicked.find("return", ibus_turn)
    following = ibus_clicked[ibus_turn : ibus_turn_end if ibus_turn_end >= 0 else None]
    assert any(dispatch in following for dispatch in ibus_click_dispatches), following
    ibus_turn = ibus_clicked.find("pointer_turn(engine);", ibus_turn + 1)
assert "s.translation_page * page_size + index" not in ibus_clicked
assert 'row_id.value("index", s.translation_options.size())' in ibus_clicked
# 别的视图画上来时译文页的状态随之丢掉，点选、数字键和 Esc 不再按一页已经不在面板上的译文去选或恢复。
assert 'overlay["translation_page"] = s.translation_page;' in ibus_source
assert 'if (state(engine).translation_candidates_active && !view.contains("translation_page")) {' in body(ibus_source, "void render(IBusEngine *engine, const Json &view) {")
# 托盘菜单「候选翻页」的上一页、下一页走 property_activate 再调 page()，同样是指针翻页。
menu_paging = ibus_source.index('if (candidate_name == "CandidatePreviousPage" ||')
assert menu_paging < ibus_source.index("pointer_turn(engine);", menu_paging) < ibus_source.index("page(engine, candidate_name == \"CandidatePreviousPage\"", menu_paging)
# 宿主自己写入译文后丢弃组字用命令 19，生命周期核心把组字的结束记成 HostCommit 而不是取消。
assert source.count("command(MSIME_CANCEL_AFTER_HOST_COMMIT);") == 2
assert ibus_source.count("msime_client_command(s.session, MSIME_CANCEL_AFTER_HOST_COMMIT)") == 2
assert "setAuxDown(fcitx::Text(candidateAux()))" in source
assert "setAuxUp(fcitx::Text(candidateAux()))" not in source
# /fy's translation goes to the selected service whatever the gloss switches say, and never to an offline gloss.
for host in (source, ibus_source):
    assert "command_translation_query(" in host
assert ibus_source.index("music.release(session") < ibus_source.index(
    "msime_client_destroy(session)"
)
# Both hosts name the installed built-in sound packs, which the parent project installs.
assert (
    'MSIME_SOUND_PACKS="${CMAKE_INSTALL_FULL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/sound-packs"'
    in cmake_fcitx5
)
assert "installed_sound_pack_directory(" in ibus_source
assert (
    'DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/sound-packs")'
    in cmake
)
# IBus 候选属性菜单用共享文案（置顶、固定到第 N 位、取消固定、删除），按 macOS「候选操作」菜单的顺序排列，并从 CandidateFixN 动作名取格位。
assert "msime::linux_host::candidate_pin_label" in ibus_source
assert "candidate_fix_label(fix[12] - '0')" in ibus_source
assert "ibus_text_new_from_static_string(msime::linux_host::candidate_clear_fix_label)" in ibus_source
assert (
    ibus_source.index("ibus_text_new_from_static_string(msime::linux_host::candidate_clear_fix_label)")
    < ibus_source.index("ibus_text_new_from_static_string(msime::linux_host::candidate_remove_label)")
)
assert '"删除候选"' not in ibus_source
assert '"固定候选"' not in ibus_source
assert '"固定到 1"' not in ibus_source
assert 'std::string("取消固定 ")' not in ibus_source
# 仅用户词库来源的主词库候选提供操作，新旧框架菜单都由共享策略决定。
assert (
    'candidate_dictionary_actions_available(msime::linux_host::strict_json_value(state_.view_, "scheme", 0u), item->source())'
    in source
)
assert "candidate_dictionary_actions_available(scheme, item.source())" in menu_actions
assert (
    'source_(msime::linux_host::strict_json_value(candidate, "source", uint64_t{}))'
    in source
)
assert 'text_(candidate.at("text").get<std::string>())' in source
assert (
    'fixed_position_(msime::linux_host::strict_json_value(candidate, "fixed_position", uint8_t{}))'
    in source
)
assert "candidate_dictionary_removal_available(scheme, item.source(), item.text())" in menu_actions
assert "item.fixedPosition()" in menu_actions
assert "state_.session_ != item->session()" in source
assert "!state_.ic_.hasFocus() || !state_.input_enabled_" in source
assert "state_.privateInput() || state_.session_ != item->session()" in source
assert "voice_cancelled_" in source
assert "msime_voice_stream_inline_enabled(" in source
assert "voice_preedit_" in source and "voice_transcript_" in source
assert "msime_voice_result_or_transcript(" in source
assert "clipboard_generation_" in source
assert 'result.value("_path", std::string{}) == clipboard_path_' in source
assert (
    'msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == clipboard_generation_'
    in source
)
assert "const auto generation = clipboard_generation_" in source
assert "cloud_clipboard_generation_" in source
assert "cloud_clipboard_enabled_" in source
assert 'help_action_{&factory_, "help", msime::linux_host::menu_label::kHelp}' in source
assert 'feedback_action_{&factory_, "feedback", msime::linux_host::menu_label::kFeedback}' in source
assert '(std::strcmp(panel_, "voice") == 0 && !state->voice_enabled_)' in source
assert "if (!state || state->restricted() ||" in source
assert (
    "!state->ensure()) return;"
    not in source[
        source.index("class FcitxDesktopPanelAction") : source.index(
            "// The shared `floating_toolbar`"
        )
    ]
)
assert (
    "state->restricted() || state->privateInput()"
    not in source[
        source.index("class FcitxDesktopPanelAction") : source.index(
            "// The shared `floating_toolbar`"
        )
    ]
)
assert "toggleFloatingToolbar" in source
assert 'saveNestedBooleanPreference("floating_toolbar", "enabled", enabled)' in source
assert "toolbar_enabled_action_" in source
assert "toggleVoiceEnabled" in source
assert 'saveNestedBooleanPreference("voice_input", "enabled", enabled)' in source
assert "voice_enabled_action_" in source
assert "voice_enabled_);" in source
assert "voice_action_);" in source
assert "reloadFcitxService" in source
assert "reload_service_action_" in source
assert "maintenance_reload_held_" in source
# Fcitx5's ReloadConfig never reaches addons, so the reset runs in process: the controller's ReloadAddonConfig, the chord and the status-menu action all end in resetSessions.
assert "void reloadConfig() override { resetSessions(); }" in source
assert "engine_->resetSessions();" in source
assert "return reloadFcitxService();" not in source
assert "fcitx5-remote" in source
assert "ClientInputModeMemory.h" in source
assert "fcitx_app_input_modes" in source
assert "fcitx_global_input_mode" in source
assert "ic_.program()" in source
assert "restoreInputMode" in source
assert 'result.value("_socket", std::string{}) == cloud_clipboard_socket_' in source
assert (
    'msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == cloud_clipboard_generation_'
    in source
)
assert 'cloud_clipboard_enabled_ = result.value("enabled", true)' in source
# The provider relays the account API page as is, so the list is `items` ([{id,text,updated_at}]) like the shared panel reads it; `entries` is the cloud dictionary's shape.
assert 'result.value("items", Json::array())' in source
assert (
    'result.value("entries", Json::array())'
    not in source[
        source.index("void refreshCloudClipboard()") : source.index(
            "bool pasteCloudClipboard("
        )
    ]
)
assert "if (!cloud_clipboard_enabled_) return false;" in source
assert "emoji_generation_" in source
assert (
    'msime::linux_host::strict_json_value(result, "_generation", uint64_t{}) == emoji_generation_'
    in source
)
assert 'result["_generation"] = generation' in source
assert (
    'msime::linux_host::strict_json_value(item, "source", uint64_t{255}) == source'
    in source
)
assert "Json candidates = Json::array();" in source
assert "std::string panelPreview(const std::string &text)" in source
assert "fcitx::utf8::nextNChar(text.begin(), 40)" in source
assert "text.substr(0, 40)" not in source
assert "if (voice_job_.valid()) {" in source
assert (
    "voice cancellation does not wait for the provider future"
    in (root / "fcitx5/tests/native.cpp").read_text()
)
assert "msime-helpcode-schema" in source
assert "cycleHelpcodeSchema" in source
# The fixed IBus menu entry for the active helpcode-pack marker must be hidden
# when no pack is selected; sensitivity only disables an entry and still leaves
# the empty marker visible.
assert "ibus_property_set_visible(helpcode_schema_pack" in ibus_source
assert "toggleLocalMode" in source
assert "msime-local-unicode" in source
assert "msime-local-temporary-japanese" in source
# 五个可配置模式快捷键及各自的宿主状态都要保留，避免设置能保存却不生效。
for name in (
    "mode_shift_enabled_",
    "mode_ctrl_enabled_",
    "mode_ctrl_space_enabled_",
    "mode_ctrl_alt_space_enabled_",
    "character_set_shortcut_enabled_",
    "japanese_toggle_enabled_",
):
    assert name in source, name
for key in (
    "switch_language_shift",
    "switch_language_ctrl",
    "switch_language_ctrl_space",
    "switch_language_ctrl_alt_space",
    "switch_language_caps_lock",
    "toggle_character_set_ctrl_shift_f",
    "toggle_japanese_ctrl_shift_j",
):
    assert key in source, key
# Ctrl+Shift+J 在中文方案和日语之间切换：两个宿主都读这个键（Fcitx5 启动和热重载各一次，IBus 两处），离开中文时补记 last_chinese_scheme，英文模式之后才处理，Engine 自己的英文模式里交给应用，Fcitx5 一次按下只切换一次。
assert source.count('"toggle_japanese_ctrl_shift_j"') == 2
assert ibus_source.count('keybindings.value("toggle_japanese_ctrl_shift_j", false)') == 2
assert "msime::linux_host::japanese_toggle_target(" in source
assert "msime::linux_host::japanese_toggle_target(" in ibus_source
assert 'saveStringPreference("last_chinese_scheme", running);' in source
assert '{"last_chinese_scheme", leaving_chinese ? running : next}' in ibus_source
assert source.index("if (!toggleJapanese()) return false;") > source.index(
    "English mode still honours fullwidth output"
)
assert '!view_.value("dedicated_english", false)) {\n    if (!toggleJapanese()) return false;' in source
assert '!s.view.value("dedicated_english", false)) {\n    if (!toggle_japanese_scheme(engine))' in ibus_source
assert "toggle_chord_held_ = FcitxKey_j;" in source
assert "sym == FcitxKey_J ? FcitxKey_j : sym" in source
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
# 空格锁定固定为开：设置页不再给开关（`voice_space_lock_switch` 为 false），宿主也不读存下的值。
assert '"hotkey_hold_space_lock"' not in source
assert '"hotkey_hold_space_lock"' not in ibus_source
assert "bool voice_hotkey_hold_space_lock_ = true;" in source
assert 'voice_options_.value("asr_provider", std::string{"doubao"})' in source
assert "fcitx_system_dark_theme()" in source
# The portal read is a blocking D-Bus round trip: it runs once per addon on a worker, never from each context's loop tick.
assert source.count("fcitx_system_dark_theme()") == 1
theme_step = source[
    source.index("uint64_t stepSystemTheme()") : source.index(
        "void applySystemTheme(bool dark)"
    )
]
assert "system_theme_job_ = detachedJob(" in theme_step
assert theme_step.index("detachedJob(") < theme_step.index("fcitx_system_dark_theme()")
assert (
    "~FcitxEngine() override" in source
    and "system_theme_job_.wait_for("
    in source[source.index("~FcitxEngine() override") :]
)
assert "refreshSystemTheme" not in source
assert "system_theme_probe_due_" not in source
assert "setNextInterval(stepSystemTheme())" in source
assert (
    "pkg_check_modules(GIO REQUIRED IMPORTED_TARGET gio-2.0)"
    in (root / "fcitx5/CMakeLists.txt").read_text()
)
assert "fcitx_system_dark_theme" in (root / "fcitx5/SystemTheme.cpp").read_text()

# Runtime option reload must follow the same clipboard path precedence as
# initial session setup and fence reads started against the old history file.
assert 'nextClipboard = options.value("preferences_directory", std::string{})' in source
assert 'options.value("clipboard_history_path", std::string{})' in source
assert "++clipboard_generation_" in source
assert "clipboard_loading_ = false" in source
# Explicit clipboard_history_path has precedence over the derived preferences
# directory in both initial setup and runtime reload.
assert (
    'auto clipboard_path = options.value("clipboard_history_path", std::string());'
    in source
)
assert (
    'auto nextClipboard = options.value("clipboard_history_path", std::string{});'
    in source
)
# The shared option names the history file, while the Host API clipboard ABI
# receives its containing preferences directory.  Fcitx5 must normalize the
# explicit default filename before loading or mutating history.
assert "std::filesystem::path(clipboard_path).parent_path().string()" in source
assert "std::filesystem::path(nextClipboard).parent_path().string()" in source

# Cloud clipboard follows the provider socket contract: explicit options,
# environment override, then the per-user runtime socket, and hot reload must
# fence requests started against an old endpoint.
assert "cloudClipboardSocket" in source
assert "cloud-clipboard.sock" in source
assert "MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET" in source
assert "nextCloudClipboard" in source
assert "++cloud_clipboard_generation_" in source
assert "cloud_clipboard_enabled_ = true;" in source
assert 'result.value("_socket", std::string{}) == translation_socket_' in source
assert '{"_socket", socket}' in source
assert "FcitxSentenceTranslationAction" in source
assert "msime-translate-sentence" in source
assert "void translateSentence()" in source
assert "translation_manual_sentence_" in source
assert '"TranslateSentence"' in ibus_source
assert "void translate_sentence(IBusEngine *engine)" in ibus_source

# Voice provider discovery must match IBus and the standalone Linux provider
# contract, including the runtime socket fallback during hot reload.
assert 'providerSocket(options, "voice_provider_socket"' in source
assert '"MSIME_VOICE_PROVIDER_SOCKET", "voice.sock"' in source
assert 'preferences_.value("clipboard_history", false)' in source
assert 'if (!preferences_.value("clipboard_history", false))' in source
assert "translationSocket" in source
assert '"MSIME_TRANSLATION_PROVIDER_SOCKET", "translation.sock"' in source
assert '"MSIME_TRANSLATION_PROVIDER_SOCKET"' in ibus_source
assert '"translation.sock"' in ibus_source

# Moving preferences_directory while focused must invalidate the old async
# store read and retry state before loading from the new directory.
assert "nextPreferencesDirectory" in source
assert "preferences_job_session_ = 0" in source
assert "preferences_snapshot_ = Json()" in source
assert "preferences_save_retry_.reset()" in source

# Fcitx5 menu preference writes retain the last failed field and expose a retry
# action. A failed revision comparison must not leave the user with a silent
# diagnostic-only failure as the old async save path did.
assert "struct PendingPreferenceSave" in source
assert "startPreferenceSave" in source
assert "retryPreferenceSave" in source
assert "preferences_save_retry_" in source
assert "FcitxPreferenceSaveRetryAction" in source
assert "重试保存设置" in source

# Native Fcitx5 sessions use the same non-focus-stealing X11/Wayland voice
# surface as IBus when one is available, while the auxiliary panel remains the
# explicit fallback for headless or unsupported desktops.
for marker in (
    "WaveOverlayModel",
    "WaveOverlaySurface",
    "WaveOverlayX11Surface",
    "WaveOverlayWaylandSurface",
    "create_fcitx_wave_overlay_surface",
    "MSIME_WAVE_OVERLAY_BACKEND",
    "updateVoiceOverlay",
    "hideVoiceOverlay",
    "wave_overlay_.set_input_level",
    "syncVoiceOverlaySources",
    "wave_overlay_dismissed_",
    "dismissVoiceFailure",
    "msime::linux_host::wave_overlay_feedback_text(wave_overlay_)",
    "msime::linux_host::kWaveOverlayFailureMilliseconds",
):
    assert marker in source, marker
# 圆形操作按钮何时显示由浮层决定（空格锁定的录音或正在识别时，与 macOS 一样）；宿主不再在每次录音开始时打开它们。
assert "actions_visible = true" not in source
# 在浮层 `update()` 里处理的点击可能重置正在运行回调的帧定时器，销毁闭包；回调在 `update()` 之后不经捕获读取任何东西。
frame_timer = source[source.index("wave_overlay_frame_timer_ = loop_->addTimeEvent("):source.index("} else if (!animate && wave_overlay_frame_timer_) {")]
after_update = frame_timer[frame_timer.index("self->wave_overlay_surface_->update(self->wave_overlay_);"):]
assert "auto *const self = this;" in frame_timer
assert "self->wave_overlay_frame_timer_.get() == timer" in after_update
assert "\n            if (wave_overlay_" not in after_update
# 在 IBus 上浮层画出的失败提示同样不因继续打字而消失，与 macOS 和 Fcitx5 宿主一样，后者的 `render()` 从不收起它。
assert "s.wave_overlay.failure && s.wave_overlay_surface && s.wave_overlay_surface->animates()" in body(ibus_source, "void render(IBusEngine *engine, const Json &view) {")
# 失败提示只由浮层承载，与 macOS 一样：IBus 不再往辅助行另写一份（画不出浮层时 `FallbackSurface` 退到辅助文字），Fcitx5 只在画不出浮层时写辅助行；否则浮层收起后那份副本一直留在候选窗里。
assert "ibus_engine_update_auxiliary_text" not in body(ibus_source, "void show_voice_failure(IBusEngine *engine, const char *message) {")
assert "if (!(wave_overlay_visible_ && wave_overlay_surface_->animates())) {" in body(source, "void showVoiceFailure(const char *message) {")
# IBus 会话的每条出口都收起浮层并撤掉帧定时器和 fd 监听：`guarded()` 兜底和词典维护释放会话之后不再经过 `render()`。
assert "release_wave_overlay();" in body(ibus_source, "\n  void close() {")
assert "g_source_remove(wave_overlay_timer);" in body(ibus_source, "void release_wave_overlay() {")
# 浮层对 Cairo 和 Pango 的依赖不放进 `X11_SURFACE`，因为后者还决定是否构建 X11 的 Caps Lock 释放。
assert "pkg_check_modules(X11_SURFACE QUIET IMPORTED_TARGET x11 xfixes xrandr)\n" in (root / "CMakeLists.txt").read_text()
assert "WaveOverlayX11Surface.cpp" in (root / "fcitx5/CMakeLists.txt").read_text()
assert "WaveOverlayWaylandSurface.cpp" in (root / "fcitx5/CMakeLists.txt").read_text()

# The floating toolbar's eight component switches decide what the "工具栏" submenu
# contains. They decided nothing here before, while the settings page showed all of
# them for this platform, so each switch is pinned to the entry it governs.
assert "msime-toolbar" in source
assert "rebuildToolbarMenu" in source and "refreshToolbar" in source
# 默认值与共享的 FloatingToolbarPreferences::default（preferences.rs）一致：表情、手写、屏幕键盘和语音默认关。
for key, default in (
    ("english_mode", "true"),
    ("input_scheme", "true"),
    ("fullwidth", "true"),
    ("punctuation", "true"),
    ("character_set", "true"),
    ("emoji", "false"),
    ("handwriting", "false"),
    ("screen_keyboard", "false"),
    ("voice", "false"),
    ("settings", "true"),
):
    assert f'toolbar.value("{key}", {default})' in source, key
# The entries are the existing actions, not copies: a toolbar entry that behaved
# differently from the status-area action beside it would be a second
# implementation of the same toggle.
for action in (
    "input_mode_action_",
    "english_action_",
    "scheme_action_",
    "width_action_",
    "chinese_punctuation_action_",
    "traditional_action_",
    "desktop_emoji_action_",
    "handwriting_action_",
    "keyboard_action_",
    "voice_action_",
    "settings_action_",
    "promotion_action_",
    "website_action_",
):
    assert f"append(" in source and action in source, action
# A rebuild removes exactly what it added; the menu is shared across contexts.
assert "toolbar_entries_" in source
# english_mode 管的是中英文那一项（macOS 的 _inputModeButton），工具栏里没有「英文候选模式」；托盘留着失焦的上下文时不列语音和宣传，失焦时重建子菜单。
toolbar_menu = source[source.index("void FcitxEngine::rebuildToolbarMenu(fcitx::InputContext *ic) {") :]
toolbar_menu = toolbar_menu[: toolbar_menu.index("class FcitxFactory")]
assert 'append(toolbar.value("english_mode", true), &input_mode_action_);' in toolbar_menu
assert "&english_action_" not in toolbar_menu
assert "&& focused, &voice_action_);" in toolbar_menu and "append(focused, &promotion_action_);" in toolbar_menu
hold = source[source.index("void holdMenuContext(fcitx::InputContext *ic) {") :]
hold = hold[: hold.index("\n  }\n")]
assert hold.index("rebuildToolbarMenu(ic);") < hold.index("arrangeStatusArea(ic);")
# IBus 的工具栏子菜单用同一套菜单文案（MenuLabels.h），同一个开关管同一项。
ibus_toolbar = ibus_source[ibus_source.index("IBusProperty *toolbar_property(IBusEngine *engine) {") :]
ibus_toolbar = ibus_toolbar[: ibus_toolbar.index("\n}\n")]
assert "Toolbar/EnglishMode" not in ibus_source
assert 'append_toggle("Toolbar/InputMode", msime::linux_host::menu_label::kChineseInput,' in ibus_toolbar
assert 'available && toolbar.value("english_mode", true), s.input_enabled);' in ibus_toolbar
for label in ("kChinesePunctuation", "kFullwidth", "kTraditionalOutput", "kSettings", "kScheme"):
    assert f"msime::linux_host::menu_label::{label}" in ibus_toolbar, label

# 还没做首次配置时，激活和按键两条路径都要给出具体提示，而不是笼统的「请检查运行配置」；但只有激活会打开设置窗口，打字途中弹出的窗口可能抢走键盘焦点。引导走与 IBus 启动器同一个脚本，每个登录会话只弹一次的限制因此两边共用。
assert (
    source.count(
        "catch (const OptionsNotConfigured &) { notConfigured(*state, true); }"
    )
    == 1
)
# Key events and the in-process reset show the hint without the guide.
assert (
    source.count(
        "catch (const OptionsNotConfigured &) { notConfigured(*state, false); }"
    )
    == 2
)
resetSessions = source[source.index("void resetSessions() {") :]
resetSessions = resetSessions[: resetSessions.index("\n  }\n")]
assert (
    "notConfigured(*state, false)" in resetSessions
    and "notConfigured(*state, true)" not in resetSessions
)
keyEvent = source[source.index("void keyEvent(const fcitx::InputMethodEntry &") :]
keyEvent = keyEvent[: keyEvent.index("\n  }\n")]
assert (
    "notConfigured(*state, false)" in keyEvent
    and "notConfigured(*state, true)" not in keyEvent
)
assert "kFirstRunHint" in source and "kFirstRunGuideProgram" in source
assert 'fcitx::startProcess({guide, "--host", "fcitx5"})' in source
assert 'MSIME_BINDIR="${CMAKE_INSTALL_FULL_BINDIR}"' in cmake_fcitx5
assert (
    "msime-linux-first-run-guide"
    in cmake[
        cmake.index("foreach(MSIME_EDITION_SCRIPT") : cmake.index(
            "endforeach()", cmake.index("foreach(MSIME_EDITION_SCRIPT")
        )
    ]
)

# The Korean Hanja keys (Hangul_Hanja and a bare F9), which also open the Zhuyin list, and the open lists are read through the shared core/KoreanHanja.h and core/InputSchemes.h in both hosts. The keys are decided before the rules that would finish the composition and hand the key to the application, so a trigger the Engine leaves unhandled (a lone jamo) never writes the syllable out and then leaks the key. Down is the second key that opens the Zhuyin list, so each host sends the command twice.
fcitx_convert = "command(MSIME_OPEN_CANDIDATE_LIST)"
ibus_convert = "msime_client_command(s.session, MSIME_OPEN_CANDIDATE_LIST)"
assert source.count(fcitx_convert) == 2 and ibus_source.count(ibus_convert) == 2
for host in (source, ibus_source):
    assert "msime::linux_host::korean_hanja_key(" in host
    assert "msime::linux_host::candidate_list_composition(" in host
    assert "msime::linux_host::zhuyin_list_down_key(" in host
    assert "msime::linux_host::korean_hanja_list_open(" in host
    assert "msime::linux_host::opened_candidate_list(" in host
assert "msime::linux_host::korean_hanja_punctuation_key(" in ibus_source
fcitx_key = source[source.index("bool FcitxState::key(fcitx::KeyEvent &event) {") :]
fcitx_key = fcitx_key[: fcitx_key.index("\n}\n")]
assert fcitx_key.index(fcitx_convert) < fcitx_key.index(
    "if (composing && commitsOnBlur() && !openedList)"
)
assert fcitx_key.index(fcitx_convert) < fcitx_key.rindex(
    "if (composing) command(MSIME_FINISH_COMPOSITION);"
)
ibus_key = ibus_source[
    ibus_source.index(
        "gboolean process_key(IBusEngine *engine, guint key, guint keycode, guint flags) {"
    ) :
]
ibus_key = ibus_key[: ibus_key.index("\n}\n")]
assert ibus_key.index(ibus_convert) < ibus_key.index(
    "commits_on_blur && has_composition)"
)
# With a list open Return chooses the highlighted Hanja or Zhuyin candidate in both hosts rather than writing the composition out and breaking the line.
assert "command(openedList ? MSIME_COMMIT_CANDIDATE : MSIME_COMMIT_RAW)" in fcitx_key
ibus_return = ibus_key[ibus_key.index("case IBUS_Return:") :]
assert ibus_return.index("if (opened_list) {") < ibus_return.index(
    "command = MSIME_COMMIT_RAW;"
)
# 两个宿主都提供藏文（Engine 方案 8），面板符号为「藏」。藏文和越南文一样离开组字时写出组字内容：IBus 的导航键必须发 MSIME_FINISH_COMPOSITION，若发 MSIME_COMMIT_CANDIDATE，Engine 会把它当作空格，多上屏一个音节点。
assert '{&scheme_tibetan_action_, "msime-scheme-tibetan"}' in source
assert 'FcitxSchemeItemAction scheme_tibetan_action_{&factory_, 8, "藏文"};' in source
assert 'if (scheme == "tibetan") return "藏文";' in menu_labels
assert 'case msime::linux_host::InputModeIndicator::Tibetan: return "藏";' in source
assert '"Scheme/Tibetan"' in ibus_source
assert (
    'case msime::linux_host::InputModeIndicator::Tibetan: symbol = "藏"; break;'
    in ibus_source
)
assert (
    "korean_hanja_list || zhuyin_scheme || vietnamese_scheme || tibetan_scheme"
    in ibus_key
)

# 两个宿主的菜单都只在有 msime-stroke.db 时提供笔画（Engine 方案 9）：Fcitx5 注册它的动作，单独出现 msime-stroke.db 时也重建菜单；IBus 列出 Scheme/Stroke，让 PropertyActivate 放行它并映射到 "stroke" id。IBus 缺任何一半，选笔画都会悄无声息地什么也不做。状态区的方案动作也要标出「输入方案（笔画）」，否则会落到默认的全拼。
assert '{&scheme_stroke_action_, "msime-scheme-stroke"}' in source
assert 'FcitxSchemeItemAction scheme_stroke_action_{&factory_, 9, "笔画"};' in source
assert 'if (scheme == "stroke") return "笔画";' in menu_labels
assert 'schemeAvailable("stroke")' in source
assert 'if (scheme == "stroke") return "Scheme/Stroke";' in ibus_source
assert 'property_name != "Scheme/Stroke"' in ibus_source
assert 'property_name == "Scheme/Stroke" ? std::string("stroke")' in ibus_source

# #5988：主题同步每 250 ms 跑一拍。经典界面的 getConfig() 每次都扫描并解析全部已装主题，所以状态页只读落盘的 classicui.conf；主题同步先比主题输入，已接管写好（且不是一次新的主动选择、落盘的活动主题、跟随深色、字体与 Wayland 字体 DPI 也没变）就不重写主题、也不调 getConfig()，于是卸载还原或用户手改的主题不会被接管回去；还没接管成功时只在输入或落盘选择变化、或写失败到了重试时间时才调一次 getConfig()，比较时不栅格化主题；「重启输入法服务」清掉两份记录。拍子本身仍要同步主题，否则在 fcitx5-configtool 里改回默认主题后不会恢复接管。
def body(text, start, end):
    begin = text.index(start)
    return text[begin:text.index(end, begin + len(start))]
publish = body(source, "  void publishCandidatePanelStatus() {", "\n  }\n")
assert "getConfig" not in publish and "read_classicui_theme_selection()" in publish
apply_theme = body(source, "  bool applyCandidatePanelTheme(fcitx::AddonInstance *classicui,", "\n  }\n")
# 一次主动选择（chosen）时即使输入没变也重新接管，所以返回条件多了它；主题输入与落盘选择不变时仍不调 getConfig()。
assert apply_theme.count("classicui->getConfig()") == 1
assert apply_theme.index("if (inputs == candidate_theme_applied_ && !chosen && hint_stamp == classicui_hint_stamp_) return false;") < apply_theme.index("read_classicui_theme_selection()") < apply_theme.index("if (attempt_key == candidate_theme_attempt_ && now < candidate_theme_retry_at_) return false;") < apply_theme.index("classicui->getConfig()")
assert apply_theme.index("hint_inputs_ = fcitx_hint_inputs(current, system_dark);") < apply_theme.index("if (inputs == candidate_theme_applied_ && !chosen) {") < apply_theme.index("if (!chosen && !host::fcitx_theme_replaceable")
assert '{"hint", hint_stamp}' in apply_theme
assert "candidate_theme_retry_at_ = now + kCandidateThemeRetry;" in apply_theme
assert "host::fcitx_candidate_theme(" not in apply_theme
assert "set_classicui_config(*classicui, current, config);" in apply_theme
reset = body(source, "  void resetSessions() {", "\n  }\n")
assert "candidate_theme_applied_.clear();" in reset and "candidate_theme_attempt_.clear();" in reset
assert "syncCandidatePanelTheme(false);" in body(source, "  void refreshProviderSockets() {", "\n  }\n")
assert 'fcitx::readAsIni(config, "conf/classicui.conf");' in source

# 整句改字：两个入口都按 core/InputSchemes.h 的同一条 edits_sentence 决定不带修饰键的左右键。全拼和双拼的左右键发改字命令（引擎进不了改字时自己退回字母光标）；改字时行内画 conversion_preedit 给出的整句并显式放光标。
assert "edits_sentence(view_) ? MSIME_CONVERSION_LEFT : MSIME_MOVE_LEFT" in source
assert "edits_sentence(view_) ? MSIME_CONVERSION_RIGHT : MSIME_MOVE_RIGHT" in source
conversion_render = body(source, "  } else if (const auto conversion = msime::linux_host::conversion_preedit(view_); conversion.active) {", "  } else if (")
assert "preedit.setCursor(static_cast<int>(conversion.caret_bytes));" in conversion_render
assert "fcitx::TextFormatFlag::HighLight" in conversion_render
ibus = (root / "src/core/ClientEngine.cpp").read_text()
assert "edits_sentence(s.view) ? MSIME_CONVERSION_LEFT : MSIME_MOVE_LEFT" in ibus
assert "edits_sentence(s.view) ? MSIME_CONVERSION_RIGHT : MSIME_MOVE_RIGHT" in ibus
assert "msime::linux_host::conversion_preedit(view)" in ibus
# Shift+左右和 Ctrl+左右（只按这一个修饰键）在组字时按音节移动拼音光标，所有方案都一样，全拼、双拼和五笔不再分开：共享运行时在横排候选窗里会把字母光标命令（MSIME_MOVE_LEFT/RIGHT）换成移动候选，分段命令从不被换，所以这两个组合键不能再按 edits_sentence 退回字母光标。锁光标的方案（韩文、注音、越南文、藏文）和没有组字时照旧；Ctrl+退格不变，Shift+退格不进分段分支。
segment = body(source, "  if (composing && !commitsOnBlur()) {\n", "\n  }\n")
assert "edits_sentence" not in segment and "MSIME_MOVE_LEFT)" not in segment and "MSIME_MOVE_RIGHT)" not in segment
assert "if (ctrl && !alt && !shift && sym == FcitxKey_BackSpace) return command(MSIME_BACKSPACE_SEGMENT);" in segment
assert "ctrl != shift && !alt &&" in segment
assert "fcitx::KeyState::Super, fcitx::KeyState::Hyper,\n                                         fcitx::KeyState::Meta, fcitx::KeyState::Mod5" in segment
assert "if (segmentChord && (sym == FcitxKey_Left || sym == FcitxKey_KP_Left))\n      return command(MSIME_MOVE_LEFT_SEGMENT);" in segment
assert "if (segmentChord && (sym == FcitxKey_Right || sym == FcitxKey_KP_Right))\n      return command(MSIME_MOVE_RIGHT_SEGMENT);" in segment
# 裸 Shift 在松开时才切换中英文。Shift+方向键被上面的分段分支提前返回之前，Shift 必须已经记成「用在组合键里」并撤销布防，否则松开 Shift 会把这次组合当成手势。
fcitx_key = source[source.index("bool FcitxState::key(fcitx::KeyEvent &event) {"):]
fcitx_segment_at = fcitx_key.index("  if (composing && !commitsOnBlur()) {\n")
assert fcitx_key.index("if (raw.test(fcitx::KeyState::Shift)) shift_in_combination_ = true;") < fcitx_segment_at
assert fcitx_key.index("\n  pure_shift_candidate_ = false;\n  pure_ctrl_candidate_ = false;\n") < fcitx_segment_at
ibus_segment = body(ibus, "    const bool shift_only = modifiers == IBUS_SHIFT_MASK &&\n", "\n      return;\n    }\n")
assert "edits_sentence" not in ibus_segment and "MSIME_MOVE_LEFT :" not in ibus_segment
assert "(ctrl_only && (key == IBUS_BackSpace || left || right))" in ibus_segment
assert "(shift_only && (left || right))" in ibus_segment
assert "if (!locks_caret && segment_edit_key &&" in ibus_segment
assert "(left ? MSIME_MOVE_LEFT_SEGMENT : MSIME_MOVE_RIGHT_SEGMENT)" in ibus_segment
ibus_process = ibus[ibus.index("gboolean process_key(IBusEngine *engine, guint key, guint keycode, guint flags) {"):]
assert ibus_process.index("\n  s.pure_shift_candidate = false;\n  s.pure_ctrl_candidate = false;\n") < ibus_process.index('\n  guarded(engine, "process_key", [&] {\n') < ibus_process.index("    const bool shift_only = modifiers == IBUS_SHIFT_MASK &&\n")
# 候选窗显示时 Home 发 MSIME_FIRST_CANDIDATE_OR_HOME：高亮不在首选时回到首选，已在首选时把拼音光标移到开头；End 不变。日文转换用空格循环回首选是无条件的，仍发 MSIME_FIRST_CANDIDATE。
assert "case FcitxKey_Home: case FcitxKey_KP_Home:\n      return command(MSIME_FIRST_CANDIDATE_OR_HOME);\n    case FcitxKey_End: case FcitxKey_KP_End:\n      return command(MSIME_LAST_CANDIDATE);" in source
assert "return command(MSIME_FIRST_CANDIDATE);" not in source
assert "command(action == Action::StepFirst ? MSIME_FIRST_CANDIDATE\n" in source
assert "? MSIME_FIRST_CANDIDATE_OR_HOME\n                               : MSIME_LAST_CANDIDATE;" in ibus
assert "action == Action::StepFirst ? MSIME_FIRST_CANDIDATE\n" in ibus
# 候选列表打开（韩文汉字列表、注音列表）时，两个入口都与 Windows 的 korean_hanja_key 一样让四个方向键移高亮：←↑ 上一个、→↓ 下一个，不看导航开关。
list_arrows = body(source, "  if (composing && openedList &&\n", "\n  }\n")
assert "case FcitxKey_Left: case FcitxKey_KP_Left: case FcitxKey_Up: case FcitxKey_KP_Up:\n      return command(MSIME_PREVIOUS_CANDIDATE);" in list_arrows
assert "case FcitxKey_Right: case FcitxKey_KP_Right: case FcitxKey_Down: case FcitxKey_KP_Down:\n      return command(MSIME_NEXT_CANDIDATE);" in list_arrows
ibus_list_arrows = body(ibus, "    if (opened_list && modifiers == 0 &&\n", "\n    }\n")
assert "backwards ? MSIME_PREVIOUS_CANDIDATE : MSIME_NEXT_CANDIDATE" in ibus_list_arrows
assert "key == IBUS_Up || key == IBUS_KP_Up;" in ibus_list_arrows

# 脚本插件的注释：两个宿主都经 candidates/CandidateScriptAnnotation.h 读 `script_annotation`，画在整行最后。它不走引擎注释那条按辅助码开关、本地模式和训音决定画不画的路，也不做繁体转换；IBus 用序号的颜色只画它自己的区间。
for host in (source, ibus_source):
    assert "msime::linux_host::candidate_script_annotation(candidate)" in host
    assert "msime::linux_host::candidate_script_annotation_suffix(" in host
    assert '"script_annotation"' not in host
fcitx_row = body(source, "FcitxCandidateDisplay candidateDisplay(", "\n}\n")
assert fcitx_row.index('candidate.at("translation")') < fcitx_row.index(
    "candidate_script_annotation(candidate)"
) < fcitx_row.index("row.append(display.suffix,") < fcitx_row.index(
    "fcitx::TextFormatFlags{fcitx::TextFormatFlag::DontCommit});\n  }\n  display.classic = std::move(row);"
) < fcitx_row.rindex("return display;")
ibus_row = body(
    ibus_source,
    "  for (size_t index = 0; index < candidates.size(); ++index) {\n    const auto &candidate = candidates.at(index);\n    auto value = candidate.at(\"text\").get<std::string>();",
    "    ibus_lookup_table_append_candidate(table, text);",
)
ibus_script = ibus_row[
    ibus_row.index("candidate_script_annotation(candidate)") : ibus_row.index(
        "auto text = ibus_text_new_from_string(value.c_str());"
    )
]
assert "traditional_display" not in ibus_script
assert ibus_row.index(
    "value += traditional_display(state(engine), view, std::move(tail));"
) < ibus_row.index("candidate_script_annotation(candidate)")
assert "*row_number_color,\n                                 script_start, script_end);" in ibus_row

print("Fcitx5 addon metadata passed")
