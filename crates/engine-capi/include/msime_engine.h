/*
 * 水杉输入法引擎的 C 接口（msime-engine-capi）。
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * 本文件由 cbindgen 从 crates/engine-capi/src 生成，不要手改；用法、线程与内存约定见 crates/engine-capi/README.md。
 */

#ifndef MSIME_ENGINE_H
#define MSIME_ENGINE_H

#include <stddef.h>
#include <stdint.h>

// ABI 主版本。同一个主版本内只追加函数、常量和结构体尾部的字段，不改已有的含义；Linux 的 SONAME 带的就是它（`libmsime_engine.so.1`）。
#define MSIME_ENGINE_ABI_MAJOR 1

// ABI 次版本，每次追加加一。调用方可以按 `msime_engine_abi_version()` 判断运行时的库有没有某个后加的函数。
#define MSIME_ENGINE_ABI_MINOR 1

// 成功。
#define MSIME_ENGINE_OK 0

// 参数不合法：选项 JSON 有未知键或取值不对、索引越界、不是 UTF-8、`struct_size` 比 1.0 的布局还小等。
#define MSIME_ENGINE_INVALID_ARGUMENT 1

// 引擎没能完成操作（例如换到一个没有词库的方案）。
#define MSIME_ENGINE_FAILED 2

// SQLite 或文件系统出错。
#define MSIME_ENGINE_STORAGE 3

// 这个库不认识的选项编号、模型种类或标志位。
#define MSIME_ENGINE_UNSUPPORTED 4

// 传入的 `revision` 不是会话当前的候选修订号：候选已经变了，先重新 `msime_engine_state_get`。
#define MSIME_ENGINE_STALE 5

// 会话正被另一个线程调用（同一会话的调用不能并发），或者词库正被别的会话、别的进程占着（维护要等所有会话关掉）。什么也没做，稍后重试。
#define MSIME_ENGINE_BUSY 6

// 之前的某次调用在库里 panic 了，会话已作废，只能 `msime_engine_session_free`。进程级函数 panic 时也返回它，但不作废任何东西。
#define MSIME_ENGINE_PANIC 7

// 必需的指针参数是 NULL。
#define MSIME_ENGINE_NULL 8

#define MSIME_ENGINE_SCHEME_QUANPIN 0

#define MSIME_ENGINE_SCHEME_SHUANGPIN 1

#define MSIME_ENGINE_SCHEME_WUBI 2

#define MSIME_ENGINE_SCHEME_JAPANESE 3

#define MSIME_ENGINE_SCHEME_KOREAN 4

#define MSIME_ENGINE_SCHEME_CANTONESE 5

#define MSIME_ENGINE_SCHEME_ZHUYIN 6

#define MSIME_ENGINE_SCHEME_VIETNAMESE 7

#define MSIME_ENGINE_SCHEME_TIBETAN 8

#define MSIME_ENGINE_SCHEME_STROKE 9

#define MSIME_ENGINE_COMMAND_BACKSPACE 0

// 上屏首选（引擎顺序的第 0 行）。宿主自己高亮了别的行时用 `msime_engine_select`。
#define MSIME_ENGINE_COMMAND_COMMIT_CANDIDATE 1

// 上屏按键原文（回车）。专用英文里打的词学进英文词库；选项 `english.learn_typed_spellings_on_enter` 打开时，临时英文的字母和拼不成音节的拼音也学。
#define MSIME_ENGINE_COMMAND_COMMIT_RAW 2

// 丢弃组字（Esc）。越南文和藏文的第一次只把显示切回按键原文，再发一次才丢弃；开着可打开的候选列表（韩文汉字）时第一次只关列表。
#define MSIME_ENGINE_COMMAND_CANCEL 3

#define MSIME_ENGINE_COMMAND_MOVE_LEFT 4

#define MSIME_ENGINE_COMMAND_MOVE_RIGHT 5

#define MSIME_ENGINE_COMMAND_MOVE_HOME 6

#define MSIME_ENGINE_COMMAND_MOVE_END 7

#define MSIME_ENGINE_COMMAND_DELETE_FORWARD 8

// 只用于日文：切换假名写法。
#define MSIME_ENGINE_COMMAND_CYCLE_KANA_VARIANT 9

// 只用于日文：上屏假名读音。
#define MSIME_ENGINE_COMMAND_COMMIT_READING 10

// 打开或关闭当前方案可打开的候选列表：韩文当前音节的汉字、注音整句的其他转换。
#define MSIME_ENGINE_COMMAND_OPEN_CANDIDATE_LIST 11

// 整句改字的光标左移一个字；进不了改字时与 `MOVE_LEFT` 相同。
#define MSIME_ENGINE_COMMAND_CONVERSION_LEFT 12

// 整句改字的光标右移一个字；不在改字里时与 `MOVE_RIGHT` 相同。
#define MSIME_ENGINE_COMMAND_CONVERSION_RIGHT 13

