#!/usr/bin/env python3
"""检查真实 Windows 设置验收的页面清单与原生导航一致。"""

from pathlib import Path
import re


ROOT = Path(__file__).resolve().parent.parent
navigation = (ROOT / "platforms/windows/settings/SettingsNavigation.h").read_text(encoding="utf-8")
smoke = (ROOT / "platforms/windows/tests/tools/settings_ui.ps1").read_text(encoding="utf-8-sig")
pages = re.search(r"\bpages\{\{(.*?)\}\};", navigation, re.S)
assert pages, "无法读取原生设置页面清单"
expected = re.findall(r'\{"([a-z]+)",\s*\d+,\s*PageHost::Native', pages[1])
visited = re.search(r"^\$pages\s*=\s*@\((.*?)\)", smoke, re.M | re.S)
assert visited, "无法读取 Windows 设置验收的页面清单"
actual = re.findall(r"'([a-z]+)'", visited[1])
assert expected, "原生设置页面清单为空"
assert actual == expected, f"Windows 设置验收页面与导航不一致：{actual} != {expected}"
print("windows settings UI: smoke visits every native page in navigation order")
