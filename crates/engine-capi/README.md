# msime-engine-capi

C ABI of the 水杉输入法 (Metasequoia IME) input engine: a C library (shared and static), a generated header and a pkg-config file.

Overview, integration examples and FAQ: <https://msime.app/engine/> (in Chinese).

水杉输入法输入引擎的 C 接口。它把核心层的 `msime_engine::Session` 包成 35 个带版本的 C 函数，符号一律以 `msime_engine_` 开头。v1 覆盖词格整句、拼写纠错、学习、个人词典和 10 个方案（全拼、双拼、五笔、日文、韩文、粤语、注音、越南文、藏文、笔画）；九键、触点纠错、神经重排和搭配模型在 Rust crate 里可用，C 接口留到后续版本。设计与取舍见 `.agents/notes/implemented/architecture/2026-10-29-engine-capi.md`。

头文件 [`include/msime_engine.h`](include/msime_engine.h) 由 cbindgen 从 `src/` 生成并提交，每个函数、结构体和常量的说明都在里面；这份 README 讲它们之间的约定。

## 快速上手

不用装 Rust，也不用克隆仓库：从 [msime-engine-capi 的发布](https://github.com/metasequoiaime/msime/releases?q=msime-engine-capi&expanded=true) 下载自己平台的归档和数据包，校验值在同一个发布的 `SHA256SUMS` 里。

| 文件 | 内容 |
| --- | --- |
| `msime-engine-capi-<版本>-linux-x86_64.tar.gz` | Linux x86_64，在 Ubuntu 22.04 上构建，需要 glibc 2.35 或更新 |
| `msime-engine-capi-<版本>-macos-universal.tar.gz` | macOS 11 起，arm64 与 x86_64 合在同一个库里。库只有 ad-hoc 签名：用浏览器下载的先 `xattr -dr com.apple.quarantine <解开的目录>` |
| `msime-engine-capi-<版本>-windows-x86_64-msvc.zip` | Windows x86_64，MSVC 工具链，链接动态 CRT：运行要有 Visual C++ 运行库，静态链接时自己的代码也用 `/MD` 编译 |
| `msime-engine-data-quanpin-<词库版本>.tar.gz`、`.zip` | 全拼、双拼的最小数据包：`msime-pinyin.db`、`msime-english.db`、`msime-bigram.bin`、`msime-trigram.bin` 和 `msime-scowl_Copyright.txt`，逐字节取自 [msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 `dict-v*` 发布，解开的目录就是 `resources`。**这份数据不是 MPL-2.0**：主体是 GPL-3.0，n-gram 表是 CC-BY-SA-4.0，逐项见包里的 `README.md` |

库的归档解开后是一个装好的前缀：`include/msime_engine.h`；`lib/` 里的共享库和静态库（Windows 的 DLL 在 `bin/msime_engine.dll`，导入库是 `lib/msime_engine.dll.lib`，静态库是 `lib/msime_engine.lib`）；`lib/pkgconfig/msime-engine.pc`，它的 prefix 写的是相对 `.pc` 所在目录的 `${pcfiledir}/../..`，归档解到哪里都能直接用；macOS 的共享库 install name 是 `@rpath/libmsime_engine.1.dylib`。另外有 [C 示例](examples/c/example.c) 和它的 CMakeLists.txt，以及 `LICENSE`、`licenses/`、`THIRD_PARTY_NOTICES.txt`、`SOURCE.txt`（见[二进制归档与第三方声明](#二进制归档与第三方声明)）。

下面把归档里的 C 示例编出来，在数据包上打一句全拼 `nihaoshijie`、选第一个候选上屏、冲刷学习、再读个人词典。示例的三个参数是 `resources`、`user_data`、`cache`，都要是绝对路径；后两个由库建出来。

Linux、macOS 上用 pkg-config（macOS 把归档名换成 `macos-universal` 那个）：

```sh
tar -xzf msime-engine-capi-0.1.1-linux-x86_64.tar.gz
tar -xzf msime-engine-data-quanpin-2.0.39.tar.gz
capi=$PWD/msime-engine-capi-0.1.1-linux-x86_64
export PKG_CONFIG_PATH=$capi/lib/pkgconfig
cc -o example "$capi/examples/c/example.c" $(pkg-config --cflags --libs msime-engine) -Wl,-rpath,"$capi/lib"
./example "$PWD/msime-engine-data-quanpin-2.0.39" "$PWD/user" "$PWD/cache"
```

没有 pkg-config 时直接写：`cc -o example "$capi/examples/c/example.c" -I"$capi/include" -L"$capi/lib" -lmsime_engine -Wl,-rpath,"$capi/lib"`。静态链接把 `-lmsime_engine` 换成 `"$capi/lib/libmsime_engine.a"`，再接上 `.pc` 的 `Libs.private` 列的系统库（`pkg-config --static --libs msime-engine` 打印的除 `-lmsime_engine` 以外的部分，见[构建](#构建)）。

Windows 上在 x64 的 Developer Command Prompt 里：

```bat
tar -xf msime-engine-capi-0.1.1-windows-x86_64-msvc.zip
tar -xf msime-engine-data-quanpin-2.0.39.zip
set CAPI=%CD%\msime-engine-capi-0.1.1-windows-x86_64-msvc
cl /nologo /MD /I "%CAPI%\include" "%CAPI%\examples\c\example.c" /link "%CAPI%\lib\msime_engine.dll.lib"
copy "%CAPI%\bin\msime_engine.dll" .
example.exe "%CD%\msime-engine-data-quanpin-2.0.39" "%CD%\user" "%CD%\cache"
```

每个平台也都可以用 CMake，动态和静态链接一样写，静态库要的系统库它从 `.pc` 里读：

```sh
cmake -S "$capi/examples/c" -B build -DMSIME_ENGINE_PREFIX="$capi" -DMSIME_ENGINE_LINK=static
cmake --build build --config Release
```

在 dict-v2.0.39 的数据包上，示例打出（库的版本号随发布）。「你好世界」本来就是词库里的词，学习只调它的词频，不往个人词典里加一条；在 `tests/fixture` 的最小词库上它是整句联想解出的句子，那里会读回一条个人词条：

```text
msime-engine 0.1.1, ABI 1.1
preedit: ni'hao'shi'jie (revision 11, 192 candidates)
  1. 你好世界 [ni'hao'shi'jie] source 0
  2. 你好是饥饿 [ni'hao'shi'ji'e] source 8
  3. 你哈哦世界 [ni'ha'o'shi'jie] source 8
  4. 你哈哦是饥饿 [ni'ha'o'shi'ji'e] source 8
  5. 你好 [ni'hao] source 0
commit: 你好世界
personal dictionary: {"entries":[],"has_more":false}
the dictionary already has this word: learning adjusted its frequency instead of adding a personal entry
ok
```

Rust 程序不需要这个库，直接 `cargo add msime-engine`，见 [msime-engine 的快速上手](https://github.com/metasequoiaime/msime/blob/develop/crates/engine/README.md#快速上手)。

## 构建

用 [cargo-c](https://github.com/lu-zero/cargo-c)（`cargo install cargo-c --locked`，要 0.10.26 或更新）：

```sh
cargo cinstall --locked --release -p msime-engine-capi --prefix /usr/local --libdir lib
```

装出来的是：

| 文件 | 说明 |
| --- | --- |
| `include/msime_engine.h` | 头文件，`#include <msime_engine.h>` |
| `lib/libmsime_engine.so.1.1.0`、`libmsime_engine.so.1`（Linux） | 共享库，SONAME 是 `libmsime_engine.so.1`，主版本就是 ABI 主版本 |
| `lib/libmsime_engine.1.1.0.dylib`（macOS） | install name 是 `<libdir>/libmsime_engine.1.dylib`，compatibility version 是 ABI 主版本 |
| `bin/msime_engine.dll`、`lib/msime_engine.dll.lib`（Windows MSVC） | DLL 和导入库 |
| `lib/libmsime_engine.a`、`lib/msime_engine.lib`（MSVC） | 静态库 |
| `lib/pkgconfig/msime-engine.pc` | `pkg-config --cflags --libs msime-engine`；静态链接用 `--static`，`Libs.private` 列出静态库要的系统库。`Version` 是 ABI 版本（1.1.0），不是 crate 版本，可以写 `msime-engine >= 1.1` 按 ABI 做编译期开关。没有默认数据目录变量：cargo-c 生成的 `.pc` 不能加自定义变量，库也没有默认数据目录，四个根目录总由前端传入 |

静态库要的系统库因平台而异，以 `.pc` 的 `Libs.private` 为准：macOS 是 `-liconv -lSystem -lc -lm`，Linux 是 `-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc` 一类，Windows 是 `kernel32`、`advapi32`、`ntdll`、`userenv`、`ws2_32` 等。

两条链接上的限制：

- 静态库里带着整份 Rust 标准库和 bundled SQLite 的全局 `sqlite3_*` 符号。和系统的 libsqlite3 一起链接能链上（C 示例的 `static-system-sqlite` 变体就是这样），但 C 代码里调用的 `sqlite3_*` 解析到的是库里自带的那份。一个进程只能链接一个 Rust 静态库：两个都带 Rust 标准库和 SQLite 的静态库放在一起会有重复符号，要用其中一个的共享库形式。
- 苹果平台上库不剥调试信息（`Cargo.toml` 的 `package.metadata.capi.library.target`）：Xcode 27 的 `strip` 剥过的 dylib，链接器报 `mis-aligned LINKEDIT string pool` 拒绝链接。

改了 `src/` 里公开的类型、常量或函数之后重新生成头文件：

```sh
bash crates/engine-capi/tests/c-example.sh --update-header
```

这个脚本也是 CI 的 `engine-capi` 作业（加 `--prefix <目录>` 时不构建，对一个已经装好的前缀做同样的检查和示例，发布流程用它验收解开的归档）：用 cargo-c 装进 `target/engine-capi/stage`，核对提交的头文件没有漂移，再用 CMake 把 [C 示例](examples/c/example.c) 按动态库、静态库（Linux、macOS 上再加一个同时链接系统 SQLite 的静态版本）各编一遍，在 `tests/fixture` 生成的最小词库上打一句全拼、选词上屏、冲刷学习，再从个人词典读回学到的这句。

## 生命周期

```c
msime_engine_prepare_runtime(&paths, "dict-v2_0_39", 12, &dictionaries); // 准备代次，交回代次目录
msime_engine_session_new(options_json, len, &session);                 // 选项 JSON 见下文
msime_engine_key_char(session, 'n', 0, &result);                       // 按键……
msime_engine_state_get(session, &state);                               // 读状态和修订号
msime_engine_candidate_get(session, state.revision, i, &candidate);    // 按下标取候选
msime_engine_select(session, state.revision, i, &result);              // 选择，修订号不对就是 STALE
msime_engine_session_free(session);                                    // 写下学习，交还连接和租约
msime_engine_flush_learning(MSIME_ENGINE_FLUSH_STOP_THREAD);           // 退出或卸载前
msime_engine_string_free(dictionaries);
```

- **准备运行时**：`msime_engine_prepare_runtime` 在 `user_data/dictionaries/<content_id>` 原子地准备一个代次：从 `resources` 复制工作词库，回放学习日志。`content_id` 是 1 到 128 个 `[0-9A-Za-z_-]`，随包数据一变就换一个（例如数据发布的版本号，点要换掉：`dict-v2.0.39` 写成 `dict-v2_0_39`）；同一个 id 再调只补回放。`paths.dictionaries` 这时还没有，不读也不用填。`msime_engine_prepare_runtime` 按全部方案准备，`resources` 里必须有 `msime-pinyin.db` 和 `msime-english.db`；只用几个方案时用 `msime_engine_prepare_runtime_for`（ABI 1.1 起），多传一个方案位集合（第 n 位是序号为 n 的方案，与选项的 `enabled_schemes` 相同），`resources` 只要有这些方案读的文件（见[要带的数据文件](#要带的数据文件)）。它交回的代次目录就是建会话时的 `paths.dictionaries`。三个根目录都要是绝对路径、互不重叠：`resources` 只读，`user_data` 是用户数据（学习日志 `msime_user.db` 和代次），`cache` 随时可以清空。
- **会话**：`msime_engine_session_new` 建会话时取这两个目录（`user_data` 和代次）上的共享词库租约，`msime_engine_session_free` 时交还。会话结束时写下最近约 2 秒排着的上下文学习、关掉自己的日志连接。
- **维护**：`msime_engine_user_dict_edit` 和 `msime_engine_reset_learning`（用 `_for` 准备的代次用 `msime_engine_reset_learning_for`，传同一个方案集合）要独占租约，同一份数据上还有会话开着（本进程、别的进程，包括 msime 自己的输入法）时返回 `MSIME_ENGINE_BUSY`、什么也不改：先释放会话。`msime_engine_user_dict_list` 只读，随时可以调。进行中的维护也让 `msime_engine_session_new` 返回 `MSIME_ENGINE_BUSY`。
- **退出与卸载**：会话都释放之后调 `msime_engine_flush_learning(MSIME_ENGINE_FLUSH_STOP_THREAD)` 写完并停掉后台写线程。删除或替换数据目录之前调 `msime_engine_close_databases`。引擎还有进程级的缓存和单位换算的后台线程，没有办法全部收回，所以不要 `dlclose` 这个库后再重新加载，把它当作每个进程一份。

## 按键路由

引擎收的是语义按键，键位映射是前端的事：字母、数字、标点按 ASCII 交来，编辑键是命令。哪个键走哪个入口由前端决定，规则和 msime 自己的宿主（`crates/input-runtime`）一样：

1. 字母一律 `msime_engine_key_char`。单独按下的 Shift 加字母（大写字母开本地模式）带 `MSIME_ENGINE_KEY_SHIFT_ONLY`。
2. 数字、标点和空格先看 `state.spelling_symbols`：列在里面的此刻是拼写的一部分（表达式模式的运算符、`/` 和 `@` 的入口、注音的声调和空格、拆字的分隔符等），交给 `msime_engine_key_char`。`'` 在 `state.flags` 带 `MSIME_ENGINE_STATE_TAKES_SEPARATOR` 时也交给 `msime_engine_key_char`。
3. 其余标点交给 `msime_engine_key_punct`：有组字时它以首选结束组字再接上中文标点。前端高亮的不是首选时，先 `msime_engine_finish(revision, 高亮行)`，再 `msime_engine_key_punct`。`handled` 为 0（中文标点关着、锁成 ASCII、这个键没有中文标点）时前端自己把字符交给应用，有 `commit` 就先交 `commit`。
4. 数字 1–9 先交 `msime_engine_key_char`（Unicode 模式、日期模式等会自己用数字）。它没处理（`handled` 为 0、没有 `commit`）、按键之前有候选且不是联想（`MSIME_ENGINE_STATE_PREDICTION`）时，前端把它当作本页的选择键：`msime_engine_select(result.revision, 本页起点 + 数字 - 1)`。修订号要用这次 `msime_engine_key_char` 交回的 `result.revision`：交给引擎的每个字符都让修订号变，没处理的也一样，按键之前读到的 `state.revision` 这时已经过期（`MSIME_ENGINE_STALE`）；候选行没有变，前端手里的下标照旧有效。
5. 空格：不在 `spelling_symbols` 里时，有候选就 `msime_engine_select(revision, 高亮行)`；联想的候选只在用户明确选择时上屏，空格和回车不上屏它们。
6. 编辑键是命令：回车 `COMMIT_RAW`（只上屏、不学英文词的回车用 `COMMIT_RAW_WITHOUT_LEARNING`，例如 Shift+回车），Esc `CANCEL`（越南文、藏文和开着的韩文汉字列表第一次只退一步，再发一次才丢弃），退格 `BACKSPACE`，Delete `DELETE_FORWARD`，←/→ `MOVE_LEFT`/`MOVE_RIGHT`（全拼、双拼整句改字用 `CONVERSION_LEFT`/`CONVERSION_RIGHT`），Home/End `MOVE_HOME`/`MOVE_END`。
7. 没有组字（`state.editing_text` 为空、没有候选）时引擎不处理的键，前端照常交给应用。

**分页归前端。** 引擎给出整张候选表，`state.candidate_count` 是行数，前端按自己的每页行数用 `msime_engine_candidate_get` 按下标取，并把每页行数经 `MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS` 告诉引擎（它决定首页并入多少补全词）。单字母全拼一开始只给出一部分候选，前端翻过第一页时调 `msime_engine_expand_candidates` 要回其余的。

**下标和修订号。** 候选下标是引擎顺序里的行号。每次可能改变候选的调用都让修订号（`state.revision`、`result.revision`）变；`msime_engine_candidate_get` 和选择类函数都要交回修订号，不是当前的就返回 `MSIME_ENGINE_STALE`、什么也不做，这时重新读状态。光标、字节与字的单位写在字段名里：`caret_byte` 按 `editing_text` 的字节计，`conversion_focus_*_scalar` 按 `conversion` 的 Unicode 标量计。

## 线程

- 会话可以在任何线程上用，也可以在两次调用之间换线程，但同一个会话的调用不能并发。库里只做 `try_lock`：另一个线程正在调同一个会话时返回 `MSIME_ENGINE_BUSY`，不等待。`msime_engine_session_free` 同样如此，释放之后句柄不能再用，释放与别的线程的调用之间的先后由前端保证。
- 不同的会话可以在不同线程上同时用。
- 进程级函数（版本、`prepare_runtime` 与 `prepare_runtime_for`、`flush_learning`、`release_caches`、`close_databases`、`preload_model`、个人词典的列表、编辑与两个重置）可以在任何线程上同时调用。
- 库里任何 panic 都挡在边界内。会话里的 panic 让会话作废：这一次和之后的调用都返回 `MSIME_ENGINE_PANIC`，只能释放它；别的会话不受影响。
- `msime_engine_last_error` 是线程局部的：它说明的是本线程上一次调用，有效到本线程下一次调用这个库。

## 内存与字符串

- **输入**一律是借用的 `(ptr, len)` UTF-8，不要求 NUL 结尾，调用返回后库不再读它。`ptr` 为 NULL 时 `len` 必须是 0（当作空串）。
- **热路径输出**（`msime_engine_result`、`msime_engine_state`、`msime_engine_candidate` 里的 `msime_engine_str`）借自会话：`len` 个字节，不以 NUL 结尾，空串时 `ptr` 也不是 NULL。它们有效到同一会话下一次改变状态的调用：按键、命令、选择与逐候选学习、`set_*`、`switch_scheme`、`set_caret`、`reset_context`、`expand_candidates`。`state_get`、`candidate_get`、`state_json` 这类只读调用不让它们失效，所以可以先取状态再逐个取一页候选，上屏文字在这期间也一直有效。
- **库分配的字符串**（`state_json`、`user_dict_list` 的 JSON、`prepare_runtime` 交回的路径）用 `msime_engine_string_free` 释放，不要用 `free()`。`msime_engine_version` 和 `msime_engine_last_error` 不要释放。
- 只有冷路径用 JSON：建会话的选项、调试快照、个人词典。按键路径上没有 JSON，库也不为交回的结果另外分配（引擎内部的分配不在这个承诺里）。

## 版本与结构体

- `msime_engine_abi_version()` 返回 `(major << 16) | minor`。主版本与头文件的 `MSIME_ENGINE_ABI_MAJOR` 不同就不要用这份库；次版本小于头文件的 `MSIME_ENGINE_ABI_MINOR` 时，后加的函数不可用。同一个主版本内只追加函数、常量、标志位和结构体尾部的字段，已有的编号与含义冻结；1.1 追加了 `msime_engine_prepare_runtime_for`、`msime_engine_reset_learning_for`、命令 `COMMIT_RAW_WITHOUT_LEARNING` 和选项 `english.learn_on_enter`、`english.learn_typed_spellings_on_enter`（1.0 的库不认识这两个键，建会话会返回 `INVALID_ARGUMENT`）；方案、命令和候选来源沿用核心层的序号（命令与 host-api 的编号不同）。
- 每个进出的结构体都以 `uint32_t struct_size` 开头，调用前设为 `sizeof`。输出时库写 min(调用方大小, 库内大小) 个字节，把 `struct_size` 改成写入的大小，调用方多出的部分清零；输入时库只读这么多，读不到的字段按零。比 1.0 的布局还小是 `MSIME_ENGINE_INVALID_ARGUMENT`。
- 返回码：`OK` 0、`INVALID_ARGUMENT` 1、`FAILED` 2、`STORAGE` 3、`UNSUPPORTED` 4、`STALE` 5、`BUSY` 6、`PANIC` 7、`NULL` 8。1、2、3、6 与引擎的 `ErrorCode` 共用编号。`msime_engine_result.diagnostic_code` 是引擎的诊断码（学习没能存下之类，键照样处理了），0 是没有，0xFFFF 是表外的文字。

## 会话选项

`msime_engine_session_new` 的选项是一个 JSON 对象，版本 1。只有 `version` 和 `paths` 必填，其余缺省取引擎库的默认值（`SessionOptions::new`：全拼、小鹤双拼、学习和个人上下文开、中文标点和成对标点开、辅助码关）。不认识的键、取值不对都是 `MSIME_ENGINE_INVALID_ARGUMENT`。

```json
{
  "version": 1,
  "paths": {"resources": "/abs/res", "user_data": "/abs/user", "cache": "/abs/cache", "dictionaries": "/abs/user/dictionaries/dict-v2_0_39"},
  "scheme": "quanpin",
  "enabled_schemes": ["quanpin", "shuangpin", "wubi"],
  "shuangpin": {"profile": "xiaohe", "preedit_shows_keys": true},
  "fuzzy_pinyin": ["z_zh", "in_ing"],
  "autocorrect": ["transposition", "neighbor"]
}
```

| 键 | 取值 |
| --- | --- |
| `version` | 1 |
| `paths` | `resources`、`user_data`、`cache`、`dictionaries` 四个绝对路径，`dictionaries` 是 `msime_engine_prepare_runtime` 交回的代次目录 |
| `scheme` | 起始方案：`quanpin`、`shuangpin`、`wubi`、`japanese`、`korean`、`cantonese`、`zhuyin`、`vietnamese`、`tibetan`、`stroke` |
| `enabled_schemes` | 会话允许的方案，缺省全部；`scheme` 和之后的 `msime_engine_switch_scheme` 都要在里面，只为其中的方案读词库 |
| `shuangpin.profile` | `xiaohe`、`ziranma`、`shoudao`、`microsoft`、`smart_abc`、`custom` |
| `shuangpin.custom` | `profile` 为 `custom` 时的键位表：`{"initials": {"zh": "v", …}, "finals": {"iang": "l", …}, "zero_initials": {"a": "aa", …}}`；键是小写字母或 `;`，`ü` 写作 `v`，表不合法时建会话失败 |
| `shuangpin.preedit_shows_keys` | 预编辑显示按键原文（缺省 true）还是还原出的全拼 |
| `wubi` | `profile`（`wubi86`、`wubi98`）、`mixed_pinyin`（码表没有时按全拼出字）、`z_wildcard`（`z` 匹配任一码位） |
| `vietnamese` | `input_method`（`telex`、`vni`）、`tone_style`（`modern` 声调在第二个元音上、`classic`） |
| `korean.layout` | `dubeolsik`、`romaja` |
| `helpcode` | `enabled`、`schema`（`resources/helpcodes/` 下的表名，例如 `wubi86`） |
| `punctuation` | `chinese`、`paired`、`lock`（`follow`、`chinese`、`ascii`） |
| `learning`、`personal_context` | 学习词频与用户词、学习词序与搭配 |
| `next_word_prediction` | 上屏后联想，缺省关；`MSIME_ENGINE_OPTION_PREDICTION_ALLOWED` 是它的即时门禁 |
| `fuzzy_pinyin` | 模糊音规则：`z_zh`、`c_ch`、`s_sh`、`n_l`、`f_h`、`r_l`、`an_ang`、`en_eng`、`in_ing`、`ian_iang`、`uan_uang` |
| `autocorrect` | 全拼纠错：`transposition`、`neighbor`、`deletion`、`insertion` |
| `english` | `mixed_candidates`（中文里混入英文单词）、`minimum_prefix`（1–8）、`first_page_count`（1–3）、1.1 起还有 `learn_on_enter`（回车学英文词的总开关，缺省 true：只学专用英文里打的词）、`learn_typed_spellings_on_enter`（缺省 false；打开后回车也把临时英文的字母和拼不成完整音节的拼音学成英文词，九宫格组字、整句改字、计算等生成文本的模式和网址不学，这是 msime 自己的策略） |
| `first_page_rows` | 每页候选数，1–255 |
| `language_dictionaries` | `cantonese`、`zhuyin`、`stroke`、`japanese` 各自数据库的路径；没有时对应方案激活失败（日文只出假名） |

建好之后能改的选项经 `msime_engine_set_int`、`msime_engine_set_string`，编号是头文件里的 `MSIME_ENGINE_OPTION_*`。

## 个人词典

不需要会话。`msime_engine_user_dict_list` 交回 `{"entries":[{"kind":"pinyin","key":"ni'hao","value":"你好","weight":100000}],"has_more":false}`，`kind` 是 `pinyin`、`wubi`、`wubi98`、`quick_phrase`、`english`。`msime_engine_user_dict_edit` 收 `{"previous": 条目或 null, "replacement": 条目或 null, "request_id": "add-1"}`：只有 `replacement` 是新增，只有 `previous` 是删除（它要和列表里的那条完全一致），两个都有是替换；`weight` 可以省略。`request_id` 让同一次编辑重试时不重复生效。代次里没有 `msime-pinyin.db`（用 `msime_engine_prepare_runtime_for` 为没有全拼、双拼、五笔的方案准备的）时只能编辑英文词，列表也只列英文词；连 `msime-english.db` 也没有时什么词都存不下。

## 要带的数据文件

库本身不带词库。运行时文件放在 `resources` 里，可以从 [msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 `dict-v*` 发布取，也可以自己按同样的格式生成（[docs/data-formats.md](https://github.com/metasequoiaime/msime/blob/develop/docs/data-formats.md)）。全拼、双拼要的那几个文件连同它们要求随附的许可证，在本库的发布里打成了一个数据包（见[快速上手](#快速上手)）。数据许可证跟着数据走，不随引擎：采用方按自己的发布方式挑选数据。

`msime_engine_prepare_runtime` 和 `msime_engine_reset_learning` 按全部方案准备，所以要 `msime-pinyin.db` 和 `msime-english.db`。只用其中几个方案时用 `msime_engine_prepare_runtime_for` 和 `msime_engine_reset_learning_for`，`resources` 只要有下表里这些方案的文件：没有全拼、双拼、五笔时两个库都不要求。

| 方案 | 至少要有 | 数据许可证 |
| --- | --- | --- |
| 全拼、双拼、五笔 | `msime-pinyin.db`、`msime-english.db`，以及 `msime-english.db` 的 SCOWL 版权文件 `msime-scowl_Copyright.txt`（分发 `msime-english.db` 时必须一同携带） | `msime-pinyin.db` 是 GPL-3.0（rime-ice 加补充），与万象共有词条的权重改编自 rime-wanxiang（CC BY 4.0，署名与改动说明在 `resources/licenses/rime-wanxiang-CC-BY-4.0.txt`）；`msime-english.db` 是 GPL-3.0（rime-ice-en 加补充），英文词表含 SCOWL 的词 |
| 全拼、双拼 | 排序与 msime 一样还要 `msime-bigram.bin`、`msime-trigram.bin` | n-gram 的计数来自 zhwiki（CC-BY-SA-4.0），词表来自 GPL-3.0 的 `msime-pinyin.db` |
| 五笔 | 再加 `msime-wubi.db` | 混有 Apache-2.0（86 极点码表）、GPL-3.0（rime-ice 词条与人工词条补入的词）、GPL-3.0-or-later（Fcitx 的 98 表）、Unlicense（98 五笔小组的表）和 WTFPL（区划名称）的来源，逐项见 `resources/licenses/msime-engine-dictionary-NOTICE.md` |
| 日文 | `msime-japanese.dat`（没有时只出假名） | Mozc 开源词库（BSD-3-Clause 加词典附加条款），带它的 README 与 LICENSE（`msime-mozc_dictionary_oss_README.txt`、`msime-mozc_LICENSE.txt`） |
| 韩文、越南文、藏文 | 什么也不用带 | 无 |
| 粤语、注音、笔画 | `msime-cantonese.db`、`msime-zhuyin.db`、`msime-stroke.db`（经 `language_dictionaries`） | 粤语 CC BY 4.0；注音 LGPL-2.1-or-later 加 MIT；笔画 LGPL-3.0-only 加 GPL-3.0-only（笔顺码来自 rime-stroke，排序权重来自 rime-ice 的字频，其中与万象共有的读音改编自 rime-wanxiang，CC BY 4.0），带 `resources/licenses/` 下对应的许可证文件 |
| 专用英文（上一行那些方案里可选） | `msime-english.db` 与 `msime-scowl_Copyright.txt` | 同第一行的 `msime-english.db`。不带时专用英文只出键入的字母、回车不学词，个人词典也存不下英文词（`msime_engine_user_dict_edit` 返回 `MSIME_ENGINE_FAILED`） |
| 辅助码 | `resources/helpcodes/` 下的表 | 只有 `wubi86` 有来源与许可说明；仓库里其余几张没有再分发授权，不要随产品分发 |

数据格式的规格文档还没有写完（引擎库方案的 P5），在那之前以 `crates/engine` 的读端为准。

## 二进制归档与第三方声明

C 库的二进制静态链接了整张 Rust 依赖图和 bundled SQLite，每个二进制归档都要带上这些依赖的声明。本库发布的预编译归档已经带齐下面这些（由 [`packaging/package.sh`](packaging/package.sh) 组装，`.github/workflows/release-engine-capi.yml` 在三个平台上调用，`SOURCE.txt` 写明对应源码的提交和 rink-core 的源码位置）；自己构建、自己分发二进制时照同样的清单准备。用 Linux 打包同一个收集脚本，只走 `msime-engine-capi` 的依赖图：

```sh
python3 -X utf8 platforms/linux/collect-notices.py cargo dist/THIRD_PARTY_NOTICES.txt msime-engine-capi
```

在要打包的那个平台上运行，Cargo 按该平台解析依赖图；`-X utf8` 是给 Windows 的，那里 Python 默认按 cp1252 读 cargo 的输出。它逐字复制每个 crate 自带的许可证、NOTICE 和版权文件（包括带 SQLite 的 `libsqlite3-sys`），本仓库自己的 crate 不列；自己不带许可证文件的 crate（现在是 `ewts`、`exmex`、`rink-core`）只列出声明的许可证表达式。归档里另外要放：

- 引擎的许可证：`crates/engine/LICENSE`（MPL-2.0 全文与 Exhibit A 声明，引擎、`msime-path-trust` 和 rink-core 用），以及这个目录的 `LICENSE`（同一份 MPL-2.0 全文，这个 crate 用）。
- rink-core 的源码位置（MPL-2.0 §3.2）：crates.io 上的 `rink-core` 0.9，上游是 codeberg.org/tiffany/rink。
- `ewts` 的许可证：`resources/licenses/ewts-MIT.txt`。crate 自己不带许可证文件，这份 MIT 全文末尾还附着它转换表来源 ewts-js 的 Apache-2.0 署名。
- 编进引擎的数据表和改编代码的许可证与署名：逐个文件见 [`crates/engine/README.md`](https://github.com/metasequoiaime/msime/blob/develop/crates/engine/README.md) 的「许可证」一节，表里「全文」一列的文件都在 `resources/licenses/` 下，原样放进归档。

CI 的 `engine-capi` 作业每次都跑一遍上面的命令，核对它能为这张依赖图生成声明，并且不带许可证文件的 crate 仍只有上面列出的三个：依赖图里多出一个时作业失败，先在这里补上它的许可证文本。

## 不在这个 ABI 里

在线、云与 AI 候选；插件指令；翻译与释义；宿主喂入的表（指令表、联系人、快捷短语）；同步与备份；只为 msime 自己的基准测试公开的接口；排序内部与重排模型；实验性能力（多语言、日文混输、触点与滑行、九键与 14 键、手写、emoji 目录、英文补全与音标）。整句只给引擎认为最好的那一种读法（`sentence_alternatives` 固定为假），候选是引擎原始的顺序。引擎的警告日志写到标准错误，前缀 `msime: `；ABI 1 还没有日志回调。

## 许可证

这个目录里的代码（`src/`、`include/`、`examples/`、`tests/`）以 Mozilla Public License 2.0 发布，全文在 [`LICENSE`](LICENSE)，不附 Exhibit B（与 GPL 兼容）：

> This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

向这个目录提交的贡献同样以 MPL-2.0 授权。

它链接的 `msime-engine` 和 `msime-path-trust` 同样是 MPL-2.0，所以从这里构建出的二进制整体按 MPL-2.0 分发；链接进去的第三方 crate 和编进引擎的数据表保留各自的许可证。仓库其余部分仍是 GPL-3.0-only。
