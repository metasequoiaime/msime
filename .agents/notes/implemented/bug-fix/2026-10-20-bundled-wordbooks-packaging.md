# Agent Note: 背单词内置词书进安装包

Status: implemented

## Problem

背单词的八本内置词书（中考、高考、CET-4、CET-6、考研、雅思、托福、GRE）从 2026-09-23 功能上线（#632）起就没有进过任何安装包。`scripts/fetch_wordbooks.py` 能从 `resources/wordbook.lock.json` 锁定的 ECDICT 提交生成它们，`crates/client-services/src/vocabulary/builtin.rs` 也会在资源根的 `wordbooks/` 里读它们，但没有任何发版工作流运行这个脚本，也没有任何打包脚本把 `wordbooks/` 放进包里。上线时的验证是手工把词书暂存到资源根之后做的，所以没有暴露。

结果是每个安装上的设置页「背单词」词书列表都是空的，下拉框因为 `books.length === 0` 被禁用，只显示「未选择」；用户要先自己导入词表才能用这个功能。缺词书从来不会让构建失败：`builtin.rs`、`host.rs` 和设置应用的注释都把「宿主没暂存词书」当成正常情况，只列导入的词表。

## Decision

内置词书和英文释义表一样，作为资源目录的同级目录 `wordbooks/` 随包分发，发版时必须在包里。

- `scripts/fetch_wordbooks.py` 在八本 `<id>.json` 旁边写出 `wordbooks-NOTICE.txt`：说明来源和提交，再逐字附上锁定提交的 ECDICT `LICENSE`（MIT 要求版权与许可声明随数据分发）。`resources/wordbook.lock.json` 因此多了 `license_file`，与 `resources/pronunciations.lock.json` 指向同一份 LICENSE。宿主只按 `<id>.json` 读书，NOTICE 放在同一目录里不会被当成一本书。
- 词书不分产品版本：背英文单词和有没有中文方案无关，日文、越南文、藏文版也带。
- 各平台的打包脚本在设了 `MSIME_REQUIRE_WORDBOOKS=1` 时，缺了锁文件列出的任何一本就失败；发版工作流和打包检查工作流都设它，本地打包不设时有就带上。这条约束由 `scripts/test-macos-package-resources.py` 对 macOS 两个工作流做静态检查，Windows 由 `platforms/windows/installer/tests/package-files.ps1` 用夹具检查。Android 由 `scripts/test-android-wordbooks.py` 对两个打包脚本和 `release-android.yml`、`android-device.yml` 做静态检查。

### 各平台的资源根

`builtin::directory(resources)` 是 `resources.join("wordbooks")`，不向上找，所以每个宿主传进来的必须是放着 `EngineResources/`（或等价目录）的那一层，而不是资源目录本身。

| 平台 | 读的进程 | `wordbooks/` 的位置 | 接入 |
| --- | --- | --- | --- |
| macOS | 设置应用 `MSIME.app`，按 `resource_dir()` | `MSIME.app/Contents/Resources/wordbooks` | `stage-resources.sh` 暂存到 `target/macos/wordbooks`，`package-release.sh` 拷进包并在 `check_app` 里核对；`release-macos.yml`、`ci-macos-package.yml` 生成词书 |
| Windows | `msime-client-settings.exe` 和 `MSIME.exe`，都以所在的 Server 目录为资源根 | 安装目录 `server\wordbooks` | `Prepare-PackageFiles.ps1` 装进 `server_exe\wordbooks`；`release-windows.yml` 生成词书 |
| Linux | 设置窗口 `msime-desktop`，按 HostOptions 求随包资源的目录（host-api 的 `bundled_resource_root`，经 `apps/desktop/src-tauri/src/lib.rs` 的 `vocabulary_resources`），每次命令时现读 | `share/<client>/wordbooks`，与 `resources`、`offline-glosses` 同级 | CMake 的 `MSIME_WORDBOOKS`（缺省从 `target/wordbooks` 取）和 `MSIME_REQUIRE_WORDBOOKS`；`package-container.sh` 和 `release-linux.yml` 打发布页的 deb、rpm；发行版定义见下 |
| iOS | 设置 App 的 `VocabularyReviewStore`，传键盘扩展里 `EngineResources` 的上一级（原来传的是 `EngineResources` 本身，共享层在它里面找 `wordbooks/`，永远找不到） | 键盘扩展包内 `wordbooks/`，与 `EngineResources` 同级 | `stage-resources.sh` 暂存到 `target/ios/wordbooks`，`project.yml` 把它作为文件夹打进键盘扩展和测试宿主；`release-ios.yml` 生成词书 |
| 鸿蒙 | 设置页，传 `context.resourceDir` | HAP 的 `resfile/wordbooks`，与 `engine` 同级 | `stage-resources.sh` 暂存；`release-harmony.yml` 生成词书 |
| Android | 原生设置的背单词页 `VocabularyReviewPage`，传 `files/bootstrap`；Tauri 合包的背单词页按 HostOptions 求（`vocabulary_resources` 对 Android 也走 `bundled_resource_root`，`resource_dir()` 在 Android 上是 `asset://localhost/`） | APK 的 `assets/wordbooks`，Bootstrap 解到 `files/bootstrap/wordbooks`，与 `bootstrap/resources` 同级 | `build-apk.sh` 和 `build-client-apk.sh` 暂存（`MSIME_WORDBOOKS` 可改来源），每个版本和瘦包都带，打包后核对每个暂存的文件都进了 APK；`release-android.yml` 和 `android-device.yml` 生成词书 |

