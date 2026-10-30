# Agent Note: 语言词库的许可证表达式写全来源，移动端带上词库来源说明

Status: implemented

## Problem

[词库里改编自 rime-wanxiang 的权重重新署名](2026-10-29-rime-wanxiang-weights-attribution.md)留下两处缺口。

一是 `msime-stroke.db` 的 `license` 元数据只写 `LGPL-3.0-only AND GPL-3.0-only`，可它的权重是 `single-chars.txt` 一个字各读音的字频之和，`dict-v2.0.30` 起其中与万象共有的读音改编自 rime-wanxiang（CC BY 4.0，「万」2702880 = 2701552 + 1328）；随库分发的 `msime-rime_stroke_LICENSE.txt`（由 `resources/licenses/rime-stroke-LGPL-3.0.txt` 复制）也只署名 rime-ice。按同样的标准核对另外两个语言词库：`msime-zhuyin.db` 只写 `LGPL-2.1-or-later`，但补充表的词条源自 libtabe 的 `tsi.src`（BSD-3-Clause），McBopomofo 的改动和用来补权重的 `phrase.occ` 是 MIT；它的许可证文本带了 McBopomofo 的 MIT 原文，却只说补充表「按适用的 BSD 条款分发」，没有 BSD-3-Clause 第 2 条要求随二进制分发附上的版权、条款与免责声明。`msime-cantonese.db` 的 `CC-BY-4.0` 已经覆盖 rime-cantonese 与 HKCanCor，没有缺口。桌面词库（`msime-pinyin.db` 等）的文件里没有许可证字段，构建器也不写，不存在同类遗漏。

二是 Android、iOS、HarmonyOS 的通知渠道不带 `msime-engine-dictionary-NOTICE.md`。三端都分发 `msime-bigram.bin` 与 `msime-trigram.bin`（统计自中文维基百科，CC BY-SA 4.0 要求再分发时署名）和 `msime-wubi.db`（几份不同许可的码表），这些来源只写在这份说明里。

## Decision

- `stroke.rs` 的 `LICENSE` 改为 `LGPL-3.0-only AND GPL-3.0-only AND CC-BY-4.0`，`zhuyin.rs` 的改为 `LGPL-2.1-or-later AND BSD-3-Clause AND MIT`：表达式按进入库里的每个来源各列一项。`single-chars.txt` 的读音按 MIT 的 pinyin-data 校正，但笔画库不存读音，所以不列 MIT。两个模块各有一个测试按字面值固定表达式，并检查随库分发的许可证文本里有对应的署名。
- `rime-stroke-LGPL-3.0.txt` 加一段万象署名：作者、发布与固定提交、CC BY 4.0 地址、改动（取最高、乘 9.42 取整、按字求和）、不背书和免责声明的指引，完整署名与全文指向随各渠道分发的 `rime-wanxiang-CC-BY-4.0.txt`。语言词库资源包只有数据库和各自的许可证文本，用户按需下载笔画库时，这一段就是随数据走的署名；CC BY 4.0 第 3(a)(1)(C) 条允许用许可证地址代替全文。顺带改正同一段里「没有删去任何字」的说法：构建器丢掉基本区与扩展 A 区以外、没有字频的字。
- `libchewing-data-LGPL-2.1.txt` 逐字收入 libtabe 的 `tsi-src/COPYING`（取自 `kcwu/libtabe` 提交 `ea382a829c916f2cceda2022afdf8e8471e6e7b5`，保留原文的「Copyrighy」拼写），并写明 McBopomofo 的 MIT 同时覆盖它对补充表的修改。
- `msime-engine-dictionary-NOTICE.md` 接入 Android `build-native.sh`（进 `assets/native-notices`，关于页按目录列出）、iOS `project.yml` 与 `project.pbxproj`（只加 XcodeGen 生成的四行）、iOS 关于页（`LicenseNotice` 加 `fileExtension`，这份说明是 `md`）、HarmonyOS `stage-resources.sh`。`scripts/test-language-data-notices.py` 的 `ENGINE_NOTICE_CHANNELS` 要求九个渠道都带它，`LICENCES` 要求笔画与注音的许可证文本保留上面加的段落。

