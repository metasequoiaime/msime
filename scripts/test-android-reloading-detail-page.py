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
    "CloudClipboardPage.java",
    "ClipboardSearchPage.java",
    "DeveloperPage.java",
    "DevicesPage.java",
    "ExpressionPage.java",
    "HandwritingPage.java",
    "KeyboardOptionsPage.java",
    "LexiconDetailPage.java",
    "LexiconPage.java",
    "PhrasesPage.java",
    "ProfilePage.java",
    "PrivacyPage.java",
    "SkinsPage.java",
    "TypingPage.java",
    "TranslationServicePage.java",
    "VoicePage.java",
    "VocabularyReviewPage.java",
)


def main() -> int:
    errors = []
    if not BASE.exists():
        errors.append(f"{BASE}: 缺少自动重载详情页基类")
    else:
        source = BASE.read_text(encoding="utf-8")
        required = (
            "public abstract class ReloadingDetailPage extends ColumnDetailPage",
            "@Override protected final void onColumnReady(Bundle args)",
            "onContentReady();",
            "protected void onContentReady()",
            "if (shouldReloadOnContentReady()) reload();",
            "protected boolean shouldReloadOnContentReady()",
            "onInitialReloadScheduled();",
            "protected void onInitialReloadScheduled()",
            "@Override protected final void onBecameVisible()",
            "if (contentColumn() != null) onVisible();",
            "protected void onVisible()",
            "if (shouldReloadWhenVisible()) reload();",
            "protected boolean shouldReloadWhenVisible()",
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
        # 键盘页可由专用工具栏入口继承；是否 final 不改变共享生命周期合同。
        if not re.search(rf"public (?:final )?class {class_name} extends ReloadingDetailPage\b", source):
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

    devices = (HOME / "DevicesPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in devices
        or "showLoading();" not in devices
        or "@Override protected boolean shouldReloadOnContentReady()" not in devices
        or "return false;" not in devices
        or "@Override protected void onContentDestroyed()" not in devices
        or "generation++;" not in devices
        or "loadedSessionId = null;" not in devices
        or "LinearLayout current = contentColumn();" not in devices
    ):
        errors.append("DevicesPage: 首次可见时读取或旧请求失效处理未保留")

    cloud_clipboard = (HOME / "CloudClipboardPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in cloud_clipboard
        or "render();" not in cloud_clipboard
        or "@Override protected void onContentDestroyed()" not in cloud_clipboard
        or "generation++;" not in cloud_clipboard
        or "overview = null;" not in cloud_clipboard
        or "loadedApi = null;" not in cloud_clipboard
        or "busy = false;" not in cloud_clipboard
        or "signedOut = false;" not in cloud_clipboard
    ):
        errors.append("CloudClipboardPage: 首次渲染或视图绑定数据清理未保留")

    clipboard_search = (HOME / "ClipboardSearchPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in clipboard_search
        or "SearchPill search = new SearchPill(context);" not in clipboard_search
        or "renderResults();" not in clipboard_search
        or "@Override protected boolean shouldReloadOnContentReady()" not in clipboard_search
        or "return false;" not in clipboard_search
        or "@Override protected boolean shouldReloadWhenVisible()" not in clipboard_search
        or "return !busy;" not in clipboard_search
        or "@Override protected void onContentDestroyed()" not in clipboard_search
        or "results = null;" not in clipboard_search
        or "busy = false;" not in clipboard_search
    ):
        errors.append("ClipboardSearchPage: 搜索框、可见刷新或视图销毁状态未保留")

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

    lexicon_detail = (HOME / "LexiconDetailPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected boolean shouldReloadWhenVisible()" not in lexicon_detail
        or "return query.isEmpty();" not in lexicon_detail
        or "@Override protected void onContentDestroyed()" not in lexicon_detail
        or "main.removeCallbacksAndMessages(null);" not in lexicon_detail
        or "entries = null;" not in lexicon_detail
        or "if (current == null || contentColumn() == null) return;" not in lexicon_detail
    ):
        errors.append("LexiconDetailPage: 搜索期间不刷新或视图销毁时的状态清理未保留")

    voice = (HOME / "VoicePage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in voice
        or "ResourcePackService.addListener(packListener);" not in voice
        or "@Override protected void onContentDestroyed()" not in voice
        or "ResourcePackService.removeListener(packListener);" not in voice
        or "contributeRow = null;" not in voice
        or "lastState = null;" not in voice
        or "testing.clear();" not in voice
        or "if (contentColumn() == null" not in voice
    ):
        errors.append("VoicePage: 资源包监听器或视图绑定状态未迁移到生命周期钩子")

    typing = (HOME / "TypingPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in typing
        or "getArguments()" not in typing
        or "ARG_ADD_LANGUAGE" not in typing
        or "ResourcePackService.addListener(packListener);" not in typing
        or "@Override protected void onContentDestroyed()" not in typing
        or "ResourcePackService.removeListener(packListener);" not in typing
        or "lastState = null;" not in typing
        or "if (contentColumn() == null) return;" not in typing
    ):
        errors.append("TypingPage: 添加语言深链或资源包监听器生命周期未保留")

    profile = (HOME / "ProfilePage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in profile
        or "loaded = null;" not in profile
        or "render();" not in profile
        or "@Override protected boolean shouldReloadOnContentReady()" not in profile
        or "return false;" not in profile
        or "@Override protected void reload()" not in profile
        or "reload(null);" not in profile
        or "@Override protected void onContentDestroyed()" not in profile
    ):
        errors.append("ProfilePage: 首次可见时刷新或临时状态清理未保留")

    privacy = (HOME / "PrivacyPage.java").read_text(encoding="utf-8")
    if (
        "@Override protected void onContentReady()" not in privacy
        or "LinearLayout target = contentColumn();" not in privacy
        or "@Override protected boolean shouldReloadOnContentReady()" not in privacy
        or "return false;" not in privacy
        or "@Override protected void onContentDestroyed()" not in privacy
        or privacy.count("rows.clear();") < 2
    ):
        errors.append("PrivacyPage: 开关绘制、首次可见读取或旧视图引用清理未保留")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android reloading detail pages share the lifecycle base class")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
