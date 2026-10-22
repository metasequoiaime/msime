# Agent Note: Linux 移除宣传彩蛋快捷键

Status: implemented

## Problem

#9277：IBus 宿主把 \`Ctrl+Shift+P\` 吃掉并在当前输入框插入产品介绍。Ubuntu、VS Code、JetBrains 和终端等常见环境把这个组合键用于命令面板或隐私窗口，输入法不应抢占应用快捷键。

## Decision

IBus 和 Fcitx5 都只保留工具栏菜单里的宣传入口，删除 IBus \`process_key\` 中对 \`Ctrl+Shift+P\` 的特殊处理、按键占用记录和相关文案。固定宣传文本和工具栏动作继续保留；其他快捷键的处理顺序不变。

## Alternatives considered

- **继续保留 IBus 快捷键**：改动最小，但会继续阻断应用的通用命令面板快捷键，直接违背 issue 的诉求。
- **新增偏好让用户关闭快捷键**：能保留旧用户习惯，但需求明确要求不占用任何快捷键，而且为一次固定彩蛋增加设置与跨宿主同步没有必要。

## Consequences

Linux 两个宿主的宣传入口行为一致，应用可以正常收到 \`Ctrl+Shift+P\`；用户仍可从工具栏菜单插入宣传文本。宣传功能不再有键盘快捷入口。

## Verification

- \`python3 platforms/linux/tests/core/fcitx5_contract.py\`
- \`bash platforms/linux/build-container.sh\`（远端 \`rbuild\`）
