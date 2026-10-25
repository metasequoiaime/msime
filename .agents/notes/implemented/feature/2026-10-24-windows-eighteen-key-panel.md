# Agent Note: Windows 十八键经焦点租约接入原生会话

Status: implemented

## Problem

#8936 的[共享十八键引擎](../../implemented/feature/2026-10-24-eighteen-key-quanpin-engine.md)已经落地，但 Windows 屏幕键盘仍只注入确定虚拟键。把 `BN` 送成 `B` 会丢掉 `N` 的读音，把组码打给其他输入法则会误输入。TSF 与 Server 的两个会话必须接收同一种输入来源。

## Decision

共享屏幕键盘在 Windows 上提供十八键全拼入口。组码不经过 `SendInput`：面板对当前焦点线程的 TIP 消息窗口发送有界、版本化 `WM_COPYDATA` 请求。`Query` 用面板生成的非零随机 64 位 nonce 取得 500 毫秒、至多八项的租约；TIP 绑定完整焦点代次、worker 令牌、context、前台窗口和实际焦点窗口，只返回布尔值。`Press` 核验同一所有者并消费租约后，将复制的来源标志与逻辑字母放入现有 FIFO。已消费的租约只确认，不重复入队；过期、失焦、未协商、离线和非全拼一律拒绝。Query 超时不发送 Press，Press 超时不重发、不回退字母注入。

来源沿 `_KEYSTROKE_STATE`、延迟项和编辑会话 DTO 显式传递，TIP 与 Server 都用网格编号 4 和 `grid_key`。普通硬件字母仍用 `character`，空组合开始时恢复全键；组码组合中切回实体字母时，先在两端确认当前候选，再开始一段确定全拼，两个会话共用 `PanelCharacter.h` 的切换规则，不能丢掉这个硬件字母。组字中不能从已有确定全拼切入十八键；未执行的确定按键同样阻止切入。回车选择候选；候选为空时保持组字，不能上屏多义组码。实体字母切入而剩余组码没有候选时，只保留已选词段，取消无法解码的剩余组码，再开始确定全拼。旧组件通过独立能力位 `1 << 11` 拒绝新来源，组码元数据用 `0x04000000`，编译期断言它与所有既有标志不重叠；包大小和既有输入协议版本不变。

组码组合不能压成原始字母检查点：它会丢掉每个字母是否多义。传输失败时取消这一批组字，不把它交给离线原文上屏或重连的确定字母恢复；保留确定键盘现有恢复行为。

## Historical audit

- 共享十八键笔记：部分重叠，持有分组与算法决定；这里补 Windows 投递、焦点和恢复边界，双方互链。
- [屏幕键盘设置](../../implemented/feature/2026-10-11-windows-settings-screen-keyboard.md)：部分重叠，仍从原生设置拉起共享面板，不迁移设置归属。
- [键盘状态抽取](../../implemented/simplification/2026-10-19-keyboard-panel-state.md)：部分重叠，复用有界发送队列和 Windows 松开发送，不另建面板会话。
- [桌面候选路由](../../implemented/feature/2026-10-24-windows-desktop-candidate-routing.md)：无替代关系，仍由既有 TSF/Server 会话和候选 UI 接管结果。

## Alternatives considered

**带标记的 SendInput**能直接利用现有按键链，但前台使用其他输入法时仍会输入组首字母，且延迟回放不能重新推断消息来源，因此不用。

**独立面板会话**能避开 TSF 协议，却让面板、实体键盘和候选选择分属不同组字，违背原生会话归属，因此不用。

**新增 Server 面板管道再转给 TIP**可复用管道鉴权，但需要新增 listener、第三段转发和另一套请求到焦点的绑定；当前 TIP 已有线程消息窗口，先使用它并在所有阶段核对租约。

## Testing

