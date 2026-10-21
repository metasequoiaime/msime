#!/usr/bin/env python3
"""检查 IBus 组件文件里的引擎描述，不需要运行中的 IBus。"""

from pathlib import Path
import xml.etree.ElementTree as ElementTree

root = Path(__file__).resolve().parents[2]
component = ElementTree.parse(root / "data/msime-linux.xml.in").getroot()
engines = component.findall("engines/engine")
assert engines, "组件文件里没有引擎"
for engine in engines:
    name = engine.findtext("name")
    # 引擎写具体布局（例如 us）时，IBus 面板切到它会把键盘换成那个布局，GNOME Shell 也一样，Dvorak、Colemak 用户就只能用 QWERTY（#6365）。default 让当前布局保持不变。
    assert engine.findtext("layout") == "default", f"{name} 的 <layout> 必须是 default，实际是 {engine.findtext('layout')!r}"
    for field in ("layout_variant", "layout_option"):
        assert engine.find(field) is None, f"{name} 不应写 <{field}>，它同样会改写用户的键盘布局"
    # 图标名必须是装上的应用图标（CMakeLists.txt 按 msime-linux 安装），名字不存在时 ibus-setup 和输入源切换器只能画一个缺图标的占位。
    assert engine.findtext("icon") == "msime-linux", f"{name} 的 <icon> 应为 msime-linux，实际是 {engine.findtext('icon')!r}"
print(f"IBus component contract passed ({len(engines)} engine)")
