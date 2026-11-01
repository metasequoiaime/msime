# Agent Note: Linux 原生与辅助进程崩溃采集

Status: implemented

## Problem

Linux 的 IBus、Fcitx5、准备程序、provider、剪贴板监听和语音辅助进程没有共同的崩溃采集入口。输入法宿主已有共享遥测生命周期，但独立工具若自行猜测偏好目录，可能把另一套配置的授权当成自己的授权；Python 子进程还可能把异常消息和用户路径写入记录。发布包另外会剥离原生模块，导致后台只能看到偏移而无法按构建还原源码。

## Decision

Linux 入口统一复用共享 `CrashRecorder`、`Telemetry` 和 Host API，只在进程启动后取得实际配置目录时建立 `CaptureOnly`。原生辅助工具的显式偏好目录必须是绝对路径；没有父进程明确传入时，从运行时 options 解析 `preferences_directory`，无法证明归属就停用采集，不回退到另一份默认目录。IBus 与 Fcitx5 保持各自的遥测目录和会话边界。

Python 入口通过随安装包分发的 `libmsime-python-crash.so` 桥接同一套原生记录器。桥接只接受固定组件名、绝对路径和经过筛选的文件名/函数/行号；未捕获异常只写合成摘要和产品帧，不写异常文本、变量或私有目录。HTTP worker 只继承父进程明确选择的偏好与 options 路径，不从请求内容推断授权。查询、帮助和下载清单路径不初始化采集。

语音和在线 provider 的停止处理器只设置标志，在 `socketserver` 的安全点退出服务循环，再执行既有的线程、子进程、套接字和锁清理。这样信号落在 Python 线程注册锁期间不会重入同一把锁，停止也不会被崩溃采集误记成故障。发布构建在保持 Release 优化的同时保留行号，先归档带构建身份的符号，再剥离安装副本；符号只在明确发布调用时上传到维护者专用的 Linux 归档。

## Alternatives considered

- **每个入口自行实现信号处理和偏好读取**：局部改动少，但会复制授权代次、记录头和路径安全规则，未来撤回或升级时容易出现不同步；因此统一走共享捕获器。
- **缺少路径时沿用默认配置目录**：对旧启动方式兼容性较好，但无法证明这个进程属于哪套配置，可能在撤回后写入新授权队列；因此无法证明归属时停用。
- **Python 直接把 `BaseException` 文本写入记录**：诊断信息更丰富，但异常消息可能包含输入、账号或私有路径；因此只保留固定摘要和白名单源码帧。
- **在信号处理器中启动关闭线程或抛异常**：响应看似立即，但会在 Python 线程注册锁上重入或被弱引用回调吞掉；因此改为标志加服务循环安全点。

## Verification

- 所有 Linux Python 文件通过 AST 语法检查，`git diff --check` 通过。
- Linux 构建门禁新增真实原生辅助程序、Python 八入口、IBus 启动补报、停止竞态和 full/wubi 安装目录复跑；CI 需在 Linux 容器中完成编译与 ctest。
- Linux 发布和 legacy 发布脚本归档 DEB/RPM 原生符号与构建清单，安装副本仍保持 Release 优化并由 CPack 剥离。
- pve9 当前缺少 Docker、CMake、Ninja、pkg-config 和 Rust 工具链，因此本轮不能把远端命令行静态检查扩大成 Linux 编译通过；待 CI 或具备完整工具链的 Linux runner 返回结果后，将把真实测试结果补入 PR 验证段。

## Consequences

Linux 独立入口在授权目录无法确认时宁可不记录，避免跨目录污染；Python 故障记录可按版本、模块和源码帧还原，但不包含异常消息。停止处理的安全点受现有服务循环轮询间隔限制，仍遵守原有清理期限。发布流程额外保留维护者符号归档，用户安装包不携带调试行。

共享语义见[共享崩溃记录基础](../../implemented/architecture/2026-11-01-shared-crash-reporting-core.md)。