- 共享界面的三文件 17 项测试、TypeScript 类型检查和生产构建通过，覆盖十八键入口、二十六字母覆盖、显式组码载荷、回车载荷、清除修饰键、保留宿主九键双拼及投递不确定后清空队列。
- Windows 上的 `msime-host-api`、`msime-host-windows` 构建和面板外壳 `cargo check -p msime-desktop --lib --locked` 通过；面板契约六项和偏好旧格式往返一项通过，核心 all-targets 严格 Clippy 通过，旧请求序列化保持原样。
- MSVC 已构建 TIP、Server、租约测试、双会话测试和原有会话与回复测试。真实独立的 TIP/Server C ABI 会话在合成词库上验证 `bihai` 的「你好」「波好」、退格、回车、切回确定字母、组字中拒绝切入和空候选保护；它不等于真实编辑器上屏。
- 租约测试在 macOS 的 Clang 与 Windows MSVC 下通过，覆盖完整 64 位代次和令牌、context、前台与实际焦点 HWND、过期、满表、重复与重入。真实恢复方法的抽取回归通过，组码不会生成普通字母检查点，组码在执行前或执行后遇到重试都取消整批并释放引用。
- 应用按键回放 191 项、延迟分类 225 项均通过；分类测试执行真实 FIFO 和 drain，确认组码保存来源与完整所有者。测试中的系统/COM 替身不代表真实焦点租约已验收。
- 原生通用 `windows-session` 与 `windows-reply-composer` 仍失败。在固定 `c208d5748a` 对应的原生源码上重建运行后，分别同样报 `Preference retry task failed` 与回复断言失败；没有增加豁免或删掉断言。
- 同步 `feaeb3ae4d` 后 Windows 完整 quick 实际退出 1；Rust workspace 与 pipe-only 编译通过，七项发布/平台/环境契约仍失败：Android、Linux、Windows 发布页，macOS 公证流程、发布附加内容、设置更新器，以及语音语言目录。没有新增豁免；合并标记和符号组的旧失败已消失。四项原生专项（租约、主管道、双会话、回车）均通过，71 份原生与协议源码的哈希与此前通过输入一致。
- GNU CI 暴露生命周期夹具漏了两个新增网格 C ABI 符号，MSVC 同样复现链接失败。夹具补齐两个入口并在误入时直接失败；MSVC 链接已通过。初次验证的生命周期用例在部分选词后的 `recorded` 断言失败：固定 `feaeb3ae4d` 的相关原生源码和原夹具重建后在第 344 行同样失败，恢复特性后为同一断言的第 353 行。Win7 集成复验确认替身仍返回旧式逐段 commit，而 Server 已请求 Runtime 的 `phrase_preedit`；替身补齐暂存前缀、最终合并、失焦／取消清理，并断言创建请求启用该契约。原统计、特效和调用顺序断言全部保留；生命周期、组码租约、主管道与双会话四项专项在 MSVC x64／x86 全部通过。主管道计数使用无符号常量以修正 x86 `/WX` 下的比较警告，不降低警告等级。实际 Win7 与最终安装证据归[产品构建](2026-10-24-windows7-product-build.md)。
- 五笔抽取夹具同步后在 macOS 重新执行 32 项，全部通过。回车观察 17 项、Shift 观察 33 项、游戏兼容与 Shift 组合键回归均通过。Host Windows 严格 Clippy 仍被既有 `read_clipboard_text` 的 `Result<_, ()>` 挡住；核心 all-targets 严格 Clippy 通过，没有放宽 lint。

## Consequences

组码沿既有 TSF 与 Server 双会话推进，面板没有独立输入状态；默认仍为二十六键。入口只在 Windows 出现，非全拼、本地模式、专用英文、未协商与失焦都不可投递。

租约引入固定八槽、500 毫秒有效期；超过容量时明确拒绝，Press 返回成功只证明入队，不证明编辑器上屏。UIPI 不允许的目标保持不可用，不放宽消息过滤权限。真实 Windows 非激活面板、跨线程焦点、第三方编辑器、旧输入源与断线现场仍缺验收；本 PR 使用 `Refs #8936`，不能声明 issue 已完整解决。

上游 `c208d5748a` 的偏好测试与 C 头文件合并残留、既有抽取夹具诊断替身缺口已由后续独立修复处理；本分支同步到 `feaeb3ae4d`，只保留新增组码来源所需的夹具适配。
