#!/usr/bin/env python3
"""鸿蒙一台手机在云剪贴板里只登记成一台设备。

设置应用和键盘扩展各有各的 `files/state`。两边各自让驱动生成设备身份，同一台手机就会在账号、自建中继或 WebDAV 网盘的空间里登记成两台，另一台设备上要批准两次。规则是：只有设置应用生成身份，键盘的每个云剪贴板请求都带 `create_identity: false`，本机没有身份时 Rust 答 `clipboard_identity_missing`、不登记；设置应用的密钥串经共享沙箱镜像给键盘（`SharedStateMirrorPolicy.CLOUD_CLIPBOARD_IDENTITY_ENTRIES`，由 `shared-state-mirror-policy.test.ts` 测），设置应用那边还没有时不删键盘的副本。

请求怎么拼由 `platforms/harmony/tests/account-requests.test.ts` 测，Rust 那一侧由 `clipboard_webdav_tests.rs` 的 `a_process_that_borrows_the_identity_never_registers_a_device_of_its_own` 和 host-api 的 FFI 测试测。把它们接到调用点的是 `.ets` 文件，node 下编不了，改回去测试也看不见；这个检查盯住调用点。文本检查不是编译器：它按调用找到括号里的参数，只看最后一个参数。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ETS = ROOT / "platforms/harmony/entry/src/main/ets"
KEYBOARD_CLIENT = "keyboard/clipboard/CloudClipboardClient.ets"
SETTINGS_BRIDGE = "account/HarmonyAccountCloudBridge.ets"
BUILDERS = ("cloudClipboardRequest", "relayCloudClipboardRequest")
NATIVE_CALLS = ("client.accountCloudClipboard(", "client.relayCloudClipboard(")


def call_arguments(text: str, name: str) -> list[str]:
    """每个 `name(` 调用的最后一个参数，跳过 import 列表里的同名标识符。"""
    found: list[str] = []
    for match in re.finditer(rf"\b{name}\(", text):
        depth = 0
        start = match.end()
        position = start
        while position < len(text):
            character = text[position]
            if character in "([{":
                depth += 1
            elif character in ")]}":
                if depth == 0:
                    break
                depth -= 1
            position += 1
        arguments = text[start:position]
        found.append(arguments.rsplit(",", 1)[-1].strip())
    return found


def main() -> int:
    if not ETS.is_dir():
        print("harmony cloud clipboard identity: no HarmonyOS sources, skipped")
        return 0
    problems: list[str] = []

    keyboard = (ETS / KEYBOARD_CLIENT).read_text(encoding="utf-8")
    for name in BUILDERS:
        last = call_arguments(keyboard, name)
        if not last:
            problems.append(f"{KEYBOARD_CLIENT}: no {name}( call")
        for argument in last:
            if argument != "false":
                problems.append(f"{KEYBOARD_CLIENT}: {name}( must end with createIdentity false, got `{argument}`")

    settings = (ETS / SETTINGS_BRIDGE).read_text(encoding="utf-8")
    for name in BUILDERS:
        last = call_arguments(settings, name)
        if not last or any(argument != "true" for argument in last):
            problems.append(f"{SETTINGS_BRIDGE}: the settings app is the one that creates the identity ({name})")

    # 键盘进程里的云剪贴板请求只经 CloudClipboardClient 发出。
    for path in sorted(ETS.rglob("*.ets")):
        relative = path.relative_to(ETS).as_posix()
        if relative in (KEYBOARD_CLIENT, SETTINGS_BRIDGE):
            continue
        text = path.read_text(encoding="utf-8")
        for call in NATIVE_CALLS:
            if call in text:
                problems.append(f"{relative}: `{call}` outside {KEYBOARD_CLIENT} or {SETTINGS_BRIDGE}")

    sync = (ETS / "keyboard/SharedStateSync.ets").read_text(encoding="utf-8")
    start = sync.find("  static keyboardSync(")
    body = sync[start : sync.find("\n  }\n", start)] if start >= 0 else ""
    keep = body.find("SharedStateMirrorPolicy.keepsLocalWhenAbsent(name)")
    fallthrough = body.find("return SharedStateSync.accountFileDecision(name);")
    if keep < 0 or fallthrough < 0 or keep > fallthrough:
        problems.append("keyboard/SharedStateSync.ets: keyboardSync must keep the keyboard's identity while settings has none")

    if problems:
        for problem in problems:
            print(f"harmony cloud clipboard identity: {problem}")
        return 1
    print("harmony cloud clipboard identity: the keyboard only borrows the settings app's device identity")
    return 0


if __name__ == "__main__":
    sys.exit(main())