// ABI 1.1 起：与 `COMMIT_RAW` 上屏同样的原文，但不学英文词（例如宿主的 Shift+回车）。1.0 的库对它返回 `MSIME_ENGINE_UNSUPPORTED`。
#define MSIME_ENGINE_COMMAND_COMMIT_RAW_WITHOUT_LEARNING 14

#define MSIME_ENGINE_SOURCE_DATABASE 0

#define MSIME_ENGINE_SOURCE_USER_DATABASE 1

#define MSIME_ENGINE_SOURCE_CLOUD_SUGGESTION 2

#define MSIME_ENGINE_SOURCE_AI_SUGGESTION 3

#define MSIME_ENGINE_SOURCE_ENGLISH_DICTIONARY 4

#define MSIME_ENGINE_SOURCE_QUICK_PHRASE 5

// emoji 行，也用于打拼音出符号的符号行：选中时整段上屏，不学习。
#define MSIME_ENGINE_SOURCE_EMOJI 6

#define MSIME_ENGINE_SOURCE_KAOMOJI 7

// 词格解出的整句。
#define MSIME_ENGINE_SOURCE_GENERATED 8

// 解码器给不出词时的原文或兜底行。
#define MSIME_ENGINE_SOURCE_FALLBACK 9

#define MSIME_ENGINE_SOURCE_NEURAL_KEYBOARD 11

#define MSIME_ENGINE_SOURCE_AI_PENDING 12

#define MSIME_ENGINE_SOURCE_MIXED_JAPANESE 13

#define MSIME_ENGINE_SOURCE_PLUGIN 14

#define MSIME_ENGINE_SOURCE_CLOUD_PENDING 15

#define MSIME_ENGINE_SOURCE_CONTACT 16

#define MSIME_ENGINE_SOURCE_COLLOCATION 17

#define MSIME_ENGINE_SOURCE_MULTILINGUAL 18

#define MSIME_ENGINE_LOCAL_MODE_NONE 0

#define MSIME_ENGINE_LOCAL_MODE_UNICODE 1

#define MSIME_ENGINE_LOCAL_MODE_DATE_TIME 2

#define MSIME_ENGINE_LOCAL_MODE_QUICK_PHRASE 3

#define MSIME_ENGINE_LOCAL_MODE_EMOJI 4

#define MSIME_ENGINE_LOCAL_MODE_KAOMOJI 5

#define MSIME_ENGINE_LOCAL_MODE_SUPER_JIANPIN 6

#define MSIME_ENGINE_LOCAL_MODE_TEMPORARY_ENGLISH 7

#define MSIME_ENGINE_LOCAL_MODE_TEMPORARY_JAPANESE 8

#define MSIME_ENGINE_LOCAL_MODE_EXPRESSION 9

#define MSIME_ENGINE_LOCAL_MODE_COMMAND 10

#define MSIME_ENGINE_LOCAL_MODE_MENTION 11

#define MSIME_ENGINE_LOCAL_MODE_URL 12

#define MSIME_ENGINE_LOCAL_MODE_CHAIZI 13

#define MSIME_ENGINE_LOCAL_MODE_SYMBOL 14

// `msime_engine_key_char` 的 `flags`：这是单独按下的 Shift 加字母（大写字母开本地模式的入口）。
#define MSIME_ENGINE_KEY_SHIFT_ONLY (1 << 0)

// `msime_engine_state::flags`：候选是上屏后的联想，只接受显式选择，空格、回车不上屏它们。
#define MSIME_ENGINE_STATE_PREDICTION (1 << 0)

// `msime_engine_state::flags`：专用英文模式开着。
#define MSIME_ENGINE_STATE_DEDICATED_ENGLISH (1 << 1)

// `msime_engine_state::flags`：方案可打开的候选列表（韩文汉字）正开着。
#define MSIME_ENGINE_STATE_CANDIDATE_LIST_OPEN (1 << 2)

// `msime_engine_state::flags`：此刻的 `'` 是分隔符（单位与目标之间、`/fy` 的词之间），要按字符交给 `msime_engine_key_char`，不能当标点结束组字。
#define MSIME_ENGINE_STATE_TAKES_SEPARATOR (1 << 3)

// `msime_engine_candidate::flags`：按模糊音读出的行。
#define MSIME_ENGINE_CANDIDATE_FUZZY (1 << 0)

// `msime_engine_candidate::flags`：这一行回答的是整段输入，不是它的前缀或更长的补全。
#define MSIME_ENGINE_CANDIDATE_ANSWERS_KEY (1 << 1)

// `msime_engine_candidate::flags`：整句联想解出的整句。
#define MSIME_ENGINE_CANDIDATE_SENTENCE (1 << 2)

