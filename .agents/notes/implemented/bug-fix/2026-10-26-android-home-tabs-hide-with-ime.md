# Agent Note: Android 页面输入法弹出时收起主页导航

Status: implemented

## Problem

主页的底部导航是 `HomeActivity` 中与 `home_content` 重叠的兄弟视图。搜索框或多行文本框获得焦点后，系统输入法通过 `WindowInsets` 把可用窗口压缩，但导航仍可见，因而出现在键盘上方，遮住输入页面底部（#9775、#9780–#9783）。

## Decision

`HomeActivity` 在根视图的 insets 回调中读取 `WindowInsetsCompat.Type.ime()` 的可见状态，并把它交给 `WindowInsetsPolicy.shouldShowHomeTabs`。只要输入法可见就收起主页底部导航；输入法收起后恢复。AI 对话二级页仍按既有规则隐藏导航，因此策略同时保留页面状态和输入法状态两个条件。

## Alternatives considered

- **只给每个搜索页增加底部内边距**：只能覆盖当前页面，遗漏新页面和嵌入式社区列表；导航仍会占用输入页面的可见区域。
- **把导航额外向上平移到输入法顶部**：会在 `adjustResize` 已经缩短窗口的设备上重复避让，导航仍可能遮住内容，且每个页面的 inset 计算会分叉。

## Consequences

所有主页详情页和嵌入式社区搜索页在输入法打开时都让出底部导航，输入框不会再被导航覆盖；用户需要先收起输入法才能点击底部 Tab。输入法收起后导航自动恢复，原有对话页与系统栏避让逻辑不变。

## Verification

- 先在 `WindowInsetsPolicySmoke` 加入输入法可见/不可见与对话页组合的断言，确认修改前编译失败。
- 修改后运行 `bash platforms/android/check-host.sh`：276 个 Android JVM 冒烟、245 个设备套件源码编译、JNI/API/资源/契约检查全部通过。
- 仍需在 API 28/35 模拟器或报告者的 Android 11 真机打开搜索框复验实际窗口层级；本次本地环境未完成该设备验收。
