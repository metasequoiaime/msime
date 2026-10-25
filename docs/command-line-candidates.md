# 命令行查询候选

`msime candidates` 输出一串拼音或按键在你现在的设置下会给出的候选，格式是 JSON，供不改水杉输入法源码、只想测试输入引擎的测试和演示程序使用（#9663）。方案、双拼键位、模糊音、快捷短语、插件、你的词库和学过的词频都按你在设置里选的来，命令行上没有、也不接受任何方案配置：结果应当和你在输入框里打这串键时看到的一致，差别见文末。

命令随桌面安装包分发，是 `msime-mcp` 的一个子命令：

| 平台 | 命令 |
| --- | --- |
| macOS | `MSIME.app/Contents/MacOS/msime-mcp`；Homebrew cask 把它以 `msime` 和 `msime-mcp` 两个名字链接进 `PATH` |
| Linux | 完整版的包装 `/usr/bin/msime-mcp` 和指向它的 `/usr/bin/msime`；其他版本装在 `/opt/msime-linux-<id>` 下，不带 `msime`；Nix 包目前不带 `msime-mcp` |
| Windows | 安装目录下 `server` 文件夹里的 `msime-mcp.exe`，不在 `PATH` 上，用完整路径运行，或者自己加进 `PATH` |

下面的例子都写作 `msime`；在 Windows 上换成 `msime-mcp.exe`。

## 打开

候选来自你的词库，词库里有你打过的词，任何能运行这个命令的程序都能借此读到它们，所以它默认关闭。要用时在水杉输入法的设置里打开「维护与诊断」→「命令行工具」→「命令行查询候选」：macOS 和 Linux 在设置应用里，Windows 在原生设置窗口（Windows 7 版本的设置窗口也有这一项）。这个开关存为偏好 `diagnostic_log.command_line_candidates`，和诊断日志开关一样只属于这台电脑，不随账号同步，也不写进导出的设置文件；连接的 AI 助手不能经 MCP 工具或 `msime config set` 把它打开。

命令每查一次都重新读设置。开关关着时它在 stdout 输出一行错误 JSON、在 stderr 说明怎么打开，以退出码 3 退出；从 stdin 逐行读的进程在你关掉开关之后的下一行停下。

这个开关守的是你的意愿，不是安全边界：能以你的身份运行程序的软件本来就能直接读词库文件。

## 用法

```sh
msime candidates nihao               # 一串键
msime candidates 'ni|hao'            # | 标出光标位置：光标在 ni 之后
msime candidates nihao --limit 5     # 最多 5 个候选（1 到 500，默认 50）
printf 'nihao\nzhong|guo\n' | msime candidates   # 不给按键时从 stdin 逐行读
```

- 按键是可打印的 ASCII 字符（不含空格），至多 128 个。`|` 是光标标记，不算按键，一串里至多一个，省略时光标在末尾；在 shell 里要加引号。光标位置和打字时用左方向键移过去一样，全拼和双拼下只转换光标之前的部分。
- 大写字母按 Shift 加字母送出。数字键、标点和五笔四码唯一时的自动上屏照打字时那样上屏，上屏的文字在 `committed` 里，之后打的键接着组字。
- 从 stdin 读时，每读到一行就输出一行 JSON 并立刻刷新输出，可以一问一答地用；行首行尾的空白和 Windows 的 `\r` 会去掉。读完 stdin 以 0 退出。

## 输出

每次查询在 stdout 输出一行 JSON，例如（候选为合成示例）：

```json
{"keys":"nihao","caret":5,"preedit":"ni'hao","editing_text":"nihao","editing_caret":5,"committed":[],"page_size":6,"candidates":[{"text":"你好","code":"ni'hao","source":"dictionary","annotation":"","corrected":false}]}
```

| 字段 | 含义 |
| --- | --- |
| `keys` | 打下的键，去掉了光标标记 |
| `caret` | 光标在 `keys` 里的位置，即它前面有几个键 |
| `preedit` | 输入法画在行内的预编辑串 |
| `editing_text` | 还在组字的那段按键（最后一次上屏之后打的键） |
| `editing_caret` | 光标在 `editing_text` 里的位置 |
| `committed` | 打这串键的途中上屏的文字，按先后 |
| `page_size` | 候选窗每页几个，`candidates` 的前这么多个就是第一页 |
| `candidates` | 候选，按候选窗里的顺序；每个有 `text`（开了繁体输出时已转成繁体）、`code`（对应的编码）、`source`（来源：`dictionary`、`user_dictionary`、`english`、`quick_phrase`、`emoji`、`kaomoji`、`generated`、`sentence_model`、`japanese`、`plugin` 等）、`annotation`（辅助码等注释，没有时为空）和 `corrected`（是否纠正了拼写才给出） |

查不了时输出 `{"keys": …, "error": {"code": …, "message": …}}`，同时把原因写到 stderr。

## 退出码

| 退出码 | `error.code` | 原因 |
| --- | --- | --- |
| 0 | | 成功；逐行读 stdin 时是读完了 |
| 1 | | 意料之外的失败 |
| 2 | | 命令行写错了，例如给了两串按键、`--scheme` 或越界的 `--limit`；这时 stdout 不输出 |
| 3 | `disabled` | 没在设置里打开「命令行查询候选」 |
| 4 | `not_set_up` | 读不到输入法的运行时选项或偏好：输入法没有在这个用户下安装、设置过，或者文件坏了 |
| 5 | `busy` | 词库正在维护（导入、恢复或切换代次），稍后再试 |
| 6 | `unavailable` | 词库打不开 |
| 7 | `invalid_input` | 按键、光标位置不合规，或者输入法在那里不收某个键（例如没在组字时的数字键，打字时它会原样进入应用） |

逐行读 stdin 时，`busy`、`unavailable` 和 `invalid_input` 只让那一行得到错误行，后面的行照常查询；`disabled` 和 `not_set_up` 让命令输出错误行后以对应的退出码停下。

## 和打字时的差别

命令不连到正在运行的输入法进程，而是在自己的进程里，用输入法建会话时读的同一份运行时选项和偏好，经同一个共享内核建一个输入会话来打这串键；输入法不需要正在运行。为什么这样做见[决策笔记](../.agents/notes/implemented/feature/2026-10-25-command-line-candidates.md)。由此有这几处已知的差别：

- 候选出自命令自己带的那份引擎。macOS 上输入法本体由应用内更新单独升级，`MSIME.app`（命令在其中）不跟着升级，两边版本不同时排序规则可能不同；装同一版本的整套安装包即可一致。
- 云候选和 AI 候选要输入法替会话去网络上取，命令不取，它们的占位行也不列出。
- 只看不学：选词和上屏都不调词频、不造词、不记上下文，也不计入打字统计。一串键在途中上屏之后，打字时会按刚上屏的词给后面的候选做上下文重排，命令不做。
- 每次查询都是一个新会话，没有前文：打字时紧接在上一个词后面打，候选会按前文重排。
- 结果按打完这串键停下来的样子给出，包括停顿之后的重排；Linux 的 Fcitx5 宿主不做停顿重排，在 Fcitx5 下用停顿重排模型的用户，第一页的顺序可能与此不同。
- 输入法刚学到、还没写下的上下文记录（大约 2 秒内）读不到。
- macOS 的输入法在词库目录缺少内置辅助码表时会退回读自己安装包里的那份；命令不知道输入法装在哪里，这种情况下辅助码筛选和注释可能不同。
