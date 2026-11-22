# Agent Note: #10947 Lanxin+ macOS IMK 兼容调查

Status: rejected — issue #10947 已由报告者在 2026-11-20 关闭，没有关联的修复 PR，也没有可复现的宿主环境，不再单独跟踪

## Problem

#10947 报告在 Intel macOS 15.7.4 的企业 IM Lanxin+ 2.7.1004（8418）中，选中水杉后菜单栏显示为「A」，聊天框和搜索框都直接上屏英文字母；系统简体拼音和其他第三方输入法在同一输入框中正常。报告者附的两份诊断日志都只显示输入法反复激活/停用（第二份只有 `focus_in`、`focus_out`、`preferences_applied`），没有 `keyboard_loaded`、按键或候选事件；升级到 0.59.1 nightly 后同样可以重现。仓库没有 Lanxin+ 的可执行复现环境。

报告者在 2026-11-20 以 completed 关闭了该 issue，关闭时没有留言，也没有关联的修复 PR。

## Proposal

将本 issue 作为兼容性 Draft PR 跟踪。先收集 Lanxin+ 的最小 IMK 会话信息和删去输入内容/私人路径的诊断片段，判断是宿主拒绝、客户端生命周期错配还是输入法加载失败；确认产品拥有责任路径后，再加入最小兼容修复和真实宿主回归。

## Alternatives considered

- 修改所有 macOS 输入法生命周期或强制延长会话：会影响全部 IMK 宿主，无法证明能修复 Lanxin+。
- 用普通编辑器或合成 IMK 客户端代替 Lanxin+ 验收：无法覆盖报告者的宿主行为。

## Acceptance criteria

- 得到 Lanxin+ 或最小等价 IMK 宿主的可复现生命周期证据。
- 修复在该宿主和普通编辑器中都有回归覆盖。

## Risks

未经真实宿主复现的 IMK 生命周期改动可能让其他应用反复激活、停用或丢失组字。
