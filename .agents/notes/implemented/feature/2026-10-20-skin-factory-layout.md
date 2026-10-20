# Agent Note: 共享皮肤布局继承 Windows 出厂清单

Status: implemented

## Problem

共享扫描器保留皮肤声明的布局，却未补齐 `msime-windows` 基底的出厂几何。`canvas` 未声明内边距与高亮圆角时，Linux 画成 12 DIP 和直角，来源为 5 DIP 和 4 DIP；`microsoft` 与 `willow_green` 的排列默认也丢失。直接在 `SkinSummary.geometry` 补值会污染编辑器的“未声明”语义，且简单合并两张排列表会让基底的排列默认压过皮肤的公共声明。

## Decision

固定 `msime-windows` 默认分支 `develop@a60e2948dbba29f32bec58ee4bf44d68ce953a0b`，内嵌八份出厂清单，由共享解析器生成可选的 `resolvedLayout`。只对 Windows 外观基底与 `canvas` 提供，保留其余主题的宿主默认。每种排列依次叠加出厂公共、出厂排列、皮肤公共、皮肤排列；em 留到宿主按实际字号换算，显式零值与 `false` 照常覆盖。原始字段继续服务编辑器，绘制路径选用解析结果。Windows 与 Linux 已有布局消费者接入该结果，用户配置保持现有优先级。

## 历史审计

- [清单对齐](../../implemented/feature/2026-10-19-windows-skin-schema-parity.md)：部分重叠，继续持有字段校验和基底识别，本篇补出厂继承。
- [Windows 布局字段](../../implemented/feature/2026-10-19-windows-candidate-skin-layout-keys.md)：部分重叠，继续持有实际绘制与用户偏好优先级。
- [Linux 布局](../../implemented/feature/2026-10-20-linux-canvas-layout.md)：部分重叠，继续持有测量、绘制与命中，本篇替换未声明字段的缺省来源。
- [Linux 外观](../../implemented/feature/2026-10-20-linux-canvas-appearance.md)：部分重叠，继续持有高亮、竖条和细分颜色。

## Alternatives considered

- 各宿主各补一张默认表：接入工作小，也不用扩展共享 JSON；但外观与排列默认在六个宿主间继续漂移，因此共用固定清单。
- 直接填满原始字段：旧宿主无需改动就能继承；但编辑器会把基底默认误作皮肤声明，且无法表达四层覆盖顺序，因此单独提供绘制结果。
- 只提取几何片段：更省字节，但升级时需要人工重制清单；保留来源原文件使固定提交逐字核对成为可能。暂不使用其中的颜色和工具栏字段，避免改变现有颜色回退契约。

## Consequences

一份固定出厂清单统一基底默认和覆盖顺序，后续宿主可直接接入；代价是增加静态清单体积与 JSON 布局输出，升级来源时要核对八份原文件。增加的 JSON 字段需要绘制宿主主动接入。移动平台、macOS、网页和 Linux Wayland/IBus 的完整布局消费仍需后续验收，不能以本次共享输出和桌面两条路径宣称全平台视觉对齐。Linux 列表内翻页已进入共同基线，其 `pageArrows`、`expandPanel` 与 `fitContent` 同样读取共享解析结果，避免两套缺省来源；旧 catalog 仍从原字段读取。长文本组合行为仍需后续验收。

共享测试覆盖八种基底、横竖排默认、四层覆盖、em 延迟换算、显式零值与关闭开关、原始声明不变和普通主题不继承。Windows 布局测试覆盖新输出与旧 catalog；Linux 原生测试检查合并结果进入测量，GTK 六组横竖排、2 倍缩放与合成器验收检查出厂圆角和显式零值热切换。验证结果随本次提交记录，不把编译或单测当作 Windows 真机目测。