Linux 的设置窗口原来按 `resource_dir()` 找，那是 `<prefix>/lib/<productName>`，安装时从不创建，所以即使装了词书也读不到。也不能简单取 HostOptions 里 `resources` 的上一级：词库在线更新后 `resources` 指向状态目录下的下载代次，那里没有随包资源。`bundled_resource_root` 这时取更新记录里的原安装包目录，与 `prepare_host_with_updates` 给音效包和插件包定位的规则相同。发行版的几条路径各自取词书：COPR、OBS 和 PPA 用的 vendor 包由 `make-source-tarballs.sh` 放进生成好的八本词书和 NOTICE（源 CSV 缓存在包外，词书的完整性由 vendor 包的 SHA-256 保证，构建农场不联网也不重新生成），`debian/rules` 和 `rpm/msime.spec` 直接用它；Arch 的 `msime` 和 Gentoo 在构建时按锁文件下载 ECDICT 并生成，Gentoo 的 ECDICT CSV 和 LICENSE 由 `render.py` 写成 distfile，放在 `fetch_wordbooks.py` 的缺省缓存目录 `wordbook-source` 里，离线核对后生成。legacy 包不带设置窗口，不装词书。

Android 原先不带词书：发布的 APK（`build-apk.sh`）不含 Tauri 管理界面，原生设置里的「背单词」只提示「需要管理界面合包」，带上词书用户也用不到。现在原生设置有了自己的背单词页（`home/VocabularyReviewPage.java`，从词库页「学习」组进入），内容和文字与 iOS 的 `VocabularyReviewSettingsView` 相同：今日待复习、已完成、未开始，词书选择（内置和导入的都列出词数），每日新词，经系统文件选择器导入 CSV / TXT（8 MiB 上限，与共享层一致），删除导入的词表，复习卡片和清空进度。排期全在共享层，页面只画 `VocabularyReviewModel`。进度用与打字统计同一个共享状态目录（偏好目录，没有配置时是 `files/bootstrap/state`），也就是 Tauri 合包背单词页用的那个，两边看到的是同一份进度。资源根直接传 `files/bootstrap`：词书总在那里，词库在线更新只换 `resources` 指向的代次，不动 `bootstrap/wordbooks`。`Bootstrap.installWordbooks` 与语言词库同样跟着安装包走：`lastUpdateTime` 标记、同级暂存目录、原子改名，失败只记一条不带路径的日志，不影响词库准备；词书不是按需资源包，没有收编这一步。关于页的开源许可列出 `wordbooks/wordbooks-NOTICE.txt`。`scripts/test-android-wordbooks.py` 静态核对两个打包脚本、Bootstrap、关于页和两个工作流的接线，设备上由 `VocabularyReviewDeviceCheck`（`smoke.sh` 的核心部分以 `SettingsDeviceSmoke -e vocabulary true` 运行，API 28 也跑；Android 35 每个测试清单最多 100 项 instrumentation，已经到顶，不能另加入口）核对解包出的八本书经真实 JNI 列得出来、能作答。

## Alternatives considered

