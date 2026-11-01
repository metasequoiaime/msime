# Agent Note: macOS 语音运行库与五笔码表改为按需下载

Status: implemented

## Problem

0.54.0 的 universal `MSIME.app` 约 300 MiB。其中两块只有一部分用户用得到：输入法 bundle 里的 sherpa-onnx 运行库 `Contents/Frameworks/libsherpa-onnx-c-api.dylib` 约 40 MB，只有选了本地语音识别的人会加载它；`EngineResources/msime-wubi.db` 约 14 MB，只有五笔用户会读它。发布包早已把日文词典、粤拼/注音/笔画词库和手写模型改成设置应用按需下载到 `<state_root>/resource-packs/<id>/`，本地语音模型也是按需下载。维护者要求把语音运行库和五笔码表也挪过去。

运行库不能照搬 Android 的做法。Android 的 `voice-runtime` 资源包直接从上游 `.aar` 里取库，而 macOS 的输入法和 `msime-voice-local` 都开着 hardened runtime，库验证只加载同一 Team ID 签名的库。上游原样的 dylib 下载下来，`dlopen` 会被拒绝。

五笔码表也不能只从包里删掉。准备工作词库代次时，`msime-wubi.db` 从资源目录并进代次的 `msime-pinyin.db`，日志回放把五笔学习写进去。码表不在时，日志里的五笔行会让整个代次回放失败，这样连拼音也起不来。五笔方案也会在没有码表的情况下照常运行，只是打不出字。

## Decision

### 语音运行库：资源包 `voice-runtime-macos`

- **签名与发布**：`.github/workflows/publish-macos-voice-runtime.yml` 手动触发，调用 `platforms/macos/scripts/sign-voice-runtime.sh`。流程是：
  - 按 `resources/voice-runtime.lock.json` 取上游 universal dylib 并校验；
  - `strip -x`，与以前随包的那份一样；
  - 用发版同一套 Developer ID 证书签名，带 hardened runtime 和安全时间戳；
  - 核对 Team ID 与锁文件的 `team_id` 一致；
  - 打成只含这一个文件的 ZIP，有凭据时提交公证；
  - 勾选 `publish` 时上传为本仓库 Release `voice-runtime-macos-v<sherpa 版本>` 的资产，标为 prerelease、不设为 latest。

  签名带时间戳，每签一次字节都不同，所以每个 sherpa-onnx 版本只签、只发一次。同一标签下已有归档时 workflow 拒绝替换：已经发出去的 App 钉的是第一次的摘要。标签不以 `macos-v` 开头，因为发布说明的起点查找（`git tag --list 'macos-v*'`）和更新检查都按这个前缀认 App 发布。
- **固定**：`resources/voice-runtime-macos.lock.json` 记录来源提交、上游平台、标签、Team ID、归档和其中 dylib 的长度与 SHA-256。`client-providers` 校验来源提交与上游锁一致、归档地址正好是该标签下的资产、归档里只取 `libsherpa-onnx-c-api.dylib`。安装复用 Android 运行库已经在用的 `install_archive_members`：先校验归档摘要，再校验取出的 dylib 摘要，然后原子发布。
- **没固定时资源包不存在**：锁文件的 `archive` 为 `null` 时，`ResourcePack::pinned` 为假，`from_id` 认不出它，`list` 不列出，安装、导入、收编都报 `local_model_unknown`。合并本改动时锁文件就是这个状态，发布包照旧内置运行库。
- **打包**：只有两个条件都满足时，`package-release.sh` 才以 `-DMSIME_BUNDLE_VOICE_RUNTIME=OFF` 构建：锁文件固定了归档，而且这次签名身份的 Team ID 等于锁文件的 `team_id`。这时运行库不进 bundle。打包脚本在编译前用同一个安装器下载这份运行库，核对签名、Team ID 和两种架构。`check_app` 与 CTest `bundle-contents --without-voice-runtime` 断言 bundle 里没有运行库。没有签名身份（ad-hoc、fork、CI 的未签名包）或身份属于别的团队时照旧内置运行库，否则装出来的输入法加载不了下载的库。开发构建默认 ON。
- **加载**：`MSIMELocalVoiceRuntimePath` 按顺序判断：
  - bundle 里有运行库时返回空串，辅助进程照旧在 `../Frameworks` 找；
  - 否则问 host-api 的 `msime_client_resource_packs`，`voice-runtime-macos` 是 `installed` 且文件是普通文件时，把路径作为 `--runtime` 交给 `msime-voice-local`（`set_sherpa_library_path` 早已存在，Android 在用）；
  - 都没有时，开始识别就报「本地语音识别运行库还没有下载」。
