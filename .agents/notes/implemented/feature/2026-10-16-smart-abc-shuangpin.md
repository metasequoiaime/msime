# Agent Note: 桌面端智能 ABC 双拼

Status: implemented

## Problem

#8188 延续 #8088 的智能 ABC 双拼支持，要求 Windows、macOS 与 Linux 的桌面宿主都能直接选择并正确展示该方案。现有 Engine、共享偏好和桌面原生入口只列小鹤、自然码、首道、微软；只添加菜单名称不能改变按键解释，用户也不应先手工填写自定义表才能使用标准方案。

## Decision

在 Engine 增加内置智能 ABC 键位表，复用双拼切分、候选、简拼、提示及自定义表导出。`ShuangpinProfileKind::SmartAbc` 追加 ABI 值 5，保留已有 0–4；共享偏好名字为 `smart_abc`。Linux IBus 属性菜单、Fcitx5 轮换菜单、Windows 原生设置与菜单、macOS 原生设置及共享桌面设置页提供“智能 ABC”。Windows 的工具栏、托盘和键位提示、macOS 的键位提示、云词典校验和共享 Swift 候选请求校验同步接受这个偏好值。三个桌面宿主的打字来源都写入 `smart_abc`，共享统计存储、双拼成就和两套统计页面同步识别，避免新方案被误记成小鹤或整批拒收。MCP 同步接受这个偏好值。

账号服务现有 schema 只收四个旧内置方案，本次不向它写入新取值；本机选中智能 ABC 时，也不让账号中的旧方案覆盖本机选择，沿用自定义方案的兼容规则。Windows 与 macOS 的原生入口只扩展既有方案枚举和展示映射，不改变 `custom` 的编辑边界；macOS AppKit 设置仍为冻结的回退窗口，补的是既有双拼偏好的枚举兼容，正常入口沿用共享 React 设置页。不增加移动端触屏方案或网页引擎入口。

## Sources

