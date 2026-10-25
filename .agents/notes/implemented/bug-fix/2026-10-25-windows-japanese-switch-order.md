# Agent Note: Windows 日语方案切换与后续按键的顺序

Status: implemented

## Problem

#9431 的快捷键把保存请求交给独立线程后立即返回；TIP 的模式分类先随通知改变，本地宿主会话却仍由独立缓存和 500 毫秒定时器刷新。下一键可能在任意一端沿用旧日语方案。已有[唤醒与缓存失效修复](../../implemented/bug-fix/2026-10-23-windows-japanese-switch-wake.md)缩短延迟，但明确不承诺同修订交接，本篇部分补充其边界。

## Decision

新增双方显式协商的可选能力。快捷键的保存结果通过有界工作线程的 future 交给 SessionPump；只在管道线程等待，不在输入队列或焦点锁内等待。保存后取得不可变完整快照，Pump 在输入队列发布，随后给发起焦点与请求发送带修订号的 worker 回执，才继续读取下一键。没有执行或 CAS 拒绝也回复，不重试不确定送达的切换。

配置广播为新版端点发送带存储修订的模式通知，旧端点保持原来的单字符帧。TIP 在发送快捷键前建立屏障；后续键进入既有 FIFO 并延迟分类。worker 只校验和交接数据，所属 STA 核对焦点、请求和配置缓存修订，调用宿主 update_preferences 成功后更新模式并放行。等待指定修订时短暂轮询缓存，不把后台读取完成当成新快照已到；超时使会话失效，禁止把切换当普通键重放。

## 历史笔记审计

旧唤醒笔记部分重叠，保留其工作线程生命周期、通知不丢失和缓存重读理由并互链。本篇承担新的修订回执、FIFO 与失败处理，未否定旧决定。既有 Server RevisionFence 只限制发送代次，不替代共享存储 revision。

## Alternatives considered

- **缩短刷新周期**：实现小且能改善常见延迟，但无法保证下一键之前完成两个独立队列，继续保留竞态，因此不用。
- **在输入队列直接保存或等待**：能自然排列按键，但会把文件锁等待带入全局输入队列并阻塞其它宿主，因此不用。
- **经 worker 传完整偏好**：能省去 TIP 重读，但现有固定帧需要额外分片与大载荷生命周期，且宿主路径与词典配置仍由本地缓存持有；本次采用经过校验的修订号与完整本地快照交接。

## Consequences

切换请求现在把保存结果交给 Pump 的 future；等待时释放全局事务锁，不占输入线程或焦点锁，最多等两秒并检查停止、断连与焦点失效。发布按至少该修订收敛，监视器晚到的旧读取也沿用实际已发布的新值；同修订内容冲突仍拒绝。新焦点在 FocusSessionReady 之前接收所需修订。TIP 的既有 FIFO 保留后续键，完成回执与完整缓存快照同时满足后才应用偏好并分类；宿主本身保留正在组字时的延迟应用规则。模式从刚应用的同一快照计算，不能再读可能已变新的缓存。音节型方案的上屏继续走原 edit session 路径。

代价是增加一个可选能力位、固定 worker 帧和等待期间的短轮询。普通空闲刷新仍维持原周期。窗口消息只唤醒所属线程，有界邮箱持有值而不传裸指针；消息暂满时普通定时器也能消费。断连或焦点清理丢弃旧屏障，切换不按不确定送达重放。

验证现状：真实 Windows 11 的新增 Pump 用例先在 `next_key_used_saved_preferences` 断言失败，再由保存快照入队修复转绿。扩展用例覆盖立即完成、待完成、拒绝三种结果及两种协商状态，待完成用例让无关输入任务先完成以证明等待不占输入队列；进一步从真实日语引擎起始，核对下一键的 `view.scheme` 已切回全拼，拒绝保存时仍为日语。新焦点还检查修订帧先于 `FocusSessionReady`。

Windows 构建先用既有 Rust 缓存定位回归，再复制依赖缓存到本分支规定的 `target/debug`，只重编工作区 crate，在 1 分 7 秒内完成同源 host-api；MSVC 的 Server、TSF DLL 和测试随后重新链接并使用这份 DLL。Windows 会话、输入队列、配置帧、协议与屏障、真实命名管道 I/O、Aux 监听器、异步缓存、日语快捷键接线共八项通过。本机 Clang 独立协议与屏障运行通过，笔记树、格式和四份归档封印通过。

