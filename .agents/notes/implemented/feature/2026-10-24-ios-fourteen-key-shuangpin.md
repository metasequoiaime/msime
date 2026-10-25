# Agent Note: iOS 双拼 14 键

Status: implemented

## Problem

iOS 已有全拼 14 键和多种 26 键双拼方案，但双拼只能使用 26 键。#9190 需要讯飞式的双拼 14 键，同时保留用户当前的小鹤、自然码、微软或首道双拼 profile。共享引擎的 14 键双拼解码由 #9404 提供，iOS 需要把它接到原生设置、会话和键面。

## Decision

复用 #9404 的 `touch_shuangpin_layout`（`twenty_six_key` / `fourteen_key`）作为 iOS 与 Android 共用的独立布局偏好。iOS 设置页在当前方案是双拼时提供 26 / 14 键选择，布局不改变 `shuangpin_profile`；键盘会话按文档读取布局，14 键使用现有 `FourteenKeyLayout` 和 `grid_key`，26 键继续走原有双拼字母路径。Host API 的自动网格门控扩展到 `HostPlatform::Ios`，从而新会话、会话重建和设置热加载保持一致。

布局选择写入共享 `PreferencesStore`，再镜像到键盘扩展的 App Group。切换双拼方案、重建会话和回到中文时保留选择；英文、本地模式和不支持组码的输入框回落到 26 键。物理字母仍经 26 键双拼路径，触摸 14 键按组首字母经 `grid_key` 进入共享引擎。iOS 账号同步继续使用原有全拼布局字段，不上传这个仅本机的双拼布局选择。

键盘控制器单独保存已应用的双拼布局，并把是否显示 14 键纳入布局缓存。设置页写入 App Group 镜像不会提前改变键盘对旧布局的判断：空闲时热加载立即换键面和读音行；组字期间收到的布局留到提交或取消后再应用，与 Host API 的延迟应用一致。14 键不使用 26 键的分音节退格与快速空格滑动分段，也不参加平板横屏分离布局。微软双拼的分号与 L 共键，键面显示 `L;` 并向 VoiceOver 说明，组码、统计标识和既有全拼键面不变。

### 历史笔记检索

- [iOS 十八键](2026-10-25-ios-eighteen-key.md)：部分重叠，`touch_shuangpin_layout` 在 iOS 上追加 `eighteen_key`，沿用本篇的布局缓存、延迟热加载和英文回落。
- [Android 14 键双拼](2026-10-23-android-fourteen-key-shuangpin.md)：部分重叠，共用 `touch_shuangpin_layout`、Host API 门控、14 键双拼引擎契约和多义读音处理；iOS 只增加原生 Swift 设置、会话桥和键盘扩展接线。
- [全拼 14 键](2026-10-10-fourteen-key.md)：部分重叠，复用键位、触摸来源、拼音选择和候选面板契约；本方案保留双拼 profile。
- [九键双拼](2026-10-10-nine-key-shuangpin.md)：部分重叠，复用两键多义解码、拼音选择和退格恢复；不改变九键数字方案。
- [自定义双拼方案](2026-10-10-custom-shuangpin-profile.md)：部分重叠，方案解析及非法表回退仍归现有实现，14 键只消费解析结果。

## Alternatives considered

- **增加一个名为「双拼 14 键」的独立输入方案**：设置入口简单，但会把布局和双拼 profile 绑在一起；从小鹤切到自然码时会丢掉用户的方案，且共享 `touch_keyboard_schemes` 没有这个方案值，因此不采用。
- **只在 iOS App Group 保存布局，并由键盘每次手动调用 `set_key_grid`**：可以避免扩展 Host API 的平台门控，但新会话、词库维护重建和跨进程设置读取会出现不同步，且 canonical `PreferencesStore` 不知道用户选择，因此不采用。
- **把 iOS 布局写进 `touch_keyboard_layout`**：字段已经表示全拼、九键和手写的方案布局，双拼另有当前 profile；混用会让全拼与双拼的选择互相覆盖，因此沿用 #9404 的独立字段。
- **直接读取 App Group 镜像作为当前布局**：少存一份控制器状态，但设置页保存时已更新镜像，键盘重载前就失去了旧布局信息，而且组字中的键面会提前改变，因此控制器只在能应用时更新自身布局。

## Consequences

- iOS 原生设置、键盘扩展和 Host API 共享同一个布局字段；切换双拼 profile 不会丢失 14 键选择，默认值仍是 26 键。
- 14 键双拼复用现有键面、拼音选择条、候选面板、分词和退格契约，物理字母输入保持原有 26 键行为。
- 旧版客户端不认识 `touch_shuangpin_layout` 时会按 #9404 已接受的降级边界处理；iOS 账号同步不携带该本机布局，因此跨设备不会覆盖本机选择。
- #9404 已合入 `develop`；本分支将其后的 3 个 iOS 提交迁到固定基点 `4c4571f8797548ac4d24b5806a96a2b84438ffe5`，直接面向 `develop`。迁移无需手工解决冲突，Android 共享基础不再混入本 PR 的差异。

## Verification

