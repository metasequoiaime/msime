#!/usr/bin/env python3
"""设置页导航图标按约定画成浅色，不用 currentColor。

侧栏和手机「全部设置」页把这些 SVG 当图片显示（`<img>`），图片里的 currentColor 取不到页面的文字颜色，一律按黑色画：暗色主题下就是一个黑图标。约定是 SVG 自己画浅色（#C7D5EB / #CCDAF5），亮色主题下由 `light-theme:[filter:invert(1)_brightness(0.25)]` 反转成深色。用遮罩显示的图标（手机标签栏、反馈渠道的单色标）不在这份清单里，不受这条约束。
"""
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "packages/ui/src/settings/settings-page-registry.ts"


def main() -> int:
    source = REGISTRY.read_text(encoding="utf-8")
    icons = sorted(set(re.findall(r'new URL\("\.\./assets/([^"]+\.svg)"', source)))
    if not icons:
        print(f"{REGISTRY}: 没有找到任何页面图标", file=sys.stderr)
        return 1
    failures = [
        name
        for name in icons
        if "currentColor" in (REGISTRY.parent.parent / "assets" / name).read_text(encoding="utf-8")
    ]
    for name in failures:
        print(f"packages/ui/src/assets/{name}: 设置页图标用了 currentColor，作为图片显示时会是黑色", file=sys.stderr)
    if failures:
        return 1
    print(f"settings navigation icons: {len(icons)} icons draw their own light colour")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
