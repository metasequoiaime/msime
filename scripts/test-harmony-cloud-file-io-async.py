#!/usr/bin/env python3
"""Large Harmony cloud exports and restores must use asynchronous file I/O."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SETTINGS = (ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets").read_text(encoding="utf-8")
CLOUD = (ROOT / "platforms/harmony/entry/src/main/ets/account/HarmonyAccountCloudBridge.ets").read_text(encoding="utf-8")

if "fs.writeSync(file.fd, request.contents)" in SETTINGS:
    raise SystemExit("dictionary export staging must not synchronously write the whole export")
if "fs.copyFileSync(staging, destination)" in SETTINGS:
    raise SystemExit("dictionary export delivery must not synchronously copy a large file")
# 云词库导出和整份快照（最多 512 MiB）由 Rust 写进私有目录，ArkTS 只在私有目录和选择器的 URI 之间复制，这两步也不能同步做。
if "fs.copyFileSync(staging, destination)" in CLOUD:
    raise SystemExit("cloud dictionary export must not synchronously copy a large file")
if "fs.copyFileSync(sourceFile.fd, destination)" in CLOUD or "fs.copyFileSync(source, destination)" in CLOUD:
    raise SystemExit("cloud snapshot restore must not synchronously copy a large file")
for source, needle in ((SETTINGS, "await fs.write(file.fd, request.contents)"),
                       (SETTINGS, "await fs.copyFile(staging, destination)"),
                       (CLOUD, "await fs.copyFile(staging, destination)"),
                       (CLOUD, "await fs.copyFile(sourceFile.fd, destination)")):
    if needle not in source:
        raise SystemExit(f"missing asynchronous file operation: {needle}")

print("harmony cloud export and restore file I/O is asynchronous")
