# 架构与工程约束

这份文档说明水杉输入法共享客户端的分层方式，以及哪些边界是有意为之、不能顺手打破的。修改代码前请先读这里；具体流程见[贡献指南](CONTRIBUTING.md)，公开发布前的检查见[开源发布清单](docs/open-source-release.md)。

## 分层

仓库按「谁拥有什么状态」分层，而不是按语言或平台分。

```
平台宿主 (platforms/)          系统输入法入口，各自进程
    ↓  C ABI / JNI / Objective-C++
crates/host-api                版本化 C 接口，线程绑定会话句柄
    ↓
crates/input-runtime           会话编排、焦点、候选分页、带代次的选择
    ↓
crates/engine                  输入算法与组合状态（纯 Rust）
```

`host-api` 的词库管理接口（个人词条、固定资源的校验与编辑）直接调用 `crates/engine`，不经过 `input-runtime`；按键与会话一律走上面这条链路。

`crates/client-core` 与上面这条链路平行，负责本地配置和固定资源的分代安装，不参与按键处理。`packages/ui` 与 `apps/desktop` 是共享的 React 设置页和承载它们的 Tauri 层，各平台共用同一个 Rust 入口库与同一套页面。

**产品本体是 `platforms/<os>` 的原生宿主。** Android、iOS、HarmonyOS、Linux、macOS、Windows 一律如此：最终安装、启动、被系统识别为输入法的都是原生宿主。Tauri/React 是跨平台共享功能与界面的公共组件，由原生宿主按需承载，不单独作为某个平台的产品去启动或验收。`apps/desktop` 的目录名和 Tauri 生成的工程都不改变这一点。

## 四条不能打破的边界

**输入算法归 Rust 引擎 crate `crates/engine`。** 组词状态机、候选排序、学习回放都在 engine 里。`input-runtime` 仍然不复制组词逻辑：它只维护宿主编排和展示状态——它知道当前是第几页、焦点在不在、这次选择属于哪一代快照，但它不知道「ni hao」应该出什么词。在 `input-runtime` 或宿主里再写一份组词规则，就等于让两份实现开始漂移。

**`client-core` 不依赖 Tauri、React、Engine 或平台宿主。** 平台能力一律通过接口注入。这条保证了它可以被链进任何宿主进程，包括没有 Tauri 运行时的键盘扩展。

**平台库不依赖桌面应用。** iOS 键盘扩展不依赖常驻桌面服务，Android 的 `:ime` 进程不需要设置窗口活着。设置窗口关闭不能结束输入法进程；跨进程的设置变更需要明确的持久化与通知机制，不能靠共享内存里的一个全局变量。

**Windows TSF DLL 与 Server 的进程和协议边界保持不变。** TSF DLL 被系统加载进每个宿主应用的进程，Server 是独立进程，两者之间是显式协议。把逻辑从一侧挪到另一侧之前，先确认它在新的一侧还能满足 TSF 的线程和生命周期要求。

## 上游 Engine

输入引擎是本仓库里的 `crates/engine`（`msime-engine`），随 workspace 一起构建和测试，没有锁文件，也不再从外部拉取源码归档。它由 C++ 版 MSIME-Engine 移植而来：移植时用参考实现（固定 commit 加当时的 overlay）录下了行为基准，提交在 `crates/engine/tests/golden/`，由 `crates/engine/tests/golden.rs` 对照；录制方法和参考实现的来源记在 `tools/engine-golden/README.md`，只在需要从归档的参考构建重新录制时才用得上。引擎行为的改动直接改 `crates/engine` 并更新对应的基准，不再有 overlay 这一层。

平台仍要用到的非引擎文件已随仓库提交：Windows 与各宿主共用的 IPC 契约头文件在 `shared/contracts/`，辅助码表在 `resources/helpcodes/`，Windows 提示音用的 miniaudio 在 `platforms/windows/third_party/miniaudio/`。

引入新的上游代码、字体、图标、模型或服务 SDK 时，同时提交来源提交、许可证文本、通知位置和分发限制。

## 验证

