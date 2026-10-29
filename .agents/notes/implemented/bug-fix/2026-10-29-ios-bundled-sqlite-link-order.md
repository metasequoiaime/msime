# Agent Note: iOS 的主机库排在系统 SQLite 前面链接

Status: implemented

## Problem

2026-10-29 用 Xcode 27.1 本机编译 Debug 版装到 iPhone XR（iOS 18.7.10）验收按键布局，键盘扩展一启动就被 dyld 杀掉，系统随即退回自带键盘：`Symbol not found: _sqlite3_set_errmsg`，当天同一台机器上有 12 份这样的崩溃报告，还在我们的代码跑起来之前。

原因在链接顺序。`project.yml` 里每个链接主机库的目标，`OTHER_LDFLAGS` 都是 `$(inherited)` 在前、`-lmsime_host_api` 在后，再跟一个 `-lsqlite3`。键盘扩展的 `$(inherited)` 里有 MLKit pod 带进来的 `-l"sqlite3"`。`libmsime_host_api.a` 里有 rusqlite `bundled` 打包的整份 SQLite，可用 Xcode 27.1 的 SDK 链接时，`MSIMEKeyboardExtension.debug.dylib` 里 62 个 `sqlite3_*` 全部绑到了 `/usr/lib/libsqlite3.dylib`。rusqlite 0.40.2 出错时调用 `sqlite3_set_errmsg`（`src/error.rs:558`），27.1 SDK 的 `libsqlite3.tbd` 导出这个符号，但 iOS 27 之前的系统库里没有，于是 iOS 18 上加载即失败。

已发布的版本没受影响：发布和 CI 固定用 Xcode 26.3（`release-ios.yml`、`ci-ios.yml` 的 `DEVELOPER_DIR`）。ios-v0.52.0 的模拟器包是 26.2 SDK 链接的，`nm -m` 显示 `_sqlite3_set_errmsg` 定义在扩展自身里。等 CI 换到 Xcode 27，部署目标仍是 iOS 17.0，低于 iOS 27 的用户一打开键盘就会崩。

## Decision

五个链接主机库的目标（MSIMEApp、MSIMEKeyboardExtension、MSIMEKeyboardTests、MSIMEServiceTests、MSIMESharedTests）都把 `-lmsime_host_api` 挪到 `OTHER_LDFLAGS` 第一位，`project.yml` 和提交的 `project.pbxproj` 一起改。挪完之后 `dyld_info -imports` 在 App 和扩展的 debug dylib 里都看不到从 libsqlite3 导入的符号，`_sqlite3_set_errmsg` 定义在扩展自身里，XR 上键盘正常起来。

`ProjectConfigurationTests.test_host_library_links_before_the_system_sqlite` 检查两份工程文件里的这个顺序。把任一目标改回原来的顺序，这条测试就失败。

## Alternatives considered

- **只删 `-lsqlite3`**：先试的就是这个。不采用：`$(inherited)` 里 MLKit 的 `-l"sqlite3"` 还在前面，删完仍然是 62 个导入。
- **对主机库用 `-Wl,-force_load`**：同样能让打包的 SQLite 赢，但会把静态库里没被引用的目标文件全部链进来，体积变大。换顺序就够了，不采用。
- **iOS 上 rusqlite 不用 `bundled`，改用系统 SQLite**：系统 SQLite 的版本随 iOS 版本走，这次的崩溃恰恰就是 rusqlite 调到了老系统里没有的函数；`crates/engine` 在所有平台都开 `bundled`，iOS 单独换掉等于让引擎在部署目标范围内跑在一堆不同版本的 SQLite 上。不采用。

## Consequences

进程里仍然链着两份 SQLite：主机库自带的一份给 Rust 用，系统的 libsqlite3 仍按 `-lsqlite3` 和 MLKit pod 的要求链接，留给真正引用它的其他代码。Xcode 26 链接出来的已发布版本本来就是这样分开的。

## Verification

- `python3 platforms/ios/tests/settings/ProjectConfigurationTests.py` 31 项通过；把 `project.yml` 或 `project.pbxproj` 里任一目标改回原顺序，新测试失败。
- iPhone XR（iOS 18.7.10）上，用 Xcode 27.1 编译、只改了 MSIMEApp 和 MSIMEKeyboardExtension 两处顺序的 Debug 版：键盘扩展正常启动，九键、26 键都打过字；`dyld_info -imports` 显示没有 sqlite3 导入。
- 用 Xcode 26.3 的构建看这个 PR 的 iOS CI。
