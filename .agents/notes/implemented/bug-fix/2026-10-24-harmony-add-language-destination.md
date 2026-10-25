# Agent Note: 鸿蒙键盘添加语言进入输入页

Status: implemented

## Problem

真机键盘「输入方式」中的「添加语言」打开词库页，那里只能管理词条，无法添加语言。入口沿用设计原型的词库跳转，实际产品的语言管理已在输入页的 `LanguageCard`。

## Decision

「添加语言」经 `openSettings('input')` 打开「输入」页的「语言与方案」。ArkTS 的 `SettingsPageLink.PAGES` 和网页的 `LINKED_PAGES` 同时放行 `input`，冷启动与已打开设置窗口共用原有 Want 路由。功能面板「词库」仍进入词库页。

检索活跃笔记中的「添加语言」「语言与方案」和 `dc.html`：[输入语言与中文方案分层选择](../feature/2026-10-19-input-language-scheme-hierarchy.md) 讨论桌面选择控件，与本次入口修复无关；[iOS 语言包](../feature/2026-10-13-ios-language-packs.md) 讨论资源下载，与本次无关；[鸿蒙工具栏](../feature/2026-10-17-harmony-toolbar-tools.md) 持有设置深链接白名单，本次沿用其双端校验。

## Alternatives considered

保留词库页跳转可以直接安装输入资源，但词条管理不等于语言选择，用户到达后没有对应操作，因此不采用。

## Consequences

入口名称与目标页面一致，新增一个允许的设置页深链接。语言仍由已有卡片添加，不改变默认启用的方案或词库。页面与 ArkTS 路由白名单的校验沿用 `test-harmony-settings-links.py`，Want 参数回归覆盖 `input`。

## Verification

- `test-harmony-settings-links.py` 与鸿蒙全部逻辑测试通过，覆盖 `input` Want 参数。
- 共享设置页类型检查、生产构建、HAP 的 ArkTS 编译与打包通过。
- 笔记校验和 `git diff --check` 通过。
- 本地 quick 已运行，鸿蒙编译通过；全局检查仍因 Docker 构建连接中断、Windows 交叉构建及 Apple bridge 构建失败而未通过。
- 签名 HAP 覆盖安装到 HarmonyOS 真机：冷启动 Want 打开输入页；在键盘「输入方式」点击「添加语言」也进入该页；展开后显示日语、韩语、越南语和藏语的添加项。验收未改变用户已启用的语言。
