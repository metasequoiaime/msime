# Agent Note: iOS App 与键盘扩展崩溃采集

Status: implemented

## Problem

iOS App、键盘扩展和 MetricKit 的诊断路径原先各自记录，现场信号记录与延迟诊断可能重复上报，也可能把旧授权期间发生的诊断归入新授权。键盘扩展的捕获源文件还位于扩展专属目录，无法让 App 和独立测试共用同一套原生记录器。

## Decision

把信号记录器移到 SharedUI，由 App 和键盘扩展共同编译；记录头保存捕获时的构建身份、模块 UUID、架构和相对偏移。App 启动时接收 MetricKit 的 crash 与 hang，按共享层返回的持续授权代次、诊断时间范围和进程身份补录；信号现场已写下的记录与 MetricKit 使用同一捕获 UUID，只保留一条。系统强制结束仍作为独立 crash 记录，不归入键盘会话。关闭授权立即撤下信号路径、清理队列，读偏好失败则停采而不猜测默认目录。

键盘展示和独立 App 捕获各自管理生命周期，授权观察、撤回和正常收尾使用串行队列；故障处理器只读取已发布的内存快照，不在故障现场读盘或联网。Xcode 工程和生成配置同步加入 SharedUI 记录器及宿主故障夹具，测试数据全部为合成值。

## Alternatives considered

- **只依赖 MetricKit**：诊断延迟且缺少现场首帧，无法稳定关联键盘会话；因此保留原生信号记录并用 MetricKit 做补录。
- **按进程号直接去重**：进程号可能被键盘扩展复用，不能证明是同一次故障；因此按捕获 UUID 和持续授权代次绑定。
- **在每个扩展内复制记录器**：会让 App、扩展和测试的信号处置逐渐分叉；因此移动到 SharedUI 并由 Xcode 工程显式复用。
- **读取旧 UserDefaults 推断历史授权**：会把关闭前的延迟诊断重新认领；因此只接受共享 Host API 返回的有效代次和起始时间。

## Verification

- Swift、C 源文件无冲突标记；`swiftc -parse`、C 语法检查和 `git diff --check` 通过。
- iOS 单元测试新增持续授权、诊断去重、现场故障首帧、模块快照和授权撤回覆盖；宿主故障夹具从 Xcode 单元测试目标排除，避免把非 XCTest 的可执行文件当作测试源码。
- 本机不是 macOS，未把语法检查当作 iOS 模拟器或真机通过；提交后由 iOS Simulator CI 运行工程构建、单元测试和符号门禁，真实 MetricKit 投递与真机 dSYM 仍需设备验收。

## Consequences

同一 iOS 故障在现场记录和 MetricKit 延迟诊断之间保持单条事件，撤回授权后旧进程不能写入新队列。新增的 SharedUI C 记录器需要在 App、键盘扩展和测试目标中维持相同的编译定义；完整验证依赖 Apple CI 和设备环境。

共享语义见[共享崩溃记录基础](../../implemented/architecture/2026-11-01-shared-crash-reporting-core.md)。
