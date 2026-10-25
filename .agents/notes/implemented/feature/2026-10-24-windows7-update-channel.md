# Agent Note: Windows 7 更新只选择专用发布与安装包

Status: implemented

## Problem

普通 Windows 发布依赖现代设置和运行库。兼容宿主如果继续使用 `platform: windows` 检查更新，会把 Win7 用户引向不能运行的普通安装包；仅在 UI 标注系统要求无法保证下一次更新仍保持支持。

## Decision

共享更新服务增加 `windows7` 平台契约，只读取 `windows7-v<版本>` 正式 tag，安装包限定当前 edition 的 `MetasequoiaIME-<Edition>_Setup_v<版本>_win7_<x64|x86>.exe`。完整产品的[分架构布局](2026-10-24-windows7-product-build.md)取代最初同包承载的假设：`x86_64`／`x64` 选择 x64，`x86`／`i686` 选择 x86，空或未知架构拒绝；不能提供另一架构或普通 Windows 包。原生关于页发送实际编译架构并再次核对返回包名。继续使用同一共享下载、超时、版本排序、摘要与唯一资产规则，不新增一套网络实现。没有专用发布或资产时不回退普通发布；现代 `windows` 行为保持。

该契约与最终 Inno 输出名称一致，不触发发布。Win7 打包、受信任测试签名、安装与升级已完成实机验收；线上专用发布未创建。

### 既有笔记审计

- [更新规则归共享 Rust](../../implemented/architecture/2026-10-09-update-check-in-rust.md)：部分重叠，继续使用该服务，只增加明确平台和资产命名分支。
- [Windows 关于页](../../implemented/feature/2026-10-20-windows-settings-about-tabs.md)：部分重叠，兼容页面复用现有展示模型，但请求专用平台。
- [完整 Windows 7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分重叠，此契约防止更新丢失支持，不能替代整体安装验收。

## Alternatives considered

- **复用普通 Windows tag，再由文件名筛选。** 同一版本只有一篇发布说明，维护更少。但缺少兼容安装包时普通发布仍可能被报告为可更新，因此使用独立平台前缀，并且仅将包含本 edition 唯一兼容安装包的正式发布列为候选。

## Testing

最终包名回归覆盖 x64／x86 与别名、缺失或未知架构、同 edition 缺另一架构、重复资产、草稿、预发布、版本与页面不匹配及旧包名拒绝。原生 HWND 用例验证请求携带真实架构，并拒绝另一架构和现代包，错误后清除旧下载入口。

- Windows runner 的 Win7 x64 目标执行 `cargo test -p msime-client-services --lib update_check::tests --release`，15 项通过，包括 3 项新专用发布测试和全部现代选择回归；同一目标 `cargo clippy -p msime-client-services --lib --release -- -D warnings` 通过。Tier 3 的 clippy 需要将 `-Z build-std=std,panic_unwind` 放在 `clippy` 子命令后面，放在 cargo 全局位置不会为它构建标准库。
- 同源 Win7 x64／x86 核心完成重建、真实 ABI／字体／音频枚举加载和已知导入阻断检查；x64 MCP 也完成构建。
- 原生关于页以合成发布响应验证专用请求、普通安装包拒绝、旧链接清除、None／Current／Available／Error、系统与问题报告复制、匿名统计 CAS 和帮助／反馈定位。完整版 x64／x86、五笔与日文 x64 的八套设置 CTest 全部通过。设置 EXE 已知导入审计为 x64 338／x86 332 项。

## Consequences

兼容宿主新增关于／帮助／反馈页：当前版本、检查更新、系统信息、匿名统计、随包许可、隐私链接、问题草稿和浏览器预填 issue 入口。诊断只报告实际系统版本、应用版本和方案，不含用户路径；复制与提交时重新读取草稿，不能依赖程序化文本变化一定发出 `EN_CHANGE`。窗口只在用户点击后打开浏览器，测试不发送消息、不打开浏览器、不改变真实账号。

专用发布命名隔离现代安装包，代价是打包流程必须遵守同一契约并分别提供两个架构。完整安装、升级、双架构实机输入已通过，MCP 有独立原生连接页，帮助和反馈入口已接通；第三方助手是否支持 Win7 由其自身决定。最终 quick 见[产品构建](2026-10-24-windows7-product-build.md)。