- **发布预先生成好的词书，打包时按锁下载。** 这是 `pronunciations` 等释义表的做法（锁里的 `built` 指向 release 附件），最强的理由是打包不必再下载 66 MB 的 ECDICT CSV、产物有固定哈希。没有采用：词书的生成是确定性的，输入已经由锁按 SHA-256 固定，几秒就能生成完；预先发布要多维护一个 release 附件和一组哈希，换来的只是省一次下载。以后打包时间或下载稳定性成了问题，可以改成这种方式，宿主一侧不用改。
- **把八本 JSON 提交进仓库。** 打包完全不依赖网络。没有采用：3.3 MB 的生成数据会随每次重新生成整段变动，`fetch_wordbooks.py` 的设计本来就是源文件和产物都不进版本库，与 `settled-model`、释义表一致。
- **Android 发布包改为打开 Tauri 合包的背单词页。** 共享 React 页已经写好，不必在 Java 里再画一遍。没有采用：Android 发布 APK 的设置界面是原生的，不含 WebView 管理界面（docs/ARCHITECTURE.md「设置界面按平台归属」），为一个页面带上 Tauri 合包等于换掉整个设置界面；iOS 走的也是原生页加共享 C ABI 这条路，排期规则仍只在共享层一份。
- **保持可选，不加 `MSIME_REQUIRE_WORDBOOKS`。** 和其他同级资源一样有就带、没有就跳过，改动最小。没有采用：词书正是在「可选、缺了也能打包」的状态下从上线起就漏掉了，没有任何检查报出来；发版必须带上，才能让以后再漏掉时打包失败。

## Consequences

- 每个桌面安装包增加约 3.3 MB（八本词书合计）。
- Android 每个 APK 增加约 1 MB（词书在 APK 里压缩存放），装好后 `files/bootstrap/wordbooks` 占约 3.3 MB。full 的瘦包因此到了约 130.5 MB，超过 `build-apk.sh` 原来 130,000,000 字节的体积上限；上限提到 135,000,000，仍挡得住日文词典（约 20 MB）或语音运行库（约 27 MB）回到瘦包里。
- 从源码构建的 Arch `msime` 和 Gentoo 每次构建多下载一次 ECDICT CSV（约 66 MB），Gentoo 作为 distfile 只下载一次；COPR、OBS 和 PPA 的 vendor 包增加约 3.3 MB。
- macOS 和 Windows 的发版、macOS 的 develop 打包检查每次多下载一次 ECDICT CSV（约 66 MB，按锁核对哈希）并生成词书。
- 本地用 `package-release.sh` 打包时不设 `MSIME_REQUIRE_WORDBOOKS`，没先跑 `python3 scripts/fetch_wordbooks.py` 的话包里照旧没有内置词书，背单词只列导入的词表。
- Linux 的验证：把 CMake 那段放进临时工程，确认有词书时装到 `share/msime-client/wordbooks`、要求模式下缺一本或缺 NOTICE 时配置失败、不要求且没有词书时照常配置；`resources_parent` 的单测和 `msime-desktop` 的 vocabulary 测试、`clippy -D warnings` 在 macOS 上通过，Linux 分支的编译由 CI 的 `linux-desktop` 任务跑；Gentoo `render.py --no-crates` 渲染出 ECDICT 的两个 distfile。发布页 deb、rpm 的完整打包和各发行版构建没有在本机跑过。
- 验证：本机跑 `fetch_wordbooks.py` 产出八本词书和 NOTICE；`stage-resources.sh` 把它们暂存到 `target/macos/wordbooks`；`cargo test -p msime-client-services --lib vocabulary` 104 个用例通过；`scripts/test-macos-package-resources.py` 和 `scripts/run-checks.sh` 通过。macOS 的完整 universal 打包和 Windows 的安装包测试（`package-files.ps1`，需要 PowerShell）由 CI 运行，没有在本机跑过。
- Android 的验证：`check-host.sh`（含新加的 `VocabularyReviewTextSmoke`）、`scripts/run-checks.sh`、`cargo fmt --check` 和 macOS 上 `msime-desktop` 的 `clippy -D warnings` 通过；`vocabulary_resources` 的 Android 分支在 macOS 上编译不到，Tauri 合包（`build-client-apk.sh`）没有构建，它的背单词页没有在设备上看过。arm64 的 full APK 在 Mac Studio 上构建（瘦包和全带两种，`MSIME_REQUIRE_WORDBOOKS=1`），在本机 arm64 模拟器上：API 28 和 API 35 都打开「词库 → 学习 → 背单词」，八本内置词书带词数列出，选 CET-4 出卡片、翻开释义、作答后计数更新；API 28 上另外走过导入 CSV（经系统文件选择器）、删除导入的词表、改每日新词、清空进度和关于页的 ECDICT 许可；`smoke.sh --core`（API 28）和整套 `smoke.sh`（API 35）通过，其中包括 `SettingsDeviceSmoke -e vocabulary true`。
