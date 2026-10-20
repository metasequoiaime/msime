#!/usr/bin/env python3
"""Android 设备测试包的 instrumentation 入口不能超过 100 项。

Android 35 解析清单时限制 instrumentation 子元素最多 100 个，超过一个，测试包在 API 35 上就装不上（`INSTALL_PARSE_FAILED_UNEXPECTED_EXCEPTION: The number of child instrumentation elements exceeded the max allowed in manifest`），而 API 28 照装不误、`check-host.sh` 的编译也全绿，只有跑到 API 35 的整套设备验收才发现。新的设备专项要复用已有入口的显式参数（例如 `SettingsDeviceSmoke -e skin-delete true`、`-e vocabulary true`），不要另加入口。
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "platforms/android/tests/device/AndroidManifest.xml"
LIMIT = 100


def main() -> int:
    count = len(re.findall(r"<instrumentation\b", MANIFEST.read_text(encoding="utf-8")))
    if count > LIMIT:
        print(f"{MANIFEST.relative_to(ROOT)} declares {count} instrumentation entries; Android 35 refuses to install more than {LIMIT}. Reuse an existing entry with an explicit argument instead.", file=sys.stderr)
        return 1
    print(f"Android device test manifest declares {count} of at most {LIMIT} instrumentation entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
