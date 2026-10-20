# Agent Note: 网页候选皮肤的卷轴展开与版本化点选

Status: implemented

## Problem

固定的 `msime-windows develop@a60e2948dbba29f32bec58ee4bf44d68ce953a0b` 用 `expand_panel` 与 `expand_rows` 控制横排首行下面的滚动网格。网页目前只传当前页，无法显示或可靠选择页外候选。把当前页重复铺进网格会展示错误数据；只传全局下标又会让迟到的点击选中新一代同位置的字。

## Decision

网页 Rust 宿主沿用现有引擎展开与排序映射，在用户展开后发布完整排序列表。普通帧仍只传当前页。列表身份由 WASM 实例编号与 Rust 列表版本组成；重置、换组字、顺序或引擎座位变化均使旧身份失效。扩展点选同时核对身份、全局位置和画出来的文字。当前页点选增加可选的身份、文字与 `pageStart`，兼容原有单参数接口。翻页不改变列表身份，因此当前页路径另核对起始座位。打开必须使用当前身份；同一实例收起允许较旧的列表版本，避免收起请求排在输入之后而失效。

展开开关通过 SDK、Worker、WASM 到 Rust 宿主，返回的同一帧携带展开后的当前页与完整列表；输入继续时自动刷新展开列表，上屏、取消、重置或失焦时收起。界面保留当前页数字键语义，下面显示当前页之后的真实候选，按来源的 2.6 倍字号等宽格与跨格布局，最多每批增加 300 个节点。`expandRows` 决定可见网格行数，浏览器原生滚动提供触屏与滚轮操作。竖排和关闭开关的皮肤保持原有界面。

历史审计：[Windows 卷轴展开](../../implemented/feature/2026-10-19-windows-candidate-skin-expand-panel.md)持有来源交互与 Windows 协议，本篇补网页协议；[出厂布局](../../implemented/feature/2026-10-20-skin-factory-layout.md)继续持有清单继承规则，本篇补其中尚未消费的两个字段；两篇均保留。

## Alternatives considered

- 每次展开都预先下载完整列表：界面无需等待，但每个普通按键都传全部候选，放大 Worker 消息与浏览器分配；改为展开期间才发布完整列表。
- 只按位置选候选：协议与界面最简单，但排队中的按键、辅助码或方案切换会让旧位置对应另一个候选，必须同时校验列表身份和文字。
- 让网格逐页调用原有翻页：可以复用所有旧接口，但改变首行、数字键和当前高亮，还无法证明迟到点击对应哪一页，因此不采用。

## Testing

- `cargo test --locked -p msime-engine-wasm` 全部通过，其中 61 项路由测试覆盖真实尾部放出、页外点选映射、过期与伪造点击拒绝、翻页不换列表身份、重置和同码新组字换身份；原生 clippy 通过；独立运行 `20261020-192902` 的 wasm32 clippy 与原始 glue/WASM 冒烟也通过。
- `node --test packages/web-engine/test/*.test.mjs` 的 46 项测试通过，SDK 与真实 Worker handler 核对展开、收起和点选参数及 u32 转换前的非法数值拒绝。
- WASM release、npm 包组装及 `packages/web-engine/test/smoke.mjs` 通过，合成词库真实尾部上屏、实例重建、旧版本及错误页起点拒绝均经过 SDK → Worker handler → WASM。共享出厂布局的 7 项测试通过。
- Chrome 候选栏检查通过：内置 `wechat_scroll`、直接对象和 catalog 三入口，显式关闭与行数覆盖、实际高度、跨格、300→600→631 节点、滚动位置、页内/全局点选参数、迟到回复、失焦、解除绑定与快速排列切换。
- TypeScript 声明检查不使用 `skipLibCheck`；格式、笔记与差异校验通过。最终集成源码的远端运行 `20261020-192148` 中 `scripts/verify-local.sh --quick` 通过：171 项 Windows 可移植策略测试、HarmonyOS 设置构建、Rust/Android/WASM 检查、Java/JNI 冒烟、Linux 设置壳及原生 93 项测试、macOS 宿主和 Apple bridge 通过；Windows 系统构建和 HarmonyOS HAP 缺少远端前置而跳过。远端沿用固定产物目录，不在磁盘不足的开发机创建构建树。

## Consequences

完整列表会增加展开期间的消息体积；浏览器节点按批追加，普通未展开帧不承担这项成本。宿主自画候选栏要主动接入新增接口，旧调用保持原有当前页行为。此切片不代表移动端或 Linux Wayland 的剩余布局字段已经完成。
