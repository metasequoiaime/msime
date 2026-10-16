# Agent Note: 设备测试包和 ArkTS 的编译缺口

Status: implemented

## Problem

develop 上有两样东西坏着，CI 全绿，没有任何门禁发现：

- Android 设备测试包 `tests/device/build-editor.sh` 编不过，有 21 个「cannot find symbol」。这个脚本手工列出要编译的源文件：测试类，加上它们引用的应用类。最近一批重构把逻辑挪进了 `BoundsPolicy`、`JsonPolicy`、`ViewPolicy` 等新辅助类，却没有把新类补进这份清单。设备套件只在 `tests/device/smoke.sh` 里构建，而那要模拟器，CI 从来不跑，所以整个设备套件都构建不出来也没人知道。
- HarmonyOS 的 `hvigorw assembleHap` 报 10 个 ArkTS 错误，来自 e2e389ba0、f69c6b657、33609ad6f、812541dfe 四次提交。这和 `platforms/harmony/README.md` 记的上一次（13 个错误）是同一种情况：`tsc`、逻辑测试、quick 门禁都只看 TypeScript，没有一道门禁真正编译过 ArkTS。

## Decision

- 修掉全部 31 个错误：设备测试包的清单补上 18 个缺的应用类；ArkTS 的 10 处按规则改写。
  - catch 里原样重新抛出写成 `throw error as Error`，抛出的还是同一个对象，调用方读得到错误码。
  - `{ fd: number }` 换成已有的 `LocalAsrTextFile` 接口。
  - `voiceInputConfiguration()` 里的对象展开改成 JSON 往返拷贝再设 `local_model_root`，缓存的配置不被改动。
  - 补上 `MAX_SESSION_BYTES` 的 import。
- 设备测试包的源文件清单移到 `platforms/android/tests/device/editor-sources.txt`，`build-editor.sh` 和 `check-host.sh` 共用它。`check-host.sh`（CI 的 Android 检查会跑）按这份清单只做 javac 编译，不打包也不签名；清单再漏类，PR 上当场失败。
- `scripts/test-harmony-arkts-subset.py`（quick 门禁和 HarmonyOS CI 都跑）新增三条只认窄写法的纯文本规则：以展开开头的对象字面量（arkts-no-spread）、在 catch 块里原样重新抛出没有类型的 catch 变量（arkts-limited-throw）、函数参数或返回值上的对象字面量类型（arkts-no-obj-literals-as-types）。这次 10 个错误里有 7 个落在这三条上。
- 接口体内的对象字面量类型也按 `arkts-no-obj-literals-as-types` 拒绝。nightly `38806414451` 的 `NineKeyShuangpinLayoutReply.value?: { keys?: ... }` 曾漏过函数位置检查；接口体只有类型声明，可在匹配其闭合括号后检查成员类型，不扩大为全文件的 `key: {` 匹配。注释与字符串仍先屏蔽，正常对象值不受影响。检查成功的提示只说明已覆盖的规则通过，不宣称源码一定能通过 ArkTS 编译。

## Alternatives considered

- **设备测试包改成编译整个 `platforms/android/java`，不再维护清单**：清单不会再漏。但应用的源文件按功能分在子目录里却都在同一个包下，整树编译会把 Tauri 生成代码、依赖 Gradle 才有的类一起拉进来，测试包也会带上整个应用的类；`-sourcepath` 按包找文件又找不到它们。维护清单、由 CI 编译兜底，代价小得多。
- **在 PR CI 里跑 `hvigorw assembleHap`**：能抓住全部 ArkTS 错误，包括漏 import 和类型兼容性。发布与 nightly 已能从私有工具链仓库下载 SDK 并编译 HAP，但现有 PR job 不读取该仓库的凭据，fork PR 也拿不到这些 secrets；把长期凭据交给 PR 构建还需要独立设计工具链分发与权限边界。本次只补已有文本检查，不改凭据使用或发布链，也不把它当作真实 HAP 编译的替代品。
- **ArkTS 规则写得更宽，比如 `key: {` 一律报**：能多抓一些写法。但对象字面量里的值也长这样，上一版脚本已经试过，会报出两百个编译器接受的写法。

## Consequences

- **收益**：设备套件重新能构建，HarmonyOS 的 HAP 重新能编出来；两类缺口里能靠文本或 javac 发现的部分，今后在 PR 上就会失败。
- **代价**：`check-host.sh` 多一次 52 个文件的 javac。新增设备测试、或者设备测试用到的应用类挪进新类时，要同时改 `editor-sources.txt`。
- **仍然没有覆盖**：漏 import、无类型对象字面量这类 ArkTS 错误仍然只有真编译器能发现。设备测试在这里只保证能编译；在模拟器上真正运行见 [Android 模拟器上的 PR 门禁和发版门禁](2026-10-08-android-device-gates.md)。

## Verification

- `bash platforms/android/check-host.sh`：171 个 JVM 冒烟，并编译了 52 个设备套件源文件。
- `bash platforms/android/tests/device/build-editor.sh`：打出了签名后的 `editor-test.apk`。
- `ohpm install` 后跑 `hvigorw assembleHap --no-daemon`：BUILD SUCCESSFUL。
- `python3 scripts/test-harmony-arkts-subset.py`：修复后零报告；对修复前的 develop 源码报出 7 处，正是编译器在这三条规则下的 7 个错误，没有误报。
- `bash platforms/harmony/tests/run.sh`，以及全部 `scripts/test-harmony-*.py`。
- 接口成员检查的合成回归覆盖 nightly 响应类型、具名接口修复、多行与嵌套空类型、泛型继承、注释与字符串、接口之外的正常对象值，以及原有参数和返回值规则。原始 nightly 源码在旧检查中零报告，新检查准确拒绝 `KeyboardSession.ets:630`；当前 41 份宿主源码零报告，6 项合成回归全部通过；只读数组赋给可变状态的两处类型错误仍需真实编译器检查。
- 本次接口门禁补丁通过 `verify-local.sh --quick`、笔记检查与差异检查，设置页另行构建为单文件通过。quick 的原生 ArkTS 阶段因未暂存原生库、资源与模块依赖而跳过；本次不改 `.ets`，不宣称重新打过 HAP 或完成了新一轮 nightly。
