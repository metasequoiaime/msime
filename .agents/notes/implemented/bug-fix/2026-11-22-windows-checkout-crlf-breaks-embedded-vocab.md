# Agent Note: Windows 检出把内置词表转成 CRLF，模型安装必报长度不符

Status: implemented

## Problem

2026-11-22 真机反馈（KaihongOS）：语音模型「从文件导入」报「下载失败：传输中途断开，文件没有下载完整」，与下载失败同话；用户已按上一轮交付的本地副本（逐字节核对过 SHA-256）导入仍然失败。排查证明导入链路本身没有毛病：宿主把选择器给的文档 URI 复制进缓存，Rust 按长度认出文件、SHA-256 校验通过、解压全部完成，死在 `install_model` 最后写内嵌 `bpe.vocab` 之前的长度核对（extra 循环里 `bytes.len() != extra.size`）：目录钉 69594 字节，编译进 HAP 的却是 74594 字节。

多出的 5000 字节正好是词表的 5000 行：本机 `core.autocrlf=true` 把这个纯文本文件检出成 CRLF，`include_bytes!` 在这台 Windows 构建机上把 CRLF 版本编了进去。仓库里 `resources/voice-models/` 没有任何 `.gitattributes` 规则；同类坑此前已在拆字表、语音语言目录等处各修过一次，这次新增的词表漏标了。Linux/macOS 检出（CI、桌面发版机）永远是 LF，所以桌面和 CI 从不触发，只有本机出的鸿蒙 HAP 中招；下载路径若哪天真传输完成，也会死在同一处——两个症状是同一个根因。

## Decision

- `.gitattributes` 给 `resources/voice-models/*.vocab` 补 `text eol=lf`：提交时归一成 LF，检出时强制 LF，和其他按字节校验的资源同一待遇。只圈 `*.vocab` 不圈整个目录——将来放进来的 onnx 之类真二进制不该被强制按文本处理。
- 本机工作区强制重新检出（删除后 `git checkout`），恢复 LF 字节再编 HAP；词表的 blob 从未变过，提交内容只有 `.gitattributes`。
- 页面侧 `localModelImportErrorMessage` 补 `local_model_size_mismatch` 的说法：导入的文件按长度认出、整份校验过才可能走到这个码，此时「传输中途断开」必然是撒谎——改为「应用内置的模型数据不完整，请更新输入法后重试」。下载路径的说法（传输中途断开）保持不变，那里它仍然是真话。

## Alternatives considered

- **改目录，把 `bpe.vocab` 钉成 74594 字节 / CRLF 版本的摘要**——最强理由是不动 `.gitattributes` 一行就能让校验通过。否掉：那是把被检出环境污染的字节写进正册，Linux 检出编出来的二进制（69594 字节）反过来又挂；目录钉的必须是仓库里唯一确定的字节（LF blob），不是某台机器检出后的样子。
- **宿主绕过：鸿蒙导入不走共享 `install_model`，自己拼目录产物**——否掉：内嵌资源核对是共享层唯一守这里的地方，绕过它等于两条路各养一份安装逻辑，目录一改两边漂移。
- **只修本机不改 `.gitattributes`**（检出后手动转一次 LF）——否掉：换机器、重克隆立刻复发，没有任何东西守着。

## Consequences

- 这台机器此后检出词表保持 LF，HAP 内嵌字节与目录一致，下载和从文件导入都能真正装完；桌面与 CI 行为不变（它们本来就绿）。
- 现有守卫 `the_embedded_catalog_is_complete_and_its_resources_match_their_checksums`（client-providers）在检出被换行转换污染的机器上会转红——测试本来就在，这次让它在本机也被跑过。
- 其余 `include_str!` 资源都是解析后使用，CRLF 无害；内嵌且按字节校验的只有这一份词表，不扩面。

## Verification

- 本机重新检出后：`resources/voice-models/x-asr-zh-en-bpe.vocab` 69594 字节、SHA-256 `28fc94d6…f914`、0 个 CRLF，与目录钉值一致；上游归档（2026-06-05）里没有 `bpe.vocab`，目录的 extra 从初版提交起就与 LF blob 一致。
- `apps/desktop` 的 `local-voice-models.test.tsx` 补断言：导入的 `local_model_size_mismatch` 印新句子，28 项全绿；`tsc --noEmit` 干净；`pnpm run verify-notes` 随提交执行。
- 守卫测试在本机也跑过一次：本机默认 msvc 工具链没有链接环境（无 Visual Studio），改用 gnu 工具链跑 `cargo test -p msime-client-providers --lib the_embedded_catalog`，1 通过——重新检出后的内嵌字节与目录钉值一致；全套 local_models 测试与 CI 的 Linux 运行为准。
- 真机验收（KaihongOS，待办）：新包上重试「从文件导入」，中英流式应能装上并出现在已安装列表。