// `msime_engine_select_edge` 的 `edge`：上屏这一行的第一个汉字。
#define MSIME_ENGINE_EDGE_FIRST_HAN 0

// `msime_engine_select_edge` 的 `edge`：上屏这一行的最后一个汉字。
#define MSIME_ENGINE_EDGE_LAST_HAN 1

// `msime_engine_flush_learning` 的 `flags`：写完之后停掉后台写线程并等它结束。卸载库或退出进程前用；之后会话照常可用，下一次学习重新起线程。
#define MSIME_ENGINE_FLUSH_STOP_THREAD (1 << 0)

// `msime_engine_user_dict_list` 的 `flags`：同时列出随包多字词被学习或编辑过的拼音权重。
#define MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED (1 << 0)

// `msime_engine_preload_model` 的 `kind`：`msime-japanese.dat`。
#define MSIME_ENGINE_MODEL_JAPANESE 1

// `msime_engine_set_caret` 的 `caret_byte` 取这个值（或任何负数）时光标回到末尾。
#define MSIME_ENGINE_CARET_END -1

// 整数：中文标点开关，0 或 1。
#define MSIME_ENGINE_OPTION_CHINESE_PUNCT 1

// 整数：成对标点开关，0 或 1。
#define MSIME_ENGINE_OPTION_PAIRED_PUNCT 2

// 整数：标点锁，0 跟随模式、1 强制中文、2 强制 ASCII。
#define MSIME_ENGINE_OPTION_PUNCT_LOCK 3

// 整数：专用英文模式，0 或 1；只在模式真的改变时丢弃组字。
#define MSIME_ENGINE_OPTION_DEDICATED_ENGLISH 4

// 整数：辅助码开关（全拼和双拼一起），0 或 1。
#define MSIME_ENGINE_OPTION_HELPCODE_ENABLED 5

// 字符串：辅助码表名，例如 `wubi86`；读不到表时 `MSIME_ENGINE_FAILED`。
#define MSIME_ENGINE_OPTION_HELPCODE_SCHEMA 6

// 整数：个人上下文学习（词序与搭配），0 或 1；关掉时也结束当前上下文。
#define MSIME_ENGINE_OPTION_PERSONAL_CONTEXT 7

// 整数：宿主的即时隐私门禁，0 时清掉上屏后联想、之后不再给出，选项 `next_word_prediction` 本身不变。
#define MSIME_ENGINE_OPTION_PREDICTION_ALLOWED 8

// 整数：宿主每页的候选数（1 到 255）；决定全长组的同音词够多少行就不再并入更长的补全词。
#define MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS 9

// 字符串：整句模型参照的已上屏文字（只看最后 64 个字），宿主在光标前文字变化时交来。
#define MSIME_ENGINE_OPTION_RESCORING_CONTEXT 10

// 整数，ABI 1.1 起：调频方式，`MSIME_ENGINE_FREQUENCY_*`，与选项 `frequency.mode` 相同。从下一次选择起按新方式学，已经学下的排序照旧生效，眼前的候选不重排。
#define MSIME_ENGINE_OPTION_FREQUENCY_MODE 11

// 整数，ABI 1.1 起：同一个词要被选中几次才挪位（1 到 10），与选项 `frequency.trigger_count` 相同。
#define MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT 12

// 整数，ABI 1.1 起：`MSIME_ENGINE_FREQUENCY_LINEAR` 每次前移几位（1 到 10），与选项 `frequency.linear_step` 相同。
#define MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP 13

// 整数，ABI 1.1 起：五笔拼音混输（码表没有时按全拼出字），0 或 1，与选项 `wubi.mixed_pinyin` 相同；五笔组字中切换时立即按新设置重查候选。
#define MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN 14

// 不调频：选词不改变以后的排序（缺省）。
#define MSIME_ENGINE_FREQUENCY_DISABLED 0

// 选中的词直接排到首位。
#define MSIME_ENGINE_FREQUENCY_PIN 1

// 选中的词挪到原位置的一半处：下标 n 挪到 n/2（下标从 0 数）。
#define MSIME_ENGINE_FREQUENCY_HALVE 2

// 选中的词前移 `linear_step` 位。
#define MSIME_ENGINE_FREQUENCY_LINEAR 3

// 选中的词前移一位；下标大于 4 的直接挪到下标 4（第 5 位）。
#define MSIME_ENGINE_FREQUENCY_PROMOTE 4

// 不透明的会话句柄，由 `msime_engine_session_new` 建、`msime_engine_session_free` 释放。
typedef struct msime_engine_session msime_engine_session;

// 日志回调，ABI 1.1 起。`message` 是一行 UTF-8 的警告（不带 `msime: ` 前缀和换行），NUL 结尾，`message_len` 是不含 NUL 的字节数；只在这次调用期间有效。`user_data` 是注册时交来的那个指针，库不碰它。
typedef void (*msime_engine_log_callback)(void *user_data, const char *message, size_t message_len);

