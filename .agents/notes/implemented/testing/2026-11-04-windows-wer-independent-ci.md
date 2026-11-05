# Agent Note: 将系统 WER 验证从 Windows 设置作业拆出

Status: implemented

## Problem

Windows 原生设置作业在同一台 runner 顺序构建 Rust 宿主、WinUI、兼容设置、多版本面板，并执行系统 WER 与设置界面验收。2026-11-04 的 PR #10802、#10841 都触发整条作业 60 分钟上限，设置界面验收还在正常推进就被取消；#10809 甚至完成所有验收，只在收尾上传时超时。三次系统 WER 步骤分别占用 4 分 58 秒、5 分 21 秒、4 分 57 秒。

另一个独立问题是 #10844 和 #10755 的 x86 系统 WER 超时。已上传诊断符合[已知的系统侧等待形态](2026-11-02-windows-system-wer-x86-stall-retry.md)，但重试后仍失败，不能靠设置作业其余步骤通过或延长等待掩盖。这次拆分不宣称修复该投递问题。

## Decision

`ci-platforms.yml` 用独立的 `Windows TSF system WER` 作业执行原有 x64/x86 合成工程、生命周期和回调测试、真实系统投递验收及诊断上传。它与 `Windows native settings` 同样依赖 `changes` 的 Windows 范围判断，使用同一提交的生产代码，在 Windows runner 上执行。新增作业不链接 Rust 宿主，只独立准备 `nlohmann-json` 头文件，沿用现有安装命令与标准产物路径。

原设置作业保留 WinUI、兼容设置、手写、多版本、设置页面与语音窗口的全部验收和截图；60 分钟上限保持。独立 WER 作业上限为 15 分钟；每个用例原有的 45 秒退出、30 秒投递窗口、唯一一次受限重试及正负断言全部保持。任意一组失败都会显示在同一个 Native Platform CI 运行中，WER 失败也不会让设置验收被跳过。

## Alternatives considered

- **提高设置作业的总时限**：改动最少，能容纳目前的冷构建波动；但不解除独立检查的串行依赖，WER 提前失败仍阻止所有后续界面验收，因此采用独立作业。
- **去掉或放宽 WER 验收**：减少耗时最直接，但会丢失 x86 系统入口覆盖；不采用。

## Consequences

x86 真实系统投递之后改为非阻塞，失败以 warning 注解和作业摘要显示，见[x86 系统投递非阻塞](2026-11-05-windows-system-wer-x86-nonblocking.md)。

两组验收并行执行，设置作业收回约五分钟余量，失败归属更清楚。代价是多一次 Windows runner 启动和小型头文件安装；不重复构建 Rust、WinUI 或完整产品。

本地使用 `actionlint` 检查 workflow，比较移动前后的 WER 命令与上传路径，确认除了依赖目录迁移以外没有改变验收步骤。本机没有可连接的 Windows MSVC 主机，真实 Windows 作业结果以本 PR 的两个独立作业为准，未通过前不合并。
