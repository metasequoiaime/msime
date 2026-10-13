# Agent Note: 旧账号会话迁移不覆盖新登录

Status: implemented

## Problem

Windows 设置应用和原生窗口会把旧目录中的 `account-session.json` 搬到与输入法共用的新目录。迁移先用 `symlink_metadata` 检查目标不存在，再调用 `std::fs::rename`。另一个进程如果在检查后登录并写入新会话，`rename` 会替换它；用户刚登录的新账号被旧令牌覆盖。Rust 标准库明确规定 `rename` 会替换已存在的目标，这个检查与移动之间没有原子性。

## Decision

迁移改用目标已存在即失败的移动：Linux、Android 和 Apple 平台用现有 `rustix::fs::renameat_with(..., NOREPLACE)`；Windows 等其他目标用硬链接原子发布目标后删除旧名字。硬链接建立失败时保留旧文件。迁移失败仍沿用原有的尽力而为语义：不覆盖新会话，旧文件留在原处。

## Alternatives considered

- 给检查和 `std::fs::rename` 加进程内锁：设置应用和原生窗口是不同进程，不能共享该锁。
- 目标存在时先读会话再决定新旧：判断仍发生在移动前，竞态不消失，而且令牌轮换后不能仅凭时间戳判断哪个会话有效。

## Consequences

新目录中已发布的会话总是胜出；迁移失败不会删除旧会话。Windows 使用标准库的安全文件系统 API，无需新增依赖。

## Verification

新增测试在缺席检查后发布合成新会话，旧 `std::fs::rename` 实现使断言失败；改为目标存在即失败的移动后通过。`cargo test -p msime-client-core --lib`：1383 项通过、1 项忽略。变基到包含 #7619 的 `develop` 后，`bash scripts/verify-local.sh --quick` 通过，包括 Rust workspace、Android、wasm、Linux 原生宿主 83 项测试、macOS 和 Apple bridge。首次 PR CI 的 Windows GNU 交叉编译指出工作区禁止 `unsafe`，因此 Windows 改用标准库 `hard_link` 加 `remove_file`；仍需在 CI 验证该修订。Windows MSVC 本地检查在依赖 `ring` 的 C 编译处因当前 macOS 环境缺少 `assert.h` 停止。
