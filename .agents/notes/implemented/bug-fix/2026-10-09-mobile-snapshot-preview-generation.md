# Agent Note: 移动端快照预览按账户代次发布

Status: implemented

## Problem

Android 和 iOS 下载云词库快照后，在异步任务返回时直接写入预览表。若账户在下载结束与写表之间退出，退出流程可能先清空预览，旧任务随后又把文件和令牌放回表中。

## Decision

下载前记录账户代次与用户 ID。下载和本机检查结束后，在 `BackendAccountSession::with_generation` 持有账户锁期间写入预览表。代次失效或写表失败时删除新下载的文件；成功发布后删除被替换的旧预览文件。

## Alternatives considered

- 只在下载结束后检查一次账户状态：检查与写表之间仍可发生退出或切换账户。
- 依靠退出流程再次清理：退出和预览任务的完成顺序无法保证。

## Consequences

旧账户的下载任务不能在账户代次失效后重新发布预览。`with_generation` 的闭包只锁预览表，不再次调用账户会话。

## Verification

共享层测试覆盖有效代次发布与替换，以及退出后旧代次发布被拒绝；移动端编译与本地 quick 门禁见对应 Pull Request。