// 库借给调用方的一段 UTF-8 文本：`len` 个字节，不以 NUL 结尾，按长度读。空串时 `ptr` 也不是 NULL。指向会话里缓存的数据，有效期见各个函数：热路径上的都到同一会话下一次改变状态的调用为止。
typedef struct msime_engine_str {
  const char *ptr;
  size_t len;
} msime_engine_str;

// 运行时的四个根目录（ABI 1.0），都要是绝对路径，UTF-8。库只读 min(调用方大小, 库内大小) 个字节，读不到的字段按全零处理。
typedef struct msime_engine_paths {
  uint32_t struct_size;
  // 只读的随包资源：词库、辅助码表、模型。
  struct msime_engine_str resources;
  // 用户数据：学习日志 `msime_user.db` 和代次目录 `dictionaries/`。
  struct msime_engine_str user_data;
  // 可以随时清空的缓存。
  struct msime_engine_str cache;
  // 代次目录，`msime_engine_prepare_runtime` 准备并交回；`msime_engine_prepare_runtime` 不读这一项，调用方可以不填。
  struct msime_engine_str dictionaries;
} msime_engine_paths;

// 一次按键、命令或选择的结果（ABI 1.0）。调用前把 `struct_size` 设为 `sizeof(msime_engine_result)`；库写入 min(调用方大小, 库内大小) 个字节，把 `struct_size` 改成写入的大小，多出的部分清零。`commit` 和 `diagnostic` 借自会话，到同一会话下一次改变状态的调用为止有效。
typedef struct msime_engine_result {
  uint32_t struct_size;
  // 引擎接管了这一键（非零）。为零时宿主按自己的规则处理，例如把字符原样交给应用。
  uint32_t handled;
  // 有上屏文字（非零），文字在 `commit`；上屏的也可能是空串。
  uint32_t has_commit;
  // 诊断的稳定数字码（引擎的 `DiagnosticCode`）；0 是没有诊断，0xFFFF 是表外的文字。诊断不是失败：键照样处理了，只是学习没能存下之类。
  uint32_t diagnostic_code;
  // 调用之后的候选修订号，与 `msime_engine_state::revision` 相同。
  uint64_t revision;
  struct msime_engine_str commit;
  // 诊断的说明文字（英文），没有时为空串。
  struct msime_engine_str diagnostic;
} msime_engine_result;

// 组字状态（ABI 1.0），`msime_engine_state_get` 填写，收发规则同 `msime_engine_result`。字符串借自会话按修订号缓存的快照，`msime_engine_state_get`、`msime_engine_candidate_get` 这类只读调用不让它们失效。
typedef struct msime_engine_state {
  uint32_t struct_size;
  // 当前方案，`MSIME_ENGINE_SCHEME_*`。
  uint32_t scheme;
  // 候选修订号：每次可能改变候选的调用都让它变，从 0 起只增不减。按下标选择和取候选时原样交回。
  uint64_t revision;
  uint32_t candidate_count;
  // `MSIME_ENGINE_STATE_*` 的组合。
  uint32_t flags;
  // `MSIME_ENGINE_LOCAL_MODE_*`。
  uint32_t local_mode;
  // 组字光标在 `editing_text` 里的字节偏移。
  uint32_t caret_byte;
  // 整句改字的光标，按 `conversion` 的字（Unicode 标量）计：光标在第 `conversion_focus_start_scalar` 个字之前，`[start, end)` 是候选替换的那一段；不在改字时两者都是 0。
  uint32_t conversion_focus_start_scalar;
  uint32_t conversion_focus_end_scalar;
  // 给用户看的组字：全拼按音节分开（`ni'hao`），双拼默认是按键原文，本地模式与专用英文是按键原文。
  struct msime_engine_str preedit;
  // 按键原文（ASCII），`caret_byte` 指向它；为空就是没有在组字。
  struct msime_engine_str editing_text;
  // 整句改字时改好的整句，宿主在它非空时把它画在行内代替拼音。
  struct msime_engine_str conversion;
  // 此刻要按字符（`msime_engine_key_char`）而不是按标点交来的非字母键，见 README 的按键路由。
  struct msime_engine_str spelling_symbols;
} msime_engine_state;

