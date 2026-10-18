#!/usr/bin/env python3
"""Android 云同步写回本机：RustCloudSync.write 读的键名与 Rust 的 HostWrites、HostKeyboardFeedback 序列化出来的一致。

#8201 迁移计划 5d 起一轮回答 `pending_commit` 时，Java 把 Rust 交回的 `host_writes` 写进本机再提交。`write` 用 `optJSONObject` 这类读法，键名写错时那一项被静默跳过、照样提交，游标前进以后云端的值再也写不回这台设备，而设备冒烟（SessionStoreDeviceSmoke.cloudSyncHostWrites）CI 不跑。这里对着 Rust 源码核对键名：HostWrites 的每个字段（iOS 的 `ios_native` 除外）和按键反馈的三个 camelCase 字段都要在 `write` 里按原名读到。
"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
RUST_WRITES = ROOT / "crates/host-api/src/cloud_sync.rs"
RUST_FEEDBACK = ROOT / "crates/client-services/src/account/settings_sync.rs"
JAVA = ROOT / "platforms/android/java/app/msime/android/account/RustCloudSync.java"
# 只有 iOS 写的字段。
NOT_ANDROID = {"ios_native"}


def struct_fields(source: str, name: str, path: Path) -> tuple[list[str], str]:
    """返回结构体的字段名和它前面的属性行。"""
    match = re.search(r"((?:^[ \t]*#\[[^\n]*\]\n)*)^pub(?:\(crate\))? struct " + name + r" \{\n(.*?)^\}", source, re.M | re.S)
    if match is None:
        raise SystemExit(f"{path}: 找不到 struct {name}")
    body = match.group(2)
    if "#[serde(rename" in body:
        raise SystemExit(f"{path}: {name} 的字段有单独的 serde rename，这里认不出序列化后的键名")
    fields = re.findall(r"^\s*pub(?:\(crate\))? (\w+):", body, re.M)
    if not fields:
        raise SystemExit(f"{path}: struct {name} 没有读到字段")
    return fields, match.group(1)


def camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(part.capitalize() for part in rest)


def main() -> int:
    writes, _ = struct_fields(RUST_WRITES.read_text(encoding="utf-8"), "HostWrites", RUST_WRITES)
    feedback, attributes = struct_fields(RUST_FEEDBACK.read_text(encoding="utf-8"), "HostKeyboardFeedback", RUST_FEEDBACK)
    if 'rename_all = "camelCase"' not in attributes:
        print(f"{RUST_FEEDBACK}: HostKeyboardFeedback 不再按 camelCase 序列化，下面的键名要跟着改", file=sys.stderr)
        return 1
    java = JAVA.read_text(encoding="utf-8")
    method = re.search(r"^    static void write\(.*?^    \}\n", java, re.M | re.S)
    if method is None:
        print(f"{JAVA}: 找不到 write 方法", file=sys.stderr)
        return 1
    body = method.group(0)
    errors = []
    for field in writes:
        if field in NOT_ANDROID:
            continue
        if f'"{field}"' not in body:
            errors.append(f"{JAVA}: write 没有读 host_writes.{field}（Rust 的 HostWrites 字段）")
    for field in feedback:
        key = camel(field)
        if f'"{key}"' not in body:
            errors.append(f"{JAVA}: write 没有读 host_writes.feedback.{key}（Rust 的 HostKeyboardFeedback.{field}）")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Android cloud sync writes back every HostWrites field ({len(writes) - len(NOT_ANDROID & set(writes))}) and feedback key ({len(feedback)}) by Rust's names")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
