#!/usr/bin/env python3
"""Every feature the reference has shipped has been looked at.

The configuration keys say what the reference can be told to do; the interface actions say what its
windows can ask its host to do. Neither notices a feature that changes behaviour without adding a
key or a message - the candidate window that stopped strobing under load, the pinyin buffer that
grew a ceiling, the installer page that stopped putting people online without asking.

The reference's own changelog does notice those, because it is generated from its commits: a `feat:` commit becomes a bullet. So this keeps the list of bullets that have been read, together with where each one landed here, and fails on a bullet nobody has looked at yet. The point is not that every feature must exist here - several deliberately do not - but that none goes unnoticed.

The changelog only covers what the reference has released, and the fixed source is well past its last release; the history before the changelog began is not in it either. So the `feat:` commits of the fixed source are read too, straight from its log: each subject must match a reviewed changelog bullet (written as the changelog would write it) or have its own entry in REVIEWED_COMMITS.

The reference checkout is optional. Without it the check reports what it would have needed and
passes, the same as every other stage that depends on something not every machine has.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

from reference_source import pinned_reference, reference_root, show_file

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHANGELOG = "CHANGELOG.md"


REFERENCE = reference_root(ROOT)

# Each feature bullet the reference has shipped, and what became of it here. The text is the bullet
# with its trailing commit links stripped, exactly as the changelog writes it.
#
# Keep the answers specific. "Already supported" is the sentence that makes this check worthless;
# name the file, the constant or the decision, so the next person can check the claim rather than
# trusting it.
REVIEWED: dict[str, str] = {
    "**contracts:** sync engine product lock contract": (
        "Engine-side release plumbing for an Engine released on its own. Nothing is pinned here: "
        "the input engine is the workspace crate crates/engine, built and tested with the rest of "
        "this repository, so there is no separately released product to lock."
    ),
    "**contracts:** verify shared product lock primitives": (
        "Same release plumbing as the bullet above."
    ),
    "**installer:** 首次安装时询问云候选，不再默认静默联网": (
        "platforms/windows/installer/msime_setup.iss: CreateInputOptionPage after the licence "
        "page, skipped on upgrade; on a fresh install the choice goes to installer-choices.json in the "
        "data directory, which the Server applies to the shared preferences on its first preparation "
        "(platforms/windows/src/system/FirstRun.h take_installer_cloud_choice), and to "
        "[general].cloud_candidates of the config.toml this install created. macOS: platforms/macos/src/input/InputController.mm activateServer: "
        "prompt (requestCloudCandidatesConsentIfNeeded) plus the MSIMEClientCloudCandidatesConsent "
        "key in platforms/macos/src/settings/AppearancePreferences.mm, asked on a fresh profile only."
    ),
    "**installer:** include release PDB symbols": (
        "Packaging. platforms/windows/installer/Prepare-PackageFiles.ps1 collects what this "
        "product ships; symbol publication is a release decision for the repository owner."
    ),
    "**langbar:** switch IME mode icons by system light/dark theme": (
        "platforms/windows/tsf/LanguageBar/LanguageBar.cpp: IsSystemDarkMode reads "
        "Personalize\\SystemUsesLightTheme; ModeIconResource picks the mode button's icon and "
        "ResolveThemeIconIndex the others'. Assets are tsf/assets/{cn,en,cap,jp,kr}-{light,dark}.ico "
        "plus one pair per further scheme (tsf/assets/README.md) - the reference has no Japanese "
        "or per-scheme indicator."
    ),
    "**product:** keep WebView and native contracts in one locked combination": (
        "Release plumbing; see the product lock bullets above."
    ),
    "**product:** lock release inputs and share negotiated IPC contracts": (
        "Release plumbing; see the product lock bullets above."
    ),
    "**product:** lock the shared-session stack and validate dictionary formats": (
        "Dictionary format validation is crates/client-core/src/dictionary/import.rs, which is "
        "where the shared import path enforces the same rules."
    ),
    "**product:** require locked commits to be on their default branch at release": (
        "Release plumbing; see the product lock bullets above."
    ),
    "**product:** validate and lock the shared-session release combination": (
        "Release plumbing; see the product lock bullets above."
    ),
    "**release:** mark automatically built releases in the title and notes": (
        "Release process, not product behaviour."
    ),
    "**release:** separate the automatic build channel from the release channel": (
        "Release process, not product behaviour."
    ),
    "**server:** show translations in horizontal candidate windows": (
        "platforms/windows/src/candidate/CandidateWindow.cpp appends `  · <translation>` to the "
        "candidate label in both the horizontal and the flyout paths."
    ),
    "also sync double/single char status when switch windows": (
        "platforms/windows/src/input/InputQueue.cpp accepts DoubleSingleByteSwitch so ModeMailbox "
        "can publish the host's notification to the toolbar."
    ),
    "notify UI process on punctuation mode change": (
        "InputQueue.cpp routes PuncSwitch to FocusedSession::set_chinese_punctuation, and the "
        "toolbar repaints on a changed chinese_punctuation in FloatingToolbarWindow.cpp."
    ),
    "set max len limit for pinyin input": (
        "MAX_PINYIN_LENGTH in platforms/windows/tsf/Header/Define.h, applied in Key/KeyEventSink.cpp "
        "to the deferred key budget, the shadow raw input and the reported input length."
    ),
    "support exact candidate character commits": (
        "Word-to-character commits without appended punctuation: "
        "platforms/windows/tests/input/word_character_policy.cpp covers the policy."
    ),
    "sync IME status to UI on thread focus and punctuation switch": (
        "InputQueue.cpp carries the punctuation state on StatusSnapshot and FocusRestored, so a "
        "client taking focus reports it rather than the toolbar guessing."
    ),
    "synchronous tsf and server double/single char status via ipc": (
        "Same DoubleSingleByteSwitch path as the window-switch bullet above."
    ),
}

# The `feat:` commits of the fixed source that no changelog bullet above stands for, keyed by commit subject exactly as `git log --format=%s` prints it. A subject shared by two commits (a cherry-pick or a re-land) is one feature and one entry.
#
# Same rule as REVIEWED: name where it landed. An answer beginning "Not ported" or "Partly ported" is a known parity gap, not a review that found the feature.
REVIEWED_COMMITS: dict[str, str] = {
    "feat(engine): 新增加加辅助码（拼音加加），第六套辅助码方案": (
        "crates/engine/src/assets.rs registers the table carried in "
        "resources/helpcodes/jiajia_helpcode.txt as the sixth scheme; `jiajia` is a helpcode "
        "scheme in crates/client-core/src/preferences.rs and platforms/windows/installer/config.default.toml."
    ),
    "feat: 输入统计（DLL 采集、Server 聚合存储与设置页展示）": (
        "Collected in the Server rather than the DLL: platforms/windows/src/input/TypingStatistics.h "
        "hands each commit to the shared store in crates/client-core/src/typing_statistics.rs. Keys "
        "the TIP passes through are counted in the DLL by platforms/windows/tsf/IPC/PassthroughStatistics.h. "
        "The settings page shows them (apps/desktop/tests/settings/typing-statistics.test.tsx)."
    ),
    "feat(installer): 安装前检查 WebView2 与 VC 运行库": (
        "platforms/windows/installer/msime_setup.iss: ReadWebView2Version and VCRuntimeKey registry "
        "checks before install, pinned by scripts/test-installer-prerequisites.py."
    ),
    "feat(engine): 整句候选把词格排到 Google 解码器之前": (
        "Settled by dropping the Google decoder: crates/engine/src/quanpin/dictionary.rs and "
        "crates/engine/src/shuangpin/dictionary.rs build the sentence block from the lattice alone, "
        "inserted by crates/engine/src/lattice/merge.rs, so no Google sentence is left to put in "
        "front of it in either scheme."
    ),
    "feat(installer): 打包时生成并安装词格语言模型 sc.lm": (
        "Deliberately absent with the KenLM scorer below: the lattice reads msime-bigram.bin/msime-trigram.bin, "
        "which ship with the pinned dictionary release in resources/desktop-dictionary.lock.json."
    ),
    "feat(engine): 词格整句改用 kenlm 三元模型打分": (
        "Deliberately absent: crates/engine scores lattice paths with its own bigram/trigram "
        "tables (crates/engine/src/lattice/ngram.rs, crates/engine/src/lattice/decode.rs), "
        "calibrated on this repository's sentence sets; crates/engine/src/lattice/merge.rs keeps "
        "the reference's exact-reading fix without the KenLM model."
    ),
    "feat(windows): Ctrl+左右键按分词移动光标": (
        "platforms/windows/src/input/InputKeyPolicy.h is_segment_caret_key (Ctrl only) maps to "
        "MSIME_MOVE_LEFT_SEGMENT/MSIME_MOVE_RIGHT_SEGMENT in platforms/windows/src/input/KeyEvent.h; "
        "the TIP side is _HandleCompositionSegmentEdit in platforms/windows/tsf/Key/KeyHandler.cpp."
    ),
    "feat(server): Ctrl+Backspace 按分词删除前置拼音（#187）": (
        "is_segment_backspace_key in platforms/windows/src/input/InputKeyPolicy.h sends "
        "MSIME_BACKSPACE_SEGMENT; once the reading is empty, ReplyComposer::restore_segment in "
        "platforms/windows/src/ipc/ReplyComposer.cpp and _creatingWordRestoreHistory in "
        "platforms/windows/tsf/Key/KeyHandler.cpp take back the last selected segment."
    ),
    "feat(installer): 安装时可自定义数据目录位置": (
        "platforms/windows/installer/msime_setup.iss: DataDirPage and GetDataDir, the DataDir "
        "registry value and MigrateUserDataDir for a moved directory."
    ),
    "feat(candidate): Ctrl+Enter 上屏候选右侧的译文": (
        "IsTranslationCommitShortcut in platforms/windows/tsf/Key/KeyEventSink.cpp and "
        "ReplyComposer::commit_candidate_translation in platforms/windows/src/ipc/ReplyComposer.cpp; "
        "multi-sense translations open a sub-page (platforms/windows/tests/runtime/session_smoke.cpp)."
    ),
    "feat(server): Backspace 删空剩余拼音后退选已选分词（#35）": (
        "Not ported for plain Backspace. On Windows only Ctrl+Backspace takes a selected segment "
        "back (see the #187 entry); plain Backspace that empties the reading ends the composition in "
        "_HandleCompositionBackspace (platforms/windows/tsf/Key/KeyHandler.cpp). The shared runtime "
        "has the rule (retreat_phrase_selection in crates/input-runtime/src/runtime.rs) but only "
        "behind phrase_preedit, which the Windows host never requests."
    ),
    "feat(windows): 重构智能标点": (
        "The five smart_punctuation* keys in platforms/windows/installer/config.default.toml, read "
        "in platforms/windows/src/entrypoints/server_main.cpp; the space-convert and same-key revert "
        "edits are FUNCTION_SMART_PUNCTUATION_CONVERT/REVERT in "
        "platforms/windows/tsf/IME/MetasequoiaIMEBaseStructure.h."
    ),
    "feat(japanese): 日语模式禁用 -/= 翻页并用 - 打长音符": (
        "platforms/windows/src/input/PunctuationPolicy.h (no -/= paging in the Japanese scheme) and "
        "EditPolicy.h (the minus key goes to the Engine as the long-vowel mark); covered by "
        "platforms/windows/tests/input/japanese_keys.cpp."
    ),
    "feat(ipc): reserve dedicated voice control pipe": (
        "platforms/windows/src/voice/VoiceControllerListener.h listens on FanyImeVoiceControlNamedPipe."
    ),
    "feat(ipc): dispatch validated voice lifecycle commands": (
        "platforms/windows/src/voice/VoiceControllerDispatch.h."
    ),
    "feat(ipc): validate voice control leases": (
        "VoiceControllerDispatch.h binds each session to a FocusLease and revalidates it before "
        "acting; see also platforms/windows/src/voice/VoiceControllerConnection.h."
    ),
    "feat(voice): expose serialized lifecycle controls": (
        "platforms/windows/src/voice/VoiceControllerMailbox.h keeps one outstanding request per "
        "endpoint, feeding VoiceControllerDispatch.h."
    ),
    "feat(installer): add SimplySign packaging scripts": (
        "platforms/windows/installer/Package-SimplySign.ps1, Sign-Installer-SimplySign.ps1 and "
        "Sign-PackageBinaries-SimplySign.ps1."
    ),
    "feat(settings): API Key 配置项增加连通性测试按钮": (
        "The settings page calls the test_api_credential command (apps/desktop/src-tauri/src/lib.rs, "
        "apps/desktop/src/account/credential-test-client.ts); probes live in "
        "crates/client-core/src/credential/."
    ),
    "feat(translation): 候选词翻译支持小牛翻译（NiuTrans）": (
        "crates/host-api/src/niutrans_translation.rs builds the request; "
        "platforms/windows/src/candidate/TranslationWorker.cpp sends it; [niutrans] in "
        "platforms/windows/installer/config.default.toml."
    ),
    "feat(voice): 豆包语音识别支持新版控制台单 API Key 鉴权": (
        "crates/client-core/src/credential/doubao_auth.rs picks the headers per auth mode; "
        "platforms/windows/src/voice/DoubaoAsrClient.cpp passes auth_mode through."
    ),
    "feat(tsf): 成对标点补全加修饰键闸门与配对栈": (
        "_pairedPunctuationStack and the deferred caret move in "
        "platforms/windows/tsf/Composition/Composition.cpp; paired_punctuation in "
        "platforms/windows/installer/config.default.toml."
    ),
    "feat(engine): ü 系拼写别名归一，非标准拼法候选带轻标记": (
        "The alias normalisation is in crates/engine/src/pinyin/syllables.rs and "
        "crates/engine/src/pinyin/autocorrect.rs (reference commit 94abc08e). Windows projects "
        "the view's `corrected` flag in platforms/windows/src/candidate/CandidatePresentation.h "
        "and draws the same light `*` marker as the other hosts."
    ),
    "feat(engine): 全拼纠错补齐漏字/多字，k-best 切分按词频消解歧义": (
        "crates/engine/src/pinyin/autocorrect.rs and crates/engine/src/pinyin/typos.rs, with the "
        "typo edges in crates/engine/src/quanpin/typo_edges.rs (reference commit 3c2f3ae3)."
    ),
    "feat(installer): 出厂配置模板补模糊音播种标记": (
        "fuzzy_seeded in platforms/windows/installer/config.default.toml."
    ),
    "feat(server): 模糊音首次启用播种规则并持久化标记": (
        "crates/client-core/src/preferences.rs seeds every rule on the first enable and keeps "
        "fuzzy_pinyin.seeded across saves."
    ),
    "feat(installer): 出厂配置模板补模糊音总开关": (
        "fuzzy_pinyin in platforms/windows/installer/config.default.toml."
    ),
    "feat(ui-html): 模糊音分区改为可折叠风格并接总开关": (
        "The fuzzy-pinyin section of packages/ui/src/index.tsx (fuzzyPinyinGroups), shown on "
        "Windows because HostCapabilities sets fuzzy_pinyin (crates/client-core/src/host_surface.rs)."
    ),
    "feat(server): 模糊音增加总开关并在聚合 getter 门控": (
        "FuzzyPinyinPreferences::active_rules in crates/client-core/src/preferences.rs is empty "
        "while the master switch is off."
    ),
    "feat(installer): 出厂配置模板补 11 个模糊音键": (
        "The eleven fuzzy_* rule keys in platforms/windows/installer/config.default.toml."
    ),
    "feat(ui-html): 设置页输入分区新增模糊音开关": (
        "Same settings section as the collapsible fuzzy-pinyin entry above."
    ),
    "feat(server): 模糊音规则接入配置与会话注入": (
        "crates/host-api/src/lib.rs sets options.fuzzy_pinyin_rules from the preferences; the "
        "Windows Server session is a host-api session (platforms/windows/src/ipc/ServerSession.cpp)."
    ),
    "feat(server): WebView2 候选窗口支持滚轮翻页，并加上开关": (
        "The candidate window here is native: platforms/windows/src/candidate/CandidateWheel.h "
        "folds wheel deltas into pages, gated by paging_mouse_wheel in "
        "platforms/windows/installer/config.default.toml."
    ),
    "feat(server): support mouse wheel paging on candidate window (fixes #81)": (
        "Same CandidateWheel.h path as the WebView2 wheel entry above."
    ),
    "feat(server): 设置窗口关闭后先隐藏，十分钟后才真正退出": (
        "apps/desktop/src-tauri/src/lib.rs: DesktopSettingsLinger hides the main window on "
        "CloseRequested and exits after 10 * 60 seconds unless it was shown again."
    ),
    "feat(settings): add a restart-server button under the UI backend setting": (
        "restartInputMethod in packages/ui/src/index.tsx; windows_restart_payload in "
        "apps/desktop/src-tauri/src/lib.rs sends RestartServer, handled in "
        "platforms/windows/src/entrypoints/server_main.cpp."
    ),
    "feat(ui): DeviceResources 与 Window 支持渲染 DPI 覆写": (
        "SetDpiOverride in platforms/windows/msimeui/include/msimeui/DeviceResources.h and Window.h."
    ),
    "feat(ui-html): 设置页新增输入纠错双开关": (
        "quanpinAutocorrect toggles in packages/ui/src/index.tsx "
        "(apps/desktop/tests/settings/settings.test.tsx)."
    ),
    "feat(installer): 出厂配置加入全拼纠错双开关": (
        "autocorrect_transposition and autocorrect_neighbor in "
        "platforms/windows/installer/config.default.toml."
    ),
    "feat(server): 全拼纠错分类开关配置接线与候选标记": (
        "The two switches reach the Engine through crates/host-api/src/lib.rs "
        "(options.autocorrect_transposition/neighbor), and Windows carries each candidate's "
        "`corrected` flag through platforms/windows/src/candidate/CandidatePresentation.h to the "
        "display-only `*` marker."
    ),
    "feat(tsf): convert edit session keys for client router (#301)": (
        "client_key_event in platforms/windows/tsf/Key/KeyStateCategory.h."
    ),
    "feat(tsf): add client key router boundary (#300)": (
        "platforms/windows/tsf/Key/ClientKeyRouter.h, wrapped by AuthenticatedClientKeyRouter.h."
    ),
    "feat(server): 批量导入兼容 Rime userdb.txt 与 dict.yaml": (
        "ImportFormat::Rime in crates/client-core/src/dictionary/import.rs."
    ),
    "feat(protocol): use pinned typed messages across WebView pages": (
        "No WebView2 message protocol here: settings is the Tauri app in apps/desktop whose commands "
        "are typed Rust signatures, and the native candidate window, toolbar and tray have no pages. "
        "scripts/test-reference-ui-actions.py maps each reference message to what handles it here."
    ),
    "feat(webview): validate shared message contracts before native dispatch": (
        "Same as the typed-messages entry above: Tauri deserialises every command argument into its "
        "Rust type before the handler runs."
    ),
    "feat(quanpin): wire up typing autocorrect config and tests": (
        "Superseded in the reference by the two-switch split; see the 全拼纠错分类开关 entry."
    ),
    "feat(ui): add a WeChat-inspired skin with light/dark variants and synchronized candidate and toolbar previews": (
        "Replaced by the global themes: the built-in skins are gone and the five built-in themes of "
        "crates/client-core/src/skin/theme.rs, each one fixed palette for the candidate window, "
        "toolbar and keyboard, take their place, previewed by "
        "packages/ui/src/candidate/appearance-candidate-preview.tsx and the theme cards of packages/ui/src/index.tsx."
    ),
    "feat(composition): support phrase creation from incomplete pinyin": (
        "update_creating_word_progress in crates/engine/src/session/composition.rs accumulates "
        "canonical pinyin per pick; the Windows side "
        "keeps the picked prefix in platforms/windows/src/ipc/ReplyComposer.cpp. "
        "crates/engine/examples/phrase_creation_dictionary.rs checks the stored phrase."
    ),
    "feat: add word-to-character setting": (
        "word_to_character and word_to_character_keys in platforms/windows/installer/config.default.toml."
    ),
    "feat: add word-to-character candidate selection": (
        "platforms/windows/src/input/WordCharacterPolicy.h; "
        "platforms/windows/tests/input/word_character_policy.cpp."
    ),
    "feat(candidate): support cross-page arrow navigation": (
        "Arrow keys send MSIME_NEXT_CANDIDATE/MSIME_PREVIOUS_CANDIDATE "
        "(platforms/windows/src/input/NavigationPolicy.h); the shared runtime keeps one highlight "
        "across pages and clamps at the ends (Action::NextCandidate in crates/input-runtime/src/runtime.rs)."
    ),
    "feat(settings): show Direct2D splash with DWM rounded corners on startup Replace DWM cloak with a themed splash overlay (spinner + copy) so the window responds immediately while WebView2 loads. Align the overlay to the host frame and apply the same system rounded corners.": (
        "Not reproduced as a splash: settings is the Tauri window, which is given chrome_background "
        "as its colour before the page paints (apps/desktop/src-tauri/src/lib.rs), so there is no "
        "blank frame to cover; there is no spinner overlay."
    ),
    "feat(settings): add compact dictionary export controls": (
        "Export in the local dictionary manager: the dictionary_request export operation in "
        "apps/desktop/src/main.tsx, written to Downloads by apps/desktop/src-tauri/src/shared/export_file.rs."
    ),
    "feat(settings): add user dictionary export support": (
        "Same export path as the compact export entry above."
    ),
    "feat(settings): add voice input hotkey controls": (
        "The hotkey_* keys under voice in platforms/windows/installer/config.default.toml, edited "
        "from the voice page in packages/ui/src/index.tsx."
    ),
    "feat(voice-input): add configurable hotkeys and dynamic hook lifecycle": (
        "platforms/windows/src/voice/VoiceHotkey.cpp and VoiceHotkeyPolicy.h."
    ),
    "feat(voice): allow Esc to cancel active voice input": (
        "voice_escape_cancels in platforms/windows/src/voice/VoiceHotkeyPolicy.h, used by VoiceHotkey.cpp."
    ),
    "feat: add customizable floating toolbar components": (
        "platforms/windows/src/candidate/FloatingToolbarSettings.h (items, language, scale, font size)."
    ),
    "feat: persist and apply floating toolbar component settings": (
        "The floating_toolbar_* keys in platforms/windows/installer/config.default.toml feed "
        "FloatingToolbarSettings.h."
    ),
    "feat(theme): add per-surface themes for Direct2D input interfaces": (
        "theme_cand/theme_ftb/theme_menu/... in platforms/windows/installer/config.default.toml, "
        "resolved by surface_theme_mode (platforms/windows/src/voice/VoiceTheme.h) in "
        "platforms/windows/src/entrypoints/server_main.cpp."
    ),
    "feat(settings): add candidate appearance controls and sync preview behavior": (
        "Appearance page and live preview in packages/ui/src/candidate/appearance-candidate-preview.tsx; "
        "applied by platforms/windows/src/candidate/CandidateAppearance.h and CandidateFontSettings.h."
    ),
    "feat(settings): add configurable candidate appearance and page-size digit gating": (
        "Appearance as above. Digits select only within the current page: ServerSession::key in "
        "platforms/windows/src/ipc/ServerSession.cpp swallows a digit past the page instead of "
        "selecting."
    ),
    "feat(settings): add pure Chinese phrase import entry": (
        "Folded into the local dictionary import as the `hans` format "
        "(apps/desktop/src-tauri/src/dictionary_import.rs)."
    ),
    "feat(settings): import pure Chinese phrases with cpp-pinyin": (
        "The `hans` import asks the Engine for readings instead of cpp-pinyin; see "
        "apps/desktop/src-tauri/src/dictionary_import.rs."
    ),
    "feat(settings): add quanpin dictionary batch import UI": (
        "The import operation of the local dictionary manager, apps/desktop/src/main.tsx over "
        "crates/client-core/src/dictionary/import.rs."
    ),
    "feat(settings): support batch import of quanpin user phrases": (
        "crates/client-core/src/dictionary/import.rs (standard, windows, rime and hans formats)."
    ),
    "feat: add ranking settings and fixed-position candidate menus": (
        "[frequency_adjustment] in platforms/windows/installer/config.default.toml; pin, fix at 1-5 "
        "and clear in platforms/windows/src/candidate/CandidateMenuLayout.h."
    ),
    "feat: integrate candidate ranking, pinning, and cache refresh": (
        "PinCandidate/FixCandidatePosition/ClearCandidatePosition in "
        "crates/input-runtime/src/runtime.rs, reached from CandidateMenuLayout.h."
    ),
    "feat: preserve user dictionary changes across updates": (
        "ReplayUserDictionary in platforms/windows/installer/msime_setup.iss; the journal replay is "
        "described at crates/host-api/src/lib.rs (package upgrade)."
    ),
    "feat: persist and apply appearance theme mode for settings and small windows": (
        "theme_mode and theme_settings in platforms/windows/installer/config.default.toml; the small "
        "windows resolve it in platforms/windows/src/entrypoints/server_main.cpp."
    ),
    "feat: add Fluent light theme and dynamic theme switching for settings UI": (
        "html[data-theme] light and dark palettes in packages/ui/src/styles.css; the candidate skin is "
        "replaced by the global themes of crates/client-core/src/skin/theme.rs, where `system` follows the "
        "light/dark mode on the platform's own tokens."
    ),
    "feat: hide ftb wnd immediately when tsf ime deactiavted": (
        "platforms/windows/src/candidate/FloatingToolbarVisibilityPolicy.h: a real IME deactivation "
        "hides the toolbar, a temporary focus loss does not."
    ),
    "feat: Synchronize floating toolbar status via IPC": (
        "platforms/windows/src/input/ModeMailbox.h publishes the mode the toolbar paints in "
        "FloatingToolbarWindow.cpp."
    ),
    "feat: Implement punctuation mode switch IPC and UI sync": (
        "Same PuncSwitch path as the punctuation bullet in REVIEWED."
    ),
}

FEAT_SUBJECT = re.compile(r"^feat(\((?P<scope>.*)\))?!?:\s*(?P<text>.*)$")


def changelog_features() -> tuple[set[str], str, str] | None:
    shown = show_file(ROOT, CHANGELOG)
    if shown is None:
        return None
    text, ref, sha = shown
    return parse(text), ref, sha


def feature_commits() -> dict[str, list[str]] | None:
    """Every `feat:` subject in the history of the fixed source, with the short shas that carry it."""
    pinned = pinned_reference(ROOT)
    if pinned is None:
        return None
    checkout, _, sha = pinned
    log = subprocess.run(
        ["git", "log", "--format=%H%x09%s", sha],
        cwd=checkout,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    commits: dict[str, list[str]] = {}
    for line in log.splitlines():
        full, _, subject = line.partition("\t")
        if FEAT_SUBJECT.match(subject):
            commits.setdefault(subject, []).append(full[:8])
    return commits


def as_bullet(subject: str) -> str:
    """The subject as the changelog writes it: `feat(scope): text` becomes `**scope:** text`."""
    match = FEAT_SUBJECT.match(subject)
    assert match is not None
    scope, text = match.group("scope"), match.group("text")
    return f"**{scope}:** {text}" if scope else text


def parse(text: str) -> set[str]:
    """The bullets under every `### Features` heading, without their commit links."""
    features: set[str] = set()
    in_features = False
    for line in text.splitlines():
        if line.startswith("### "):
            in_features = line.strip() == "### Features"
            continue
        if line.startswith("## "):
            in_features = False
            continue
        if in_features and line.startswith("* "):
            # `* text ([abc1234](url))` and `* text ([#12](url)) ([abc1234](url))`
            features.add(re.sub(r"\s*\(\[.*$", "", line[2:]).strip())
    return features


def main() -> int:
    resolved = changelog_features()
    if resolved is None:
        print("skipped: no MSIME-Windows checkout beside this repository to compare against")
        print(f"  expected a git checkout at {REFERENCE} carrying {CHANGELOG}")
        return 0
    features, ref, sha = resolved

    unreviewed = sorted(feature for feature in features if feature not in REVIEWED)
    for feature in unreviewed:
        print(f"FAIL {feature}: the reference shipped this and nobody here has looked at it yet", file=sys.stderr)
    if unreviewed:
        print(
            "\nRead it against this repository, then record in REVIEWED where it landed - the "
            "file, the constant or the decision, not just that it was seen.",
            file=sys.stderr,
        )
        return 1

    commits = feature_commits()
    assert commits is not None  # show_file found the same checkout a moment ago
    unaccounted = sorted(
        subject for subject in commits if as_bullet(subject) not in REVIEWED and subject not in REVIEWED_COMMITS
    )
    for subject in unaccounted:
        print(f"FAIL {' '.join(commits[subject])} {subject}: a feat commit of the fixed source nobody here has looked at yet", file=sys.stderr)
    if unaccounted:
        print(
            "\nRead it against this repository, then record in REVIEWED_COMMITS, under its exact "
            "subject, where it landed - or that it is deliberately absent or not ported.",
            file=sys.stderr,
        )
        return 1

    gone = sorted(feature for feature in REVIEWED if feature not in features)
    stale = sorted(subject for subject in REVIEWED_COMMITS if subject not in commits)
    gaps = sorted(subject for subject, answer in REVIEWED_COMMITS.items() if answer.startswith(("Not ported", "Partly ported")) and subject in commits)
    count = sum(len(shas) for shas in commits.values())
    print(f"reference feature log: all {len(features)} feature bullets of {ref} ({sha[:8]}) reviewed")
    print(f"  and all {count} feat commits ({len(commits)} subjects) up to {sha[:8]} accounted for, {len(gaps)} of them known parity gaps:")
    for subject in gaps:
        print(f"    {' '.join(commits[subject])} {subject}")
    if gone:
        # The changelog is append-only in practice, so this means a bullet was reworded. Worth naming: a reworded entry would otherwise reappear as unreviewed and be recorded twice.
        print(f"  recorded but no longer in the changelog: {', '.join(gone)}")
    if stale:
        print(f"  recorded but not a feat subject of the fixed source: {', '.join(stale)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