## 三个移动端分发的词库文件

| 平台 | 随包 | 按需下载的资源包 |
| --- | --- | --- |
| Android（`build-apk.sh`、`build-client-apk.sh`；`release-android.yml` 给 full 和拼音版设 `MSIME_ANDROID_OMIT_ON_DEMAND=1`） | 瘦包的 `assets/dictionary`：`msime-pinyin.db`、`msime-english.db`、`msime-scowl_Copyright.txt`、`msime-bigram.bin`、`msime-trigram.bin`、`msime-others.db`、`msime-dictionary-manifest.json`、`sentence-model.safetensors`；五笔版、日文版、越南文版、藏文版按 `resources/editions/<id>.lock.json` 全部随包，五笔版带 `msime-wubi.db`，日文版带日文词典与两份 Mozc 文本 | 日文词典组、`msime-wubi.db`、语言词库包（三个数据库各带许可证文本） |
| iOS（`stage-resources.sh`；`release-ios.yml` 设 `MSIME_IOS_OMIT_ON_DEMAND=1`） | 键盘扩展的 `EngineResources`：桌面词库锁里除日文词典组以外的全部文件，含 `msime-wubi.db`、两个 n-gram 表 | 日文词典组、语言词库包 |
| HarmonyOS（`stage-resources.sh`；`release-harmony.yml` 设 `MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1`） | `resfile/engine`：桌面词库锁的全部文件，含日文词典与 Mozc 文本；`resfile/language-dictionaries`：三个语言词库与各自的许可证文本 | 无 |

## Alternatives considered

- **给笔画库在语言词库包里另附一份 `rime-wanxiang-CC-BY-4.0.txt`** — 署名和全文都随数据走，最完整。但语言词库包的文件清单由 `resources/language-dictionaries.lock.json` 固定，三端打包脚本和 Android 的打包检查都按「数据库加一份许可证文本」的配对处理，多一个文件要动所有这些地方；而每个渠道已经带着这份全文，笔画库自己的许可证文本里写明署名和许可证地址就满足第 3(a) 条。
- **注音库的 libtabe 声明只写来源名，不收原文** — 改动小，原来就是这样。但 BSD-3-Clause 第 2 条要求二进制分发在文档里重现版权、条款与免责声明，只写名字不满足。
- **移动端只补 n-gram 表的维基百科署名，不带整份说明** — 文件更小。但五笔码表、颜文字、SCOWL 等来源同样只写在这份说明里，桌面渠道和 C 库数据包带的也是整份，分开维护会让两边漂移。

## Consequences

- **收益**：之后用这个构建器发布的笔画与注音库，文件内的 `license` 和随库的许可证文本都覆盖所有来源；六个平台都带词库来源说明，渠道里漏掉它契约测试就失败。
- **代价与已知上限**：`license` 元数据和随库的许可证文本只有在 msime-dictionary 用合入后的构建器重新发布语言词库（`release-built-dictionaries.yml`，`msime_ref` 指向含这次改动的 develop），并把 `resources/language-dictionaries.lock.json` 换到那次发布后才进入安装包。这次发布是 `dict-v2.0.41`（构建器 `09a69f20b4`，源数据与 `dict-v2.0.40` 相同），两份锁随后换到它，见[换锁笔记](../bug-fix/2026-10-27-dictionary-lock-latest.md)；`dict-v2.0.40` 的构建器还不含这次改动，它和更早发布的笔画、注音库仍是旧的表达式与许可证文本。libtabe 的 `COPYING` 取自 GitHub 上的源码镜像，不是 libtabe 的原始发布包。当时 iOS 和 HarmonyOS 不带 msime 自己的 GPL-3.0 全文（Android 带 `client-LICENSE.txt`），拼音库的 rime-ice 数据也是 GPL-3.0；维护者随后决定补上，见 [iOS 与 HarmonyOS 随包带上 GPL-3.0 全文](2026-10-29-mobile-gpl-licence-text.md)。CC BY-SA 4.0、CC BY 4.0 与 GPL-3.0 数据合在一起分发的关系未经法务审阅。