- **下载时机**：
  - 设置页在用户刚把识别服务切到本地、或在本地识别下刚选用模型时 `ensure`，并在模型列表上方显示下载行；
  - 设置应用启动时，已保存的识别服务是本地且选用了模型（`asr_model_path` 非空），`ensure_saved_packs` 补下。出厂服务改为本地后加了模型这个条件，免得每个新用户一启动就下载，见[六端出厂识别服务改为本地模型](2026-10-26-voice-local-default.md)；
  - `offered_by` 只在 macOS、且 `~/Library/Input Methods` 里的输入法没有自带运行库时列出它。

### 五笔码表：资源包 `wubi`

- **发货规则按组**：`on_demand_artifacts("macos")` 改为文件组的列表，`[日文词典三件套, msime-wubi.db]`；iOS 仍是 `[日文词典三件套]`，Android 后来也加上五笔码表，见 [Android 五笔码表按需下载](2026-10-28-android-on-demand-wubi.md)。`ResourceSet::as_shipped_in_groups` 逐组应用「整组缺席才去掉」。`verify_resources` 加了 `--platform <os>`，不给时只有日文组，iOS 的暂存脚本这样调用；Android 的 `build-apk.sh` 给 `--platform android`。运行时校验只容许五笔码表缺席。打包时，默认方案是五笔的版本（水杉五笔）由 `stage-resources.sh` 把它加回来，`check_app` 对五笔版断言它在、对其余版本断言它不在。拼音版以前也带着它，它没有五笔方案，现在一并省掉。
- **资源包**：`ResourcePack::Wubi` 是 `desktop-dictionary.lock.json` 里 `msime-wubi.db` 那一项，`schemes = ["wubi"]`。设置页选五笔时下载（`resourcePackForScheme`）。设置应用启动时，当前方案或上一次的中文方案是五笔，就补下。`offered_by` 只在 macOS、本版本有五笔方案、而 HostOptions 的资源目录里没有它时列出。词库自动更新只给本机已有的五笔码表下载新版本：在资源目录里，或 `wubi` 资源包已装。
- **「资源目录里有码表」只认随包的那份**：判断交给 host-api 的 `packaged_wubi_dictionary`，资源目录在 `<preferences_directory>/dictionary-updates/` 下（词库更新下载的代次）时不算随包，即使代次里有 `msime-wubi.db`。更新只给已有的码表下新版本，资源目录又是上一个代次，所以代次里的码表可以一路沿袭自不带码表之前的发布包；而代次绑在下载它的那份词库锁上，应用升级换锁后 `dictionary_updates::selected` 不再选用它，会话回到不带码表的发布包。只有资源包能跨过换锁。设置应用读按需资源的文档在 macOS 上取 `RuntimeOptionsState`（「准备词库」重新准备后会更新它，`DictionaryHostOptions` 只是启动时的快照），`repair_macos_oobe_resources` 成功后再跑一次 `ensure_saved_packs`。Linux 的 `packaged_language_dictionaries` 按同样的规则不把语言词库的更新代次当作随包带齐。
- **找码表**：host-api 的 `wubi_dictionary` 先看资源目录（随包或词库更新目录），有就不另给路径；没有时用词库更新里的那份或资源包里的那份，交给新增的 `EngineOptions::wubi_dictionary`。聚焦时 `refresh_resource_packs` 发现它变了，就在输入空闲时重建 Engine，和其他资源包相同。
- **缺码表时回退，不让输入法坏掉**：
  - 本平台可以不带码表、资源目录里没有、也没有下载的那份时，`wubi_dictionary_missing` 为真；
  - `LanguageDictionaries::serve(Wubi)` 随之为假，`effective_scheme` 像缺词库的粤拼一样回退到上一次的中文方案，再不行就用本版本的默认方案；
  - 输入法的 `MSIMEInputSchemeAvailable` 用同一规则，菜单不列出五笔。
- **日志不阻断代次**：回放时代次里一张五笔表都没有，五笔行计入 `skipped` 而不是 `failed`，和没有拼音库的代次跳过非英文行一样。只要有一张五笔表就照旧逐行写，缺另一张仍按失败回滚。
- **码表到了再并入**：`session_options` 建会话时，如果 `wubi_dictionary` 非空、工作主词库里还没有五笔表，就调用 `merge_downloaded_wubi`。它把下载的表并进来，再回放一遍日志补上先前跳过的五笔行，然后补反查索引。已有五笔表时只读一次 schema。并不进来只记日志，不让会话失败。

## Alternatives considered

