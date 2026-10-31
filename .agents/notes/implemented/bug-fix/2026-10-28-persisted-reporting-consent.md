# Agent Note: 共享偏好原子保存上报同意的持续代次

Status: implemented

## Problem

iOS App 与键盘每五百毫秒观察一次上报开关。两次观察之间关闭再开启，布尔值相同，键盘无法识别已经撤回过的授权；独立的 UserDefaults 起点也不能证明共享偏好里的持续同意。授权代次必须由所有共享偏好写入方共同维护，不能只靠设置页回调。

## Decision

在 `Preferences` 内增加可选的本机 `usage_reporting_consent`，含随机 UUID 与毫秒时间起点。`PreferencesStore` 在现有偏好锁和原子替换内管理它，提交方不能指定或复活旧代次。正常读取不修改磁盘；需要持续同意的宿主显式调用确保接口，第一次从当前时间建立授权历史。无关设置保存保持代次；明确关闭移除，关闭后重开取得新代次。损坏的元数据不妨碍其余偏好读取，但确保接口只能建立新的历史。整份偏好修复也不继承损坏文档中的授权元数据。

Android 旧键面迁移沿用普通保存的 `initialize_reporting = false`：迁移底行和数字行不建立授权历史，持续代次仍只由明确的授权入口初始化。

字段放在偏好对象内，避免顶层 `deny_unknown_fields` 让旧版拒绝整个文档。设置导出、导入和账号同步保留本机边界；账号映射已核对为逐项白名单，不存在已经证实的远端改写上报开关。默认上报值仍为开启。

这是持续授权的存储基础。[采集与队列绑定代次](2026-10-28-epoch-bound-crash-collection.md)在同一偏好锁内核对授权、准备采集与系统补录，每条发送前再校验；故障处理器不读盘，网络发送不持有偏好锁。实际设备故障与后台收件仍不由存储接口证明。

## Related notes

动手前检索活跃笔记的同意、代次、`usage_reporting` 和 epoch。[全平台提案](../../proposed/feature/2026-10-27-cross-platform-crash-reporting.md)与[宿主采集笔记](../../implemented/feature/2026-10-27-native-crash-capture-and-symbol-identity.md)部分重叠，仍持有平台接入与验收；[iOS 异常捕获](../../implemented/bug-fix/2026-10-27-ios-app-uncaught-exception-stack.md)部分重叠，保持内存故障快照和锁忙放弃。[Android 语音贡献授权](../../implemented/bug-fix/2026-10-11-android-voice-contribution-consent.md)、Windows 设置文件对齐和手机隐私页与本次存储决定无关。没有完全吸收或需要否决的旧笔记。

## Alternatives considered

- **Swift 单独写 App Group 授权文件**：便于快速实现跨进程代次，但授权文件与偏好开关之间需要跨文件事务，且其他保存入口可能漏写；共享偏好已有互斥锁和原子替换，直接保存更明确。
- **偏好 revision 直接作为授权代次**：不增加配置格式，但任意无关设置都会结束授权历史并丢弃延迟诊断，无法表达持续同意。
- **只观察 Bool 或只在设置页回调更新**：修改最少，但跨进程短暂撤回仍会被漏掉；故障处理器现场读盘也无法安全弥补。

## Verification

- 明确关闭再开启，即使观察者没有读到关闭，也获得不同代次；进程重启、无关保存和重复确保保持同一代次。
- 陈旧客户端、伪造提交、设置导入不能复活旧代次；损坏/缺失的元数据从现在开始，偏好读失败保持错误。
- 设置恢复与损坏修复分别遵守实际开关和不追认旧历史的约束；默认值与旧版未知字段兼容。
- 最终源码的共享核心 Rust 单测 667 项通过，零失败、零跳过；`cargo clippy -p msime-client-core --all-targets -- -D warnings` 和 `cargo fmt --all --check` 通过。首轮 665 项单测也通过；Clippy 指出既有 Android 皮肤槽位测试的初始化写法，等价修正后通过，未放宽警告。
- 当前源码交叉编译四个共享 crate 的 Android 测试程序，在 API 28、35 专用 AVD 上通过仓库 `run-core-test.sh` 整套运行。每台设备：client-base 36 passed / 2 ignored / 3 filtered；client-core 666 / 1 / 0；client-providers 274 / 2 / 0；client-services 1358 / 10 / 4，均为零失败。过滤项是 runner 既有的硬链接准备限制；子进程夹具输出的一项通过不代替最外层 1358 项汇总。新增 11 项持续同意回归全部实际执行。四个源码文件与构建机 SHA-256 一致；专用设备合成测试目录已清理，两台 AVD 均已关闭。没有启动产品网络发送；这不替代全平台提案里的 Android 正式包和完整资源冒烟。
- 整轮 quick `20261028-071056` 退出 1，唯一 `FAIL:` 是偏好字段对齐检查尚未把内部的 `usage_reporting_consent` 登记在 `RUST_ONLY`；它没有可编辑设置控件。已补中文原因并在本机独立复跑通过，不把原始整轮记成退出 0。其余阶段完成：202 项 Windows 可移植用例、仓库契约与 2049 篇笔记、Rust/Android/wasm/Java、HAP、Linux 设置与原生、macOS 和 Apple bridge。Linux 116 项实际通过、1 项环境跳过，full/wubi 安装后的八入口故障通过。缺 MinGW 的 Windows 原生/pipe-only 与缺前端依赖的共享设置 bundle 明确跳过；HAP 复用原生库仅算 ArkTS 编译/打包。字段对齐检查也在构建机独立复跑通过；再次完整 quick `20261028-072634` 已退出 0（8m10s），末尾为 `quick check passed`、未检出 `FAIL:`；仍保留上述环境跳过与 HAP 复用边界。存储产品源码和设备验证字节不变。

## Consequences

收益是授权撤回和重开由同一次偏好事务留下可辨识的历史；代价是第一次显式确保需要保存偏好、推进 revision，调用方需要处理保存失败。普通 load 和未使用该接口的稳定旧文档不额外写盘。iOS 采集端与旧原生记录、系统诊断及发送端的绑定由上述配套笔记持有，设备到后台的全平台闭环仍未完成。

旧版程序不会维护新代次，不能承诺降级后依旧识别两次轮询间的撤回。直接修改磁盘或还原整份用户目录也不是受控的共享偏好保存路径。系统时钟回退时延迟诊断应保守拒绝，不能通过修改授权起点来扩大历史。
