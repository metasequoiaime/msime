# 开源发布清单

这份清单描述仓库可以公开发布的源代码范围，以及生成二进制或平台安装包时要走的检查。每次发布源码或产物前按这几节逐条过一遍。

## 源码树检查

- 只提交源代码、锁文件、构建脚本、测试夹具和必要的许可证/通知文件；`target/`、`node_modules/`、Xcode `Pods/`、HarmonyOS `entry/libs/`、本机配置和临时报告由 `.gitignore` 排除。
- 不提交真实输入、账号资料、访问令牌、私钥、证书、签名配置或包含私人路径的运行时 JSON。测试使用合成值，并在日志和诊断中去除输入正文。
- 输入引擎是仓库内的 `crates/engine`，构建时不再从外部拉取引擎源码。旧检出里残留的 `vendor/` 目录已无用处，仍被 `.gitignore` 排除。
- 提交前检查 `git status --short`、`git diff --check`、冲突标记和敏感文件列表；只暂存明确路径。
- 改动涉及出网路径时，同步核对[网络请求与数据流向](../PRIVACY.md)：新增或改变了发送内容、目的地、默认开关的，必须在那份文档里同时更新，并确认与 <https://msime.app/privacy/> 的隐私政策不冲突。

## 许可证与通知

根目录 `LICENSE` 是 GPL-3.0-only。Rust workspace、Linux 元数据和共享客户端默认使用同一许可证，但上游代码、Gradle、ML Kit、Engine 资源、词库、模型和系统 SDK 仍以各自许可证和通知为准。

用了谁、各自什么许可、通知文件在哪，汇总在[第三方组件清单](third-party.md)；下面几条是它没有覆盖的发布动作。

- Android Gradle 模板的 Apache-2.0 文本在 `apps/desktop/src-tauri/gen/android/gradle/LICENSE-2.0.txt`，来源说明在同目录 `NOTICE.md`。
- iOS ML Kit 依赖通知在 `platforms/ios/SharedResources/MLKit-NOTICES.txt` 和 `MLKit-Dependencies.txt`。
- Windows 依赖通知由 `platforms/windows/Collect-Notices.ps1` 生成；使用说明和限制见 `platforms/windows/Notices.md`。生成器不是完整许可证审计，不能用空通知文件代替上游材料。
- 引擎移植所源自的 C++ MSIME-Engine、每个固定词库和离线模型都必须在发布包中保留对应的版权、许可证和来源说明。资源锁文件只校验内容，不授予额外分发权。
- 日文词库 `dict_japanese.dat` 的许可证要求是硬性的：IPAdic 与 ICOT 的条款都规定许可证文本必须随词库分发，所以发布物中必须包含 `mozc_dictionary_oss_README.txt`。逐条构成见[第三方组件清单](third-party.md#日文词库的分发义务)。
- 自带的加加辅助码表 `resources/helpcodes/jiajia_helpcode.txt` 是这份清单里唯一一项带明确再分发限制的随包数据：它在 `crates/engine/src/assets.rs` 的 `HELPCODES` 里登记为第六套方案 `jiajia`，部分条目来自商业软件拼音加加安装包内的数据表，**本仓库的 GPL-3.0 不覆盖它的内容**。每次发布前单独确认它在目标渠道是否可接受；不可接受时从 `HELPCODES` 去掉 `jiajia` 那一行，并让该渠道的资源打包不再带上这张表，设置页会随之少一个选项，其余五套辅助码表不受影响。逐条依据见 [`resources/helpcodes/NOTICE.md`](../resources/helpcodes/NOTICE.md) 与[第三方组件清单](third-party.md#自带辅助码表resourceshelpcodes)。
- 非英语候选词的离线释义 `offline-glosses/zh-<lang>.db` 改编自英文维基词典，按 CC BY-SA 4.0 提供。随这些文件发布时必须带上生成器同时写出的 `offline-glosses-NOTICE.txt`（署名、dump 日期、Wiktextract 版本和改动说明），每行的来源英文页记在 `source` 列。发布前的准备和限制见[第三方组件清单](third-party.md#非英语离线释义resourcesoffline-glosseslockjson)。
- 整句重排模型的训练语料署名嵌在 `sentence-model.safetensors` 的 safetensors `__metadata__` 头里。原样分发该文件即满足要求；重新导出、量化或转换权重时必须把 `attribution` 字段带过去，见[第三方组件清单](third-party.md#整句重排模型的署名要求)。
- 引入新的上游代码、字体、图标、模型或服务 SDK 时，同时提交来源提交、许可证文本、通知位置和分发限制；不要只在 README 写一个链接。

## 验证

合并前执行 `bash scripts/verify-local.sh --quick`；发布前执行完整版，并把失败项与 `scripts/known-failures.txt` 对照。Rust 改动还需测试、fmt、clippy，UI 改动还需类型检查和构建。Pull Request 的 GitHub Actions 质量（workflow 校验）、依赖审查、macOS 和 iOS 检查也必须通过；改到某个原生平台或共享层时，Native Platform CI 的对应 job 同样必须通过。仓库文本契约（`scripts/` 下的 `test-*.py`）由 `scripts/run-checks.sh` 执行，`Core CI` 的 contracts job 在 Linux 上跑它，`scripts/verify-local.sh` 在本地跑同一份脚本；缺少所需工具或参考仓库的检查在 CI 上打印 skipped 并通过，所以提交前仍要自己跑过 `verify-local.sh --quick`。

平台产物的验证范围写清楚做的是哪一层：共享层单元测试、跨目标或容器构建、模拟器运行、真实设备与系统输入入口、安装与签名。发布记录里按平台注明本次实际走到哪一层，读的人不用去猜。

## 发布前人工确认

六个平台各有独立的手动发布工作流（`.github/workflows/release-<平台>.yml`），版本号默认取自 `platforms/<平台>/version.txt`，一个平台发版不牵动其余五个。触发前按下面五条逐项确认。

发布说明由 `scripts/generate-release-notes.py` 按平台 tag 和实际改动路径筛选，只传入目标平台与纯共享改动；配置仓库 Secret `EVERYAPI_RELEASE_NOTES_TOKEN` 后，会用 EveryAPI 的模型生成中文 Markdown，模型不可用时自动使用同一筛选结果生成确定性说明。可选的仓库变量 `EVERYAPI_RELEASE_NOTES_MODEL` 用来指定模型，默认是 `gpt-5.5`。

1. 确认远端默认分支、待发布平台的 `version.txt` 和变更日志与待发布提交一致。
2. 重新检查 `README.md`、各平台 README、`docs/implementation.md` 和本清单中的路径、命令与当前目录一致。
3. 生成对应平台的第三方通知和资源许可汇总，确认不包含本机绝对路径、凭据或未授权模型/词库。
4. 对每个平台分别记录构建工具版本、签名状态、安装方式和验证过的设备与编辑器范围；开发签名、测试账号和测试资源不得进入正式发布物。
5. 发布源代码时附带 `LICENSE`、第三方通知和固定上游来源；发布二进制时同时提供对应的源代码、许可证和重新构建说明。

安全问题通过 [安全策略](../SECURITY.md) 的私下报告入口提交，不要在公开 issue、PR 或发布附件中披露秘密或真实输入。
