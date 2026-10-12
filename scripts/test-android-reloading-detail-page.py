#!/usr/bin/env python3
"""检查 Android 自动重载详情页复用共享生命周期基类。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
BASE = HOME / "ReloadingDetailPage.java"
CONSUMERS = (
    "AiSettingsPage.java",
    "DeveloperPage.java",
    "ExpressionPage.java",
    "HandwritingPage.java",
    "KeyboardOptionsPage.java",
    "LexiconPage.java",
    "PhrasesPage.java",
    "SkinsPage.java",
)


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
            "onContentReady();",
            "protected void onContentReady()",
            "onInitialReloadScheduled();",
            "protected void onInitialReloadScheduled()",
            "@Override protected void onBecameVisible()",
            "if (column != null) reload();",
            "@Override public void onDestroyView()",
            "column = null;",
            "onContentDestroyed();",
            "protected void onContentDestroyed()",
            "@Nullable protected final LinearLayout contentColumn()",
            "return column;",
            "protected abstract void reload();",
        )
        for snippet in required:
            if snippet not in source:
                errors.append(f"{BASE}: 生命周期基类缺少：{snippet}")
        ready = source.find("onContentReady();")
        refresh = source.find("reload();")
        if ready >= 0 and refresh >= 0 and ready > refresh:
            errors.append(f"{BASE}: 已缓存内容必须先于首次刷新显示")
        first_refresh = source.find("reload();", ready)
        discover = source.find("onInitialReloadScheduled();", first_refresh)
        if first_refresh < 0 or discover < first_refresh:
            errors.append(f"{BASE}: 首次刷新必须先于额外加载任务")

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

    phrases = (HOME / "PhrasesPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in phrases
        or "if (document != null) render(document);" not in phrases
    ):
        errors.append("PhrasesPage: 重建页面时未先显示已缓存的常用语")

    expression = (HOME / "ExpressionPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in expression
        or "if (loaded) render();" not in expression
        or "@Override protected void onInitialReloadScheduled()" not in expression
        or "loadDiscover();" not in expression
        or "@Override protected void onContentDestroyed()" not in expression
        or "installing.clear();" not in expression
    ):
        errors.append("ExpressionPage: 首次加载或视图销毁时的专有状态未保留")

    developer = (HOME / "DeveloperPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in developer
        or "render();" not in developer
        or "@Override protected void onContentDestroyed()" not in developer
    ):
        errors.append("DeveloperPage: 首次渲染或视图销毁时的专有处理未保留")

    lexicon = (HOME / "LexiconPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in lexicon
        or "if (model != null) render();" not in lexicon
        or "@Override protected void onInitialReloadScheduled()" not in lexicon
        or "loadDiscover();" not in lexicon
        or "@Override protected void onContentDestroyed()" not in lexicon
        or "installing.clear();" not in lexicon
        or "@Override protected void reload()" not in lexicon
        or "reload(false);" not in lexicon
        or "reload(true)" not in lexicon
    ):
        errors.append("LexiconPage: 缓存显示、发现列表或手动刷新行为未保留")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android reloading detail pages share the lifecycle base class")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
