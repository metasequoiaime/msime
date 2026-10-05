#!/usr/bin/env python3
"""确保 Android 四季主题叠加层里的种子色与 Rust 生成的应用主题目录一致。

`packages/ui/src/theme/app-theme-catalog.json` 由 client-core 的主题测试生成（`MSIME_WRITE_THEME_CATALOG=1`），是应用主题种子色的唯一来源。Android 宿主把同一组颜色写死在 `res/values/colors.xml`（浅色）和 `res/values-night/colors.xml`（深色）的 `ms_<季节>_*` 里，由 `ThemeOverlay.MSIME.Season.*` 叠加；两边任何一边改了而另一边没跟，宿主就会和键盘、其他平台画出不同的季节色。本脚本逐项对照，并检查基础主题（秋杉）的别名、四个叠加层都引用了本季的颜色。
"""
from pathlib import Path
import json
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "packages/ui/src/theme/app-theme-catalog.json"
RES = ROOT / "platforms/android/res"
LIGHT = RES / "values/colors.xml"
DARK = RES / "values-night/colors.xml"
THEMES = RES / "values/themes.xml"

# 目录里的字段 → Android 颜色名的后缀。目录的 `card` 是详情页行底（rowBg），Android 的 `ms_<季节>_card` 是由它推导的 andCard，所以对的是 `row_bg`。
FIELDS = {
    "accent": "accent",
    "on_accent": "on_accent",
    "accent_soft": "accent_soft",
    "background": "bg",
    "card": "row_bg",
    "hair": "hair",
}
OVERLAYS = {
    "spring": "Spring",
    "summer": "Summer",
    "autumn": "Autumn",
    "winter": "Winter",
}
# 基础主题就是秋杉：未叠加季节时宿主画的颜色。
BASE_SEASON = "autumn"


def colors(path: Path) -> dict[str, str]:
    table = {}
    for node in ET.parse(path).getroot().iter("color"):
        name = node.get("name")
        if name and node.text:
            table[name] = node.text.strip()
    return table


def resolve(table: dict[str, str], name: str, fallback: dict[str, str] | None = None) -> str:
    """跟着 `@color/` 别名走到字面值；深色表里没有的名字回到浅色表，与 Android 的资源查找一致。"""
    seen = set()
    value = None
    current = name
    while True:
        if current in seen:
            raise AssertionError(f"color alias cycle at {current}")
        seen.add(current)
        if current in table:
            value = table[current]
        elif fallback is not None and current in fallback:
            value = fallback[current]
        else:
            raise AssertionError(f"missing color resource {current}")
        if value.startswith("@color/"):
            current = value[len("@color/"):]
            continue
        return value


def android_argb(contract: str) -> str:
    """目录颜色是 CSS 写法 `#RRGGBB` / `#RRGGBBAA`，Android 是 `#RRGGBB` / `#AARRGGBB`；统一成大写的 `#AARRGGBB`。"""
    match = re.fullmatch(r"#([0-9A-Fa-f]{6})([0-9A-Fa-f]{2})?", contract)
    if not match:
        raise AssertionError(f"unexpected catalog colour {contract!r}")
    rgb, alpha = match.group(1), match.group(2) or "FF"
    return f"#{alpha}{rgb}".upper()


def normalize(android: str) -> str:
    match = re.fullmatch(r"#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})", android)
    if not match:
        raise AssertionError(f"unexpected Android colour {android!r}")
    digits = match.group(1).upper()
    return "#" + (digits if len(digits) == 8 else "FF" + digits)


class AppThemeParity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads(CATALOG.read_text(encoding="utf-8"))["app_themes"]
        cls.light = colors(LIGHT)
        cls.dark = colors(DARK)

    def seasonal_entries(self):
        entries = [entry for entry in self.catalog if entry.get("season")]
        self.assertEqual(sorted(entry["season"] for entry in entries), sorted(OVERLAYS),
                         "every season needs exactly one fixed app theme")
        return entries

    def test_season_seeds_match_catalog(self):
        for entry in self.seasonal_entries():
            season = entry["season"]
            for mode, table, fallback in (("light", self.light, None), ("dark", self.dark, self.light)):
                for field, suffix in FIELDS.items():
                    name = f"ms_{season}_{suffix}"
                    with self.subTest(theme=entry["id"], mode=mode, field=field):
                        expected = android_argb(entry[mode][field])
                        actual = normalize(resolve(table, name, fallback))
                        self.assertEqual(actual, expected, f"{name} ({mode})")

    def test_base_theme_is_autumn(self):
        for mode, table, fallback in (("light", self.light, None), ("dark", self.dark, self.light)):
            for suffix in FIELDS.values():
                with self.subTest(mode=mode, field=suffix):
                    self.assertEqual(
                        normalize(resolve(table, f"ms_{suffix}", fallback)),
                        normalize(resolve(table, f"ms_{BASE_SEASON}_{suffix}", fallback)))

    def test_seasonal_theme_resolves_like_base(self):
        # 水杉四季本身没有固定季节；它在秋天的颜色就是基础主题的颜色。
        seasonal = [entry for entry in self.catalog if entry.get("seasonal")]
        autumn = next(entry for entry in self.catalog if entry.get("season") == BASE_SEASON)
        for entry in seasonal:
            for mode in ("light", "dark"):
                with self.subTest(theme=entry["id"], mode=mode):
                    self.assertEqual(entry[mode], autumn[mode])

    def test_overlays_reference_their_season(self):
        styles = {node.get("name"): node for node in ET.parse(THEMES).getroot().iter("style")}
        for season, suffix in OVERLAYS.items():
            style = styles.get(f"ThemeOverlay.MSIME.Season.{suffix}")
            self.assertIsNotNone(style, f"missing overlay for {season}")
            items = {item.get("name"): (item.text or "").strip() for item in style.iter("item")}
            for attribute, color in (("colorPrimary", "accent"), ("colorOnPrimary", "on_accent"),
                                     ("colorPrimaryContainer", "accent_soft"),
                                     ("android:colorBackground", "bg"),
                                     ("colorSurfaceContainerLowest", "row_bg"),
                                     ("colorOutlineVariant", "hair")):
                with self.subTest(season=season, attribute=attribute):
                    self.assertEqual(items.get(attribute), f"@color/ms_{season}_{color}")


if __name__ == "__main__":
    unittest.main()
