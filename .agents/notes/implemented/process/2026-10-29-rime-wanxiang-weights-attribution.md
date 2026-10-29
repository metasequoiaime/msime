# Agent Note: 词库里改编自 rime-wanxiang 的权重重新署名

Status: implemented

## Problem

msime-dictionary 在 #84（提交 `060c102515f176ca3dfaf2a2cba1f320901ef414`，`dict-v2.0.30` 起发布）把 `single-chars.txt`、`rime-ice.txt`、`rime-ice-supplement.txt` 里与万象共有的单字和词语的权重，改成 amzxyz/rime-wanxiang `v18.0.16`（提交 `7ab73ee0c7f80190004233e70f954833df64c80e`）的权重乘以每份文件一个系数。rime-wanxiang 是 CC BY 4.0，第 3(a) 条要求分发时署名、给出许可证、写明改动。[符号模式分组表改由 Unicode 字符数据库重建](../feature/2026-10-29-symbol-groups-from-unicode.md)以为 `resources/licenses/rime-wanxiang-CC-BY-4.0.txt` 只为分组表存在，删掉了它和十二个渠道里的引用，之后没有任何通知渠道署名 rime-wanxiang，而 `dict-v2.0.39` 的 `msime-pinyin.db` 与 `msime-stroke.db` 仍在每个平台上分发这些权重。各平台带的 `msime-engine-dictionary-NOTICE.md` 也停在 2026 年 9 月前后的 msime-dictionary `NOTICE.md`，没有这次改编的记录。

## Decision

- 用发布的产物核对哪些文件真的带了万象的数据，只署名这些：`msime-pinyin.db`（「我们」13696325 = round(1488731 × 9.2)，「象」51951 = round(5515 × 9.42)，万象权重取自上游固定提交的 `jichu`、`zi`），以及 `msime-stroke.db`（`stroke.rs` 把一个字各读音的权重相加，「万」2702880 = 2701552 + 1328）。网页引擎裁出的拼音库和 C 库数据包的拼音库原样保留权重，也算在内。
- `resources/licenses/rime-wanxiang-CC-BY-4.0.txt` 按权重的用途重写：上游、发布与提交，作者 amzxyz（上游 README 与 LICENSE 都没有给出版权声明，文件照实写明），用到的十五份词库，进入的两个产物，逐条改动（去调、取最高、乘系数取整、不加词、笔画库按字求和），不背书声明，然后是上游 `LICENSE` 逐字的 CC BY 4.0 全文。格式与 `kfcd-chaizi-CC-BY-3.0.txt` 相同。
- 文件走 [词库里的 MIT 与 Apache-2.0 数据](2026-10-29-dictionary-data-permissive-licence-texts.md)接好的全部渠道：Windows `Collect-Notices.ps1`、测试与 `Notices.md`，macOS `Resources/Licenses`、总览与包检查，Linux 安装的声明与总览，Android `assets/native-notices`，iOS App 资源、工程文件与关于页，HarmonyOS `resfile/licenses`，以及网页引擎的 `web-engine-NOTICE.md`（生成脚本的 `repo` 段与手写的词库表）和 C 库数据包的 `licenses/` 与 README。`scripts/test-language-data-notices.py` 把它放进 `DATA_LICENCES`，另要求三份总览和 `docs/third-party.md` 写出上游提交。
- `msime-engine-dictionary-NOTICE.md` 改成：开头一行写明来源，接着逐字放 msime-dictionary 提交 `75978c6a6470171cc805c74e87ef465d894a7be4`（`dict-v2.0.39` 的源数据提交，即发布的 `msime-dictionary-manifest.json` 里的 `custom_dictionary_commit`）的 `NOTICE.md`，末尾「msime 构建时另加的输入」一节保留那份文件没有、由 msime 一侧进入同一批词库的来源：颜文字、ECDICT、python-pinyin 读音、SCOWL 声明原文和中文维基百科的署名要求。契约测试核对开头写的发布等于桌面词库锁下载的发布，词库锁换了发布却没重新同步会失败。

## 不署名的几项

- `msime-wubi.db` 的 86、98 词组补充表在构建时重新派生，用 rime-ice 权重判断门槛 100 和同一编码下的先后，写进库里的是按名次从 9 递减的数，不带万象的权重。
- `msime-bigram.bin`、`msime-trigram.bin` 只用拼音库的词表切分语料；万象没有加入任何词。
- 编进引擎的 `nine_key/syllable_frequency.tsv` 是 424 个音节的权重之和，维护者已裁定按聚合统计事实处理；`docs/third-party.md` 补写了求和的权重里有改编自万象的部分。
- msime-dictionary 的 `sources/pinyin/wanxiang/` 原样文件在 `dict-v2.0.39` 里只入库，构建器不读。
- 万象 LTS 语法模型由设置应用从上游自己的发布按需下载，msime 不分发。

## Alternatives considered

- **只靠 `msime-engine-dictionary-NOTICE.md` 署名** — 同步后的副本已经有完整的改编记录，而且 macOS、Linux、Windows 和 C 库数据包都带它。但 Android、iOS、HarmonyOS 不带这份副本，那里分发的拼音库会照样缺署名；它也没有许可证全文。独立文件能按名字进契约测试。
- **副本整份换成上游 `NOTICE.md`，不加 msime 一节** — 与「逐字同步」最一致。但上游那份从来没有 n-gram 表的中文维基百科署名和 SCOWL 声明原文，Linux 总览和数据包 README 都说这些在这份副本里；整份替换会在补一个署名的同时丢掉另一个。
- **`msime-wubi.db` 也按保守做法署名** — 多一行文字，不会出错。但库里没有任何万象的数字，署名会让读声明的人以为五笔码表或权重来自万象，与「只署名确实分发的数据」相悖。

## Consequences

- **收益**：六个平台、网页引擎和 C 库数据包都带上 rime-wanxiang 的署名、改动说明与 CC BY 4.0 全文；渠道里漏掉它，或副本落后于词库锁所指的发布，契约测试就失败。
- **代价与已知上限**：`msime-stroke.db` 的 `license` 元数据仍是 `LGPL-3.0-only AND GPL-3.0-only`，没写 `CC-BY-4.0`；要改 `crates/dict-builder/src/stroke.rs` 的 `LICENSE` 并重新发布词库，这次没有动。Android、iOS、HarmonyOS 的通知渠道仍不带 `msime-engine-dictionary-NOTICE.md` 本身。各平台的 CMake、Xcode 工程与打包脚本只在 CI 的平台构建里真正打包，本地跑的是契约脚本、iOS 工程配置测试和网页引擎 NOTICE 的重新生成。CC BY 4.0 与 GPL-3.0 词库合在一起分发的关系、App Store 渠道与第 2(a)(5)(C) 条技术措施条款的关系，未经法务审阅。

## Verification

`python3 scripts/test-language-data-notices.py`、`python3 -m unittest platforms/ios/tests/settings/ProjectConfigurationTests.py`、`scripts/web-engine-notice.sh --check`（经 `rbuild`）、`pnpm run verify-notes`。