// 一行候选（ABI 1.0），`msime_engine_candidate_get` 填写，收发规则同 `msime_engine_result`，字符串的有效期同 `msime_engine_state`。
typedef struct msime_engine_candidate {
  uint32_t struct_size;
  // 这一行的下标（就是传进来的那个）。
  uint32_t index;
  // `MSIME_ENGINE_SOURCE_*`。
  uint32_t source;
  // 给出这一行的方案，`MSIME_ENGINE_SCHEME_*`（五笔混拼里的拼音行是全拼）。
  uint32_t scheme;
  // `MSIME_ENGINE_CANDIDATE_*` 的组合。
  uint32_t flags;
  // 用户固定的位置 1..=5，没固定时 0。
  int32_t fixed_position;
  struct msime_engine_str word;
  // 这一行匹配的编码，按输入的写法（全拼是带 `'` 的音节）。
  struct msime_engine_str code;
  // 注释：辅助码或纠错提示，没有时为空串。
  struct msime_engine_str annotation;
  // 纠错读出的行：用户实际打的字母；其余为空串。
  struct msime_engine_str corrected_from;
} msime_engine_candidate;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// 运行时这份库的 ABI 版本：`(major << 16) | minor`。主版本与编译时的 `MSIME_ENGINE_ABI_MAJOR` 不同就不要用它；次版本比编译时小时，后加的函数不可用。
uint32_t msime_engine_abi_version(void);

// 库的发布版本，静态的 NUL 结尾字符串，不要释放。
const char *msime_engine_version(void);

// 本线程上一次调用失败的说明（英文，NUL 结尾），没有失败时是空串。有效到本线程下一次调用这个库为止（调它自己除外），不要释放。
const char *msime_engine_last_error(void);

// 释放库交出的字符串（选项以外的 JSON、路径）。NULL 什么也不做。不要用 `free()`，库和调用方可能用不同的分配器。
//
// # Safety
// `text` 是 NULL 或这个库交出、还没释放过的字符串。
void msime_engine_string_free(char *text);

// ABI 1.1 起。把引擎的警告日志交给 `callback`，`callback` 为 NULL 时回到缺省：写到标准错误，每行前缀 `msime: `。日志是进程级的：所有会话和库的后台线程记的都交给它，再注册一次就换掉上一个。
//
// 回调可能在任何线程上被调用，也可能在几个线程上同时被调用，要自己处理并发；要尽快返回，不要在里面调这个库的任何函数（在回调里调本函数返回 `MSIME_ENGINE_BUSY`，回调期间同一线程上再记的日志被丢掉）。回调不能抛出 C++ 异常或 `longjmp` 出去。
//
// 本函数返回之后，旧回调没有还在执行的调用，以后也不会再被调用，旧的 `user_data` 可以释放；新回调在注册期间要一直可调，`user_data` 一直有效，直到用别的回调或 NULL 换掉它。进程退出前不必换回缺省。
//
// # Safety
// `callback` 是 NULL，或者在它被换掉之前一直可以在任何线程上以 `user_data` 调用的函数。
int32_t msime_engine_set_log_callback(msime_engine_log_callback callback,
                                      void *user_data);

// 准备运行时：在 `user_data/dictionaries/<content_id>` 原子地准备一个代次（从 `resources` 复制工作词库并回放学习日志），已就绪的代次只补回放。`content_id` 是 1 到 128 个 `[0-9A-Za-z_-]`，随包数据变了就换一个；带点的版本号要先换掉点（`dict-v2_0_39`）。三个根目录必须是绝对路径、互不重叠。`paths.dictionaries` 不读。按全部方案准备：`resources` 里必须有 `msime-pinyin.db` 和 `msime-english.db`；只用其中几个方案时用 `msime_engine_prepare_runtime_for`，只带那几个方案的数据。成功时把代次目录写进 `*dictionaries`（可以是 NULL），用 `msime_engine_string_free` 释放；建会话时它就是 `paths.dictionaries`。
//
// # Safety
// `paths` 按 `struct_size` 约定可读；`content_id` 指向 `content_id_len` 个可读字节；`dictionaries` 是 NULL 或可写一个指针。
int32_t msime_engine_prepare_runtime(const struct msime_engine_paths *paths,
                                     const char *content_id,
                                     size_t content_id_len,
                                     char **dictionaries);

// ABI 1.1 起。同 `msime_engine_prepare_runtime`，只为 `schemes` 里的方案准备：`resources` 只要有这些方案读的词库。`schemes` 的第 n 位是序号为 n 的方案（`1u << MSIME_ENGINE_SCHEME_KOREAN`），要与会话选项的 `enabled_schemes` 相同，不能是 0，不认识的位是 `MSIME_ENGINE_INVALID_ARGUMENT`。有全拼、双拼或五笔时与 `msime_engine_prepare_runtime` 相同，要 `msime-pinyin.db` 和 `msime-english.db`；没有时两者都不要求，`resources` 带了 `msime-english.db` 就照样复制进代次（专用英文的补全与回车学词用它），没带时代次里没有它。日文的 `msime-japanese.dat` 和粤语、注音、笔画的语言库不经过代次，会话从 `resources` 或选项给的路径直接读。
//
// # Safety
// 同 `msime_engine_prepare_runtime`。
int32_t msime_engine_prepare_runtime_for(const struct msime_engine_paths *paths,
                                         const char *content_id,
                                         size_t content_id_len,
                                         uint32_t schemes,
                                         char **dictionaries);

