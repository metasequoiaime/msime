#!/usr/bin/env python3
"""Every switch the Android sheets render must write a key something still reads.

`InputFeatureToggle` is a table of "preference key, default, label". Nothing checked that the key on the left was still live: a switch whose key the shared crate renamed or stopped reading goes on rendering its default, so the user sees a feature on that this host offers no way to change, and writing the switch does nothing at all.

Three questions per key:

1. Does the table's default match `impl Default for Preferences`? The sheets render that default whenever a snapshot lacks the key, so a stale one draws the switch in the opposite state from what the keyboard does. #3834 turned 云候选 off for new installs in the shared crate and left this table saying on, and the hand-written list in InputFeatureToggleSmoke carried the same stale value, so nothing noticed.
2. Does the shared schema still declare it? A typo or a renamed field fails here.
3. Does anything outside the schema file and outside tests read it? A key that only the schema mentions is a key that parses and is then dropped on the floor.

The first runs everywhere. The third needs ripgrep and is deliberately crude - any reference counts - because the failure it is built for is a field with *zero* readers, and a stricter rule would start arguing about what counts as a read.
"""
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
TOGGLES = ROOT / "platforms/android/java/app/msime/android/settings/InputFeatureToggle.java"
SCHEMA = ROOT / "crates/client-core/src/preferences.rs"

# `NAME(Group.X, "key", default, "title", "description")`
ENTRY = re.compile(r"^\s*[A-Z_]+\(Group\.[A-Z_]+,\s*\"([a-z0-9_]+)\",\s*(true|false)\b", re.MULTILINE)

# Defaults in `impl Default for Preferences` that are a function call rather than a literal, as that function evaluates on Android. smart_punctuation_default() is `true` on every host.
ANDROID_CALL_DEFAULTS = {"smart_punctuation_default()": True}

entries = ENTRY.findall(TOGGLES.read_text())
toggle_names = {
    key: name for name, key in re.findall(
        r'^\s*([A-Z_]+)\(Group\.[A-Z_]+,\s*"([a-z0-9_]+)"',
        TOGGLES.read_text(), re.MULTILINE,
    )
}
if len(entries) < 5:
    sys.exit(f"expected the Android toggle table to have entries, parsed {len(entries)}")

schema = SCHEMA.read_text()
block = re.search(r"^impl Default for Preferences \{\n(.*?)^\}\n", schema, re.MULTILINE | re.DOTALL)
if block is None:
    sys.exit(f"no `impl Default for Preferences` block in {SCHEMA.relative_to(ROOT)}")
default_mismatches = []
for key, android_default in entries:
    found = re.search(rf"^\s*{re.escape(key)}: (.+?),\s*$", block.group(1), re.MULTILINE)
    if found is None:
        default_mismatches.append(f"{key}: not set in `impl Default for Preferences`")
        continue
    value = found.group(1)
    if value in ("true", "false"):
        shared = value == "true"
    elif value in ANDROID_CALL_DEFAULTS:
        shared = ANDROID_CALL_DEFAULTS[value]
    else:
        default_mismatches.append(f"{key}: shared default `{value}` is not a literal; add how it evaluates on Android to ANDROID_CALL_DEFAULTS")
        continue
    if shared != (android_default == "true"):
        default_mismatches.append(f"{key}: table says {android_default}, shared Preferences default is {str(shared).lower()}")
if default_mismatches:
    for line in default_mismatches:
        print(f"{TOGGLES.relative_to(ROOT)}: {line}")
    sys.exit("an Android switch renders a default the shared Preferences does not have; the sheet shows the opposite of what the keyboard does when the key is absent")

# The reader search below is ripgrep. The ubuntu runner behind the contracts workflow does not ship it and that job deliberately avoids apt-get, so a missing rg skips like any other absent toolchain instead of crashing on the first key; verify-local.sh still runs this wherever rg is installed.
if shutil.which("rg") is None:
    print(f"android preference keys: {len(entries)} toggles, defaults match; reader search skipped: ripgrep (rg) is not installed")
    sys.exit(0)

keys = [key for key, _ in entries]
failures = []
for key in keys:
    if not re.search(rf"^\s*pub {re.escape(key)}:", schema, re.MULTILINE):
        failures.append(f"{key}: no `pub {key}:` field in {SCHEMA.relative_to(ROOT)}")
        continue
    # 共享业务代码和 Android 输入运行时都可以是消费者；设置枚举表与写设置的页面不能给自己作证。
    # 运行时经枚举读取键和默认值时，同时按对应枚举成员查找。
    # 外层使用单引号，避免双引号前多出反斜杠，导致 rg 拒绝表达式并以 2 退出。
    # 下方单独检查搜索错误，不能把查询失败的空输出当成所有键都没有消费者。
    found = subprocess.run(
        [
            "rg", "-l", rf'\.{key}\b|"{key}"|InputFeatureToggle\.{toggle_names[key]}\b',
            "--glob", "!**/tests/**", "--glob", "!**/tests.rs",
            f"--glob=!{SCHEMA.relative_to(ROOT)}",
            "crates", "apps", "platforms/android/java/app/msime/android/core",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    # rg exits 0 with matches, 1 with none, and anything else means the search itself
    # failed. A failed search returns no matches, so without this the file reports every
    # key as unread - the exact "declared and read by nothing" wording below, but about
    # the broken query instead of the key.
    if found.returncode not in (0, 1):
        sys.exit(
            f"rg failed to search for {key} (exit {found.returncode}): "
            f"{found.stderr.strip()}"
        )
    if not found.stdout.strip():
        failures.append(
            f"{key}: declared in the schema and read by nothing - a retired key still on a switch"
        )

if failures:
    for line in failures:
        print(f"{TOGGLES.relative_to(ROOT)}: {line}")
    sys.exit(
        "an Android switch writes a preference key that no longer does anything; "
        "point it at the live key or drop the row"
    )

print(f"android preference keys: {len(keys)} toggles, defaults match, every key declared and read")
