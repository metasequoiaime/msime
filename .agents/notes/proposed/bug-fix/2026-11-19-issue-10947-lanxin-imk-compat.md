# Agent Note: #10947 Lanxin+ macOS IMK 兼容调查

Status: proposed

## Problem

#10947 报告 Intel macOS 15 和 Lanxin+ 2.7.1004 中水杉输入法无法建立可用 IMK 会话，诊断日志只显示反复激活/停用，没有 `keyboard_loaded`、按键或候选事件。仓库没有 Lanxin+ 可执行复现环境。

## Proposal

将本 issue 作为兼容性 Draft PR 跟踪。先收集 Lanxin+ 的最小 IMK 会话信息、最新 nightly 构建和删去输入内容/私人路径的诊断片段，判断是宿主拒绝、客户端生命周期错配还是输入法加载失败。确认产品拥有责任路径后，在本 PR 加入最小兼容修复和真实宿主回归；证据不足时保持 Draft。

## Alternatives considered

- 修改所有 macOS 输入法生命周期或强制延长会话：会影响全部 IMK 宿主，无法证明能修复 Lanxin+。
- 用普通编辑器或合成 IMK 客户端代替 Lanxin+ 验收：无法覆盖报告者的宿主行为。

## Acceptance criteria

- 得到 Lanxin+ 或最小等价 IMK 宿主的可复现生命周期证据。
- 修复在该宿主和普通编辑器中都有回归覆盖。

## Risks

未经真实宿主复现的 IMK 生命周期改动可能让其他应用反复激活、停用或丢失组字。