GitHub Actions 在 Pull Request 上分四条线跑，各自只被相关改动叫起来。`ci.yml` 跑仓库级检查：actionlint 校验全部 workflow 和它们内嵌的 shell，dependency-review 按 high 阈值拦截依赖。`ci-macos.yml` 在 macos-15 的 arm64 与 x86_64 上构建原生宿主并跑 ctest，另有一个 ASan/UBSan 任务，构建完还会 ad-hoc 签名并断言 bundle 的架构与 Info.plist 输入法键；macOS runner 只有 5 台，所以发往 `develop` 的 PR 只跑 arm64，x86_64 与 ASan/UBSan 留给发往 `main` 的 PR、定时运行和手动触发，推到 `develop` 或 `main` 时 macOS 与 iOS 都不再构建（PR 上已经跑过，合并后的回归由每天的定时运行兜底）。`ci-ios.yml` 跑共享 Swift 后端的 `swift test`、键盘工程配置校验和 XcodeGen 工程生成，其中偏重的设置与皮肤分片、以及需要 ML Kit 的手写用例只在进 `main` 的 Pull Request 上跑。`ci-platforms.yml` 在对应平台或共享层有改动时检查 Android（宿主契约与 JVM 冒烟）、Linux（固定容器内构建并跑 ctest）、HarmonyOS（类型检查与键盘逻辑测试）和 Windows（MinGW x64 交叉构建）。纯文档改动走 `ci-docs.yml`，它跑同一组仓库级检查并补上平台 workflow 按路径跳过的那几个必需检查名，免得一个只改 README 的 Pull Request 永远等一个不会来的报告。定时的那条是 `codeql.yml`，每天扫一遍 Actions、C/C++、Python 和 Swift。

CI 之外的那一半由本地验证覆盖：Rust workspace 的 `cargo test`、clippy、前端的类型检查与 vitest、Wine 下的 Windows 套件、句子转换评测和重排延迟预算都只在本地跑。它同时也是提交前的快速反馈：

```sh
bash scripts/verify-local.sh --quick   # 只编译，合并前的门禁
bash scripts/verify-local.sh           # 全量
```

Windows 的编译门禁不需要 Windows 机器。装好 MinGW（`x86_64-w64-mingw32-g++`）后，在主工作区跑一次 `platforms/windows/build-cross.sh x64` 把 vcpkg 引导到清单基线，整台机器就位了：之后 `verify-local.sh` 在任何一个 worktree 里都会自动接管这条路径，把 host DLL、TSF DLL、Server 与全部原生测试链接一遍（vcpkg 树从 `MSIME_VCPKG_ROOT`、本工作区的 `target/tooling/vcpkg`、主工作区的同名目录依次查找；编译好的依赖放在该 vcpkg 树旁边共用，不必每个 worktree 各自把 curl 再编一遍）。没有这套环境时该阶段仍然跳过，但会把这行命令打出来——它曾经在每台机器上都只打印「skipped」，而背后的原生构建同时坏了六处。

Linux 的 Tauri 外壳同理，只要机器上有 Docker 就不需要 Linux 机器：`verify-local.sh` 会在 `platforms/linux/tests/tools/Dockerfile.desktop-check` 构建的镜像（固定摘要的 `rust:1.97.1-bookworm` 预装 webkit2gtk/gtk3/libsoup，按 checkout 打 tag）里 `cargo check -p msime-desktop --all-targets`。这一阶段的由来和 Windows 那条一样——`cargo check --workspace` 只看宿主 target，而 macOS 上 `msime-desktop` 因为缺少它当资源列出的 app bundle 被整包排除，于是 Tauri 外壳里所有 `#[cfg(target_os = "linux")]` 分支从来没被任何东西编译过，攒到 36 个编译错误：Linux 的设置窗口、全部共享面板和账号界面根本构建不出来。没有 Docker 时该阶段跳过并打印命令。

Linux 的原生宿主也一样，由 `platforms/linux/build-container.sh` 在同一个容器里编译 IBus engine、Fcitx5 插件、全部 provider 入口和单测并跑 `ctest`。这一条同样是补洞：此前没有任何阶段构建过 `platforms/linux`，而它已经不能配置了——三个测试的相对 include 比源码移动后的层级少一级，其中一个连自己的 fixture 都引不到。这不是单元测试的覆盖问题，是这个平台的产品本体构建不出来。Linux 主机上直接用系统的 ibus 开发包跑，其它主机走容器；两者都没有时跳过并打印命令。这一阶段只编译并跑单测，`platforms/linux/tests/tools/check-container.sh` 仍是需要已验证词库和真实 IBus daemon 的验收运行。

