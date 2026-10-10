#!/usr/bin/env python3
"""社区目录直接复用共享 JSON 布尔与文本策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/android/java/app/msime/android/community/CommunityCatalog.java"
SMOKE = ROOT / "platforms/android/tests/community/CommunityCatalogSmoke.java"


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    smoke = SMOKE.read_text(encoding="utf-8")
    if "strictBoolean(Object value)" in source:
        print(f"{SOURCE}: 不应保留 strictBoolean 转发方法", file=sys.stderr)
        return 1
    if "import app.msime.android.JsonPolicy;" not in smoke:
        print(f"{SMOKE}: 应直接导入 JsonPolicy", file=sys.stderr)
        return 1
    if 'CommunityCatalog.class.getDeclaredMethod("strictBoolean"' in smoke:
        print(f"{SMOKE}: 不应反射检查已删除的转发方法", file=sys.stderr)
        return 1
    if "JsonPolicy.strictBoolean" not in smoke:
        print(f"{SMOKE}: 缺少 JsonPolicy.strictBoolean 合同检查", file=sys.stderr)
        return 1
    if "static String idKey(" in source:
        print(f"{SOURCE}: 不应保留 idKey 文本策略转发方法", file=sys.stderr)
        return 1
    if "static boolean pageHasMore(" in source or "static boolean confirmedReport(Object" in source:
        print(f"{SOURCE}: 不应保留社区布尔策略转发方法", file=sys.stderr)
        return 1
    if 'TextPolicy.lowercase(item.id())' not in source:
        print(f"{SOURCE}: 重复 ID 应直接调用 TextPolicy.lowercase", file=sys.stderr)
        return 1
    if 'JsonPolicy.strictTrue(root.opt("has_more"))' not in source \
            or 'JsonPolicy.strictTrue(response.opt("reported"))' not in source:
        print(f"{SOURCE}: 分页与举报确认应直接调用 JsonPolicy.strictTrue", file=sys.stderr)
        return 1
    for forwarded in ("idKey", "pageHasMore", "confirmedReport"):
        if f'CommunityCatalog.class.getDeclaredMethod("{forwarded}"' in smoke:
            print(f"{SMOKE}: 不应反射检查已删除的 {forwarded} 转发方法", file=sys.stderr)
            return 1
    if "import app.msime.android.TextPolicy;" not in smoke or smoke.count("TextPolicy.lowercase(") < 4:
        print(f"{SMOKE}: 应直接检查 TextPolicy.lowercase", file=sys.stderr)
        return 1
    if smoke.count("JsonPolicy.strictTrue(") < 5:
        print(f"{SMOKE}: 应直接检查 JsonPolicy.strictTrue", file=sys.stderr)
        return 1
    print("Android community catalogue uses the shared boolean and text policies")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
