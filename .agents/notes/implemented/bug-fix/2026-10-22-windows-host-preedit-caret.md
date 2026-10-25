# Agent Note: Windows 预编辑文字与光标共用宿主快照

Status: implemented

## Problem

#9158 报告 Windows 全拼、原始拼音预编辑中光标偶发停在串首，当前报告尚无稳定触发步骤，也未确定受影响应用。先前排查发现 `_HandleCompositionInputWorker` 从宿主视图绘制文字，却从没有随宿主字符入口更新的旧键击缓冲计算光标；即使宿主光标在末尾，空旧缓冲也会把 TSF 选区放到组字开头。已有修补统一了状态来源，本次增量处理范围定位失败后仍发布错误选区的防御缺口；两者均有独立源码回归，仍不能认定已复现或消除报告者的偶发现象。

## Decision

[回车提交观测](2026-10-24-windows-enter-commit-observation.md)单独持有发送给 Server 的原文载荷，本篇继续负责预编辑绘制。已有按键快照的视图转换由 `KeyCompositionStateFromView` 持有，绘制时从同一次 `host->view()` 的结果取得文字、原文与光标。没有宿主会话时保留旧缓冲；有效宿主视图读取失败则返回失败，不绘制另一份旧组字。分词符映射、已选词前缀、隐藏预编辑、锁定光标方案和整句改字的优先级保持现有规则。

TSF 范围定位归 `KeyHandler.cpp` 的局部 `SetCompositionCaret` 持有，绘制、纯光标移动与左右移动三处复用：先检查起点折叠、`ShiftEnd` 返回值和实际移动长度、终点折叠，全部成功后才调用 `SetSelection`。范围引用仍由原调用方释放；定位失败不发布临时范围，也不猜测跨区域的偏移。纯光标移动传播局部定位函数的失败，`GetRange` 失败或返回空范围仍沿用原有成功返回；文字已经写入的绘制路径保留成功结果，避免把完成的输入交给失败重放。未改变引擎光标、按键排队或 TIP/Server 协议。

## Historical audit

检索 `MapRawCaretToPreedit`、`GetRenderedCaretPosition`、宿主光标及快照。已有[邮箱与网址补全](../../implemented/feature/2026-10-15-engine-email-url-completions.md)持有按键分类快照，本次复用它的转换规则，绘制入口单独归本笔记；[整句改字](../../implemented/feature/2026-10-10-sentence-conversion-edit.md)的转换文字与焦点偏移继续优先；[组字光标](../../implemented/feature/2026-10-08-composition-caret-tap.md)持有引擎编辑语义，不修改。macOS 标记文字的笔记只涉及另一宿主，无需迁移。

## Alternatives considered

- 直接调用 `GetKeyCompositionState()` 最少改动并复用现有接口；但它会再次查询宿主，文字与光标无法由同一次读取保证一致，也增加按键路径上的查询。
- 每个宿主按键后同步旧缓冲能让原调用继续工作；但引入第二份组字镜像，选词、取消与部分提交都必须同步，与既有状态归属相冲突。
- 将定位失败作为整次绘制失败返回，能直接复用错误传播；但此时文字可能已经写入，不能让调用方把完成的输入再次重放。因此定位错误只阻止发布错误选区，纯移动路径单独返回失败。
- 在三个调用点分别补判断，能减少辅助函数；但范围移动的检查顺序、部分移动的处理及错误传播容易分叉，因此复用一个局部定位函数，引用所有权保持原位置。

## Verification

`scripts/test-windows-preedit-caret.py` 从生产源码提取并编译完整 `_HandleCompositionInputWorker`，通过合成宿主视图与 TSF 文本范围记录文字写入及 `SetSelection` 的位置；旧源码在空旧缓冲的第一个续打字母上失败，修复后通过。覆盖残留旧缓冲、组字中间和越界光标、分词符、中文与扩展字词段前缀、仅含前缀的组字、日语读音与词中字母、隐藏样式、锁定光标方案、整句改字、旧入口、视图读取或解析失败及写入失败。每次绘制只查询一次宿主。测试模型不替代真实 TSF、Windows 编辑器或 Grok Bot 快速续打验收。

前一版宿主快照修补已通过 Windows 11 的 MSVC 2022 原生编译，包含 Rust Host API、实际 TSF DLL 与三项相关测试目标；`msime-tsf-preedit-caret`、`msime-tsf-host-composition` 和 `msime-tsf-key-composition-state` 的 CTest 全部通过。完整应用按键回放 183 项通过；远端 `bash scripts/verify-local.sh --quick` 通过，其中当前宿主可运行的 Windows 测试为 164 项通过、2 项因宿主差异排除、76 项需要 Windows 构建。原生构建使用已有 vcpkg 工具链，未修改依赖或构建配置。这些既有原生结果不覆盖后补的范围定位检查。

