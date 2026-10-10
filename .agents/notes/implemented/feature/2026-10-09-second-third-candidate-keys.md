# Agent Note: 用分号和单引号选第二、第三个候选

Status: implemented

## Problem

#6501：选第二、第三个候选时左手要离开主键区去按数字 2、3。搜狗、QQ 拼音等输入法提供「二三候选」快捷键，常见键位有 `;` / `'`、左右 Shift、左右 Ctrl。用户在 Windows 11 上提出，希望用左右 Shift，`;` 和 `'` 也可以。仓库里任何平台此前都没有这个功能。

## Decision

共享偏好新增 `second_third_candidate { enabled, keys }`，默认关闭，`keys` 目前只有 `semicolon_quote` 一种。关闭时不写进偏好文档，`deny_unknown_fields` 的旧版本照样能读；对象里缺字段时取默认值。

Windows 接入：组字时 `;` 选当前页第二个候选，`'` 选第三个，和数字键 2、3 走同一条选词回复（`ReplyPath::Selection`，部分选择时组字继续）。按键需要两边一起判断。TIP 把这两个键归类为数字选词（`FUNCTION_SELECT_BY_NUMBER`），Server 按同一条规则选对应的候选。规则写在 `platforms/windows/common/SecondThirdCandidatePolicy.h`，TIP 的两条归类路径（`IsVirtualKeyNeed` 和排队按键的投影）与 Server 的 `ReplyComposer::second_third_candidate` 都调用它。以下情形不接管：

- 日文。
- 由 TIP 自己组字的方案（韩文、注音、越南文、藏文）。
- Engine 自己的英文模式。
- 网址模式和 V 模式拼写的符号。
- 微软双拼声母后的韵母 ing。

键必须同时对上虚拟键码和打出的字符。

开关怎样到达两边：

- Server 从偏好快照读，随 `NavigationBindings` 进入 `configured_key`，排在以词定字之后、`basic_key` 之前，因为 `basic_key` 会把组字中的 `'` 当作音节分隔符交给 Engine。
- TIP 从新的 Worker 帧 `SecondThirdCandidateChanged`（30）读；推送到达之前视为关闭。
- WinUI 设置页在「输入 › 选词与翻页」里加了一行下拉框：关闭，或 `; / '`。

macOS、Linux（IBus、Fcitx5）、HarmonyOS 外接键盘和 Android 硬件键盘都还没有接入。共享的 React 设置页也没有加这一行，免得在不生效的平台上显示开关。

## Alternatives considered

- **左右 Shift（issue 的首选）**：在 Windows 上，裸 Shift 已经是默认的中英文切换键。TIP 用一个键盘钩子（`_BareShiftKeyboardHookProc`）和预留键来识别它，而且只靠扫描码区分左右。要做到「组字中选词、空闲时切换中英文」，需要改这条钩子链路和预留键的时序。这些代码只能在真实的 Windows 会话里验证，交叉编译和 Wine 都跑不到。格式里的 `keys` 已经留出位置，以后加 `shift` 不用改文档格式。
- **把选词键逻辑放进 input-runtime 或引擎**：运行时和引擎接收的是动作（`Action`），不是物理按键。各宿主自己把按键翻译成动作，翻页键和以词定字的键也都是在宿主层判断。把 `;` 和 `'` 的判断下沉到运行时，需要先把物理按键、键盘布局和 TIP 的同步归类全部搬过去，影响面远大于这个功能。
- **TIP 不读开关，只把这两个键交给 Server 决定（逗号、句号翻页就是这样做的）**：这样就要求 TIP 的标点路径能处理部分选词的回复（`NeedToCreateWord`），而它现在只认整段上屏，所以得新写一条 TIP 回复处理路径。`'` 在组字中又被 TIP 当作输入键（音节分隔符），不经过这条路径。改用推送开关，可以直接复用数字选词这条已经成熟的路径。
- **每次按键时 TIP 自己读偏好文件**：按键路径上会多一次读盘和 JSON 解析，而现有的帧推送机制已经能满足需要。

## Consequences

- 打开后，组字中的 `'` 不再是音节分隔符，`;` 也不再在组字时上屏标点。这是用户主动选择的取舍，设置页的说明写明了这一点。
- 当前页候选不足三个时，`'` 和超出页面的数字键一样，被吃掉但什么也不选。
- 开关刚切换的那一刻，TIP 和 Server 可能有一次按键的归类不一致（Server 已经用新快照，TIP 的推送还没到）。翻页设置也有同样的短暂窗口。
- 旧版 TIP 收到新帧会丢弃，两个键照常是标点；旧版 Server 不发这个帧，TIP 保持关闭。两边因此总是一起关着，或者一起开着。
- TIP 的宿主会话拥有组字时，TIP 从宿主会话的视图判断是否在组字，并按键的位置向宿主会话选词；没有宿主会话时只选镜像里唯一的一项，由 Server 的回复上屏。细节见 [二三候选在 TIP 中的选词位置和组字判断](../bug-fix/2026-10-10-second-third-candidate-tip-slot.md)。

## Testing

- `platforms/windows/tests/input/second_third_candidate.cpp`（`windows-second-third-candidate`）测两部分。一是规则本身：键位、方案、修饰键、英文模式、拼写符号、微软双拼 ing 的位置，以及偏好解析。二是在真实 Engine 会话上走 `configured_key`：日期时间模式下 `;` 和 `'` 分别上屏第二行和第三行；开关关闭时不选；拼音组字中的 `'` 打开后不再分隔音节；微软双拼声母后的 `;` 仍然组字。
- `tsf_config_frames` 断言新帧排在最后，载荷是 `0` 或 `1`。
- client-core 测试覆盖默认关闭且不写进文档、开启时的序列化，以及缺字段、未知键位和多余字段三种情况。
- TIP 的两条归类路径只经过交叉编译，没有在真实 Windows 会话里按过键。