关键在于基线，不在于通过率。几个测试套件有长期存在的失败，所以单看「几个失败」没有意义；合并前唯一要回答的问题是「这次改动有没有弄坏原本能用的东西」。脚本因此把每一阶段失败的**测试名**与 `scripts/known-failures.txt` 对照，只有不在基线里的名字才算回归。那个文件里的每一行都是债务而不是豁免：修好一个就删一行，不要为了让运行变绿而新增。

首次克隆后执行一次：

```sh
git config core.hooksPath .githooks
```

三个钩子分工明确。`pre-commit` 是亚秒级的，只检查冲突标记和暂存 Rust 文件的格式——编译得起来的检查放在后面两个里，因为耗时一分钟的提交钩子会被关掉，然后一个都不剩。`pre-merge-commit` 在合并提交产生的那一刻跑 `--quick`，`pre-push` 作为没走合并路径的提交的兜底。`--quick` 会按改动范围跳过碰不到的平台阶段（依据是整个分支相对 `origin/develop` 的改动加上工作区，合并时进来的文件也在其中），并为每个跳过的阶段打印原因；`MSIME_VERIFY_ALL=1` 强制全部运行。曾有六次编译中断因为「合并了但没构建合并结果」进入 `develop`，`--quick` 正是为此存在。

Rust 改动另需 `cargo fmt` 与 `cargo clippy`；UI 改动另需类型检查和构建。

## 各平台的验证覆盖面

六个宿主各有自己的自动化套件和自己的装机路径，入口不同但结构一致：先在不需要目标系统的层面跑通编译与单测，再在目标系统里跑需要真实输入法框架的那一层。报告验证结果时说清楚跑的是哪一层——交叉编译通过和系统输入法里真的出了候选不是同一件事。

- **macOS**：`platforms/macos` 的 CMake 工程注册了一百多项 ctest，覆盖候选面板、皮肤、语音、表情与剪贴板面板、词库安装与偏好持久化；`platforms/macos/scripts/install.sh` 用 Developer ID 重签后原子替换到 `~/Library/Input Methods`，`platforms/macos/scripts/check_input_source.swift` 核对输入源注册。
- **iOS**：工程由 XcodeGen 从 `platforms/ios/project.yml` 生成，`MSIMEClientTests` 聚合键盘、服务与共享三个单测 target，`MSIMEClientUITests` 跑引导流程和键盘扩展在编辑器里的界面用例；`platforms/ios/build-app.sh` 出模拟器或真机构建。
- **Android**：`platforms/android/check-host.sh` 在没有 Android 运行时的机器上校验 JNI 契约并跑 JVM 冒烟；`platforms/android/tests/device/smoke.sh` 在专用 AVD 上跑设备套件，设置、打字统计和手写各有开关；`platforms/android/build-apk.sh` 出原生 IME 包。
- **HarmonyOS**：`platforms/harmony/tests/run.sh` 做类型检查并跑键盘逻辑测试；HAP 由 DevEco 的 `hvigorw assembleHap` 打出，`ohpm install` 和真打包缺一不可——ArkTS 对 `@Builder` 体内声明、对象字面量类型的一批限制只有 `.ets` 真正编译时才报出来。
- **Linux**：`platforms/linux/build-container.sh` 在固定容器里构建 IBus engine、Fcitx5 插件和全部 provider 入口并跑 ctest；`platforms/linux/tests/tools/check-container.sh` 起独立 D-Bus 与 IBus/Fcitx5 daemon 做隔离验收。
- **Windows**：`platforms/windows/build-cross.sh` 用 MinGW 交叉构建 host DLL、TSF DLL、Server 和全部原生测试，`platforms/windows/run-tests-wine.sh` 在 Wine 下真跑这些产物；`platforms/windows/Build-Client.ps1` 是 Windows 上的 MSVC 全量构建，安装包由 `platforms/windows/installer/` 的 Inno Setup 脚本和双架构签名链产出。

各平台的构建前置、环境变量和装机步骤见对应的 `platforms/<os>/README.md`，目录与职责总览见 [README](README.md)。
