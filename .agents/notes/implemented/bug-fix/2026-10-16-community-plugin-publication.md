# Agent Note: 插件发布独立复用并隔离预检客户端

Status: implemented

## Problem

`CommunityPluginPublishDialog` 与整个插件图库共处一个模块，本机目录、选择、打包预检与发布草稿状态都绑定 JSX，调用方不能独立使用这段发布逻辑。预检 effect 随 `client` 和 `chosen` 重跑，但代次只有 `chosen`；替换客户端后、目录重读尚未结束时，旧请求与新请求共享代次。旧成功覆盖当前字段，旧失败显示过期错误，旧 finally 提前结束当前加载。

## Decision

发布界面位于 `community-plugin-publish-dialog.tsx`，状态由 `use-community-plugin-publication.ts` 持有。hook 只要求 `packPreview`、`publish`、本机目录读取与成功回调，不依赖图库。图库保留旧名称转发，包入口直接导出独立对话框及 props。

预检 owner 同时包含 `client` 与 `chosen`；继续使用公共 `useAsyncGeneration`，隔离成功、失败及 finally。目录读取与提交维持 `client`、`localPlugins` 共同 owner，重试 ID、授权、类型过滤与同步防重复沿用。IME 回车事件留在对话框。

## 旧笔记归属

- [键盘皮肤发布](../../implemented/simplification/2026-10-16-community-skin-publication.md)部分重叠：沿用独立对话框与 hook 的边界；插件的打包预检、目录过滤与返回对象独立持有。
- [发布回车](../../implemented/bug-fix/2026-10-14-community-publish-ime-submit.md)部分重叠：共享 IME 保护原样保留。
- [特效包分享](../../implemented/feature/2026-10-10-community-effect-packs-shareable.md)部分重叠：发布类型集合仍由 `community-plugin-model.ts` 持有，不调整可发布类型与服务端配额。
- [发布建议字段](../../implemented/simplification/2026-10-12-community-publication-suggestions.md)无关：持有 Rust 打包器默认值生成，不改变建议字段来源。
- 其他目录、宿主、脚本授权和平台发布笔记无关：本次不改宿主打包、磁盘目录、权限、发布工作流或服务端协议。检索未发现持有插件预检 owner 的既有笔记，没有需要拒绝或吸收的旧提案。

[候选窗口皮肤发布](../simplification/2026-10-16-candidate-skin-publication.md)沿用状态拆分边界，独立保留其预览和许可证补写流程。

## Alternatives considered

- 只迁出完整对话框：模块搬迁最小，但状态仍绑定界面，其他共享入口无法独立复用或验证流程；同时抽出 hook。
- 统一键盘皮肤与插件发布成参数化 hook：可少写部分字段接线，但插件需要类型过滤、异步打包预检与发布对象回调，已有草稿和异步工具已覆盖真正相同的部分，继续保持业务边界。
- 在预检 effect 内另设取消标记：直接贴近请求，易于理解，但已有公共 owner 代次覆盖卸载、StrictMode 与依赖替换；补齐缺失的客户端 owner 即可，不再维护第二种机制。

## Verification

4 个客户端替换回归用例在旧实现上失败，补齐 owner 后通过，覆盖旧成功/失败在新请求完成前后返回。独立状态测试覆盖目录、预检、提交与卸载边界，保留全部既有界面和 IME 测试；源码门禁迁到真实持有者。对话框的包出口与旧出口相同，可只注入预检和发布能力；空目录关闭和发布失败登录仍可直接使用。

`pnpm --filter @msime/desktop test tests/community tests/community-gallery-load-more.test.tsx tests/settings/community-gallery-subview.test.tsx` 通过：79 文件、348 项。桌面类型检查和生产构建、Harmony 类型检查、改动文件格式与 lint、notes、diff 检查通过。独立审查未发现剩余问题。

`bash scripts/verify-local.sh --quick` 通过，包括仓库契约、Rust 工作区、Android 目标与共享 Apple 桥接编译。Linux Docker、Windows MinGW 与 Harmony 原生打包环境缺失，相应阶段跳过；其他原生检查按改动范围跳过。

## Consequences

其他共享入口可以独立使用发布状态或整个对话框，不再依赖图库实现。代价是目录状态、预检与发布形成显式 hook 接口，需要跨模块接线验证。修复只丢弃已过期预检的界面写入，不取消宿主中已运行的打包；除过期回包隔离外，文案、DOM、类型过滤、重试与回调规则保持。没有原生设备验收或线上实际发布。
