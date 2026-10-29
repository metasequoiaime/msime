# Agent Note: iOS 与 HarmonyOS 随包带上水杉输入法自己的 GPL-3.0 全文

Status: implemented

## Problem

水杉输入法本身是 GPL-3.0-only（根目录 `LICENSE`），三个移动端随包分发的 `msime-pinyin.db` 含 rime-ice 的 GPL-3.0 数据。GPL-3.0 第 4、6 条要求传递目标代码时随附许可证全文。桌面渠道都带着这份全文（macOS `Resources/Licenses/GPL-3.0.txt`，Windows `LICENSE.txt`，Linux `copyright`），Android 由 `build-apk.sh`、`build-client-apk.sh` 复制成 `assets/client-LICENSE.txt`，关于页以应用名列出；iOS 与 HarmonyOS 不带，[语言词库许可证表达式那次](2026-10-29-language-dictionary-licence-expressions-and-mobile-notice.md)把它记为留给维护者的产品决定。维护者 2026-10-29 决定：iOS 与 HarmonyOS 像 Android 一样随包分发 GPL-3.0 全文；引擎（`crates/engine`、`crates/path-trust`）是 MPL-2.0，编进引擎的渠道若还没带 MPL-2.0 全文也要补上。产品许可证与应用商店元数据不变。

## Decision

- **iOS**：`project.yml` 把 `../../LICENSE` 作为 `MSIMEApp` 的资源原样打包，bundle 里叫 `LICENSE`（XcodeGen 不能给资源改名，复制一份改名的副本进仓库又要另守一致性）。键盘扩展不另带：它只作为 App 的内嵌扩展安装，没有许可页。`project.pbxproj` 只加 XcodeGen 为这个文件生成的条目（文件引用、构建文件、资源阶段一行，以及 `path = ../..` 的 `msime` 组）；组名取仓库目录名，所以在名为 `msime` 的检出里生成。关于页 `LicenseNotice` 第一项是「水杉输入法 · 本应用与拼音词库 · GPL-3.0」，`fileExtension` 为空字符串，`Bundle.url(forResource:withExtension:)` 按没有扩展名查找；页脚改说产品按 GPL-3.0 发布、全文见这一项。
- **HarmonyOS**：`stage-resources.sh` 把 `LICENSE` 复制成 `resfile/licenses/GPL-3.0.txt`，与 macOS 包和引擎 C 库数据包里的名字相同。
- **MPL-2.0**：三个移动端早已分别由 Android `build-native.sh`、iOS `project.yml` 与关于页、HarmonyOS `stage-resources.sh` 带着 `resources/licenses/MPL-2.0.txt`，`scripts/test-engine-license.py` 固定，这次不用补。
- **契约**：`scripts/test-language-data-notices.py` 的 `MOBILE_LICENCE_CHANNELS` 按渠道写出 GPL-3.0 与 MPL-2.0 全文必须出现的非注释片段（Android 两条打包路径与关于页，iOS 工程、配置测试与关于页，HarmonyOS 暂存脚本），并核对根目录 `LICENSE` 仍是 GPL-3.0 全文、`MPL-2.0.txt` 仍是 MPL-2.0 全文。`project.yml` 的 `- path:` 片段要求整行相等。iOS 的 `ProjectConfigurationTests.py` 另有一条测试核对资源条目、生成的工程、关于页第一项和键盘扩展不带第二份。

## 各端的全文与入口

| 平台 | GPL-3.0 全文 | MPL-2.0 全文 | 应用里怎么看到 |
| --- | --- | --- | --- |
| Android | `assets/client-LICENSE.txt` | `assets/native-notices/<abi>/MPL-2.0.txt` | 关于页「开源许可」列出 APK 里的通知文件，GPL 一项以应用名显示 |
| iOS | App bundle 的 `LICENSE` | App bundle 的 `MPL-2.0.txt` | 关于页「开源许可」，GPL 排第一项 |
| HarmonyOS | `resfile/licenses/GPL-3.0.txt` | `resfile/licenses/MPL-2.0.txt` | 无：「关于 › 法律信息 › 开源许可」打开 GitHub 上的 `LICENSE`，设置页不列 `resfile/licenses/` 里的任何文件 |

## Alternatives considered

- **在 `resources/licenses/` 放一份 `GPL-3.0.txt` 副本给 iOS 用** — bundle 里的名字与 macOS、HarmonyOS 一致，关于页不用处理空扩展名。但仓库里会有两份 GPL 全文，要另加一条逐字节一致的检查；直接引用根目录的 `LICENSE` 只有一个来源。
- **键盘扩展也带一份** — 扩展的 bundle 自己也满足「随目标代码附全文」。但扩展从不脱离 App 单独分发，也没有界面能打开它，多一份只占键盘扩展受限的内存与包体。
- **这次就给 HarmonyOS 做应用内的许可列表** — 能让 `resfile/licenses/` 的每份文件都在应用里看得到。但这需要新的桥接调用（列出并读取 `resourceDir/licenses`）和共享设置页的新界面，牵涉 ArkTS 与 `packages/ui`，超出「随包分发全文」这个决定；GPL 全文本身已能经「开源许可」一项在 GitHub 上看到，随包副本满足随附义务。

## Consequences

- **收益**：六个平台的安装包都带着产品的 GPL-3.0 全文，三个移动端漏掉 GPL 或 MPL 全文时契约测试失败；iOS 用户在关于页第一项就能读到全文。
- **代价与已知上限**：HarmonyOS 的随包许可证（包括这次的 GPL-3.0.txt 和此前的二十多份）在应用里都没有入口，只在 HAP 里；要补需要单独做一个列表页。iOS bundle 根目录多一个没有扩展名的 `LICENSE`，将来若有依赖也往 App 根目录放同名资源，Xcode 会报重复产物。这次没有在本机跑 Xcode 构建、HarmonyOS 暂存与打包，交给 CI 与发版前的设备验收。CC BY-SA 4.0、CC BY 4.0 与 GPL-3.0 数据合在一起经应用商店分发的关系仍未经法务审阅。