键位定义核对 Rime 的公开实现 [`double_pinyin_abc.schema.yaml`](https://github.com/rime/rime-double-pinyin/blob/01a13287cbd27819be1c34fa1ddc1b3643d5001b/double_pinyin_abc.schema.yaml)，固定提交 `01a13287cbd27819be1c34fa1ddc1b3643d5001b`。该项目 LICENSE 为 GPL-3.0；本次按公开键位对应关系独立填写本仓库既有的声母、韵母与零声母表，不引入它的 YAML、正则变换、解码器或资源文件。`zh/ch/sh` 分别对应 `a/e/v`，零声母使用 `o` 引导，复合韵母由同一份 Engine 表解析并生成提示。

## Historical audit

- [自定义双拼](2026-10-10-custom-shuangpin-profile.md)部分重叠：复用既有表结构和校验，不改变自定义方案的 ABI 值与落盘表。
- [连续大写与快捷模式](../bug-fix/2026-10-15-uppercase-run-and-local-mode-enter.md)部分重叠：Fcitx5 系统夹具先验证连续大写不会进入模式，再以小写输入结束大写串并取消组字后测 Shift 入口；IBus 临时模式的回车断言保留入口字母，与共享引擎已有契约一致。
- [九键双拼](2026-10-10-nine-key-shuangpin.md)无关：本次是标准字母键双拼，不增加九键码表。
- [双拼编码判定借用](../testing/2026-10-15-shuangpin-code-acceptance-borrow.md)部分重叠：新表须经过现有双拼共用行为测试，不另写判断路径。

## Alternatives considered

- 在 Linux 安装时写入一张自定义表，能复用已有配置，无须扩展枚举；但会覆盖用户自己的自定义方案，而且原生菜单无法识别标准方案名称，账号回写仍需特殊保护。
- 仅让用户导入自定义表，实现代价小；但不满足直接选择智能 ABC 的需求，也无法从原生菜单切换和显示正确名称。

## Consequences

- 用户直接选用标准方案，无须占用自己的自定义表；旧 ABI 0–4、小鹤默认值和自定义表保持原样。新方案共用原有的切分、候选和键位提示算法。
- IBus 的菜单显示与点击合法值校验共用 `kShuangpinProfileNames`，防止加了标签却点不动；Fcitx5 状态栏按该列表轮换。
- 账号契约仍只有旧四值，智能 ABC 不上传该键，也不接受账号旧值覆盖。服务端支持它后应补同步映射和跨端验收。
- 新偏好值无法由旧客户端识别。共享 UI 不新增触屏方案：需要推导触屏布局或走首次引导时使用小鹤键面，保留桌面双拼偏好，避免把 `smart_abc` 写入触屏枚举导致整份偏好读不进来。

## Verification

- 特色编码回归先在旧实现失败，新增内置表后双拼套件 121 项通过；全量引擎 2157 项与 golden 31 项通过，涵盖声母、零声母和复合韵母、内置与自定义表等价解码。
- client-core 1475 项、host-api 636 项、MCP 60 项单测及 11 项集成测试通过；新宿主测试使用合成 SQLite 词条，验证 ABI 5、组词中延后切换和切回智能 ABC 后上屏。
- 共享设置、引导及触屏兼容的 85 项测试通过，类型检查、构建和定向 lint 通过。上述四个 Rust crate 的 `cargo clippy --all-targets -- -D warnings` 通过。
- 原 PR 的共享设置双拼选项断言包含第五个内置方案；Linux 与 macOS 保存与重新加载由两项独立设置用例覆盖。
- 完整共享 UI 套件重新运行，651 个文件、3727 项测试全部通过。
- `cargo test -p msime-client-core -p msime-host-api -p msime-engine` 通过：client-core 380 项、host-api 636 项、engine 2167 项（33 项忽略）及 engine golden 31 项均通过。
- 桌面补齐后，云词典校验 5 项、统计存储与指标 78 项 Rust 测试通过；Windows 可在 macOS 上执行的 151 项原生测试通过，覆盖名称、键位提示、来源映射和统计展示。共享统计页 30 项测试、前端类型检查与构建、定向 lint 和格式检查通过。CI 的 Windows WinUI 原生设置编译与页面冒烟检查、完整 GNU 交叉编译已通过；真实 TSF 编辑器输入仍需系统验收，不能由本机 C++ 测试或编译代替。
- macOS 原生构建通过；`shortcut`、`shuangpin-keymap-input`、`typing-statistics` 与 `desktop-dictionary-provider` 四项 ctest 通过，覆盖五方案保存重读、两种外观的键位图、真实 C ABI 统计写入及合成云词典请求。
- 共享 Swift 候选请求 8 项测试通过，包含智能 ABC 经公开客户端 API 到达合成后端的回归；client-core 与 client-services 的 `cargo clippy --all-targets --locked -- -D warnings` 通过。
- Linux 原生构建门禁 87 项、IBus 原生 ctest 81 项和完整 IBus D-Bus 共享运行时验收通过；真实 daemon 的智能 ABC 三组编码上屏、切回全拼及崩溃恢复通过。
- `check-container.sh <固定资源目录>` 完整通过：安装与卸载、词库和剪贴板、GTK3 X11、Qt5/Qt6 X11、GTK Wayland、Qt5/Qt6 Wayland 及 IBus portal 的系统编辑器验收均运行。
- Fcitx5 完整原生上下文、独立智能 ABC 状态栏选择与保存、AI、真实 daemon 前端、GTK 与 IBus GTK 编辑器共 6 项验收通过。独立用例既检查标签和落盘，也经真实按键上屏「你好」并轮换回小鹤。
- 系统验收夹具同步现有契约：语音合成回包补齐 `ok: true`；Fcitx5 测试显式开启译文并结束大写串后测快捷模式；IBus 用 Ctrl+左右测试逐字母编辑，临时模式回车保留入口字母。均未修改相应生产行为。
- 本地 `bash scripts/verify-local.sh --quick` 有一轮通过，`pnpm run verify-notes` 与差异检查通过；HarmonyOS 真打包因缺少 staged 依赖明确跳过。该轮 Windows 完整交叉编译因其他运行占用依赖锁跳过，pipe-only 的 x86_64 与 i686 交叉编译通过。同步原 PR 最新 head 后的 quick 重跑遇到磁盘耗尽和 Docker socket 消失，Linux 容器、Windows 交叉编译、macOS Rust 库及 pipe-only 阶段失败；已清理本 worktree 的约 1.8 GiB Rust 生成产物。相同生产代码的 CI 已通过 Linux 原生宿主、Windows 完整交叉编译及 WinUI 设置编译与页面冒烟、macOS arm64 原生构建与测试。

- PR 与新版 `develop` 合并检查时，手机候选翻译缩为单目标语言使 `lowercaseTrimmed` 调用数从 7 减到 6，旧契约测试误报。契约改为逐文件确认复用共享策略，并保留拒绝嵌套调用的约束；当前分支、新版 `develop` 的单目标实现及错误嵌套反例均已验证，不改变 Android 产品行为。
