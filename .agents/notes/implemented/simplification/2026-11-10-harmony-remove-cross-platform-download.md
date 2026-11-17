# Agent Note: 移除鸿蒙手机的其他平台下载入口

Status: implemented

## Problem

HarmonyOS 应用的“我的”页仍显示“其他平台下载”，并能进入共享下载页，引导用户获取 Android、iOS 和桌面版本。AppGallery 审核明确要求删除应用内引导用户下载安卓版和 iOS 版的内容；该入口也没有为 HarmonyOS 手机提供审核所需的应用内功能。

## Decision

HarmonyOS 手机的账号页不再渲染“其他平台下载”行，也不再注册或恢复 `download` 账号子页。共享下载页组件继续保留给其他设置页面使用；Android 和 iOS 的账号页继续通过各自宿主传入的 `onOpenDesktopDownload` 显示入口，HarmonyOS 2-in-1 的桌面账号页继续显示“电脑版下载”。

## Alternatives considered

- **只隐藏“其他平台下载”行，保留 HarmonyOS 下载子页。** 改动较小，但旧历史记录或深链接仍能打开被审核要求删除的内容，因此没有采用。
- **删除整个共享下载页和所有宿主入口。** 可以彻底避免误用，但会破坏 Android、iOS 和桌面宿主仍在使用的下载能力，因此没有采用。

## Consequences

- HarmonyOS 手机“我的”页不再引导用户下载其他平台，旧的 `accountSubpage=download` 历史值会回到账号主页。
- Android、iOS 和 HarmonyOS 2-in-1 的既有下载行为保持；对应行为测试分别覆盖入口保留、入口隐藏和旧历史值降级。
- HarmonyOS 手机少一个账号子页，账号页历史路由只承载资料和 QQ 群助手页。
