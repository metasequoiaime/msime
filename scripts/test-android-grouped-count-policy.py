#!/usr/bin/env python3
"""Android 社区、词库和统计页面复用共享的数字展示策略。"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
NUMBER = ROOT / "platforms/android/java/app/msime/android/NumberPolicy.java"
COMMUNITY = ROOT / "platforms/android/java/app/msime/android/community/CommunityRequest.java"
COLLECTIONS = ROOT / "platforms/android/java/app/msime/android/dictionary/DictionaryCollectionsStore.java"
SUMMARY = ROOT / "platforms/android/java/app/msime/android/statistics/TypingStatisticsSummary.java"
DETAIL = ROOT / "platforms/android/java/app/msime/android/home/LexiconDetailPage.java"
PAGE = ROOT / "platforms/android/java/app/msime/android/home/LexiconPage.java"
STATISTICS_PAGE = ROOT / "platforms/android/java/app/msime/android/home/StatisticsFragment.java"
DICTIONARY_SMOKE = ROOT / "platforms/android/tests/dictionary/DictionaryCollectionsStoreSmoke.java"
COMMUNITY_SMOKE = ROOT / "platforms/android/tests/community/CommunityRequestSmoke.java"
STATISTICS_SMOKE = ROOT / "platforms/android/tests/settings/TypingStatisticsSummarySmoke.java"


def main() -> int:
    number = NUMBER.read_text(encoding="utf-8")
    community = COMMUNITY.read_text(encoding="utf-8")
    collections = COLLECTIONS.read_text(encoding="utf-8")
    summary = SUMMARY.read_text(encoding="utf-8")
    pages = DETAIL.read_text(encoding="utf-8") + PAGE.read_text(encoding="utf-8")
    statistics_page = STATISTICS_PAGE.read_text(encoding="utf-8")
    dictionary_smoke = DICTIONARY_SMOKE.read_text(encoding="utf-8")
    community_smoke = COMMUNITY_SMOKE.read_text(encoding="utf-8")
    statistics_smoke = STATISTICS_SMOKE.read_text(encoding="utf-8")
    errors = []
    if "public static String groupedCount(long count)" not in number:
        errors.append(f"{NUMBER}: 缺少共享条数展示方法")
    if community.count("NumberPolicy.groupedCount(") != 1:
        errors.append(f"{COMMUNITY}: 没有调用共享条数展示方法")
    if "public static String entriesLabel(" in community:
        errors.append(f"{COMMUNITY}: 不应保留条数展示转发方法")
    if "parts.add(NumberPolicy.groupedCount(entries))" not in community:
        errors.append(f"{COMMUNITY}: 社区副标题应直接调用共享条数展示方法")
    if "NumberPolicy.grouped(BoundsPolicy.nonNegative(count)) + \" 条\"" in community:
        errors.append(f"{COMMUNITY}: 仍保留重复的条数拼接逻辑")
    if "import app.msime.android.NumberPolicy;" not in community_smoke:
        errors.append(f"{COMMUNITY_SMOKE}: 应直接导入 NumberPolicy")
    if "CommunityRequest.entriesLabel(" in community_smoke or "NumberPolicy.groupedCount(" not in community_smoke:
        errors.append(f"{COMMUNITY_SMOKE}: 应直接检查 NumberPolicy.groupedCount")
    if "public static String countLabel(" in collections:
        errors.append(f"{COLLECTIONS}: 不应保留条数展示转发方法")
    if pages.count("NumberPolicy.groupedCount(") != 6:
        errors.append("Android 词库页面没有直接调用共享条数展示方法")
    if "DictionaryCollectionsStore.countLabel(" in pages:
        errors.append("Android 词库页面仍通过存储类转发条数展示")
    if "import app.msime.android.NumberPolicy;" not in dictionary_smoke:
        errors.append(f"{DICTIONARY_SMOKE}: 应直接导入 NumberPolicy")
    if "DictionaryCollectionsStore.countLabel(" in dictionary_smoke or dictionary_smoke.count("NumberPolicy.groupedCount(") < 2:
        errors.append(f"{DICTIONARY_SMOKE}: 应直接检查 NumberPolicy.groupedCount")
    if "public static String grouped(" in summary:
        errors.append(f"{SUMMARY}: 不应保留数字展示转发方法")
    if "import app.msime.android.NumberPolicy;" not in statistics_page:
        errors.append(f"{STATISTICS_PAGE}: 应直接导入 NumberPolicy")
    if "TypingStatisticsSummary.grouped(" in statistics_page or statistics_page.count("NumberPolicy.grouped(") != 1:
        errors.append(f"{STATISTICS_PAGE}: 应直接调用 NumberPolicy.grouped")
    if "import app.msime.android.NumberPolicy;" not in statistics_smoke:
        errors.append(f"{STATISTICS_SMOKE}: 应直接导入 NumberPolicy")
    if "TypingStatisticsSummary.grouped(" in statistics_smoke or statistics_smoke.count("NumberPolicy.grouped(") != 1:
        errors.append(f"{STATISTICS_SMOKE}: 应直接检查 NumberPolicy.grouped")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android count labels share the grouped count policy directly")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
