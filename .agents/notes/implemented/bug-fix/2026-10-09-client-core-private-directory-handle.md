# Agent Note: client-core 私有目录逐级句柄访问

Status: implemented

## Problem

client-core 删除匿名会话、剪贴板历史和读取或发布词库租约时，最终目录虽然使用了 `O_NOFOLLOW`，但目录路径仍由一次路径型打开解析。祖先目录在检查后被替换为符号链接时，操作可能进入外部目录，导致删除、读取或租约发布作用于错误的存储位置。

## Decision

在 `storage` 中提供统一的 Unix `open_private_directory`：从根或当前目录开始逐级使用 `openat(O_NOFOLLOW)`，只对 `msime-path-trust` 声明的系统别名放宽最后一级。私有文件删除和词库租约目录都复用这个句柄，后续 `unlinkat`、`openat` 和 `renameat` 始终相对于已打开的目录执行。

## Alternatives considered

- 继续在 `reject_symlink` 后直接 `open`：只能证明检查时的路径状态，不能覆盖检查与打开之间的祖先替换窗口。
- 每个调用方各自复制目录遍历：可以局部修复，但会让匿名会话、剪贴板和租约使用不同的系统别名与符号链接策略。
- 先 `canonicalize` 再打开：`canonicalize` 自身会跟随替换后的祖先，无法提供句柄绑定。

## Consequences

祖先目录发生替换时，删除和租约操作会失败并保持原目录及外部目录不变；正常路径和 macOS 受信任系统别名仍可用。共享实现增加了逐级遍历代码，但把多个存储入口的竞态策略集中在一处，并由祖先符号链接测试覆盖。
