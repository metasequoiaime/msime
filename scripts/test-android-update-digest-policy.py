#!/usr/bin/env python3
"""Android 更新下载与测试直接复用共享摘要策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "platforms/android/java/app/msime/android/account/UpdateApi.java"
SMOKE = ROOT / "platforms/android/tests/core/UpdateApiSmoke.java"


def main() -> int:
    api = API.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    errors = []
    if "public static String sha256Hex(" in api:
        errors.append(f"{API}: 不应保留文件摘要转发方法")
    if "static String hex(" in api:
        errors.append(f"{API}: 不应保留十六进制转发方法")
    if api.count("DigestPolicy.hex(") != 3:
        errors.append(f"{API}: 下载摘要和签名指纹应直接调用 DigestPolicy.hex")
    if api.count("DigestPolicy.sha256Hex(") != 1:
        errors.append(f"{API}: 已下载文件校验应直接调用 DigestPolicy.sha256Hex")
    if "import app.msime.android.DigestPolicy;" not in smoke:
        errors.append(f"{SMOKE}: 应直接导入 DigestPolicy")
    if "UpdateApi.sha256Hex(" in smoke:
        errors.append(f"{SMOKE}: 不应通过 UpdateApi 转发文件摘要")
    if smoke.count("DigestPolicy.sha256Hex(") != 4:
        errors.append(f"{SMOKE}: 应直接检查 DigestPolicy.sha256Hex")
    if "private static String hex(" in smoke or smoke.count("DigestPolicy.hex(") != 1:
        errors.append(f"{SMOKE}: 应直接检查 DigestPolicy.hex")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android update downloads use the shared digest policy directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