范围定位增量从远端 `develop` 固定 `42eb83abba0a00cd8f5a71d5c2d247765850d23c` 提取真实纯移动方法，合成 TSF 故障探针在旧代码上 16 项检查有 8 项失败；起点折叠失败、移动失败、移动不足与终点折叠失败仍发布错误选区并掩盖失败。既有完整绘制测试增加同样的故障注入后，在精确旧生产源码下失败，修补后通过；同时执行生产纯移动方法，覆盖局部定位失败传播、单次范围释放、`SetSelection` 失败及绘制后连续移至中间、末尾和开头；夹具每次取得独立临时范围，并检查 composition 的稳定范围不变。失败时不调用 `SetSelection` 的保证只适用于此前的定位步骤，合成 provider 在 `SetSelection` 自身失败后的状态不证明真实 provider 的行为。文字写入与前后选区由合成宿主记录，不冒充真实 COM 实现，也不能据此认定报告者具体应用的触发条件已经复现。

首次增量在 Windows 11 上从同一固定基点及三份本地改动构建：复用既有依赖缓存后，`cargo build --locked -p msime-host-api` 成功；MSVC 19.44 完成实际 x64 TSF DLL、Server 和两项光标测试的编译链接。补齐本分支 Host API 与已有依赖的 DLL 搜索路径后，`windows-preedit-caret`、`msime-tsf-preedit-caret` 两项 CTest 全部通过。首次配置选错依赖前缀和首次测试缺 DLL 的日志保留，不将未启动的测试算作断言失败；原生构建仍有既有编译警告，不声称严格无告警。提取回归另在 Windows 实测暴露 `_In_` 与 MSVC `sal.h` 冲突，夹具只在宏尚未定义时提供空定义，修正后 macOS 与 Windows 均通过，未关闭告警。完整 quick 与实际 Windows 编辑器验收仍待完成。

完整 Windows 门禁另外检出受控 TSF 编辑器的 MSVC 编译阻断：局部 `std::min` 和 `numeric_limits::max` 调用用括号避开 Win32 宏，不全局禁用仍被旧编辑控件使用的宏；省略未使用的形参名，删除未使用的局部变量，并把鼠标订阅的首个 cookie 在声明处初始化。保留 `/W4 /WX`；编辑器库与 demo 两个目标已在 MSVC 19.44 编译链接通过。Git Bash 的 Python 3 占位程序和 `link.exe` 同名解析问题仅在任务进程环境内处理，不修改系统配置。固定基点的偏好测试缺字段与重复移动值，精确复用已发布提交 `0d0f33939e43dc758df1a37372caf3c73ca179f0` 的单文件修补；不改产品默认值。修正调用环境后的完整 quick 仍失败：逐翻译单元探测器的链接诊断与代码页假设、TSF 测试缺少 UTF-8 编译选项，以及三项测试的严格 MSVC 告警仍阻断。原始失败保留；后续对应修补与实际验证由[Windows 门禁兼容记录](../../implemented/testing/2026-10-14-windows-local-gate-portability.md)持有，不能用两项光标 CTest 代替整轮 quick。

分支已快进到远端 `develop` 固定 `c208d5748a4dd9e271b8a52ec83b19d5071b7a2a`，保留当前范围定位增量并消除已合入的重复门禁修补。整合后发现公开 C 头与偏好测试中残留的 diff3 分隔行直接阻断编译，同时清掉 iOS README 的相同孤立分隔行；没有选择另一套产品实现。六项原生 CTest 在更新基点前全部通过，但当轮完整 quick 退出 1；更新基点后仍须重新运行，不能沿用旧产物结果。

更新基点后的同源 Windows 11 验证现已完成：五笔生命周期 32 项、六项真实编译器诊断、预编辑生产方法提取回归和 5 项 MSVC CTest 通过，后者包含两项预编辑、C++17／20 UTF-8 路径与日语快捷键。实验编辑器库与 demo 编译链接成功，194 项偏好测试和 `cargo clippy --locked -p msime-client-core --all-targets -- -D warnings` 通过，完整 `bash scripts/verify-local.sh --quick` 返回 0；原生 Host API、TSF／Server 及 pipe-only 编译完成。运行前逐一核对本地与远端 27 份改动文件的 SHA-256；系统缺少工具的阶段仍按门禁原规则跳过。此结果仍不覆盖真实编辑器验收，也不关闭报告中的偶发问题。

## Consequences

预编辑绘制不再依赖另一条输入入口的旧缓冲，也不引入需要同步的第二份组字状态；代价是每次绘制保存当前原文的 UTF-16 快照。生产方法提取测试依赖方法边界，源码结构变化时必须同步测试。当前 Windows 验证机存在断开的桌面会话，本轮在 SSH 服务会话中构建和运行原生测试，尚未完成桌面上的实际编辑器验收；原生构建与合成模型分别报告，不将模型执行当成现场复现。
