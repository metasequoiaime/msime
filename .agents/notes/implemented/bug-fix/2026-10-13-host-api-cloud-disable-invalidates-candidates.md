# Agent Note: Host API 云候选开关使旧候选身份失效

Status: implemented

## Problem

组字期间关闭 `cloud_candidates` 时，Host API 会拒绝关闭期间到达的云端回调，却让此前已显示的云候选继续留在候选栏。若用户随即重新打开开关，关闭前的查询也可能继续被接受。

组字期间开启云候选也会在第二席插入 `CloudPending`，但运行时原 setter 只刷新、不推进代次。旧 ID 的索引会指向不同词条或占位行，仍然被选择入口接受。

## Decision

`HostSession::update` 比较最新请求的偏好与新快照。当云候选由开变关、或来源配置变化且会话正在组字时，先调用 `clear_online_candidates(0)`，再发布新偏好。运行时清除来源 0 的缓存和显示行，同时使旧在线查询失效；重新打开后由当前组字生成新查询。相同偏好的重复更新不清理候选。来源配置、HTTP 描述符与缓存隔离见[可配置云候选 HTTP 来源](../feature/2026-10-19-custom-cloud-candidate-url.md)。

`Runtime::set_cloud_candidates_enabled` 读取引擎的实际开关，只在值变化时先调用 `advance()`、再改变引擎和刷新快照。直接 Rust 调用的开关双向变化和 Host API 延迟应用的开启都使旧候选 ID 失效；代次耗尽时在修改引擎之前拒绝操作。相同值直接返回，不能让每次无关偏好同步都撤销当前候选身份、释义或高亮。不在运行时另存一份开关缓存，避免已配置的 Session 和 Runtime 状态漂移。

## Alternatives considered

- 仅在云端回调处检查当前开关：关闭期间可拒绝新结果，但已显示行不消失，重新打开后旧回调仍可能落地。
- 每次云候选偏好更新都清理：重复同步相同配置也会推进候选代次，令当前候选身份失效。

- **仅由 Host API 比较偏好并推进代次**：宿主已经有前后偏好，改动较集中；但公开 Rust setter 仍可独立调用，且运行时才拥有候选身份，因此在实际修改引擎的入口统一保护。
- **每次 setter 都推进代次**：无需增加状态读取接口，实现最短；但 Host API 每次偏好更新都会调用它，即使没有改变云开关，也会使有效候选失效。读取引擎当前值后只对有效变化推进。

## Consequences

关闭开关会立即移除当前云行并作废旧请求；本地候选仍可用。清理会推进运行时候选代次，因此调用方应使用偏好更新返回的新视图。空闲会话没有待显示的云行，不触发清理。

开启与关闭占位行都可能使本地候选索引移动，调用方使用更新返回的新视图。关闭时既有的云缓存清理继续使旧在线查询失效；值不变不额外消耗身份。新增只读 getter 暴露现有引擎状态，不改变候选排序规则或宿主协议。

## Verification

使用合成候选的回归先证实关闭后云行仍可见；修复后覆盖关闭时移除云行、重新打开后拒绝旧查询并接受新查询。`cargo test -p msime-host-api cloud_ --lib` 的 23 项测试通过。

最新源码回归覆盖 Host API 组字期间开启云席位后拒绝旧 ID、新 ID 正常选择，双向开关推进代次、重复值不变、代次耗尽时底层完整快照不变。Host API 与运行时的身份、耗尽用例在原实现中先失败，重复值用例守住幂等约定；输入和词库均为合成值。

在 macOS 上，`cargo test -p msime-host-api -p msime-input-runtime --locked` 共 1184 项通过，另有 3 项既有忽略；`cargo clippy -p msime-engine -p msime-host-api -p msime-input-runtime --all-targets --locked -- -D warnings`、格式检查与笔记门禁通过。完整套件中既有联系人读取断言漏掉缺省空手机号，已在回退本轮生产修改后复现同一失败，并同步两处读取预期；旧格式输入仍不提供 `phone`，继续验证缺省兼容。独立审查未发现阻塞问题，没有系统输入框或真机验收。

`bash scripts/verify-local.sh --quick` 通过：157 项 Windows 可移植测试、Rust 工作区、Android 目标与 Java 宿主、两种 WASM 配置、Linux 设置外壳与原生宿主（90 项测试）、macOS 原生宿主和共享 Apple 桥接层均通过。Windows 交叉构建与 HarmonyOS 真打包因缺少前置环境跳过；没有原生输入框或设备装机验收。