// 立刻写下所有排着的个人上下文学习（平时延迟约 2 秒写）。`flags` 带 `MSIME_ENGINE_FLUSH_STOP_THREAD` 时再停掉后台写线程并等它结束：卸载库、退出进程前用。
int32_t msime_engine_flush_learning(uint32_t flags);

// 内存告警时调：放掉进程级缓存里没有会话在用的日文模型和个人学习模型。正在用的会话不受影响。
int32_t msime_engine_release_caches(void);

// 关掉所有线程上缓存着的空闲数据库连接。删除或替换数据目录之前调；正被某次调用用着的连接在那次调用结束时关。
int32_t msime_engine_close_databases(void);

// 把模型的字节直接交给引擎，给没有文件系统或想自己读文件的宿主。`kind` 是 `MSIME_ENGINE_MODEL_*`，`path` 是会话读这个模型的位置（日文是 `resources/msime-japanese.dat` 或选项 `language_dictionaries.japanese`）。`bytes` 为 NULL 且 `bytes_len` 为 0 时撤掉之前交的那份。字节不是合法模型时 `MSIME_ENGINE_INVALID_ARGUMENT`，什么也不改。库复制一份，调用返回后 `bytes` 可以释放。
//
// # Safety
// `path` 指向 `path_len` 个可读字节；`bytes` 是 NULL 或指向 `bytes_len` 个可读字节。
int32_t msime_engine_preload_model(uint32_t kind,
                                   const char *path,
                                   size_t path_len,
                                   const uint8_t *bytes,
                                   size_t bytes_len);

// 列出个人词典（用户自己加的词，含自动造的词），按种类、编码、词排序，从第 `offset` 条起最多 `limit` 条（`limit` 为 0 时取 100）。`flags` 是 `MSIME_ENGINE_USER_DICT_*`。结果是 JSON：`{"entries":[{"kind":"pinyin","key":"ni'hao","value":"你好","weight":100000}],"has_more":false}`，用 `msime_engine_string_free` 释放。
//
// # Safety
// `paths` 按 `struct_size` 约定可读；`json` 可写一个指针。
int32_t msime_engine_user_dict_list(const struct msime_engine_paths *paths,
                                    uint32_t offset,
                                    uint32_t limit,
                                    uint32_t flags,
                                    char **json);

// ABI 1.1 起。按 `msime_engine_user_dict_edit` 收新词条（`replacement`）的同一套规则检查一条词条，不碰任何文件，也不需要路径：输入是一条 `{"kind": "pinyin", "key": "NI HAO", "value": "你好", "weight": 100000}`（`weight` 可以省略），合法时把规范化后的词条写进 `*normalized_json`（编码换成小写，拼音编码里的空格换成 `'`：`NI HAO` 成为 `ni'hao`），用 `msime_engine_string_free` 释放。词条不合法时返回 `MSIME_ENGINE_INVALID_ARGUMENT`，`msime_engine_last_error` 是引擎给的英文原因（例如 `Use complete pinyin syllables separated by apostrophes or spaces`），可以直接显示给用户；`*normalized_json` 不写。给前端在用户填编辑框时就提示，不必等到真的写入。
//
// # Safety
// `entry_json` 指向 `entry_len` 个可读字节；`normalized_json` 可写一个指针。
int32_t msime_engine_user_dict_validate(const char *entry_json,
                                        size_t entry_len,
                                        char **normalized_json);

// 改个人词典：`{"previous": 条目或 null, "replacement": 条目或 null, "request_id": "..."}`，previous 为 null 是新增，replacement 为 null 是删除，都有是替换（previous 必须与列表里的那条完全相同）。`request_id`（1 到 128 个 `[A-Za-z0-9_-]`，可以省略）让同一次编辑重试时不重复生效。还有会话开着（本进程或别的进程，包括 msime 自己的输入法）时返回 `MSIME_ENGINE_BUSY`，什么也不写：先释放这些路径上的会话。
//
// # Safety
// `paths` 按 `struct_size` 约定可读；`edit_json` 指向 `edit_len` 个可读字节。
int32_t msime_engine_user_dict_edit(const struct msime_engine_paths *paths,
                                    const char *edit_json,
                                    size_t edit_len);

// 清空学到的一切：换回随包的工作词库、清空学习日志。和 `msime_engine_prepare_runtime` 一样要求 `resources` 里有 `msime-pinyin.db` 和 `msime-english.db`；代次是用 `msime_engine_prepare_runtime_for` 准备的，就用 `msime_engine_reset_learning_for` 传同一个方案集合。还有会话开着时返回 `MSIME_ENGINE_BUSY`，什么也不换。
//
// # Safety
// `paths` 按 `struct_size` 约定可读。
int32_t msime_engine_reset_learning(const struct msime_engine_paths *paths);

