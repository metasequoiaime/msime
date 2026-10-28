# Agent Note: 鸿蒙系统延迟诊断携带故障前的采集身份

Status: implemented

## Problem

鸿蒙系统回调只有版本和发生时间，当前实现临时制造 revision/arch 未知的记录头，并按 PID 认领会话。同一版本不同构建、升级补报与 PID 复用都无法证明原归属。已不再使用的单个键盘 PID 文件仍写盘，却不能解决这些问题。

## Decision

直接复用共享 `telemetryBegin` 返回的 UUID、原构建头与当前持续授权，在正常路径用 API 12 的 `hiAppEvent.setEventParam` 为 OS 域的崩溃和卡死附加一个有界 `msime_capture` 字符串。字符串保存 UUID、原始头、PID 与准备时间，不保存路径和用户内容。系统回调必须携带这份原身份，且 PID、原版本、代次与发生时间匹配；没有证据的旧报告拒绝认领，不用当前版本重建。

崩溃以原 UUID 经共享 `diagnostic_capture` 落盘，原键盘会话仍按 UUID 归属；卡死使用独立稳定诊断名与原头，不增加会话崩溃。原生帧额外保留系统给出的合法 ELF build-id，供已有符号工具确定模块。参数发布按进程串行，撤回、暂停和停止发布空值；迟到完成不重新启动采集，新需求在旧请求结束后覆盖。移除没有读取者的键盘 PID 文件及 ArkTS 文件读写。

## Related notes

已检索活跃的 `HiAppEvent`、鸿蒙、故障身份和持续授权笔记。[独立采集与构建身份](2026-10-27-native-crash-capture-and-symbol-identity.md)部分重叠，仍持有共享记录与模块符号格式；[跨沙箱只读授权](../architecture/2026-10-28-harmony-reporting-authority.md)仍持有权威来源；[采集与队列代次](../bug-fix/2026-10-28-epoch-bound-crash-collection.md)仍持有 Rust 认领边界。没有完全吸收或需要废弃的提案。

## 官方依据

[setEventParam API](https://raw.githubusercontent.com/openharmony/docs/master/en/application-dev/reference/apis-performance-analysis-kit/js-apis-hiviewdfx-hiappevent.md)从 API 12 提供本生命周期自定义参数，OS 域支持崩溃与卡死，每个参数值上限 1024 字符。[崩溃示例](https://raw.githubusercontent.com/openharmony/docs/master/en/application-dev/dfx/hiappevent-watcher-crash-events-arkts.md)与[卡死示例](https://raw.githubusercontent.com/openharmony/docs/master/en/application-dev/dfx/hiappevent-watcher-freeze-events-arkts.md)把自定义字段直接随系统 params 投递。SDK 本机声明与文档一致。

[事件规格](https://raw.githubusercontent.com/openharmony/docs/master/en/application-dev/dfx/hiappevent-watcher-crash-events.md)明确输入法应用的系统订阅从 API 22 支持，当前包最低 API 21。API 21 兼容的原生/ArkTS 致命故障采集必须继续实现和设备验证；本次原身份传递不能被表述为补上那个入口。

## Alternatives considered

- **持久保存按 PID 和时间索引的本地捕获表**：不依赖自定义系统字段，但 PID 可复用、缺失进程结束时刻、两沙箱不共用队列，会引入额外历史保留与匹配歧义。系统提供原生命周期字段，直接让系统保留原身份更明确。
- **沿用系统版本并用补报构建填充其余字段**：接线简单，但把升级前的故障误标为新源码，不满足精确符号化的目标。

## Verification

完整 `platforms/harmony/tests/run.sh` 通过。新增原身份策略 29 项断言，实际 `.ets` 适配器保留十一组生命周期回归并新增四组异步采集回归，覆盖系统参数串行、停止后的迟到完成、暂停恢复、失败重试、关闭重开时旧参数晚装入、原 UUID/原头传给 NAPI 与卡死独立身份。暂时在已发射的实际策略中移除代次比较，回归在“关闭重开不能认领旧系统参数”的断言失败；恢复后 29 项重新通过。

先 `ohpm install` 后实际 HAP 编译与打包通过（5.512 秒），使用既有原生库，仅证明 ArkTS 类型与打包。首轮因 `Record<string, hiAppEvent.ParamType>` 的非空对象字面量触发 ArkTS 限制，改为空记录再按键赋值后重编通过，不计首轮为通过。ArkTS subset、私有文本读取、原身份无文件回退及隐私门禁均通过。

实际系统回调是否携带字段、同源 NAPI/共享层入队和后台精确符号化尚未设备验收，不能用替身或 HAP 编译代替。API 21 输入法兼容采集仍未实现。完整 quick 与其余平台 CI 的进度由全平台提案持续跟踪。

## Consequences

原版本、源码提交、架构与持续授权在故障前固定，补报不随当前安装改变；崩溃用原 UUID，避免 PID 复用错误归属；卡死保持独立。删除无读取者的键盘 PID 文件和 ArkTS 遥测文件读写，存储与去重仍由共享层负责。

代价是 `setEventParam` 异步安装完成前的故障可能没有原身份，这些报告保守不认领。已提交给系统的参数不能同步撤回，Rust 继续按当前 epoch 拒绝旧身份；参数请求串行，停止后的迟到结果只促使最终清空，不重启采集。API 21 兼容入口与真机收件仍有缺口，不能由本篇称全平台完成。
