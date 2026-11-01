# Android 符号面板返回栏插图

两张素材按 #10307 的要求，通过 EveryAPI 的 OpenAI 兼容 `images/generations` 接口独立生成，模型为 `doubao-seedream-5.0-pro`，日期为 2026-10-31。角色参考本仓库 `resources/stickers/shuishan/01.webp` 的绿色长发、暖红发梢、琥珀色眼睛、头顶嫩芽与浅色中式衣服；未向接口上传用户照片、私人输入或凭据内容。

每张原图为 2048×2048 PNG，移除与画布边缘相连的近白背景、裁切并等比缩放，最终为带透明通道的 320×256 WebP。新增素材随本仓库按 GPL-3.0-only 分发，不新增第三方代码或素材包；许可证全文见仓库根目录的 [LICENSE](../../LICENSE)。只在 Android 符号返回栏使用，不进入水杉娘贴纸清单。

| 左侧，手指向右 | 右侧，手指向左 |
| --- | --- |
| ![左侧水杉娘](../../platforms/android/assets/symbol-return/left.webp) | ![右侧水杉娘](../../platforms/android/assets/symbol-return/right.webp) |

| 文件 | 字节 | SHA-256 |
| --- | --- | --- |
| `platforms/android/assets/symbol-return/left.webp` | 19430 | `72bdff1ab0068f80142189b86f8274c648d0d9426ad4ae5e7af4e240e4f11c66` |
| `platforms/android/assets/symbol-return/right.webp` | 19366 | `474e15161c735438db3cb349ca46ab39cbbc77a682c7020870598d415206e272` |

原始生成图摘要：

- `left`：`c4e47df442ce0b290ab50ccc21ef989fc7025997714918d1c6cccc25c33df051`。
- `right`：`2b14202bc919423854eb40637496849c212c806d78ce98ea8e88b441322ed4c1`。

最终提示词如下；LEFT／RIGHT 表示图片朝向，并非界面文字。返回栏文案由原生控件绘制，不嵌入图片。

## 左侧，朝右

```text
Use case: illustration-story
Asset type: small Android keyboard return-bar mascot illustration, left end of a wide clickable bar
Primary request: Draw the friendly chibi Shuishan mascot in a new waist-up pose, pointing decisively toward the RIGHT edge of the image with one large, clearly readable index finger. Her head is slightly turned toward the right and her eyes look at the rightward fingertip. Keep the long green hair with warm reddish tips, brown-gold eyes, tiny evergreen sprout on her head, pale Chinese-style top with green trim and leaf decorations. Preserve the cute proportions and clean outlined soft shading described here. Put the face in the left half and the outstretched hand in the right half. Make the hand and pointing finger notably larger than usual so the direction remains obvious at a display size of only 52 pixels wide. Keep the entire head, sprout and finger inside the canvas with small margins. Single character, no other objects, no text, no letters, no arrow, no watermark. Flat pure white background, no ground, no shadow. The background will be removed for a UI cutout; keep a clean exterior silhouette. Her pale cream Chinese blouse has green cuffs and a dark green collar, with fine evergreen branch motifs; green hair, tiny sprout, large amber eyes, reddish hair tips, and her welcoming childlike chibi expression are defining features. Do not make a generic blue-haired or school-uniform character.
```

## 右侧，朝左

```text
Use case: illustration-story
Asset type: small Android keyboard return-bar mascot illustration, right end of a wide clickable bar
Primary request: Draw the friendly chibi Shuishan mascot in a new waist-up pose, pointing decisively toward the LEFT edge of the image with one large, clearly readable index finger. Her head is slightly turned toward the left and her eyes look at the leftward fingertip. Keep the long green hair with warm reddish tips, brown-gold eyes, tiny evergreen sprout on her head, pale Chinese-style top with green trim and leaf decorations. Preserve the cute proportions and clean outlined soft shading described here. Put the face in the right half and the outstretched hand in the left half. Make the hand and pointing finger notably larger than usual so the direction remains obvious at a display size of only 52 pixels wide. Keep the entire head, sprout and finger inside the canvas with small margins. Single character, no other objects, no text, no letters, no arrow, no watermark. Flat pure white background, no ground, no shadow. The background will be removed for a UI cutout; keep a clean exterior silhouette. Her pale cream Chinese blouse has green cuffs and a dark green collar, with fine evergreen branch motifs; green hair, tiny sprout, large amber eyes, reddish hair tips, and her welcoming childlike chibi expression are defining features. Do not make a generic blue-haired or school-uniform character.
```