// ABI 1.1 起。同 `msime_engine_reset_learning`，只换回 `schemes` 读的词库，规则与 `msime_engine_prepare_runtime_for` 相同：没有全拼、双拼、五笔时不要求 `msime-pinyin.db`，`msime-english.db` 在 `resources` 里有才换回，都没有时只清空日志。`schemes` 的写法同 `msime_engine_prepare_runtime_for`。
//
// # Safety
// `paths` 按 `struct_size` 约定可读。
int32_t msime_engine_reset_learning_for(const struct msime_engine_paths *paths,
                                        uint32_t schemes);

// 按选项 JSON（版本 1，键表见 README）建一个会话，成功时把句柄写进 `*session`，失败时写 NULL。会话持有这两个目录上的共享词库租约，直到释放：租约在维护（`msime_engine_user_dict_edit`、`msime_engine_reset_learning`）进行时拿不到，这时返回 `MSIME_ENGINE_BUSY`。
//
// # Safety
// `options_json` 指向 `options_len` 个可读字节；`session` 可写一个指针。
int32_t msime_engine_session_new(const char *options_json,
                                 size_t options_len,
                                 struct msime_engine_session **session);

// 结束并释放会话：写下最近的学习，交还日志连接和词库租约。NULL 什么也不做。另一个线程正在调这个会话时返回 `MSIME_ENGINE_BUSY`、不释放；已作废的会话照样可以释放。释放之后句柄不能再用。
//
// # Safety
// `session` 是 NULL 或 `msime_engine_session_new` 交出、还没释放过的句柄。
int32_t msime_engine_session_free(struct msime_engine_session *session);

// 换方案（`MSIME_ENGINE_SCHEME_*`），丢弃组字。方案必须在选项的 `enabled_schemes` 里；新方案的词库打不开时失败，留在原方案、组字不动。
//
// # Safety
// `session` 是有效的句柄。
int32_t msime_engine_switch_scheme(struct msime_engine_session *session,
                                   uint32_t scheme);

// 设一个整数选项（`MSIME_ENGINE_OPTION_*` 里标着「整数」的）。不认识的编号是 `MSIME_ENGINE_UNSUPPORTED`，字符串选项或越界的值是 `MSIME_ENGINE_INVALID_ARGUMENT`。
//
// # Safety
// `session` 是有效的句柄。
int32_t msime_engine_set_int(struct msime_engine_session *session,
                             uint32_t option,
                             int64_t value);

// 设一个字符串选项（`MSIME_ENGINE_OPTION_*` 里标着「字符串」的），`value` 是 `value_len` 个字节的 UTF-8。不认识的编号是 `MSIME_ENGINE_UNSUPPORTED`，整数选项是 `MSIME_ENGINE_INVALID_ARGUMENT`。
//
// # Safety
// `session` 是有效的句柄；`value` 指向 `value_len` 个可读字节。
int32_t msime_engine_set_string(struct msime_engine_session *session,
                                uint32_t option,
                                const char *value,
                                size_t value_len);

// 忘掉上屏的上下文（焦点或应用变了、宿主自己插入了文字），清掉上屏后联想。
//
// # Safety
// `session` 是有效的句柄。
int32_t msime_engine_reset_context(struct msime_engine_session *session);

// 一个 ASCII 字符（0x20 到 0x7E，另收 Tab 0x09）：字母、拼写用的数字，以及 `spelling_symbols` 里列出的符号。`flags` 是 `MSIME_ENGINE_KEY_*`。结果写进 `result`（可以是 NULL）。按键路由见 README。
//
// # Safety
// `session` 是有效的句柄；`result` 是 NULL 或按 `struct_size` 约定可写。
int32_t msime_engine_key_char(struct msime_engine_session *session,
                              uint8_t character,
                              uint32_t flags,
                              struct msime_engine_result *result);

// ABI 1.1 起。数字键当选择键，引擎自己的数字选词入口：有组字时 `1`–`9` 上屏整张候选表的第 `digit - 1` 行（也就是第一页，`msime_engine_select` 的那套学习照常）；越南文 VNI 和注音此刻拼写用的数字交给拼写；没有组字时不处理（`handled` 为 0），并且忘掉上屏的上下文，因为这个数字由宿主自己交给应用；上屏后联想的候选不接数字，按下时被清掉。`digit` 只收 ASCII 数字 `0`–`9`，其余是 `MSIME_ENGINE_INVALID_ARGUMENT`。不带修订号：它选的是调用时的候选。Unicode、日期、计算这些用数字拼写的本地模式里它照样选词，所以按键路由仍是先 `msime_engine_key_char`，没被处理时再用它（只显示第一页的前端）或 `msime_engine_select`（翻过页的前端），见 README 按键路由第 4 条。
//
// # Safety
// 同 `msime_engine_key_char`。
int32_t msime_engine_candidate_key(struct msime_engine_session *session,
                                   uint8_t digit,
                                   struct msime_engine_result *result);

