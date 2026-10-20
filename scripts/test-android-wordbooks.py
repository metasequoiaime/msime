#!/usr/bin/env python3
"""Android 两条 APK 打包路径都带背单词内置词书，发版和设备验收都要求它们在包里。

词书曾经从上线起就没进过任何安装包：生成脚本存在，却没有工作流运行它，打包脚本在没有词书时照样成功，设置页的词书列表在每个安装上都是空的。这里钉住 Android 这一侧的四处接线：两个打包脚本暂存 `assets/wordbooks` 并在 `MSIME_REQUIRE_WORDBOOKS=1` 时按锁文件核对、打包后核对每个暂存的文件都进了 APK；Bootstrap 把它们解到 `bootstrap/wordbooks`；Release Android 和 Android device 两个工作流生成词书并设 `MSIME_REQUIRE_WORDBOOKS`；关于页列出 ECDICT 的许可声明。
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android"
BOOTSTRAP = ANDROID / "java/app/msime/android/core/Bootstrap.java"
ABOUT = ANDROID / "java/app/msime/android/home/AboutPage.java"
WORKFLOWS = (
    ROOT / ".github/workflows/release-android.yml",
    ROOT / ".github/workflows/android-device.yml",
)


def live_lines(path: Path) -> list[str]:
    return [line for line in path.read_text(encoding="utf-8").splitlines() if not line.lstrip().startswith("#")]


def main() -> int:
    errors = []
    for name in ("build-apk.sh", "build-client-apk.sh"):
        path = ANDROID / name
        script = "\n".join(live_lines(path))
        for snippet in (
            'wordbooks_source=${MSIME_WORDBOOKS:-$repo_root/target/wordbooks}',
            'cp "$wordbooks_source"/*.json "$wordbooks_source/wordbooks-NOTICE.txt" "$assets/wordbooks/"',
            'if [ "${MSIME_REQUIRE_WORDBOOKS:-0}" = 1 ]; then',
            '"$repo_root/resources/wordbook.lock.json"',
            'grep -qxF "assets/wordbooks/$(basename "$staged")" <<< "$apk_entries"',
        ):
            if snippet not in script:
                errors.append(f"{path.relative_to(ROOT)}: 缺少 {snippet}")
        # 词书不是按需资源包：瘦包也必须带，暂存不能挂在 omit_on_demand 的分支里。
        staging = script.split("wordbooks_source=", 1)[-1].split("\nfi\n", 1)[0]
        if "omit_on_demand" in staging:
            errors.append(f"{path.relative_to(ROOT)}: 词书的暂存不能随 MSIME_ANDROID_OMIT_ON_DEMAND 省略")

    bootstrap = BOOTSTRAP.read_text(encoding="utf-8")
    for snippet in (
        'installWordbooks(context, new File(root, "bootstrap/wordbooks"));',
        'context.getAssets().list("wordbooks")',
    ):
        if snippet not in bootstrap:
            errors.append(f"{BOOTSTRAP.relative_to(ROOT)}: 缺少 {snippet}")

    if '"wordbooks/wordbooks-NOTICE.txt"' not in ABOUT.read_text(encoding="utf-8"):
        errors.append(f"{ABOUT.relative_to(ROOT)}: 开源许可里没有列出 wordbooks/wordbooks-NOTICE.txt")

    for workflow in WORKFLOWS:
        lines = live_lines(workflow)
        name = workflow.relative_to(ROOT)
        if not any("python3 scripts/fetch_wordbooks.py" in line for line in lines):
            errors.append(f"{name}: 没有用 scripts/fetch_wordbooks.py 生成内置词书")
        if not any("MSIME_REQUIRE_WORDBOOKS: '1'" in line for line in lines):
            errors.append(f"{name}: 没有设 MSIME_REQUIRE_WORDBOOKS，缺了内置词书的 APK 照样能打出来")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android APKs carry the bundled wordbooks and the release and device workflows require them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
