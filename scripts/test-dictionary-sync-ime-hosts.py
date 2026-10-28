#!/usr/bin/env python3
"""「我的词条」网盘同步在输入法进程一侧的接线：macOS 的 IMK 进程、Linux 的 IBus 和 Fcitx5 宿主各起一次后台同步（msime_client_dictionary_sync_start），退出时停下（msime_client_dictionary_sync_stop）。

这里守着几件编译和单测碰不到的事：

- 三个入口各有一处起、一处停，状态目录都是与设置应用共用的那个目录下的 `dictionary-sync`。设置应用和输入法给的目录不同，同一台电脑就会在网盘上登记成两台设备。
- macOS 的独立设置窗口（`--preferences`）不起：它在 `MSIMEShouldShowPreferences` 的分支里先返回，起同步的调用必须在那之后。
- 输入法进程不直接调一次性的 `msime_client_dictionary_sync`：那个入口不带 `create_identity: false` 时会在本机还没有身份时生成一个新的设备身份。后台同步在 Rust 里固定只用已有身份。
- Fcitx5 插件停下时一直等后台线程退出（`UINT32_MAX`）：插件是 Fcitx5 加载的共享库，被卸载之后插件的代码不能再运行。

设置应用一侧的目录名（`apps/desktop/src-tauri/src/platform/dictionary_sync.rs` 的 `DIRECTORY`）在那份文件存在时一并核对。
"""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
START = "msime_client_dictionary_sync_start("
STOP = "msime_client_dictionary_sync_stop("
ONE_SHOT = re.compile(r"\bmsime_client_dictionary_sync\s*\(")
ENTRY_POINTS = {
    "platforms/macos/src/input/input_method_main.mm": "macOS IMK",
    "platforms/linux/src/entrypoints/ibus_main.cpp": "IBus",
    "platforms/linux/fcitx5/FcitxEngine.cpp": "Fcitx5",
}
# 输入法进程的源码：这些目录里谁都不该直接调一次性的入口。
IME_SOURCES = ("platforms/macos/src/input", "platforms/linux/src", "platforms/linux/fcitx5")
SOURCE_SUFFIXES = {".c", ".cc", ".cpp", ".h", ".hpp", ".m", ".mm", ".swift"}
SETTINGS = "apps/desktop/src-tauri/src/platform/dictionary_sync.rs"


def main() -> int:
    failures = []
    for path, host in ENTRY_POINTS.items():
        text = (ROOT / path).read_text(encoding="utf-8")
        for call in (START, STOP):
            count = text.count(call)
            if count != 1:
                failures.append(f"{path}: the {host} host should call {call[:-1]} once, found {count}")
        if '"dictionary-sync"' not in text:
            failures.append(f"{path}: the {host} host does not put the state in dictionary-sync")
        if "create_identity" in text:
            failures.append(f"{path}: the {host} host must not choose create_identity; the background sync never creates one")
    main_source = (ROOT / "platforms/macos/src/input/input_method_main.mm").read_text(encoding="utf-8")
    standalone = main_source.find("if (MSIMEShouldShowPreferences(argc, argv))")
    started = main_source.find("MSIMEStartDictionarySync();")
    if standalone < 0 or started < 0 or started < standalone:
        failures.append("input_method_main.mm: dictionary sync must start after the standalone preferences window has returned")
    fcitx = (ROOT / "platforms/linux/fcitx5/FcitxEngine.cpp").read_text(encoding="utf-8")
    if f"{STOP}UINT32_MAX)" not in fcitx:
        failures.append("FcitxEngine.cpp: the addon must wait for the background thread to exit before it is unloaded")
    for top in IME_SOURCES:
        for path in sorted((ROOT / top).rglob("*")):
            if path.suffix not in SOURCE_SUFFIXES or not path.is_file():
                continue
            if ONE_SHOT.search(path.read_text(encoding="utf-8", errors="replace")):
                failures.append(f"{path.relative_to(ROOT).as_posix()}: an input method process calls msime_client_dictionary_sync directly")
    settings = ROOT / SETTINGS
    if settings.exists() and 'const DIRECTORY: &str = "dictionary-sync";' not in settings.read_text(encoding="utf-8"):
        failures.append(f"{SETTINGS}: the settings app no longer uses dictionary-sync, the directory the input method hosts share with it")
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print("the macOS, IBus and Fcitx5 hosts start and stop the background dictionary sync once, in the directory they share with the settings app")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
