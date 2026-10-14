# Agent Note: 旧 MCP 写入的迟到冲突不打开替换确认

Status: implemented

## Problem

`McpConnectSection` 用共享异步操作守卫隔离宿主更换后的成功结果和错误提示。但首次写入返回 `mcp_entry_exists` 时，内部 `catch` 先打开替换确认，只在用户回答后才检查 `isCurrent()`。旧宿主的迟到冲突因此能在新宿主面板打开确认框，占用同一个确认入口并打断新操作。

## Decision

在写入的错误处理入口先检查操作是否仍属于当前宿主，失效时立即结束，不打开后续确认。当前宿主的冲突继续要求用户确认替换；确认回答后的代次检查继续保护实际写入。新回归用合成宿主替换和延迟失败验证旧冲突不弹框、新宿主仍能写入。

## Alternatives considered

- 仅依赖确认回答后的现有检查已经能避免旧宿主再次写入，代码无需变化；但旧确认框已显示在新面板上，会占用共享确认入口，所以结果保护必须提前到发起后续交互之前。
- 在公共异步操作 runner 中统一捕获冲突可以集中错误处理，但这里的冲突需要业务确认和二次安装，runner 不拥有这段流程。调用方继续在每个异步交互边界检查现有 `isCurrent()` 即可。

## Consequences

迟到的旧宿主冲突不打开确认、不重试安装；新宿主仍能写入。守卫复用现有代次判断，不增加公共 runner 或确认 API。它不取消已经发给宿主的首次写入请求，只阻止失效请求产生后续交互。

## Verification

新增回归先在旧实现上单独运行，结果为 1 项失败、21 项跳过，旧冲突实际打开替换确认框。修复后运行 MCP 连接、复制反馈、代次复用和挂载复用四个测试文件，27 项全部通过，覆盖当前宿主冲突的取消和确认替换。MCP 回归在操作选项前等待实际可交互的 radio 出现，避免只等到外层分组就操作的准备竞态。首次 CI 的 3177 项测试中，3176 项通过，既有重复安装用例因 Cursor 选项尚未加载而失败。用合成状态响应延迟 1 秒在旧等待方式下复现相同失败，改成按选项等待后通过；延迟只用于临时复现实验，不留在正式测试里。本地完整套件 577 个文件、3177 项测试通过。第二次 CI 中 MCP 测试通过，已有社区资源回归因只等待请求发出、未等待失败后的分页状态而失败（3183 项通过、1 项失败）。合成失败响应延迟 1 秒时复现相同断言；该回归先等旧分页出现，再等失败处理后的分页消失，延迟失败下通过。两次延迟仅用于临时复现实验。补齐等待后，MCP 与社区资源五个相关测试文件的 46 项全部通过；lint、格式、笔记与 quick 门禁通过。

改动文件的 lint 与格式检查、`pnpm --filter @msime/desktop build`（含类型检查）、`pnpm run verify-notes`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均通过。quick 的 Rust workspace、Android target 与共享 Apple bridge 编译通过；缺原生 macOS 资源而跳过 desktop crate，Linux 外壳缺 Docker、HarmonyOS 缺 hvigorw/已准备依赖、Windows 缺 MinGW，相关原生宿主按改动范围跳过。验证使用合成回调，不能代表真实助手配置写入或原生宿主设备验收。

## Related notes

检索 `McpConnectSection`、`mcp_entry_exists`、MCP 异步与代次：[复制提示生命周期](../../implemented/bug-fix/2026-10-12-mcp-copy-feedback-lifetime.md)管理复制结果与提示计时，保留其决定；[Nix 配置路径](../../implemented/bug-fix/2026-10-07-mcp-nix-profile-link.md)管理既有条目识别和稳定路径，与冲突回包的 UI 所有者无关。公共临时状态与候选皮肤协议笔记不持有本次决定。
