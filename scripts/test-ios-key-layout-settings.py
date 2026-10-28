#!/usr/bin/env python3
"""iOS「按键布局」设置：键盘页的入口只在本版本至少提供一组时显示，入口副标题和子页各组说明取自 SharedUI 里三端共用的文字（KeyLayoutStyleTests 钉住文字本身）。"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
KEYBOARD = ROOT / "platforms/ios/App/Sources/keyboard"
LAYOUT_PAGE = KEYBOARD / "KeyboardLayoutSettingsView.swift"
KEY_LAYOUT_PAGE = KEYBOARD / "KeyboardKeyLayoutSettingsView.swift"


class KeyLayoutSettingsContract(unittest.TestCase):
    def test_entry_row_is_gated_on_the_offering(self):
        source = LAYOUT_PAGE.read_text()
        gate = source.index("if TouchKeyLayoutOffering.edition.offersAnyGroup {")
        link = source.index("NavigationLink(destination: KeyboardKeyLayoutSettingsView())")
        identifier = source.index('.accessibilityIdentifier("keyLayoutSettingsLink")')
        self.assertLess(gate, link)
        # 入口和它上面的分隔线都在条件里：到入口的无障碍标识为止只剩条件块本身没闭合，紧接着闭合。
        block = source[gate:identifier]
        self.assertEqual(block.count("{") - block.count("}"), 1, "the link sits inside the offering check")
        self.assertTrue(source[identifier:].split("\n", 2)[1].strip() == "}", "the offering check closes right after the link")
        self.assertIn("DesignDivider()", block)
        self.assertEqual(source.count("KeyboardKeyLayoutSettingsView()"), 1)

    def test_entry_row_uses_the_shared_subtitle(self):
        source = LAYOUT_PAGE.read_text()
        self.assertIn('DesignNavRowLabel(title: "按键布局", subtitle: TouchKeyLayoutOffering.entrySubtitle)', source)

    def test_groups_use_the_shared_explanations(self):
        source = KEY_LAYOUT_PAGE.read_text()
        for title, footer in (("26 键", "twentySixKeyFooter"), ("14 键与 18 键", "letterGridFooter"), ("9 键", "nineKeyFooter")):
            self.assertRegex(source, re.escape(f'choiceGroup(title: "{title}", footer: TouchKeyLayoutOffering.{footer},'))
        self.assertIn('DesignToggleRow(title: "顶部数字行", subtitle: TouchKeyLayoutOffering.numberRowSubtitle,', source)


if __name__ == "__main__":
    unittest.main()
