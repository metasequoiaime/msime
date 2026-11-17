# Agent Note: 鸿蒙可见键盘采集连续复制

Status: implemented

## Problem

[#11448](https://github.com/metasequoiaime/msime/issues/11448) 已明确包含全平台。鸿蒙本机历史只有手动保存，连续复制之后只能取得最后一条。异步读取若等待前一次保存才发起，也会主动扩大漏采窗口。

## Decision

沿用已有共享 Rust `clipboard_history` 开关和缺字段关闭的约定，不新增默认值。编辑框属性已确认、键盘可见、历史开启且隐私规则允许时，取得系统已有 `READ_PASTEBOARD` 权限才监听系统 `update`；不代用户申请权限，不自动导入启动前的正文。每次变化立即发起独立异步读取，按通知顺序保存到既有移动历史 ABI。临时存储失败保留最多 128 条原文并重试，单条沿用共享移动端字符和字节上限。隐藏、失焦、销毁、切换编辑框和隐私状态丢弃旧队列，迟到结果不恢复采集。

## Existing notes audit

[移动端容量](../../implemented/feature/2026-10-15-mobile-clipboard-history-limit.md)及[历史容量](../../implemented/feature/2026-10-17-clipboard-history-capacity.md)继续持有落盘规则；[云端中继](../../implemented/feature/2026-10-27-harmony-cloud-clipboard-relay-webdav.md)只处理用户选择的云端来源，与本机采集并存。此项不改默认值和云端行为。

## Alternatives considered

同步 `getDataSync` 更容易立即得到正文，但华为文档要求避免在 UI 线程执行可能涉及远端或大数据的阻塞读取。采用每通知立即发起异步请求，完成后保持原事件顺序。只在打开面板时读取实现更小，但无法保存此前连续复制的正文，不满足本次范围。

## Verification

行为测试覆盖连续通知立即读取、异步完成倒序、落盘重试保留原文、权限拒绝与迟到授权、隐私状态丢弃、停止后的迟到读取和有界队列。完整 `tests/run.sh` 与远端 quick（`20261115-011859`）成功。现有空闲词库生命周期测试补齐新采集协作者，保留全部旧断言并增加失焦后的迟到显示不重启采集的断言。`20261115-012422` 用当前分支源码重编 Rust/NAPI、暂存锁定词库及语音运行库、构建共享设置 bundle，`ohpm install` 后真实 ArkTS 编译和 HAP 打包通过；ArkTS 适配器使用独立字段初始化，避免不支持的构造器参数字段声明。未签名包为 239,340,328 字节，Rust/NAPI 的 `.text`、`.rodata`、`.data` 与本次构建一致，18 个核心资源、3 个小型重排资源、43 个音效、26 个许可证及设置 bundle 与暂存文件逐字节一致。`hdc list targets` 返回 `[Empty]`，因此签名安装与真实系统剪贴板回放未验证，PR 保持 draft。

## Consequences

权限只按系统已有授予状态使用，未获权限时仍可见现有手动保存的限制提示。系统通知不能还原进程未运行或系统已覆盖的历史；跨设备正文读取按系统异步接口完成，不承诺无损获取任意速度的复制。最多持有约 12.8 MB 未落盘正文，停止即清除。
