# Agent Note: 词库里的 MIT 与 Apache-2.0 数据随各平台通知渠道附上全文

Status: implemented

## Problem

六个平台分发的 `msime-pinyin.db`、`msime-english.db`、`msime-others.db`、`msime-wubi.db` 里有几份宽松许可的数据：单字读音按 mozillazg/pinyin-data（MIT）校正，中英释义取自 skywind3000/ECDICT（MIT），颜文字取自 aoguai/rime_kaomoji_dict（MIT），表情与颜文字关键词的整词读音是 mozillazg/python-pinyin（MIT）的输出，86 五笔码表取自 KyleBing/rime-wubi86-jidian（Apache-2.0）。MIT 要求每份副本附上版权与许可声明，Apache-2.0 第 4(a) 条要求附上许可证副本；各平台的通知渠道此前只通过 `msime-engine-dictionary-NOTICE.md` 写出来源名字，没有这些全文。#10240 只给引擎 C 库的数据包补上了 pinyin-data 与 ECDICT。

## Decision

- `resources/licenses/` 里每个上游一份逐字的许可证文件：`pinyin-data-MIT.txt`、`ECDICT-MIT.txt`（#10240 已有）、`rime_kaomoji_dict-MIT.txt`、`python-pinyin-MIT.txt`、`rime-wubi86-jidian-Apache-2.0.txt`。来源提交写在 `docs/third-party.md`「词库里要随附全文的 MIT 与 Apache-2.0 数据」一节，同一节列出每份数据在 `crates/dict-builder` 里被读进哪个产物。
- 五份文本走与粤语、注音、笔画数据相同的全部渠道：Windows `Collect-Notices.ps1`，macOS `Resources/Licenses`，Linux 安装的声明，Android `assets/native-notices`，iOS App 资源和关于页，HarmonyOS `resfile/licenses`。词库在每个平台都分发（五笔库在部分平台按需下载），统一一份渠道清单检查起来最简单。网页引擎只分发拼音库和五笔 86 库，`web-engine-NOTICE.md` 只收 pinyin-data 与 rime-wubi86-jidian。
- rime-wubi86-jidian 在 2026-10-29（提交 `33a1f94`）把仓库许可从 Apache-2.0 改成 MIT。msime-dictionary 的码表是在那之前取用的，随附的是当时生效的 `LICENSE`（提交 `063377a`），这样不依赖对改许可是否追溯旧数据的解读；Apache-2.0 的义务包含 MIT 的义务。
- python-pinyin 进入产物的只是 `resources/dictionary-sources/pinyin-overrides.txt` 的 60 条常用词读音，读音本身是事实，未必构成可主张版权的部分。来源文件写明这是 python-pinyin 的输出，所以仍按保守做法随附 MIT 全文。
- `scripts/test-language-data-notices.py` 的 `DATA_LICENCES` 检查每份文本的版权行或条款段落，以及它在每个渠道（含 Windows 的 `Notices.md`、iOS 关于页、网页引擎 NOTICE 与生成脚本、C 库数据包）都还在；ECDICT 的提交从 `resources/dictionary-sources.lock.json` 取，两份总览和清单必须写出同一个提交。

## Alternatives considered

- **只靠 `msime-engine-dictionary-NOTICE.md` 列出来源** — 改动最小，而且那份文件已经随每个平台分发。但它只写名字和许可证类型，没有版权行和许可正文，不满足 MIT 与 Apache-2.0 的随附条件。
- **五笔码表借用 `resources/helpcodes/NOTICE-wubi86.md` 末尾的 Apache-2.0 全文** — 那份说明已经随每个平台分发，正文完全相同，不必再存一份。但它说明的是辅助码表 `wubi86_helpcode.txt`，`msime-wubi.db` 的码表借它的全文，读声明的人对不上；以后辅助码表退出，码表的许可证也会跟着消失。单独一份文件可以被契约测试按名字检查。
- **rime-wubi86-jidian 改按现行的 MIT 附上** — 文本更短，与上游现状一致。但码表取自改许可之前，按取用时的条款履行不需要判断改许可是否覆盖旧版本。

## Consequences

- **收益**：六个平台和网页引擎的声明里都有这五份全文；渠道里漏掉任何一份，`test-language-data-notices.py` 就失败。
- **代价与已知上限**：Windows、macOS、Linux 的 CMake 和 iOS 工程文件只在 CI 的平台构建里真正打包，本地只跑了契约脚本和 iOS 工程配置测试。rime-wanxiang（CC BY 4.0）给拼音库部分词条提供的权重与字频不属于这次的 MIT 与 Apache-2.0 范围，它的署名和 `msime-engine-dictionary-NOTICE.md` 副本的同步见[词库里改编自 rime-wanxiang 的权重重新署名](2026-10-29-rime-wanxiang-weights-attribution.md)。新的宽松许可数据进入随包词库时，在 `DATA_LICENCES` 加一项并接上全部渠道。

## Verification

`python3 scripts/test-language-data-notices.py`、`python3 -m unittest platforms/ios/tests/settings/ProjectConfigurationTests.py`、`scripts/web-engine-notice.sh --check`。
