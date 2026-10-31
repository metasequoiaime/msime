# Agent Note: Android 瘦包升级后补回随包五笔码表

Status: implemented

## Problem

Android 瘦包不带 `msime-wubi.db`。用户先安装瘦包、再覆盖安装带完整词库的 APK 时，已有 `runtime-options.json` 使 `Bootstrap` 只调用共享刷新；共享层允许按需文件整组缺席，因此不会报告 `dictionary_outdated`，资源目录仍缺五笔码表，设置页没有「五笔查码」。

## Decision

`Bootstrap.prepare` 在已有配置路径也检查 APK 的 `desktop-dictionary.lock.json`。当锁中完整列出某个按需文件组、而 `bootstrap/resources` 缺少该组任一正规文件时，按 APK asset 原子补回整组，再继续原有共享刷新。资源包已安装或 APK 本来是瘦包时不触发；完整组已存在时不重复复制。缺文件、坏文件或复制失败都保留原状态并在下一次启动重试，不改变共享层允许按需文件缺席的语义，也不删除已下载资源包。

## Alternatives considered

- **让共享层把按需文件缺席视为词库过期**：能触发重新准备，但会把 Android 发布包是否带可选文件的宿主边界下沉到共享校验，并可能让正常瘦包每次启动重复刷新；没有采用。
- **每次已有配置都重新解包完整词库**：能覆盖升级，但会重复写入约百 MB 的词库并增加启动耗时；没有采用。

## Verification

`BootstrapMarkerSmoke` 覆盖瘦包锁缺组、完整锁缺文件、完整文件和非正规目录四种状态；Android 宿主检查通过后，在 API 28／35 专用模拟器上复验瘦包覆盖完整包、冷启动文件存在、五笔设置入口和重复启动幂等行为。测试数据只使用合成文件和专用模拟器。

## Consequences

已有配置升级到带五笔码表的 APK 后，资源目录会补齐 `msime-wubi.db`，设置页和键盘下一次刷新可见五笔；完整包首次安装仍由原有 `extractDictionary` 处理，瘦包和已下载资源包路径不变。若 APK asset 损坏，补包失败后仍保持旧行为并在下次启动重试。
