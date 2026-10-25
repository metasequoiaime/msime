# Agent Note: Windows 词库维护期间延迟新连接的会话创建

Status: implemented

## Problem

#9434 报告 Windows 设置新增快捷短语和五笔词条时失败，偶尔提示词库被占用。Server 的 `quiesce_dictionaries` 销毁既有会话，却没有阻止 `InputState::connected` 为随后接入的新客户端创建会话；新会话重新获取共享词库锁，设置进程就不能取得独占维护锁。这是源码中可以确认的竞争路径，尚未取得报告现场的证据，不能将所有新增失败都归因于它。

## Decision

维护期间仍登记连接及字段属性，但新客户端不创建 `FocusedSession`。维护结束由现有 `resume_dictionaries` 为全部无会话客户端统一重建；正常连接继续立即创建。会话创建及销毁始终在输入队列线程进行。

会话回归中的有效偏好重试使用 `PreferenceSnapshot::load` 返回的同一份规范文档。共享存储会迁移旧文档并补上 `smart_punctuation_migrated`；发布规范快照后，不能用同修订的原始最小 JSON 模拟幂等重试。陈旧修订、同修订不同内容、超限文档和过期焦点的拒绝断言仍保留。

同一套回归对关闭拆字模式的 `Shift+C` 沿用引擎已有契约：保留普通组字 `C`，而不是要求组字为空。断言同时检查 `handled`、`local_mode=none` 和文字内容，然后通过真实 Escape 路径清空，避免污染后续本地模式用例。`V`/`C` 只有入口字母时还会列出已有字面输入规则允许的符号，精确符号断言包含这些符号，并在输入表达式数字后检查它们退出入口状态。

## Historical notes

- [Windows 词库维护让 TIP 宿主会话放开，设置窗口写进输入法读的代次](2026-10-25-windows-dictionary-maintenance-tip-sessions.md)：部分重叠，同为 #9434。本篇只处理 Server 进程内维护期间的新连接；那篇处理 Server 管不到的持锁者（各应用里 TIP 的宿主会话）、设置窗口写错代次和结果不在视野里。
- [词库访问租约结束时释放全部文件锁](../../implemented/bug-fix/2026-10-15-dictionary-access-lock-release.md)：部分重叠，处理 Rust 租约析构；本篇处理 Windows 宿主在维护窗口内重新创建持锁者，互链保留。
- [个人词库操作结束时显式释放锁](../../implemented/bug-fix/2026-10-15-personal-dictionary-lock-release.md)：无关，处理 Unix 描述符继承。
- `proposed` 与 `rejected` 中没有维护连接相关决定；其他维护命中涉及平台安装、资源准备或批处理，不规定 Windows 新连接的会话生命周期。

## Alternatives considered

- 在原始夹具里硬编码迁移标记能消除这次差异，改动也小，但夹具仍会绕过共享存储的其他规范化规则；直接复用已加载快照与真实发布路径一致。
- 放宽 Host API 的同修订冲突保护能接受这份旧格式文档，但会使真正的并发写入冲突静默通过；修订相同而内容不同的请求仍必须拒绝。

- 维护期间拒绝新连接可以阻止重新持锁，但会让 TIP 承受断连和重试；既有恢复逻辑已能处理无会话连接，保留连接更直接。
- 延长设置进程重试可以吸收短暂竞争，但新会话会一直持有共享锁，等待不能修复维护期间再次创建会话的缺陷。

## Verification

`windows-input-queue` 新增不含有效引擎配置的维护连接回归：维护期间新连接和同客户端重连均成功登记，重复释放为零，随后可断开。旧实现会尝试创建会话而失败。`windows-session` 的真实后端回归覆盖先释放原连接，再登记新连接及重连、重复释放为零、恢复两个会话和再次恢复不重复创建。

本机 Clang 和 MinGW x64 对 `InputQueue.cpp`、`input_queue.cpp`、`session_smoke.cpp` 的 C++17 严格编译检查（`-fsyntax-only -Wall -Wextra -Werror`）通过。原版真实 Host API/MSVC 会话回归因规范快照后的原始 JSON 重试退出 8；修正后先后暴露并修正已有 Shift/C/V 夹具预期，保留每轮红灯日志。

在固定 `develop` 基点 `e2dbbd6c246d6c94b0beb6edb521c3b6469605f9` 上，Windows 11 runner 的真实 Rust Host API 与 MSVC Debug 六项 CTest 全通过（7.54 秒），包括 `windows-session`、`windows-input-queue`、C++17/20 路径、Caps Lock 和日语切换；六项真实编译器诊断回归通过。逐文件 SHA-256 在运行前后核对一致，完整 `bash scripts/verify-local.sh --quick` 退出 0，输出 `quick check passed`：131 项逐翻译单元测试通过、2 项沿用宿主差异排除、116 项交给原生构建；Rust workspace、原生 Host API／TSF／Server 和 pipe-only 编译通过，1810 篇笔记与 4 项封印验证通过。

缺少前端依赖、JDK、Android SDK/NDK、DevEco、WASM、Docker、Apple 工具及参考输入的阶段仍按既有条件跳过。验证机交互桌面会话断开，Windows 设置页真实新增及 TIP／Server／编辑器组合验收仍未完成，不能据此认定报告者全部新增失败均已解决。

## Consequences

维护期间新客户端暂时不能使用引擎，与已有客户端一致；恢复失败仍沿用现有重连回退。需要补齐真实 Windows 新增词条验收才能认定 #9434 全部解决。
