# Agent Note: Linux 候选字号随整体大小缩放

Status: implemented

## Problem

Deepin 25.2 的 4K 显示器上，候选字号设为 32 后虽然确实生效，面板文字仍显得过小；Linux 候选列表由 IBus 或 Fcitx5 桌面面板拥有，原先共享的「整体大小」能力位对 Linux 关闭，用户没有办法在不改变桌面全局 DPI 的情况下放大候选文字（#7410）。

## Decision

Linux 声明支持 `candidate_scale_percent`，但只把它应用到宿主能控制的候选字体描述，不伪造桌面面板的卡片几何缩放。IBus 的 `custom-font` 和 Fcitx5 classicui 的 `Font` 都从同一份 `candidate_scale_percent` 计算字号，50–200% 的共享边界保留，设置页在 Linux 上显示「整体大小」并明确说明它只缩放候选字号。字号按整数像素四舍五入，Fcitx5 的磅转换继续在缩放后的像素值上进行；非法缩放值回到 100%。

这样 32px 在 200% 时可以写成 64px，满足高 DPI 面板的可读性需求，同时不修改桌面拥有的内边距、阴影、圆角或窗口定位。

## Alternatives considered

- **把共享候选字号上限从 32 提高到 64**：能直接增加选项，但会改变所有平台的偏好契约、移动端夹取规则和大量校验；Linux 的问题是面板缩放不足，已有整体缩放字段更适合承载这个需求。
- **继续不提供 Linux 整体大小，只扩大桌面 DPI**：需要用户改变全局桌面设置，会影响所有应用，也不能在输入法设置里解决反馈。
- **修改 Fcitx5 主题的所有内边距和阴影**：桌面面板和 IBus 不共享这些几何能力，且会把不同版本的 classicui 主题行为混在输入法配置中；本次只写入面板接受的字体描述。

## Consequences

- Linux 设置页为 4K 和高 DPI 用户提供可见的字号放大控制，IBus 与 Fcitx5 的候选文字都立即跟随偏好同步。
- Linux 面板的卡片几何仍由桌面决定，整体大小在页面上明确标为字号缩放；第三方主题、面板定位和透明度行为不变。
- 共享能力契约现在把 Linux 列为可应用 `candidate_scale_percent` 的宿主，设置页预览继续按同一百分比缩放。

## Verification

`platforms/linux/tests/candidate/candidate_font_policy.cpp` 覆盖 125%、200% 和非法缩放；`crates/client-core` 能力测试覆盖 Linux 开启该控件。将运行 Linux 容器构建与 83 项 ctest、共享 UI 类型检查、`scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check`。