- **工作流每次发版时重签运行库，并把摘要在构建时注入 App**：不用提交锁文件，第一次发版也不需要手工步骤。没有采用：每次发版的字节都不同，已装的运行库每升级一次就过期，要重新下载约 16 MB。摘要也不再来自评审过的锁文件，开发构建拿不到它。
- **只验证签名、不钉摘要**：下载后用 `SecStaticCodeCheckValidity` 核对 Team ID，换版本不必改锁文件。没有采用：资源包的完整性一律按编译进来的 SHA-256 判断，镜像也是靠这一点才可信。再加一套签名信任链，就多一个要维护的入口。库验证在加载时本来就会核对 Team ID。
- **给输入法和辅助进程加 `com.apple.security.cs.disable-library-validation`**：最省事，上游 dylib 原样可用。被明确排除：它让任何能写进资源包目录的人把代码注入一个读取所有键入内容的进程。
- **把重签的运行库挂在词库仓库 `msime-dictionary` 的 Release 上**：项目镜像 `dl.msime.app` 的放行名单已经包含它，国内下载更快。没有采用：签名和公证要用本仓库发版的那套长期凭据，跨仓库发布要另一个有写权限的 token。运行库也不是词库数据。代价是放行名单要另补本仓库的这个标签，见 Consequences。
- **按会话此刻的解析判断五笔码表在不在（资源目录、已选中的词库更新、资源包都算）**：和 host-api 的 `wubi_dictionary` 完全同一套规则，资源目录是有效的更新代次时也不会多下一份资源包。没有采用：这一刻在不等于换锁之后还在。2026-11-01 一位全功能版五笔用户从自带码表的本地构建升级到不带码表的 0.56.0，词库锁从 dict-v2.0.24 换到 dict-v2.0.41，`installed.json` 里的更新（带 `msime-wubi.db` 的 dict-v2.0.46）因 `baseline_generation` 不符整份失效，会话回到发布包，五笔回退到全拼，直到设置应用重启后才下载 `wubi` 资源包。代价是资源目录为更新代次、而资源包还没装的用户（自带码表的开发构建或旧版本升级上来）会多下载一次约 95 MB 的码表。
- **五笔码表留在包里**：五笔用户最省心，也不用改引擎。没有采用：维护者明确要求，而且拼音版和全功能版的大多数用户用不到这 14 MB。代价是升级窗口，见下文。
- **输入法发现缺码表时自己下载**：升级后五笔用户不用打开设置应用。没有采用：`InputModeIdentifiers.h` 写明输入法自身从不下载，联网只在设置应用里进行；日文和语言词库也是这个规则。
- **把五笔码表的位置放进 `RuntimePaths`，在每条准备代次的路径上合并**：重置学习数据、词库快照和个人词库后台准备的代次也会立刻带上五笔表。没有采用：`RuntimePaths` 有三十多处结构体字面量。这些路径产生的代次缺表时，回放已经不会失败，下一次建会话就会并进来。

## Consequences

- **体积**：
  - universal 发布包里输入法 bundle 少约 40 MB（剥离后的 universal dylib 41,800,064 字节）。用户选本地识别时下载约 16.5 MB 的 ZIP。开发构建（只有 arm64、未剥离）实测 bundle 从 201 MB 降到 176 MB。
  - 全功能版和拼音版的 `EngineResources` 少 14.5 MB（`msime-wubi.db` 14,536,704 字节），按 `MSIME_MACOS_OMIT_ON_DEMAND=1` 暂存实测从约 120,420 KiB 降到 106,224 KiB；五笔版仍是 116,036 KiB（它本来就不带整句模型）。
  - 解压后的 `MSIME.app` 合计约少 55 MB，从约 300 MiB 降到约 245 MiB。前提是锁文件已固定运行库；固定之前只少五笔那 14.5 MB。
  - DMG 用 LZMA 压缩，少得比解压后小。
- **第一次发版要手工走一步**：合并后锁文件是 `null`，发布包照旧内置运行库。维护者手动运行 Publish macOS voice runtime（勾选 `publish`），把运行摘要里的 `voice-runtime-macos.lock.json` 经普通 PR 提交到 `resources/`，之后的 macOS 发布才省掉运行库。以后 sherpa-onnx 升级时，先把锁文件的 `release_tag` 改成新版本并清空 `archive` 和 `artifacts`，再重复这一步。
- **镜像**：`dl.msime.app` 的放行名单目前只有 `msime-dictionary`、`chinese-ime-lm`、sherpa-onnx 和 `msime-engine`。本仓库 `voice-runtime-macos-v*` 的资产在名单之外，镜像答 403 后按换源规则落到 GitHub 原地址，国内下载会慢。名单在 msime-web 仓库，另行补上。
- **升级窗口**：
  - 用全功能版五笔的人，输入法经 Sparkle 单独更新到不带码表的版本后，打开设置应用之前五笔不可选，方案回退到上一次的中文方案或全拼。设置应用下次启动时自动补下码表，补上后下一次聚焦就恢复五笔。升级前 HostOptions 指着带码表的词库更新代次时，以前设置应用会把那份码表当成随包、不提供也不补下资源包，见上文「只认随包的那份」。
  - 用本地识别的人同理：打开设置应用之前，开始识别会提示去语音设置下载运行库。
  - 这和以前日文、粤拼、注音、笔画改为按需下载时的处理相同。水杉五笔不受影响，它照旧随包带着码表。
