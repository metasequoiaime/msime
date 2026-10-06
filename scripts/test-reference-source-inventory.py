#!/usr/bin/env python3
"""Every source file the reference builds its product from has an answer here.

Three checks already cover the reference from the outside: what it can be configured to do
(`test-windows-config-keys.py`), what its windows can ask of its host (`test-reference-ui-actions.py`)
and what it has shipped (`test-reference-feature-log.py`). None of them looks at the product's own
source tree, so none can answer the only question that settles a migration: is there anything in
there that nothing here corresponds to?

This walks all 274 `.cpp`/`.h` files under the reference's `windows/` (the TSF text service),
`server/` (the process that hosts the candidate window, the toolbar, the settings app and the
dictionaries) and `ui/src` (its own Direct2D widget framework), and requires each to resolve one of
three ways:

1. A code file of the same name exists here, allowing for the naming conventions the two use. Only non-test code under `platforms/windows/`, `crates/`, `apps/desktop/` and `packages/ui/` counts, and not the other platforms' directories inside those roots: a Linux test, a macOS header or an SVG with the same stem answers nothing on Windows, and the first version of this check let six reference files through that way.
2. `ANSWERED_BY` names the file that answers it under a different name, and that file exists.
3. `DELIBERATELY_ABSENT` records why nothing here needs to answer it.

A reference file matching none of the three is the finding: a file nobody has accounted for. The two tables are checked on their own as well: an entry whose stem the same-name rule already answers, or that names no reference source at all, is stale and fails, because an entry rule 1 shadows is never read and so its path could rot unnoticed.

Names alone would be a weak check, which is why the second form points at a path that has to exist
rather than at a sentence. The reasons in the third form are the part to read sceptically - they are
where a migration hides what it did not do - so each one says what the user gets instead.

The reference checkout is optional. Without it the check reports what it would have needed and
passes, the same as every other stage that depends on something not every machine has.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

from reference_source import pinned_reference, reference_root

ROOT = pathlib.Path(__file__).resolve().parent.parent
TREES = ["windows", "server/src", "ui/src"]


REFERENCE = reference_root(ROOT)

# Reference file stem -> the path here that answers it. The path must exist, and the stem must not also be answered by rule 1, which would make the entry dead weight.
ANSWERED_BY: dict[str, str] = {
    # Cloud and translation. The request logic is shared because no part of it is Windows-specific;
    # only the worker that runs it on the input queue stayed native.
    "cloud_ime": "platforms/windows/src/candidate/CloudCandidateWorker.cpp",
    "tencent_tmt": "crates/client-core/src/credential/translation.rs",
    "cloud_translation": "crates/client-core/src/credential/translation.rs",
    "custom_translation": "crates/client-core/src/translation.rs",
    "translation_gloss": "crates/client-core/src/translation.rs",
    "ai_assistant": "apps/desktop/src-tauri/src/ai.rs",
    "api_credential_test": "apps/desktop/src-tauri/src/ai.rs",
    # Statistics. Eleven files in the reference's Server, one shared document here plus the Windows
    # recording site; the aggregation the reference does in SQLite is done over the document.
    "stats_types": "crates/client-core/src/typing_statistics.rs",
    "stats_store": "crates/client-core/src/typing_statistics.rs",
    "stats_aggregate": "crates/client-core/src/typing_statistics.rs",
    "stats_overview": "packages/ui/src/settings/typing-statistics.tsx",
    "stats_frames": "packages/ui/src/settings/typing-statistics.tsx",
    "stats_pipe": "platforms/windows/src/input/TypingStatistics.h",
    "stats_collector": "platforms/windows/src/input/TypingStatistics.h",
    "stats_passthrough": "platforms/windows/src/input/TypingStatistics.h",
    "char_classify": "crates/client-core/src/typing_statistics.rs",
    # Dictionaries.
    "dictionary_manager": "crates/client-core/src/dictionary/import.rs",
    "dictionary_validation": "crates/client-core/src/dictionary/import.rs",
    "dictionary_page": "crates/engine/src/host/dictionary.rs",
    # Windows settings are a native WinUI 3 window. The shared Tauri shell remains the panel host,
    # so its launcher is still part of the platform boundary but is no longer the settings product.
    "settings_app": "platforms/windows/settings/main.cpp",
    "settings_splash": "platforms/windows/settings/main.cpp",
    "emoji_panel_splash": "apps/desktop/src-tauri/src/panel_window.rs",
    "settings_launcher": "platforms/windows/src/system/ShellLauncher.cpp",
    "ime_config": "crates/client-core/src/preferences.rs",
    # Candidate window, floating toolbar and tray menu: native here as they are there.
    "candidate_presenter": "platforms/windows/src/candidate/CandidateWindow.cpp",
    "candidate_view_model": "platforms/windows/src/candidate/CandidatePresentation.h",
    "candidate_size_estimator": "platforms/windows/src/candidate/CandidateCardSize.h",
    "candidate_wheel_paging": "platforms/windows/src/candidate/CandidateWheel.h",
    "candidate_selection_policy": "crates/input-runtime/src/runtime.rs",
    "candidate_skin_catalog": "crates/client-core/src/skin/catalog.rs",
    "floating_toolbar_presenter": "platforms/windows/src/candidate/FloatingToolbarWindow.cpp",
    "tray_menu_presenter": "platforms/windows/src/candidate/TrayMenuWindow.cpp",
    "ime_windows": "platforms/windows/src/candidate/CandidateWindow.cpp",
    "window_hook": "platforms/windows/src/input/MaintenanceHotkey.cpp",
    "surface_theme_config": "platforms/windows/src/voice/VoiceTheme.h",
    "svg_path_geometry": "platforms/windows/msimeui/src/Controls.cpp",
    # The document side of the reference's second candidate renderer. Its markup is vendored in
    # packages/ui/src/upstream/candidate-themes/; these two are the contracts that fill it.
    "candidate_window_template": "crates/client-core/src/candidate_document.rs",
    "inline_protocol": "crates/client-core/src/candidate_document.rs",
    # Third-party skin CSS reaching a webview document is the same policy question on both sides.
    # The shared layer parses it into a constructed stylesheet, scopes it, drops declarations whose
    # resources did not resolve, and discards @import along the way.
    "skin_css_policy": "packages/ui/src/skin/skin-toolbar-css.ts",
    # Voice input.
    "voice_batch_protocol": "platforms/windows/src/voice/VoiceControlMessage.cpp",
    "voice_control_dispatch": "platforms/windows/src/voice/VoiceControllerDispatch.h",
    # The reference's `VoiceInput::` free functions (toggle, start, stop, cancel, recording state) are the methods of this session object; the keyboard hook it refreshes is `VoiceHotkeyController`.
    "voice_input_service": "platforms/windows/src/voice/VoiceInputSession.h",
    "voice_input_overlay_utils": "platforms/windows/src/voice/WaveOverlayUtils.cpp",
    # Provider defaults and request validation are shared by all native hosts; the extraction moved
    # them out of the Windows directory, so the shared header answers both reference provider files.
    "voice_providers": "shared/voice/VoiceProviders.h",
    # Monitor selection and scale/placement moved into the Windows overlay utility.
    "mvi_utils": "platforms/windows/src/voice/WaveOverlayUtils.h",
    # Sessions and the pipe. The reference's policy headers land on this repository's own
    # decomposition of the same protocol rather than one-for-one.
    "input_session": "platforms/windows/src/input/FocusedSession.h",
    "engine_input_session": "platforms/windows/src/ipc/ServerSession.cpp",
    "session_factory": "platforms/windows/src/ipc/SessionController.cpp",
    "event_listener": "platforms/windows/src/ipc/PipeListener.cpp",
    "active_client_state": "platforms/windows/src/ipc/SessionController.h",
    "candidate_ui_owner": "platforms/windows/src/candidate/CandidateMailbox.h",
    "focus_session_policy": "platforms/windows/src/input/FocusedSession.h",
    "outbound_session_state": "platforms/windows/src/ipc/ReplyComposer.h",
    "pipe_write_policy": "platforms/windows/src/ipc/PipeIo.h",
    "async_request_origin": "platforms/windows/src/ipc/PipeTicket.h",
    "ipc_protocol_limits": "platforms/windows/common/PipeMetadata.h",
    # Diagnostics.
    "candidate_diag_log": "platforms/windows/src/ipc/DiagnosticBatch.h",
    "ftb_diag_log": "platforms/windows/src/ipc/DiagnosticBatch.h",
    # Utilities that kept their job but not their name.
    "ime_paths": "platforms/windows/src/ipc/ServerResources.h",
    "ime_utils": "platforms/windows/src/entrypoints/server_main.cpp",
    "window_utils": "platforms/windows/src/candidate/WindowShadow.h",
    "single_instance": "platforms/windows/src/entrypoints/server_main.cpp",
    "chinese_converter": "platforms/windows/src/input/ChineseTextConversion.cpp",
    "base_structures": "platforms/windows/common/PipeMetadata.h",
    "defines": "platforms/windows/common/PipeMetadata.h",
    "client_fallback": "platforms/windows/tests/runtime/server_launch.cpp",
}

# Reference file stem -> why nothing here answers it. Each reason says what the user gets instead,
# because "handled differently" is the sentence that would make this check worthless.
DELIBERATELY_ABSENT: dict[str, str] = {
    # The WebView2 candidate backend. The reference can render its candidate window either with
    # Direct2D or with a WebView2 document; this repository has only the Direct2D one and no `ui_backend` preference.
    "windows_webview2": "The candidate window has one renderer here, Direct2D. See docs/windows-parity.md.",
    "ui_backend_policy": (
        "Chooses between the two renderers per surface. With one renderer there is nothing to "
        "choose, so the shared preferences have no `ui_backend` field."
    ),
    "webview_utils": "WebView2 host helpers. The webview here is Tauri's, which brings its own.",
    # Engine-owned. These call into the Engine's own tables; the Engine is vendored whole, so the
    # calling code lives in the shared runtime rather than being reimplemented per platform.
    "emoji_ime": "Emoji lookup belongs to the Engine; the runtime asks it through the shared session.",
    "kaomoji_ime": "Kaomoji lookup belongs to the Engine, as above.",
    "english_ime": "English candidates belong to the Engine, as above.",
    # Infrastructure with no counterpart because the surrounding design differs.
    "serial_task_queue": (
        "Serialises the reference settings window's background work onto one thread. The settings "
        "window here is a native WinUI 3 application whose bounded host-api calls run on its UI thread."
    ),
}


def reference_sources() -> tuple[list[str], str, str] | None:
    resolved = pinned_reference(ROOT)
    if resolved is None:
        return None
    checkout, ref, sha = resolved
    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", sha, *TREES],
        cwd=checkout,
        capture_output=True,
        text=True,
        check=True,
    )
    sources = [
        path for path in listing.stdout.split() if path.endswith((".cpp", ".h", ".hpp"))
    ]
    return sources, ref, sha


# Where a same-name file counts as an answer, and what kind of file it has to be.
NAME_ROOTS = ("platforms/windows/", "crates/", "apps/desktop/", "packages/ui/")
CODE_SUFFIXES = {".c", ".cc", ".cpp", ".h", ".hpp", ".rs", ".ts", ".tsx"}
# Directory names that disqualify a file: tests are not the implementation, and the other platforms' code under the shared roots does not answer a Windows file.
EXCLUDED_DIRS = {"tests", "test", "__tests__", "android", "ios", "linux", "macos", "harmony"}


def local_stems() -> set[str]:
    listing = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    )
    stems = set()
    for path in listing.stdout.split():
        pure = pathlib.PurePosixPath(path)
        if (
            path.startswith(NAME_ROOTS)
            and pure.suffix in CODE_SUFFIXES
            and not EXCLUDED_DIRS & set(pure.parts[:-1])
        ):
            stems.add(pure.stem.lower())
    return stems


def pascal(name: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in name.split("_"))


def main() -> int:
    resolved = reference_sources()
    if resolved is None:
        print("skipped: no MSIME-Windows checkout beside this repository to compare against")
        print(f"  expected a git checkout at {REFERENCE}")
        return 0
    sources, ref, sha = resolved
    stems = local_stems()

    by_name: list[str] = []
    renamed: list[str] = []
    absent: list[str] = []
    unaccounted: list[str] = []
    broken: list[tuple[str, str]] = []

    def same_name(stem: str) -> bool:
        return bool({stem.lower(), pascal(stem).lower(), stem.replace("_", "").lower()} & stems)

    # The tables are checked before and independently of rule 1, so an entry rule 1 would shadow is read rather than skipped.
    stale: list[str] = []
    source_stems = {pathlib.Path(path).stem for path in sources}
    for table, entries in (("ANSWERED_BY", ANSWERED_BY), ("DELIBERATELY_ABSENT", DELIBERATELY_ABSENT)):
        for stem in sorted(entries):
            if stem not in source_stems:
                stale.append(f"{table}[{stem!r}] names no reference source")
            elif same_name(stem):
                stale.append(f"{table}[{stem!r}] is already answered by a same-name file here")
    for stem in sorted(ANSWERED_BY.keys() & DELIBERATELY_ABSENT.keys()):
        stale.append(f"{stem!r} is in both ANSWERED_BY and DELIBERATELY_ABSENT")

    for path in sources:
        stem = pathlib.Path(path).stem
        if stem in ANSWERED_BY:
            answer = ROOT / ANSWERED_BY[stem]
            (renamed if answer.exists() else broken).append(
                path if answer.exists() else (path, ANSWERED_BY[stem])
            )
        elif stem in DELIBERATELY_ABSENT:
            absent.append(path)
        elif same_name(stem):
            by_name.append(path)
        else:
            unaccounted.append(path)

    for path in sorted(unaccounted):
        print(f"FAIL {path}: nothing here is recorded as answering this file", file=sys.stderr)
    for path, answer in sorted(broken):
        print(f"FAIL {path}: recorded as answered by `{answer}`, which does not exist", file=sys.stderr)
    for entry in stale:
        print(f"FAIL stale entry: {entry}", file=sys.stderr)
    if unaccounted or broken or stale:
        print(
            "\nName the file here that answers it, or record why nothing needs to - saying what "
            "the user gets instead, not that it is handled differently.",
            file=sys.stderr,
        )
        return 1

    print(
        f"reference source inventory: all {len(sources)} sources of {ref} ({sha[:8]}) accounted for"
    )
    print(
        f"  {len(by_name)} by name, {len(renamed)} under a different name, "
        f"{len(absent)} deliberately absent"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