扩展的 `windows-server` 测试在候选窗口可见性断言（`server_smoke.cpp:140`）失败，尚未进入其管道测试阶段；当前 SSH 位于服务会话、没有活动交互桌面，不能把失败计为通过，也未据此改基线或绕过断言。真实编辑器中的切换、组字保持与断连验收仍缺。完整 quick 首轮在未改动的 Linux Fcitx 编译处失败：两个独立测试缺少语言标准声明，Clang 14 也不能直接捕获结构化绑定的 `id`。同批补齐与其它 Fcitx 目标一致的 C++20 声明，并把按值捕获改为同值的初始化捕获；不改变候选行为。第二轮 Linux 原生构建与测试已通过，但五个提取生产方法的白盒夹具尚未声明新增屏障成员或仍假定快照直接在定时器中应用。补齐真实屏障与请求类型后，延迟分类回归增加修订屏障、待消费通知阻挡 FIFO 及释放后只派发一次的检查，恢复回归检查焦点清理同时清掉旧屏障和邮箱；五笔、显式唤醒和快照应用边界检查保留。第三轮 `bash scripts/verify-local.sh --quick` 在原编译器与原构建目录退出 0：Windows 可移植用例 166 项、契约与笔记检查、Rust workspace、Android Rust 与 JVM、Wasm、Harmony 设置打包、Linux Tauri 外壳及原生宿主构建和测试通过。该 Linux 环境没有 Apple、MinGW 与完整 Harmony ArkTS 打包条件，脚本明确跳过相应阶段；Windows 原生覆盖由上述真实 Windows 八项补充。issue 使用 `Refs` 并保持开放。

PR 的 Windows GNU 交叉构建发现共享 IPC 契约仍把 `MaxKnown` 断言为旧尾项 `HostOnlineCandidates`（36），新增 `PreferenceTransition`（37）后因此编译失败。保留旧编号断言并增加新编号及尾项断言。此前本地可移植 Windows runner 只发现 Server 与 TSF 测试目录，没有运行 `shared/contracts/tests`。共享契约现在由 `scripts/test-shared-native-contracts.py` 自动发现 `*_contract.cpp`，以 C++17、完整版本定义、严格警告和启用断言的宿主编译器编译运行原测试；缺编译器明确跳过，编译或运行失败均失败。独立入口避免把不依赖 Windows 的测试与平台测试的可移植性跳过规则混在一起，也不复制测试断言。该入口通过 `run-checks.sh` 自动进入 quick 与契约 CI；先复现同一 `37 == 36` 编译失败，再修正契约。修复后本机 Clang 与 Linux GCC 均运行原四份契约通过，真实 Windows MSVC 的 `windows-ipc-contract` CMake 目标编译、ctest 通过；远端同目录完整 quick 再次退出 0。

修正契约后的 GNU CI 编到 99% 才暴露第二个限制：`server_main.cpp.obj` 含 33131 个节，超过普通 PE/COFF 的 32768 节上限。Server 入口目标现在仅在 MinGW 的 GNU C++ 编译中传递 `-Wa,-mbig-obj`，采用工具链支持的大对象格式；CMake 的语言与编译器生成表达式避免把选项交给 RC 或 MSVC。该选择保留调试信息与原优化级别。拆分入口单元能降低模板实例数量，但会扩大这次顺序修复的架构改动；关闭调试信息也能降低节数，却削弱调试能力，二者均未采用。限制与选项依据 [GNU as 的 i386 选项说明](https://www.sourceware.org/binutils/docs/as/i386_002dOptions.html)。真实 Windows 的 MSVC Server 目标重新生成、编译链接通过；远端原目录完整 quick 再次退出 0。GNU 的完整编译与链接仍由新提交的 CI 核实。

官方约束参考：[PostMessageW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postmessagew)、[TSF edit sessions](https://learn.microsoft.com/en-us/windows/win32/tsf/edit-sessions)。消息投递不代表执行完成，文档变更必须保留 edit session。
