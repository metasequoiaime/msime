#!/usr/bin/env python3
"""Android 管理界面合包能力与路由集中在共享策略。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
POLICY = HOME / "ManagementUi.java"
UI = HOME / "Ui.java"
CALLERS = {
    "AboutPage.java": ('ManagementUi.available()', 'ManagementUi.settingsPage(requireContext(), "about")'),
    "AccountFragment.java": ('ManagementUi.available()', 'ManagementUi.settingsPage(requireContext(), "account")'),
    "LexiconPage.java": (
        'ManagementUi.available()',
        'ManagementUi.settingsPage(requireContext(), "vocabulary")',
        'ManagementUi.mobilePanel(requireContext(), "cloud-dictionary")',
    ),
}


def main() -> int:
    errors = []
    if not POLICY.is_file():
        errors.append(f"{POLICY}: 缺少共享管理界面策略")
        policy = ""
    else:
        policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        'private static final String ACTIVITY = "app.msime.android.MainActivity";',
        "public static boolean available()",
        "public static Intent settingsPage(Context context, String page)",
        "public static Intent mobilePanel(Context context, String panel)",
        'putExtra("msime_settings_page", page)',
        'putExtra("msime_mobile_panel", panel)',
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")

    ui = UI.read_text(encoding="utf-8")
    if "tauriAvailable(" in ui:
        errors.append(f"{UI}: 不应保留管理界面能力判断")

    for path in HOME.glob("*.java"):
        if path == POLICY:
            continue
        source = path.read_text(encoding="utf-8")
        if '"app.msime.android.MainActivity"' in source:
            errors.append(f"{path}: 应复用 ManagementUi 中的活动类名")

    for name, snippets in CALLERS.items():
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        if "Ui.tauriAvailable()" in source:
            errors.append(f"{path}: 应直接复用 ManagementUi.available")
        for snippet in snippets:
            if snippet not in source:
                errors.append(f"{path}: 缺少 {snippet}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android management UI capability and routes use one shared policy")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
