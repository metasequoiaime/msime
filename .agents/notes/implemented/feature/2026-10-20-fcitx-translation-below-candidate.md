# Agent Note: Fcitx5 普通候选栏把译文放到正文下方

Status: implemented

## Problem

#8785 希望译文显示在候选下方。普通 classicui 路径把候选与译文用两个空格拼在一起，窗口宽度因此叠加；原生 canvas 的分段布局不能覆盖普通皮肤和桌面面板。

## Decision

普通候选文本在非空译文前加入换行，复用 Fcitx5 的 `Text::splitByLine`、classicui 多行测量与命中区域。候选身份、上屏文字、原生 canvas 的独立译文及其分段偏移保持原值；空译文不增加空白行，插件注释仍在整项末尾。

## Alternatives considered

单独改原生 canvas 可以直接控制字体与宽度，但只有指定 canvas 皮肤的 X11 会话使用它，无法修复普通 classicui。Fcitx5 从支持下限附近的 5.0.21 起已有候选多行绘制，不需要接管新的桌面窗口。

## Verification

远端运行 `20261020-184331` 的四项验收通过：原生候选文本回归检查正文与译文分行、空译文不留白和注释／训音顺序；真实 GTK 编辑器上屏通过，横排和竖排 classicui 窗口均检查新增行高、正文首行位置、正文下方的真实译文字形，以及点击新增译文区域只上屏「你好」。测试使用普通 `system` 基底，确认没有原生 canvas 窗口；配色使用共享取色器的显式合成值，不把 `system` 基底忽略包配色的行为误当作像素缺失。

完整 `fcitx5-native-context` 在「Disabled expression mode must pass through」断言失败；用固定基线 `ba47858d31` 的原实现重编后，同一断言也失败，日志保存在远端 `target/linux/baseline-native-context.log`。恢复当前源码重编后，上述四项再次通过，此次不修改无关表达式逻辑。

提交前 quick `20261020-185733` 通过：Rust workspace、Android 目标和 Java、WebAssembly、HarmonyOS 设置页构建、Linux 设置壳与原生 93 项、macOS 宿主及 Apple bridge 构建通过。Windows 原生／MinGW、pipe 与 HarmonyOS HAP 因工具链或暂存前置缺失明确跳过。前一轮门禁的 Apple bridge 失败来自复制缓存里的旧绝对路径；删除本分支该缓存并在规定目录重新配置后通过。

## Consequences

多行候选会增加候选栏高度。换行减少正文与译文相加的宽度，但不额外规定译文的固定宽度或截断长译文；其他桌面面板对换行的呈现由它们自身决定。

## 历史笔记检索

检索活跃笔记中的 `8785`、Fcitx 翻译、释义与换行。[Linux canvas 布局](../../implemented/feature/2026-10-20-linux-canvas-layout.md)和[释义分段](../../implemented/feature/2026-10-20-candidate-translation-runs.md)仅部分重叠，继续持有原生画布和共享分段的规则；本次只改变 classicui 文本组合，不替代它们。
