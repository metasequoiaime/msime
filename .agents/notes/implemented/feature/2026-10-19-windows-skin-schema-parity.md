# Agent Note: Windows 候选皮肤清单与 msime-windows 对齐

Status: implemented

## Problem

`msime-windows` 固定提交 `c0a9d371f00b7b309c07e78bc0fd27d3a0d4f5b1` 的候选皮肤清单增加了 `wechat_scroll`、`canvas`、候选细分颜色、候选窗版式、系统材质和 v2 九宫格资源。共享 Rust 加载器此前只校验其中一部分，未保存的字段会被静默丢弃；Windows 原生宿主因此可能把能安装的皮肤画成默认外观，或者无法识别 `canvas` 这类基底。

## Decision

共享 `crates/client-services` 的 catalog 现在保留现有 v1 与动画契约，并解析 `canvas` / `wechat_scroll` 基底、完整候选颜色、候选字体、版式、材质、九宫格、insets、动画图片和自由挂件字段。扫描阶段统一执行数值范围、字体、资源路径、动画帧和挂件数量校验；`canvas` 通过 `SkinSummary.canvas` 传递，不扩大 `GlobalTheme`，`canvas` 与 `wechat_scroll` 也加入保留 ID 规则。

`SkinSummary` 继续使用 camelCase JSON。Windows 候选宿主从已解析的清单中消费候选颜色、候选字体以及已有的装饰图、背景图、工具栏和固定动画资源。候选颜色覆盖预编辑文字与光标、选中候选文字、序号、释义、选中竖条和右键菜单；未声明或无法解析的颜色沿用主题默认值。几何、材质、九宫格、insets 与自由挂件字段完成安全解析与传递，但在对应 Direct2D 绘制能力接入前不伪装成已绘制。

旧皮肤缺省字段保持原有主题和布局行为，未知或越界资源继续拒绝；Windows 不执行皮肤提供的脚本或任意动画，只接受共享层声明的固定动画预设。

## Alternatives considered

- **只把新 ID 加入 Windows 外观常量**：改动最小，但清单新字段仍会被静默忽略，无法与来源的视觉协议兼容，因此不采用。
- **让 Windows 单独解析 TOML 字段**：可以更快让部分字段生效，但会复制安全校验并使其他宿主的扫描结果与 Windows 不一致，因此不采用。
- **一次性把全部新绘制原语接入 Direct2D**：视觉覆盖最完整，但会把协议迁移和渲染器重构绑在一起，难以独立验证；本次先保证字段安全、可传递并消费已有能力，后续按原语分别接入。

## Consequences

共享 catalog 与 Windows 之间的字段契约扩大，后续修改清单 schema 必须同步 Rust 模型、Windows JSON 消费和对应测试。候选皮肤可以安全携带来源新增字段，Windows 已有渲染路径会显示更完整的颜色和字体效果，旧包不受影响。

九宫格、材质、版式、insets 和自由挂件目前只完成解析与传递，仍可能与来源的完整视觉效果有差距；文档明确这一边界，避免把协议支持误报成绘制支持。新增 JSON 和 C++ 字段也扩大了构建面，因此提交前运行皮肤单测、clippy、笔记校验、差异检查和 Windows x64 交叉构建。

验证结果：`cargo test --locked -p msime-client-services skin::` 通过（246 项）；`platforms/windows/build-cross.sh x64` 通过；`git diff --check` 与 `pnpm run verify-notes` 通过。`scripts/verify-local.sh --quick` 的一次旧构建缓存曾在 `CandidateSkinAssets` 新字段补齐前报告 `-Werror=missing-field-initializers`，随后重新交叉构建已通过；未在真实 Windows 编辑器中目测所有新绘制原语。