- **离线安装**：没有网络时，五笔和本地识别要用设置页的离线导入（资源包都支持 `import`）或等联网后下载。
- **验证**：
  - 单元测试覆盖：资源包锁的解析与拒绝条件；下载器对归档摘要和 dylib 摘要的校验（篡改归档、钉错 dylib 摘要都不发布）；按组发货；回放跳过五笔行；码表到达后并入并补写；host-api 的码表查找与回退；设置应用的提供规则和启动补齐；输入法的五笔可用性和运行库路径。
  - macOS CMake 在 `MSIME_BUNDLE_VOICE_RUNTIME` 为 ON 和 OFF 时都构建通过，`bundle-contents`、`local-voice-request` 和 `backend-library-load` 都通过。
  - `MSIME_MACOS_OMIT_ON_DEMAND=1` 暂存后，全功能版、拼音版和日文版的 `EngineResources` 没有 `msime-wubi.db`，五笔版仍有，并按 `--platform macos` 规则校验通过。
  - `wubi` 资源包能从锁文件地址下载并通过校验。
  - 签名脚本在本机用维护者的 Developer ID 跑过一次（只签名，没有公证，也没有发布）：剥离后的 universal dylib 41,800,064 字节，ZIP 16,466,276 字节。把这份 dylib 以 `--runtime` 交给 `~/Library/Input Methods` 里已装、开着 hardened runtime 的 `msime-voice-local`，握手报告 `available: true`。同一个辅助进程加载上游原样（ad-hoc 签名）的 dylib 时，库验证拒绝，`dlopen` 报 code signature 错误。重签的必要性和 `--runtime` 这条加载路径都由此得到确认。
  - 发布工作流本身（证书导入、公证、上传 Release）在合并前没有跑过，第一次手动运行就是它的验收。装好资源包后由输入法发起一次真实的本地识别，要等运行库发布、锁文件固定之后才能做。
- **其他平台**：
  - Windows 的安装包同样带着 sherpa 运行库和 `msime-wubi.db`。五笔码表可以复用这里的机制：把 `"windows"` 加进 `on_demand_artifacts`，让 `Prepare-PackageFiles.ps1` 省掉它，`offered_by` 加 Windows 分支。这需要在 Windows 上验收，留作后续。Windows 运行库要先让 `resource_packs` 支持按平台的 `tar.bz2` 成员锁，Server 也要改为从用户可写的数据目录加载不签名的 DLL，这是一个单独的安全决定。
  - Linux 发布包本来就不带词库，用户用 `msime-linux-setup --download` 取回，五笔码表无需再拆。运行库随 deb/rpm 和各发行版的打包分发，x86_64 归档约 9 MB。改成按需下载同样要先支持 `tar.bz2` 成员锁，NixOS 还加载不了上游原样的 `.so`，所以留作后续。

## Related notes audit

- [iOS 语言词库按需下载](2026-10-13-ios-language-packs.md)：同一套资源包与 `on_demand_artifacts` 规则。本篇把规则改成按组，iOS 的取值不变。部分重叠，互链。
- [Android 本地语音模型](2026-10-12-android-local-voice-models.md)、[Android 离线语音导入](2026-10-12-android-offline-voice-import.md)：Android 的 `voice-runtime` 资源包直接取上游 `.aar`。本篇说明 macOS 为什么不能这样做，另立 `voice-runtime-macos`。部分重叠，Android 的决定不变。
- [本地语音模型导入](2026-10-09-local-voice-model-import.md)：模型导入与本篇无关，运行库同样可以经资源包的 `import` 离线安装。
- [Android 五笔码表按需下载](2026-10-28-android-on-demand-wubi.md)：把本篇的五笔码表规则搬到 Android 的 full 和拼音版瘦包，沿用资源包 `wubi`、host-api 的回退和代次补写；升级收编与启动补齐按 Android 的方式另做。部分重叠，互链。
- [五笔反查](2026-10-11-wubi-reverse-lookup.md)：反查编码只在五笔方案里显示，查码组只在五笔设置显示时出现，这是拼音用户用不到码表的依据。决定不变。