// 一个 ASCII 标点：有组字时以首选结束组字，再接上中文标点；中文标点关着、锁成 ASCII 或这个键没有中文标点时不处理（`handled` 为 0），宿主自己输出这个键。宿主高亮了首选以外的行时，先用 `msime_engine_finish` 以那一行结束，再调这个函数。
//
// # Safety
// 同 `msime_engine_key_char`。
int32_t msime_engine_key_punct(struct msime_engine_session *session,
                               uint8_t character,
                               struct msime_engine_result *result);

// 一个编辑命令（`MSIME_ENGINE_COMMAND_*`）。
//
// # Safety
// 同 `msime_engine_key_char`。
int32_t msime_engine_command(struct msime_engine_session *session,
                             uint32_t command,
                             struct msime_engine_result *result);

// 把组字光标放到 `editing_text` 的第 `caret_byte` 个字节前；负数（`MSIME_ENGINE_CARET_END`）回到末尾。位置限制在可编辑的范围内，九键组码时什么也不做。
//
// # Safety
// `session` 是有效的句柄。
int32_t msime_engine_set_caret(struct msime_engine_session *session,
                               int64_t caret_byte);

// 读组字状态。只读：不改变会话，之前借出的字符串照样有效。
//
// # Safety
// `session` 是有效的句柄；`state` 按 `struct_size` 约定可写。
int32_t msime_engine_state_get(struct msime_engine_session *session,
                               struct msime_engine_state *state);

// 读第 `index` 行候选。`revision` 必须是当前的候选修订号，否则 `MSIME_ENGINE_STALE`；越界是 `MSIME_ENGINE_INVALID_ARGUMENT`。只读，可以先取状态再逐个取一页候选。
//
// # Safety
// `session` 是有效的句柄；`candidate` 按 `struct_size` 约定可写。
int32_t msime_engine_candidate_get(struct msime_engine_session *session,
                                   uint64_t revision,
                                   uint32_t index,
                                   struct msime_engine_candidate *candidate);

// 单字母全拼只先给出一部分候选；宿主翻过第一页时调它要回其余的。`*grew` 写 1 表示列表变长了（修订号随之改变），否则写 0。会改变状态。
//
// # Safety
// `session` 是有效的句柄；`grew` 是 NULL 或可写。
int32_t msime_engine_expand_candidates(struct msime_engine_session *session,
                                       uint32_t *grew);

// 整个状态和全部候选的 JSON，给调试和日志用；格式不属于 ABI，随版本变化。返回的字符串用 `msime_engine_string_free` 释放。只读。
//
// # Safety
// `session` 是有效的句柄；`json` 可写一个指针。
int32_t msime_engine_state_json(struct msime_engine_session *session,
                                char **json);

// 选第 `index` 行上屏（或消耗它覆盖的那段编码，剩下的接着组字），并照常学习。
//
// # Safety
// `session` 是有效的句柄；`result` 是 NULL 或按 `struct_size` 约定可写。
int32_t msime_engine_select(struct msime_engine_session *session,
                            uint64_t revision,
                            uint32_t index,
                            struct msime_engine_result *result);

// 以词定字：上屏第 `index` 行的第一个或最后一个汉字（`MSIME_ENGINE_EDGE_*`），丢掉其余组字。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_select_edge(struct msime_engine_session *session,
                                 uint64_t revision,
                                 uint32_t index,
                                 uint32_t edge,
                                 struct msime_engine_result *result);

// 结束组字：从第 `first_index` 行（宿主高亮的那一行）起上屏，用于失焦、标点前或宿主自己的结束键。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_finish(struct msime_engine_session *session,
                            uint64_t revision,
                            uint32_t first_index,
                            struct msime_engine_result *result);

// 把第 `index` 行（词库里的词）提到前面，不上屏。不支持的行不处理（`handled` 为 0）。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_pin(struct msime_engine_session *session,
                         uint64_t revision,
                         uint32_t index,
                         struct msime_engine_result *result);

// 删掉第 `index` 行：用户自己的词从个人词典删除，随包的词不再出现。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_remove(struct msime_engine_session *session,
                            uint64_t revision,
                            uint32_t index,
                            struct msime_engine_result *result);

// 把第 `index` 行固定在第 `position` 位（1..=5）。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_fix_position(struct msime_engine_session *session,
                                  uint64_t revision,
                                  uint32_t index,
                                  int32_t position,
                                  struct msime_engine_result *result);

// 取消第 `index` 行的固定位置。
//
// # Safety
// 同 `msime_engine_select`。
int32_t msime_engine_clear_position(struct msime_engine_session *session,
                                    uint64_t revision,
                                    uint32_t index,
                                    struct msime_engine_result *result);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* MSIME_ENGINE_H */
