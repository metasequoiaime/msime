# App icon sources

The launcher icon is the standalone mark, `packages/ui/src/assets/msime.svg`: a dark rounded square whose green edge is its border, with the white stroke on it - the same picture Windows, macOS, Linux, iOS and HarmonyOS show. The launcher cuts the icon to a shape of its own choosing, so the tile is scaled until its edge meets the border of what the mask shows and the background is the edge colour: on a rounded-square launcher the mask lands on the green edge and the home screen shows the PC icon's rounded square, the ground filling in only where the mask's corners are rounder than the tile's. On a circular mask (Pixel) the tile's corners are cut off and the edge shows at the four sides only. `platforms/android/scripts/generate_app_icons.py` renders every icon; nothing here is hand-edited.

`app_icon_classic` has the tile's own green edge, `#A8DF8E`. `app_icon_forest`, `app_icon_sky`, `app_icon_dusk` and `app_icon_vermilion` differ from it by exactly one value - the colour of the edge and the ground - and stay the alternate styles the 我的 tab offers. The theme table is the one the iOS alternate icons use, so each style looks the same on both.

The plain bitmaps are the tile filling the bitmap with transparent corners, as a rounded-square launcher shows it. With `minSdk` 28 no device draws them; they are what someone browsing the repository sees:

| Density | Size |
| --- | --- |
| mdpi | 48 |
| hdpi | 72 |
| xhdpi | 96 |
| xxhdpi | 144 |
| xxxhdpi | 192 |

## Adaptive icons

Each style also ships an `<adaptive-icon>` in `drawable-anydpi-v26`, because a plain bitmap gets the launcher's legacy treatment: shrunk and set on a grey plate the design never asked for. With `minSdk` 28 these are what the launcher and the 我的 tab actually draw.

The background layer is the style's edge colour (`@color/app_icon_ground_<style>`) and the foreground is the whole tile, its outer edge on the border of the 72dp the mask shows.

| Density | Foreground size |
| --- | --- |
| mdpi | 108 |
| hdpi | 162 |
| xhdpi | 216 |
| xxhdpi | 324 |
| xxxhdpi | 432 |

Both repositories are metasequoiaime's own and share its licence; no third-party distribution restriction applies to this asset.

## 候选手势兜底

`drawable-nodpi/noto_emoji_u1faf*.png` 为 Noto Emoji 四个手势及五种肤色的 128×128 图像。来源固定在 `googlefonts/noto-emoji` 提交 `e20cbc2bbec1926686be9f9bee7d1d2cfa1fea0e` 的 `2D/svg/`；该目录使用 Apache 2.0。无字体随包。

逐文件来源和输入、输出 SHA-256 在 [`../third_party/noto-emoji/sources.json`](../third_party/noto-emoji/sources.json)，完整许可、版权与转换说明在同目录，构建时也复制到 APK 的 `assets/native-notices/noto-emoji/`。图片仅在系统字体缺少手势时用于候选显示，不改原始 Unicode。

重建时按清单从固定提交下载 SVG 并核对 `source_sha256`，用清单记录的 librsvg 执行 `rsvg-convert -w 128 -h 128 -o <output> <source.svg>`，再核对 `output_sha256`。图像不放入密度缩放目录，绘制尺寸由候选字号决定。
