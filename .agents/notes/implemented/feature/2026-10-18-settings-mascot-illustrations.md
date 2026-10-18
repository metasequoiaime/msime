# Agent Note: 设置页低频状态配水杉娘插画

Status: implemented

## Problem

水杉娘已有一整套角色素材，但在产品里一张都看不到。共享设置页的更新完成、空列表、连不上社区、下载资源包这几个状态都只有文字，用户最常停留又最需要被安抚的地方没有任何品牌形象。

## Decision

`packages/ui/src/core/mascot.tsx` 提供 `MascotIllustration`，按场景名（`updated`、`empty`、`offline`、`downloading`）画一张水杉娘。插画只做装饰：`alt=""`、`aria-hidden`，状态仍由旁边原有的文字说明，原有文字和 `role` 一字不改。

接入点只有低频页面：

- `CommunityGalleryFeedback`：读取失败（不含要求登录）画 `offline`，列表为空画 `empty`。所有社区画廊（皮肤、候选窗皮肤、插件、词库与回复模板）共用这一处。
- `InputSourceStartupNotice`（macOS）：顺利安装或更新且用户无事可做时画 `updated`；失败、要重新登录、要手动加入输入法列表时不画，把注意力留给下一步操作。
- 桌面关于页：检查更新结果为「已是最新版本」时画 `updated`。HarmonyOS 关于页头本身已是一张信息密集的卡片，不加。
- `ResourcePackRow`：下载进行中，在进度说明前画一张 32px 的 `downloading`。

候选窗等高频输入区域不接入。

## 资源与体积

四张图是 256px 的透明 WebP，放在 `packages/ui/src/assets/mascot/`，合计约 66KB。桌面按普通资源输出；HarmonyOS 设置页是单文件 HTML，会把它们内联成 base64，实测页面从 1,720,141 字节增至 1,842,413 字节，仍低于 2,048kB 的告警线。以后再加插画前要先算这条线。

插画由项目用 Seedream 5.0 pro 生成，再抠成透明底；来源与授权记在 `packages/ui/UPSTREAM.md`。角色外形以锯齿呆毛的标准设定为准，接入的图必须符合该设定。

## Alternatives considered

- 给 `SettingsEmptyMessage` 加可选插画，在词库查询等列表上使用：覆盖面更广。但词库查询的 `emptyMessage` 同时承载「输入关键词后查看」的引导和「未找到匹配词条」的结果，只能靠字符串区分，容易误判，所以先只接社区画廊这一处语义明确的空状态。
- 用 SVG 重绘：体积小、可着色，但手绘风插画转矢量要么失真要么更大，也失去和表情贴纸同一套画风。
- 在 HarmonyOS 关于页头也加插画：品牌露出更多，但会挤压已有的版本、版权和检查更新胶囊。

## Consequences

社区读取失败、空列表、macOS 更新完成、桌面已是最新、资源包下载中这些状态有了角色露出，文字与可访问性语义不变。代价是设置页多约 66KB 资源，HarmonyOS 单文件多约 122KB。`apps/desktop/tests/core/mascot-illustration.test.tsx` 覆盖装饰属性和各接入点的出现条件；关于页的「已是最新」分支依赖完整设置表单，没有单测，靠类型检查和人工核对。
