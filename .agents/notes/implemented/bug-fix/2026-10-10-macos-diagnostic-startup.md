# Agent Note: macOS 首次会话诊断日志时序

Status: implemented

## 问题

macOS 输入法在 `input_method_main` 启动时注册了 Host API 诊断出口，但诊断日志原先只在首次异步偏好加载完成后配置。首次 `MSIMEClientSession` 构造会同步加载辅助码、音效等插件；这些可恢复失败发生在日志配置之前，因而不会写入 `diagnostic.log`。

## 决策

在 `prepareSession` 调用 `MSIMEClientSession` 之前，先从 `preferences_directory` 同步读取当前偏好，用 `preferences.diagnostic_log.server` 配置诊断日志；读取失败时才退回 `runtime-options.json` 的快照。异步偏好加载完成后仍沿用同一入口按最新文档重新配置。这样保留了日志默认关闭、目录安全校验和仅记录类别的隐私边界，同时覆盖首次会话构造期间的诊断。

## 后果

首次会话构造期间产生的可恢复插件失败会进入已启用的 macOS 诊断日志；默认关闭、非法目录拒绝和异步加载后的最新偏好覆盖行为保持不变。

## 备选方案

没有在 Host API 内增加跨启动的诊断队列：这会扩大进程级状态和生命周期契约。直接使用启动时已有的运行时快照即可覆盖首条报告，且继续由现有日志安全校验负责落盘。

## 验证

- `cmake --build target/macos-isolated --target shortcut-test -j2`
- `ctest --test-dir target/macos-isolated -R '^shortcut$' --output-on-failure`
- `cargo test -p msime-host-api --lib plugin_failures_reach_the_registered_diagnostic_sink -- --nocapture`
- `cargo fmt --all -- --check`
