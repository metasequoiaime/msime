#!/usr/bin/env python3
"""检查 Android 自动重载详情页复用共享生命周期基类。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
BASE = HOME / "ReloadingDetailPage.java"
CONSUMERS = ("AiSettingsPage.java", "HandwritingPage.java", "SkinsPage.java")


def main() -> int:
    errors = []
    if not BASE.exists():
        errors.append(f"{BASE}: 缺少自动重载详情页基类")
    else:
        source = BASE.read_text(encoding="utf-8")
        required = (
            "public abstract class ReloadingDetailPage extends DetailPage",
            "@Nullable private LinearLayout column;",
            "@Override protected final void buildContent(LinearLayout column, Bundle args)",
            "this.column = column;",
            "@Override protected void onBecameVisible()",
            "if (column != null) reload();",
            "@Override public void onDestroyView()",
            "column = null;",
            "@Nullable protected final LinearLayout contentColumn()",
            "return column;",
            "protected abstract void reload();",
        )
        for snippet in required:
            if snippet not in source:
                errors.append(f"{BASE}: 生命周期基类缺少：{snippet}")

    duplicate = re.compile(
        r"(?:buildContent\(LinearLayout column, Bundle args\)|"
        r"onBecameVisible\(\)|onDestroyView\(\)|"
        r"(?:@Nullable\s+)?private LinearLayout column)"
    )
    for name in CONSUMERS:
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        class_name = path.stem
        if f"public final class {class_name} extends ReloadingDetailPage" not in source:
            errors.append(f"{path}: 未继承 ReloadingDetailPage")
        if duplicate.search(source):
            errors.append(f"{path}: 仍在重复维护内容列生命周期")
        if "@Override protected void reload()" not in source:
            errors.append(f"{path}: reload() 未实现基类合同")
        if "LinearLayout target = contentColumn();" not in source:
            errors.append(f"{path}: 渲染目标未通过 contentColumn() 获取")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android reloading detail pages share the lifecycle base class")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
