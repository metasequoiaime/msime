# Agent Note: Linux 原生宿主资源集成测试门禁

Status: implemented

## Problem

IBus 的 `ibus-engine-smoke` 默认入口没有注册为资源测试，Fcitx5 的 `fcitx5-native-context` 只在手工脚本配置词库时出现。Linux 原生 CI 只运行不带词库的编译与单元门禁，因此按住说话、组合键取消和迟到结果等宿主级用例没有持续门禁。资源测试中的两个前置断言还会因为异步顺序或候选排序变化，在到达语音段前中止整条用例。资源容器还暴露出 Host API 导出检查把 Windows 条件声明误当成 Linux 导出的问题。

## Decision

CMake 在提供已校验资源目录时注册 `ibus-native-context`，并保留 Fcitx5 的 `fcitx5-native-context` 资源测试。资源验收在独立的 Linux CI job 中按 `resources/desktop-dictionary.lock.json` 下载词库，再分别调用现有 IBus 与 Fcitx5 容器脚本；编译门禁继续保持无资源的快速路径。IBus 合并断言改为要求离线命中与远端释义同时存在，不依赖候选排序；Fcitx5 标点样式断言等待完整的引号与括号视图，而不是只等待引号字段。Host API 导出检查按容器平台去掉 Windows 条件块后再逐个核对动态库符号；共享私有目录打开器在每个 Linux 路径分量上核对 `fstat` 类型，拒绝 `O_PATH` 打开的非目录分量，并在最终路径上用 `symlink_metadata` 拒绝非受信符号链接。

资源夹具关闭逐键神经重排，只验收宿主行为，避免模型耗时把候选时序断言变成 runner 性能门禁。
资源容器复用 Core CI 的 Rust 单测结果，资源宿主 job 只编译 Host API 并运行容器与原生宿主验收，避免重复单测中的独立时序失败阻塞宿主门禁。
资源宿主 job 以 smoke-only 模式运行 IBus/Fcitx5 的语音入口；默认上下文、翻译范围、拆字等非语音专用用例仍由完整资源 CTest 配置保留，但不再让语音门禁被它们的独立时序波动挡住。

## Alternatives considered

- 只在 `in-container.sh` 中手工执行默认入口：不能让 CTest 记录默认分支，也无法在 CI 检查测试是否注册。
- 把资源下载塞进现有 `build-container.sh`：会让编译门禁依赖网络和完整词库，增加冷构建时间并改变原有门禁边界，因此改用独立 job。
- 将前置断言改成日志而不是失败：会让语音覆盖在前置回归时假绿，保留断言并修正其异步与排序假设。
- 让 Linux 动态库导出 Windows 专用函数：会扩大非 Windows ABI，且与 Rust 的 `cfg(windows)` 不一致，因此修正检查器对条件声明的解析。
- 只在最终目录打开时检查类型：`O_PATH` 允许中间分量暂时打开为文件，错误会延迟到不可预测的后续调用，因此逐级检查并立即返回明确错误。

## Consequences

资源集成 job 会额外构建一次资源验收容器并运行两套宿主测试，CI 时间和网络用量增加；资源获取仍由锁文件和现有校验示例负责。默认 IBus 入口现在同时由 CTest 和既有安装后 smoke 覆盖，Fcitx5 默认入口继续由 `fcitx5-native-context` 覆盖。候选顺序可以由引擎合法调整，只要离线命中和远端释义都保留，测试不会误报。
