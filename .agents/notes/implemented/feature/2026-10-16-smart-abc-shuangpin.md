# Agent Note: Linux 智能 ABC 双拼

Status: implemented

## Problem

#8088 希望 Linux 可以直接选择智能 ABC 双拼。现有 Engine、共享偏好和 Linux 菜单只列小鹤、自然码、首道、微软；只添加菜单名称不能改变按键解释，用户也不应先手工填写自定义表才能使用标准方案。

## Decision

在 Engine 增加内置智能 ABC 键位表，复用双拼切分、候选、简拼、提示及自定义表导出。`ShuangpinProfileKind::SmartAbc` 追加 ABI 值 5，保留已有 0–4；共享偏好名字为 `smart_abc`。Linux IBus 属性菜单、Fcitx5 轮换菜单及共享桌面设置页提供“智能 ABC”。MCP 同步接受这个偏好值。

账号服务现有 schema 只收四个旧内置方案，本次不向它写入新取值；本机选中智能 ABC 时，也不让账号中的旧方案覆盖本机选择，沿用自定义方案的兼容规则。不增加移动端触屏方案或网页引擎入口。

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
- Linux 原生构建门禁 87 项、IBus 原生 ctest 81 项和完整 IBus D-Bus 共享运行时验收通过；真实 daemon 的智能 ABC 三组编码上屏、切回全拼及崩溃恢复通过。
- `check-container.sh <固定资源目录>` 完整通过：安装与卸载、词库和剪贴板、GTK3 X11、Qt5/Qt6 X11、GTK Wayland、Qt5/Qt6 Wayland 及 IBus portal 的系统编辑器验收均运行。
- Fcitx5 完整原生上下文、独立智能 ABC 状态栏选择与保存、AI、真实 daemon 前端、GTK 与 IBus GTK 编辑器共 6 项验收通过。独立用例既检查标签和落盘，也经真实按键上屏「你好」并轮换回小鹤。
- 系统验收夹具同步现有契约：语音合成回包补齐 `ok: true`；Fcitx5 测试显式开启译文并结束大写串后测快捷模式；IBus 用 Ctrl+左右测试逐字母编辑，临时模式回车保留入口字母。均未修改相应生产行为。
- 最终 `bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和差异检查通过；HarmonyOS 真打包因缺少 staged 依赖明确跳过。
