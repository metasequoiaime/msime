# Agent Note: HarmonyOS 悬浮工具栏首帧补发内容

Status: implemented

## Problem

HarmonyOS 2-in-1 的悬浮工具栏有时只显示主题背景，按钮和拖动把手不出现。源码里存在一条可能丢失首次状态的时序：宿主在 `setUiContent()` 完成后立即刷新会话，但 ArkUI 页面可能还没有执行 `aboutToAppear()`，因此 `KeyboardSession.onToolbar` 尚未注册；页面随后只注册回调，却不会再收到当前状态，直到输入模式变化。拖动把手位于按钮遍历之外，空按钮数组本身不足以解释把手也消失；这部分现象以及实际生命周期时序仍需设备确认。

## Decision

`FloatingToolbar` 在 `aboutToAppear()` 注册 `KeyboardSession.onToolbar` 后立即调用 `KeyboardSession.shared.refreshToolbar()`。状态回调和绘制仍由同一份会话状态驱动，补发只覆盖页面生命周期与宿主刷新之间的竞态。

## Alternatives considered

- 只在宿主 `openToolbar()` 延迟或重复刷新。理由是可以不改 ArkUI 页面；没采用，因为延迟长度依赖系统调度，仍可能在页面回调注册前执行，也会让首帧时序变得脆弱。
- 给页面预填一组默认按钮。理由是背景出现时也能立即画出内容；没采用，因为页面不知道当前用户关闭了哪些组件、当前方案和主题，默认值会短暂或永久地与会话不一致。

## Consequences

- 工具栏页面挂载后会主动拉取一次当前状态，按钮、状态文字和主题不再依赖下一次模式变化才初始化。
- 这次刷新要求 `KeyboardSession` 已完成偏好准备；宿主只有在会话建立后才打开该面板，因此不会改变无会话时的默认行为。
- 首次挂载会多一次同步状态回调；该回调只读会话状态，不创建面板、不访问网络或写入文件。

## Verification

- 在 Windows 隔离 worktree 中执行 `node D:/Workspace/msime/apps/desktop/node_modules/typescript/bin/tsc --project platforms/harmony/tests/tsconfig.json`：类型检查和编译通过，输出使用规定的 `target/harmony-tests`。
- 按 `platforms/harmony/tests/*.test.ts` 枚举源码，对每份产物执行 `node target/harmony-tests/tests/<测试名>.js`：10 个 TypeScript 套件全部通过，共 4468 个断言或检查。其中键盘逻辑有 4080 个断言，自定义翻译探测有 61 个断言。
- 原先 `custom-translation-probe` 的失败来自 Windows CRLF 检出：测试用 `\n}\n` 截取 Rust 函数边界，CRLF 下找不到结束位置，并非缺少 DeepL 密钥或服务环境。提取前统一 CRLF 后通过；额外在内存中分别提供 LF 和 CRLF 的 Rust 源码运行该探测测试，两种都通过，未改动 Rust 文件。
- 本次没有运行 `tests/run.sh` 末尾的原生 C++ 音效编译和测试，上述结果只覆盖该脚本的 TypeScript 部分。
- `pnpm run verify-notes`：笔记目录、格式、相对链接与归档封印检查通过。
- 本机没有找到 `ohpm`、`hvigorw` 或 `hdc`，也没有暂存 HAP 所需的原生库、词库资源和 `oh_modules`。没有编译 `.ets` 或在 HarmonyOS 设备、2in1 模拟器上验证首帧；TypeScript 套件不编译 `FloatingToolbar.ets`，不能代替这两层验证。
- 后续需按 `platforms/harmony/README.md` 暂存构建依赖，执行 `ohpm install` 与 `hvigorw assembleHap --no-daemon`，再在 2in1 检查首次显示、关闭后重开，以及非默认按钮配置和主题的首次显示。

## 2026-11-19：编辑器重新聚焦时恢复工具栏

此节记录的全屏隐藏策略已由[全屏编辑器显示工具栏](2026-11-19-harmony-fullscreen-toolbar.md)取代，焦点重新绑定的恢复目标仍然保留。

用户反馈切换窗口后悬浮工具栏消失。此前宿主只在自己记录的全屏状态发生变化时调用 `show()` 或 `hide()`；系统若自行收起状态面板，宿主的缓存仍是「未隐藏」，下次切到非全屏编辑器时就不会请求显示。源代码存在这条可能路径，但没有读取设备日志，不能断言它是用户现象的唯一原因。

每次非全屏编辑器绑定时都请求 `show()`，全屏窗口继续沿用原有隐藏规则。`ToolbarFocus` 串行化对同一面板的异步显示和隐藏，并用焦点代次丢弃旧窗口的迟到查询；编辑器解绑或面板被关闭时废止旧代次。窗口状态查询失败时记录错误并尝试显示，避免一次查询失败让工具栏无法恢复。面板创建期间到达的编辑器也在挂载后执行同样的焦点处理。

保留可见状态缓存能够减少重复调用，但系统可以独立收起状态面板，缓存不能证明真实可见，因此不再据此跳过非全屏编辑器的显示请求。直接对每个窗口异步调用 `show()` / `hide()` 虽然更短，但旧窗口的慢请求可能在新窗口之后完成并覆盖结果，因此仍需串行化与过期检查。全屏时继续隐藏，避免同时改变现有全屏遮挡策略；全屏编辑器希望显示工具栏属于后续需要确认的产品决定。

验证：`toolbar-focus.test.ts` 使用模拟面板验证重复聚焦、全屏、旧查询迟到、解绑、面板替换、查询及显示失败恢复，以及慢隐藏和后续显示的顺序；使用 TypeScript 6 严格编译后在 Node 上通过。没有编译 ArkTS HAP 或在鸿蒙电脑与真实窗口中验证，也不能据合成测试确认设备上的具体故障原因。
