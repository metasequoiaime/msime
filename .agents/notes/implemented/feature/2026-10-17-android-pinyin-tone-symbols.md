# Agent Note: Android 符号面板增加拼音声调

Status: implemented

## Problem

#8493 反馈 Android 符号面板缺少拼音字母和声调符号，输入拼音时只能切换到其他键盘或手动寻找字符。

## Decision

在 Android 符号面板的固定分类中增加「拼音」分类，放在现有「网络」分类之后、Engine 动态目录之前。分类包含用户提供的拼音声调字符：带一到四声的 `a/e/i/o/u`、`ê`、`ü`、带声调的 `ü`，以及 `ń`、`ň`。

字符作为独立 Unicode 文本直接上屏，不改变 Engine 目录、常用记录、分类滚动或其他平台的符号布局。

## Alternatives considered

- **只把字符追加到「中文」分类。** 这样无需新增分类，但用户无法快速找到拼音专用符号，且会继续把不同用途混在一起。
- **接入 Engine 动态符号目录。** 这些字符是稳定的内置快捷符号，不需要词库分页、目录去重或字体过滤，放进固定分类更可靠。

## Consequences

Android 符号分类多一项，底栏仍沿用现有横向滚动；固定分类数量和动态目录的索引因此同步更新。没有新增资源或依赖。

## Verification

- `ANDROID_SDK_ROOT="$HOME/Library/Android/sdk" bash platforms/android/check-host.sh`
- `git diff --check`
- `pnpm run verify-notes`
