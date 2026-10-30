# Agent Note: iOS 统计 CSV 分享持有独立快照

Status: implemented

## Problem

iOS 按日明细在出现时把 CSV 写入固定临时路径，`ShareLink` 保存的是 URL。再次生成另一份统计会覆盖相同路径，先前取得的 URL 随后读到新内容；原子的文件替换只能避免半份 CSV，不能保证旧分享所引用的快照不变。实际导出函数配合两份合成 CSV 已复现这一问题。

## Decision

每次导出使用 `UUID` 命名的独立私有临时目录（权限 `0700`），内部文件名仍是 `水杉IME-打字统计.csv`；失败只清理本笔新目录，成功的文件留给系统临时目录回收。保留 URL 形式的 `ShareLink` 和现有格式化规则。

## Alternatives considered

- `DataRepresentation` 能直接持有不可变字节并减少自行管理文件的代码；但 [Apple 的 `ShareLink` 文档](https://developer.apple.com/documentation/swiftui/sharelink)指出部分分享服务只接收文件表示，仅用数据表示可能减少现有分享目标，因此保留文件 URL。
- 页面离开时立即删除文件能主动回收空间；但分享服务可能仍在异步读取，页面生命周期不能作为传输结束信号，因此不据此删除成功快照。

## Verification

`scripts/test-ios-statistics-export.py` 在 macOS 上直接编译页面的实际导出函数，只把临时根移到合成目录，统计格式化以合成 CSV 替代，不冒充完整统计模型或 iOS 界面验收。旧实现先在“两个导出必须使用独立 URL”断言失败；回归要求两次导出使用不同 URL，目录权限为 `0700`，分别保留原始 UTF-8 字节和相同显示名；清理一份不删除另一份；临时根不可写时返回空且不删除已有文件；目录已创建后撤掉写权限，实际 CSV 写入失败则清理本笔空目录。

## Consequences

检索审计：[统计模型归属](../architecture/2026-10-15-typing-statistics-model.md)部分重叠，本次只改 iOS 分享文件的寿命；[移动统计认证](2026-10-25-mobile-statistics-auth-parity.md)与分享文件内容所有权无关；[Android 剪贴板导出](../feature/2026-10-16-android-clipboard-export.md)有相同缓存避重机制，但 Android 在同步复制结束后即可清理，iOS 的异步分享不使用那个结束时刻。

每次生成都会多一份临时快照，成功后由系统回收；不能在未知分享结束时刻按页面离开或固定时间强制删除。
