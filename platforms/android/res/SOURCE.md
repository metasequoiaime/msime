# App icon sources

Everywhere else the brand mark is the standalone tile, `packages/ui/src/assets/msime.svg`: a dark rounded square whose green edge is its border, with the white stroke from `apps/desktop/app-icon.svg` on it. A launcher icon cannot carry that edge: the launcher cuts the icon to a shape of its own choosing - a circle on Pixel, a rounded square on most others - and a border drawn into the icon fights whichever shape it did not anticipate. So on Android the mask itself is the container: a field fills the icon and the white stroke sits on it at the size it has inside the tile, with nothing drawn around it. `platforms/android/scripts/generate_app_icons.py` renders every icon; nothing here is hand-edited.

`app_icon_classic` has the tile's own dark field, `#252525`. `app_icon_forest`, `app_icon_sky`, `app_icon_dusk` and `app_icon_vermilion` differ from it by exactly one value - the field colour, the theme's colour darkened (45% of it into `#1A1A1A`) so the white stroke stays legible - and stay the alternate styles the 我的 tab offers. The same theme table drives the iOS alternate icons, where it colours the tile's edge instead.

The plain bitmaps are the field and the stroke, square:

| Density | Size |
| --- | --- |
| mdpi | 48 |
| hdpi | 72 |
| xhdpi | 96 |
| xxhdpi | 144 |
| xxxhdpi | 192 |

## Adaptive icons

Each style also ships an `<adaptive-icon>` in `drawable-anydpi-v26`, because a plain bitmap gets the launcher's legacy treatment: shrunk and set on a grey plate the design never asked for. With `minSdk` 28 these are what the launcher and the 我的 tab actually draw.

The background layer is the style's field colour (`@color/app_icon_field_<style>`) and the foreground is the white stroke alone, scaled as it would be if the tile filled the 72dp the mask shows. Whatever shape the launcher cuts, the icon is that shape in the field colour with the stroke on it.

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
