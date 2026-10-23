# Agent Note: macOS 同时识别用户与系统输入法安装

Status: implemented

## Problem

#9411 报告移到 `/Library/Input Methods` 后部分应用能够使用输入法，设置应用却再次提示安装。`installed_bundle_path`、首次安装检测、注册查询和更新目标只看用户目录，`system_bundles` 又把系统副本无条件当作冲突。只改提示仍会让修复和更新写出第二份用户副本。

## Decision

把安装位置作为受限的用户或系统范围处理，路径由宿主生成，不接受页面提供目录。已有用户安装继续优先；只有系统副本时，状态、版本比较、注册和修复均指向它；两处均有时才提示重复安装。没有安装时保留当前用户默认值，首次安装页允许明确选择所有用户。

系统安装和更新先以普通用户在私有临时目录暂存并验证随附的输入法，再构建无脚本的组件安装包。组件固定安装到 `/Library/Input Methods`，禁止重定位到其他同标识符副本，采用原子替换并保留版本检查。只有用户主动安装或修复才通过系统管理员授权调用 `/usr/sbin/installer`；不让整个设置应用提权，不接收任意 shell 命令，不记录密码或原始工具输出。登记和当前登录会话的缓存刷新仍在普通用户进程执行。

启动检查只读系统副本。发现需要替换时提示主动修复，不在启动时弹管理员对话框。取消授权保持原安装，页面明确显示取消。系统安装的卸载不走只会处理用户目录的旧路径，避免删除用户数据后误称已经卸载；该范围的卸载由系统目录的手动删除提示承接。本项不迁移已经存在的用户安装、不停止其他登录用户的输入法，也不触发发布。

## Historical audit

- [首次安装等待登录](../../implemented/bug-fix/2026-10-09-macos-first-install-login-required.md) 部分重叠：保留 `RegistrationPending`、注册探针及登录提示，扩展其目标位置；系统副本不再单独构成冲突。
- [输入法 Sparkle 更新](../../implemented/feature/2026-10-15-macos-sparkle-input-method-updates.md) 部分重叠：保留单调构建号与禁止降级，扩展设置应用识别和手动修复的位置；不改 Sparkle 协议或发布脚本。
- 其余 Windows 的系统目录部署笔记涉及受信进程加载，不适用于 macOS 安装授权，不合并。

## Contract references

Apple 的 [TN2065](https://developer.apple.com/library/archive/technotes/tn2065/_index.html) 说明 `with administrator privileges` 只对指定命令授权，并要求用 `quoted form of` 处理参数；命令路径固定，安装包路径经 `argv` 传递。本机 Apple 随系统提供的 `pkgbuild(1)` 文档明确 `BundleIsRelocatable=false` 禁止搜索别处副本，`BundleOverwriteAction=upgrade` 原子替换，`BundleIsVersionChecked` 独立控制禁止降级。组件包由普通用户生成，不含安装脚本。

## Alternatives considered

- **只让状态页同时查两个目录。** 改动最小，能立即消除重复引导；但更新、修复和注册仍指向用户目录，会复制出第二份输入法，因此同时修正整个目标选择链路。
- **让启动检查直接请求管理员权限更新。** 用户打开新版设置应用即能更新系统副本；但正常打开设置会突然弹授权对话框，取消还会被误报为失败，因此启动只提示，显式安装或修复才授权。
- **提权运行现有 Rust 复制与回滚逻辑。** 可以复用用户目录的安装器；但会扩展提权进程的文件与执行边界，需自己持有系统目录的事务和权限。采用 macOS Installer 的固定组件包事务。
- **一律安装到系统目录。** 可以简化路径选择；但会改变既有默认值并让所有首次安装都要求管理员，保留当前用户默认值和明确的所有用户选项。

## Verification

- 安装路径回归在旧实现下两项失败（系统副本仍选用户目录、用户位置符号链接被接受），修复后通过；授权策略回归在旧分支下证明后台会调用写入，修复后阻止后台授权。
- 安装模块 55 项通过，含真实 `codesign`、`pkgbuild` 和 `pkgutil --expand-full`：合成 Mach-O bundle 的签名校验、固定 `/Library/Input Methods`、空重定位集合、原子 upgrade、版本检查、无安装脚本；未签名载荷被拒。
- 真实原生输入法包的 `pkgbuild --analyze` 显示 Sparkle 的更新器和两个 XPC 服务都属于主组件的 `ChildBundles`，顶层组件限制不会拒绝真实包。
- 首次范围选择、取消授权、待授权提示、系统卸载结果与修复后刷新等界面回归先红后绿，相关 73 项通过；`pnpm --filter @msime/desktop build` 通过类型检查与生产构建。
- 独立审查发现主动修复后启动状态仍停在 `authorization_required`。新增回归先失败；后端写回修复结果、清探针缓存，前端在修复结束时主动刷新，随后完整 `cargo test -p msime-desktop --lib` 278 项通过，覆盖 updated、login_required 和 cancelled。
- `/usr/bin/osacompile` 校验授权脚本语法通过；未运行脚本、未请求实际授权。
- 提交前另运行 Rust 格式、Clippy、笔记及完整 quick 门禁，结果记在 PR。

## Consequences

管理员授权和系统 Installer 是新边界，工具输出不进入日志或前端。私有暂存必须保留合法的包内相对框架链接、拒绝包外链接，并在打包前验证签名。测试仅使用合成数据和临时包，不实际提权或改写本机系统安装；真实系统级安装与取消流程需单独验收，不能把包结构检查当成已安装。
