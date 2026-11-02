# Agent Note: 上屏后联想的候选栏按键样式

Status: implemented

## Problem

#10078：Android 打开[上屏后联想](2026-10-24-post-commit-prediction.md)后，选字上屏，候选栏换成联想词，工具栏连同「收起键盘」一起被候选行替下（联想有候选，`render` 里 `idle` 为假），右端只有展开键。想不要联想只能按空格，而引擎对联想的空格是「清掉联想、交还编辑器」，编辑器照常插入一个空格。新用户找不到关闭方式，老用户每次多一个空格。报告者给出三种按键方案，维护者在 2026-10-28 确认保留现有方案、另加两种可选方案，并要求展开后也有关闭入口。

## Decision

新增共享偏好 `next_word_prediction_buttons`（client-core `NextWordPredictionButtons`），三种写法：

- `expand_only`：现有样式，只有展开键（联想多于一页时出现）。默认值，不写进文档，老用户什么都不变。
- `close_and_expand`：展开键左边加关闭键 ×。展开面板打开时候选行仍在面板上方，× 留在 ∧ 左边，所以展开后同样能关。
- `close_only`：只有关闭键，不放展开键，联想只能左右滑动候选栏选择。

关闭键送 `Command::Cancel`（`command(3)`）。引擎对联想的 Cancel 清掉建议并报已处理，宿主不往编辑器写任何字符，已上屏的词序保留；候选清空后 `render` 换回工具栏和「收起键盘」。展开面板开着时先收起再取消。普通组字时不出现关闭键，展开键照旧，三种样式只作用于 `View.prediction` 为真的那一帧，规则在纯 Java 的 `PredictionButtonsPolicy` 里，由 JVM 冒烟钉住。

设置入口在 Android「表达 → 智能」的「上屏后联想」下面，名为「联想栏按键」；联想开关关着时整行禁用。读取规则与触屏键盘底行样式一致：不认识的写法按 `expand_only` 读，不是字符串照样报类型错。

关闭键与展开键同宽同高（1 dp 分隔线加 40 dp 点按区），两个键并排时分隔线只画在 × 左边。× 的图标加进 `generate_keyboard_icons.py` 的 `ICONS` 表（`CLOSE`），只生成 Java，iOS 的 `SWIFT_CASES` 不变。

## 平台范围

只改 Android。上屏后联想目前只有 Android 启用（host-api 以 `cfg!(target_os = "android")` 门住引擎选项），iOS 与鸿蒙没有联想状态，也就没有这一行候选可放按键。三端键盘 UI 一致的约定在这里的落法是：哪一端启用上屏后联想，就同时读 `next_word_prediction_buttons`，按同样三种样式画关闭键与展开键，关闭键同样送 Cancel。鸿蒙手机启用联想时照此接入（`keyboard/candidate/NextWordPredictionPolicy.ts`），2in1 候选窗没有展开键，不放这两个键。React 设置页和 Windows 没有联想入口，字段在偏好一致性脚本的 `RUST_ONLY` 里登记原因。macOS 宿主同样没有联想状态，共享偏好在 macOS 无人读取，所以 `platforms/macos/tests/settings/preference_coverage.py` 的 `NOT_APPLICABLE` 也登记了这一项和原因；macOS 以后启用联想时要接入它并删掉这条豁免，门禁会把过期的豁免报出来。

## Alternatives considered

- **把关闭做成收起整个键盘，或者在联想时保留工具栏**：工具栏里本来就有「收起键盘」，保留它最省事；但联想时工具栏和候选行要同时占两行，键盘每次上屏都长高一行再缩回，和现在候选行替换工具栏的布局冲突，而且用户要的是「不要这些联想」，不是收起键盘。
- **只加关闭键、不做三选一**：改动最小；但维护者明确要求保留现有样式并让用户在三种里选，微信、搜狗、百度式的「只有 ×」和现有的「只有 ∨」各有用户。
- **改引擎让空格关闭联想时不插入空格**：不用加按键；但空格在联想时交还编辑器是上一篇笔记有意定下的规则（联想不能接管空格），改它会让「打完一个词接着打空格」的常见操作吞掉空格。
- **关闭键复用候选头里的「退出本地模式」文字按钮 ×**：现成；但那是读音行上的文字按钮，联想时读音行是隐藏的，字号和皮肤颜色也和展开键对不上。

## Consequences

- 收益：选了带 × 的样式后，联想可以一键关掉，不留多余空格，工具栏和「收起键盘」立即回来；展开后同样能关。默认值不变，现有用户的候选栏一模一样。
- 代价：候选行在显示 × 时窄了 41 dp，横向能露出的联想少一两个。`close_only` 下多于一页的联想只能滑动查看。新字段目前只有 Android 读，其他宿主启用联想时必须一起接入，否则设置会在那一端失效。
- 验证：`cargo test -p msime-client-core --test next_word_prediction` 覆盖默认值、三种写法往返、未知写法读作默认与非字符串报错；`msime-input-runtime` 的 `next_word_prediction_cancel_clears_suggestions_without_reaching_the_editor` 钉住 Cancel 报已处理、无上屏、清空候选；`platforms/android/check-host.sh` 编译宿主并运行 `PredictionButtonsPolicySmoke`、图标生成一致性和设置搜索索引。设备上的按键布局、深浅色皮肤和点 × 后输入框没有多出空格还没有在模拟器或真机上验收。