- `cargo fmt --all -- --check` 通过。
- `git diff --check` 通过。
- `cargo test -p msime-host-api fourteen_key_shuangpin_preference_is_applied_by_touch_hosts --locked` 通过。
- `cargo test -p msime-host-api fourteen_key_shuangpin --locked` 两项通过，覆盖 iOS/Android 宿主门控、profile 变化、硬件输入与 14 键布局保持。
- `cargo test -p msime-client-core touch_shuangpin_layout --locked` 通过。
- `bash platforms/ios/build-native.sh simulator` 在远端编译当前源码通过，静态库拉回本机后运行模拟器测试。
- iOS 27 arm64 模拟器运行 `FourteenKeyKeyboardTests` 全部 15 项通过，覆盖四种 profile 与会话重建、空闲和组字中的设置热加载、微软分号、手机键面和 iPad 横屏。多义组码的首选读音受词频影响，桥接测试明确选择 `ni` 后检查读音及「你好」候选。
- 同次运行 `LanguageSchemesTests`：共 26 项，21 项通过，5 项因模拟器缺少对应语言词库跳过；无失败。验收层级为原生键盘控制器在模拟器测试宿主中运行，尚未覆盖真机系统键盘入口。
- 系统编辑器专项 `testFourteenKeyShuangpinProfilesCommitIntoTheSystemEditor` 从原生设置选择 14 键，在系统加载的扩展里逐一切换小鹤、自然码、微软和首道，点按合成组码并显式选择读音行的 `readingSpelling_ni`，检查「你好」连续上屏、微软分号读屏及数字页往返，结束恢复原方案和布局。未签名的模拟器构建缺少 App Group 权限，首轮只得到共享目录错误和跳过；验收必须启用模拟器 ad-hoc 签名（`CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual`）。模拟器权限在 `Entitlements-Simulated.plist` 和可执行文件的模拟权限段中，不能仅凭 `codesign` 打印的空字典断定签名构建仍没有 App Group。共享目录错误属于已加载扩展的失败，不再按未启用键盘跳过。最终签名构建在 iOS 27 arm64 专用模拟器通过该专项（221 秒，1 通过、0 失败、0 跳过）：独立编辑器依次收到四段「你好」，原有文本没有被覆盖，数字页往返仍保留 14 键；四张方案截图已逐一检查，微软显示 `L;`，结束恢复原方案和布局。该结果覆盖模拟器系统键盘入口，尚未覆盖实体设备。
- 最终远端 quick（`20261024-061655`）exit 0：Rust workspace、Android/WASM、Android Java/JNI、Linux 设置壳、Linux 原生、五笔版构建、macOS 和 Apple 桥接阶段通过。Linux 原生注册 111 项，110 通过、1 项 `linux-ibus-demand-activation` 因环境条件跳过、0 失败；三项真实 classicui 主题检查实际执行。Windows MinGW、pipe-only 与 HarmonyOS 因前置条件缺失跳过；远端未安装 Node，笔记校验在本机单独通过。更早的 run `20261024-044819` 因磁盘不足主动停止（exit 143），不作为整套通过证据。最终轮同步了系统编辑器专项，运行期间没有再改生产或测试源码。
- `python3 scripts/test-fourteen-key-table.py` 通过。
- 原生产品 `MSIMEApp` 与内嵌键盘扩展的 arm64 模拟器构建通过，包含设置页的真实 SwiftUI 编译；构建前按锁文件校验并准备本地语音运行库。模拟器库只有 arm64，构建命令显式传入 `ARCHS=arm64 ONLY_ACTIVE_ARCH=YES`。
- `pnpm run verify-notes`、本次改动的 Rust 格式检查和差异检查在提交前执行。

### 依赖合入后的组合验证

上述原生 App、模拟器与系统编辑器结果对应迁移前提交，不能作为新组合的通过证明。迁移后共享 Host API 的 14 键双拼、client-core 的布局门控和 legacy 混输回归均实际执行并通过；为基点中两个旧 `MixedInputPreferences` 测试字面量补入 `..MixedInputPreferences::default()`，保留原有断言。键表一致性、Rust 格式和差异检查通过，OpenCC 输入由锁定下载器补齐并校验。

Linux 组合验证使用固定 NDK 28 中的 Clang 19 构建 WASM，并通过其检查与严格 Clippy。Windows 主机策略测试在系统 Clang 与 GNU ld 下执行 164 项通过、2 项因已有主机差异排除、76 项仍需 Windows 构建。切回 NDK Clang 默认的 LLD 时，检查脚本把它的 `undefined symbol:` 当作新编译错误；该诊断与脚本已经识别的 GNU `undefined reference`、Apple `Undefined symbols` 都表示无法独立链接的翻译单元，因此补齐同类匹配，实际编译错误继续失败。修正后在原 NDK Clang 19 与 LLD 组合下重跑完整 `bash scripts/verify-local.sh --quick`，退出 0；同一套 Windows 主机测试实际通过 164 项，Rust workspace、Android 目标与 Java 宿主、WASM、Linux 设置外壳及原生宿主均通过。HarmonyOS 真打包、Windows MinGW、macOS 与 Apple bridge 因该 Linux 构建环境缺少对应输入或宿主而按脚本规则跳过。

Apple 迁移后组合仍未重验：独立 macOS SSH 入口拒绝认证，旧 `macmini` 地址经机器身份核对实际是本机；本机系统盘不足 50 GiB，未新建完整 Apple 构建树。
