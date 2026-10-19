# App icon sources

All five launcher icons are derived from `apps/desktop/app-icon.svg`, the vector the Windows frontend ships as `windows/image/msime.ico` (metasequoiaime/MSIME-Windows commit `253b0689`). `platforms/android/scripts/generate_app_icons.py` renders them; nothing here is hand-edited.

`app_icon_classic` is the artwork's own light green frame. `app_icon_forest`, `app_icon_sky`, `app_icon_dusk` and `app_icon_vermilion` differ from it by exactly one value - the colour of that frame - and stay the alternate styles the 我的 tab offers. The same table drives the iOS alternate icons, so the two platforms show one icon in five washes.

The artwork was drawn for the Windows tray, where it floats on whatever is behind it and so supplies its own border, painted as 164 scattered brush stamps. A launcher icon is the opposite case on both counts, so the script departs from the master twice:

- It insets the artwork to 74% of the square, on the mark's own dark field. The container's mask cuts through anything near the edge, and the master's frame comes within 0.32 of the 110-unit canvas edge; shipping it at its native extent loses the top and bottom of the frame.
- It replaces the brush stamps with one stroke on the same path, at the weight the brush band carried. At icon sizes the frame is about four pixels wide, where the ragged alpha and the gaps between stamps read as a smeared edge rather than as texture.

The dark panel and the white mark are taken from the master unchanged.

| Density | Size |
| --- | --- |
| mdpi | 48 |
| hdpi | 72 |
| xhdpi | 96 |
| xxhdpi | 144 |
| xxxhdpi | 192 |

## Adaptive icons

Each style also ships an `<adaptive-icon>` in `drawable-anydpi-v26`, because a plain bitmap gets the launcher's legacy treatment: shrunk and set on a grey plate the design never asked for. The background layer is `@color/app_icon_field`, the mark's own dark field, so what the mask cuts is that field rather than a plate.

The foreground is rendered by the same script, from the same artwork, with two differences: it carries no field of its own — the background layer is that field — and it sits at 47% of the canvas rather than 74%. A circular mask shows only the middle 72 of the 108dp canvas, and a square fits inside that disc up to 72/√2 ≈ 51. The 66dp safe zone is sized for artwork that tolerates having its corners cut; this frame is the part that must not be.

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
