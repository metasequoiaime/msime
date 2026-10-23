/*
 * 水杉输入法桌面脚本插件 ABI v1 的 C 头文件。
 *
 * 模块是 core wasm（wasm32-unknown-unknown），没有 WASI，没有 libc；宿主是纯解释器，没有 JIT。编译示例（clang 自带 wasm32 后端，链接要 wasm-ld）：
 *
 *     clang --target=wasm32-unknown-unknown -O2 -ffreestanding -nostdlib \
 *         -Wl,--no-entry -Wl,-z,stack-size=65536 -o plugin.wasm plugin.c
 *
 * 规则（导入时和输入法加载时用同一份校验，`msime-pack validate` 也是它）：
 *
 * - 必须导出 `memory`（恰好一块 32 位线性内存，不能导入）、`msime_abi`（返回 MSIME_ABI_VERSION）和 `msime_on_event`（参数是钩子编号，返回负数算这一次失败）；可选导出 `msime_init`，实例化后调用一次，返回负数算初始化失败。
 * - 不能有 start 函数，不能用 threads、memory64、multi-memory、exceptions、tail-call、extended-const、GC、SIMD。
 * - import 只能是下面声明的 `msime` 函数。导入了哪个能力域的函数，清单就必须申请那个能力；申请了的能力也必须至少导入一个属于它的函数。只声明不调用的函数不会成为 import，所以整个头文件都可以包含进来。
 * - 文本都是 UTF-8。取文本的函数按这个顺序检查：没授予所属能力返回 MSIME_DENIED；字段编号不对返回 MSIME_INVALID，事件里没有或被隐私规则屏蔽返回 MSIME_ABSENT；然后检查整个 [dst, dst + cap) 在线性内存里，越界直接 trap；最后放不下返回 MSIME_LIMIT，什么都不复制，先用对应的 *_scalar 读长度。前两步返回时不看 dst，所以坏指针不一定 trap。
 * - 被隐私规则屏蔽的字段和事件里没有的字段一样，都返回 MSIME_ABSENT。
 *
 * 编号是插件和宿主之间的契约，只会追加，不会改值。完整说明见仓库的 docs/plugins.md「脚本插件」一节。
 */

#ifndef MSIME_SCRIPT_H
#define MSIME_SCRIPT_H

#include <stdint.h>

#define MSIME_ABI_VERSION 1

#define MSIME_IMPORT(name) __attribute__((import_module("msime"), import_name(#name)))
#define MSIME_EXPORT(name) __attribute__((export_name(#name)))

/* ---- 返回码：成功时是非负数（复制的字节数、字段值） ---- */

/* 没有授予这个函数所属的能力。 */
#define MSIME_DENIED (-1)
/* 这个事件没有这个字段，或者它被隐私规则屏蔽，两者故意不可区分。 */
#define MSIME_ABSENT (-2)
/* 字段编号不认识，或者参数不合法。 */
#define MSIME_INVALID (-3)
/* 目标缓冲区放不下，什么都没有复制。 */
#define MSIME_LIMIT (-4)

/* ---- 钩子：msime_on_event 的参数，也是 MSIME_EVENT_KIND 的值；清单 hooks 里写小写的名字 ---- */

#define MSIME_HOOK_FOCUS_IN 0
#define MSIME_HOOK_FOCUS_OUT 1
#define MSIME_HOOK_INPUT 6
#define MSIME_HOOK_SELECTION_RETRACTED 7
#define MSIME_HOOK_CANDIDATE_SELECTED 8
#define MSIME_HOOK_BEFORE_COMMIT 9
#define MSIME_HOOK_COMMIT 10
#define MSIME_HOOK_COMPOSITION_END 11
#define MSIME_HOOK_COMPOSITION_START 12
#define MSIME_HOOK_COMPOSITION_UPDATE 13
#define MSIME_HOOK_CANDIDATES_SHOWN 14
#define MSIME_HOOK_CANDIDATES_HIDDEN 15
#define MSIME_HOOK_CANDIDATES 17
#define MSIME_HOOK_COMMAND 18

/* ---- 不需要能力：事件的枚举字段和日志 ---- */

/* msime_event_scalar 的字段。 */
#define MSIME_EVENT_KIND 0
#define MSIME_EVENT_ORIGIN 1
#define MSIME_EVENT_HOW 2
#define MSIME_EVENT_OUTCOME 3
#define MSIME_EVENT_REASON 4
#define MSIME_EVENT_LOCAL_MODE 5
#define MSIME_EVENT_INPUT_CLASS 6
#define MSIME_EVENT_RETRACTION 7
#define MSIME_EVENT_REWRITABLE 8
#define MSIME_EVENT_CONTINUES 9
#define MSIME_EVENT_GENERATED 10

/* MSIME_EVENT_ORIGIN：上屏文本从哪来（commit、before_commit）。 */
#define MSIME_ORIGIN_SELECTION 0
#define MSIME_ORIGIN_RAW 1
#define MSIME_ORIGIN_HELD_FLUSH 2
#define MSIME_ORIGIN_SCHEME_TEXT 3
#define MSIME_ORIGIN_FOCUS_LOSS 4

/* MSIME_EVENT_HOW：怎么选中的（candidate_selected）。 */
#define MSIME_HOW_FINISH 0
#define MSIME_HOW_PUNCTUATION_FINISH 1
#define MSIME_HOW_WUBI_TOP 2
#define MSIME_HOW_SPACE_OVER_LIST 3
#define MSIME_HOW_DIGIT 4
#define MSIME_HOW_GRID_RAW 5
#define MSIME_HOW_SELECT 6
#define MSIME_HOW_ANY_CANDIDATE 7
#define MSIME_HOW_EDGE 8
#define MSIME_HOW_HIGHLIGHTED 9
#define MSIME_HOW_WUBI_AUTO 10
#define MSIME_HOW_GRID_LIMIT 11

/* MSIME_EVENT_OUTCOME：选中之后的结果（candidate_selected）。 */
#define MSIME_OUTCOME_COMMITTED 0
#define MSIME_OUTCOME_HELD 1
#define MSIME_OUTCOME_PINNED 2
#define MSIME_OUTCOME_PARTIAL 3

/* MSIME_EVENT_REASON：组字结束或候选隐藏的原因（composition_end、candidates_hidden）。 */
#define MSIME_REASON_COMMIT 0
#define MSIME_REASON_CANCEL 1
#define MSIME_REASON_DELETED 2
#define MSIME_REASON_FOCUS 3
#define MSIME_REASON_DESTROYED 4
#define MSIME_REASON_HOST_COMMIT 5
#define MSIME_REASON_OTHER 6

/* MSIME_EVENT_LOCAL_MODE：本地模式。只有 0 到 5 能读到文本，其余模式下文本类函数一律返回 MSIME_ABSENT。 */
#define MSIME_LOCAL_MODE_NONE 0
#define MSIME_LOCAL_MODE_TEMPORARY_ENGLISH 1
#define MSIME_LOCAL_MODE_TEMPORARY_JAPANESE 2
#define MSIME_LOCAL_MODE_SUPER_JIANPIN 3
#define MSIME_LOCAL_MODE_EMOJI 4
#define MSIME_LOCAL_MODE_KAOMOJI 5
#define MSIME_LOCAL_MODE_MENTION 6
#define MSIME_LOCAL_MODE_URL 7
#define MSIME_LOCAL_MODE_COMMAND 8
#define MSIME_LOCAL_MODE_EXPRESSION 9
#define MSIME_LOCAL_MODE_UNICODE 10
#define MSIME_LOCAL_MODE_DATE_TIME 11
#define MSIME_LOCAL_MODE_QUICK_PHRASE 12
#define MSIME_LOCAL_MODE_CHAIZI 13
#define MSIME_LOCAL_MODE_SYMBOL 14
#define MSIME_LOCAL_MODE_UNKNOWN 255

/* MSIME_EVENT_INPUT_CLASS：打字前那次操作的类别（input）。 */
#define MSIME_INPUT_CLASS_CHARACTER 0
#define MSIME_INPUT_CLASS_PUNCTUATION 1
#define MSIME_INPUT_CLASS_SELECTION 2
#define MSIME_INPUT_CLASS_NAVIGATION 3
#define MSIME_INPUT_CLASS_EDIT 4
#define MSIME_INPUT_CLASS_COMMIT 5
#define MSIME_INPUT_CLASS_CANCEL 6
#define MSIME_INPUT_CLASS_HOST_COMMIT 7
#define MSIME_INPUT_CLASS_OTHER 8
#define MSIME_INPUT_CLASS_FOCUS_LOSS 9

/* MSIME_EVENT_RETRACTION：撤回暂存选择的方式（selection_retracted）。 */
#define MSIME_RETRACTION_RESTORE 0
#define MSIME_RETRACTION_DROP 1

/* 枚举字段的值；这个事件没有该字段时返回 MSIME_ABSENT。 */
MSIME_IMPORT(event_scalar) int64_t msime_event_scalar(int32_t field);

/* msime_log 的级别。 */
#define MSIME_LOG_DEBUG 0
#define MSIME_LOG_INFO 1
#define MSIME_LOG_WARN 2
#define MSIME_LOG_ERROR 3

/* 写一行日志。输入法里是空操作，只有 msime-pack run 会把它打出来。 */
MSIME_IMPORT(log) void msime_log(int32_t level, const char *text, int32_t len);

/* ---- read_composition：预编辑串、读音、这次打的字符 ---- */

/* 文本：预编辑串；标量：它的字节数。composition_start、composition_update。 */
#define MSIME_COMPOSITION_PREEDIT 0
/* 文本：读音；标量：它的字节数。composition_start、composition_update。 */
#define MSIME_COMPOSITION_READING 1
/* 文本：这次打的字符；标量：它的字节数。input。 */
#define MSIME_COMPOSITION_INPUT_CHAR 2
/* 只有标量：光标前已经打了几个编码字符（按打下的字母算，不是按预编辑串）。 */
#define MSIME_COMPOSITION_CARET 3

MSIME_IMPORT(composition_text) int32_t msime_composition_text(int32_t field, char *dst, int32_t cap);
MSIME_IMPORT(composition_scalar) int64_t msime_composition_scalar(int32_t field);

/* ---- read_commit：选中的候选和上屏文本 ---- */

/* 文本：选中的候选；标量：它的字节数。candidate_selected。 */
#define MSIME_COMMIT_SELECTED 0
/* 文本：完整的上屏文本；标量：它的字节数。before_commit、commit。 */
#define MSIME_COMMIT_COMMIT 1
/* 文本：上屏文本里可改写的那一段（不含标点、顶字的尾巴）；标量：它的字节数。before_commit、commit。 */
#define MSIME_COMMIT_COMMIT_BODY 2
/* 只有标量：选中候选在呈现顺序里的座位。candidate_selected。 */
#define MSIME_COMMIT_SEAT 3

MSIME_IMPORT(commit_text) int32_t msime_commit_text(int32_t field, char *dst, int32_t cap);
MSIME_IMPORT(commit_scalar) int64_t msime_commit_scalar(int32_t field);

/* ---- read_candidates：候选窗口里的前 20 行，插件变换之前的顺序和注释（candidates_shown、candidates） ---- */

/* msime_candidate_meta 的字段。 */
#define MSIME_CANDIDATE_SOURCE 0
#define MSIME_CANDIDATE_TEXT_LEN 1
#define MSIME_CANDIDATE_CODE_LEN 2
#define MSIME_CANDIDATE_ANNOTATION_LEN 3
/* 当前页码，不看 index。 */
#define MSIME_CANDIDATE_PAGE 16
/* 高亮行在页内的下标，不看 index。 */
#define MSIME_CANDIDATE_HIGHLIGHTED 17

MSIME_IMPORT(candidate_count) int32_t msime_candidate_count(void);
MSIME_IMPORT(candidate_text) int32_t msime_candidate_text(int32_t index, char *dst, int32_t cap);
MSIME_IMPORT(candidate_code) int32_t msime_candidate_code(int32_t index, char *dst, int32_t cap);
MSIME_IMPORT(candidate_annotation) int32_t msime_candidate_annotation(int32_t index, char *dst, int32_t cap);
MSIME_IMPORT(candidate_meta) int64_t msime_candidate_meta(int32_t index, int32_t field);

/* ---- 修改类：rewrite_commit 只在 before_commit 里、其余三个只在 candidates 里有效，别处返回 MSIME_DENIED；规则见 docs/plugins.md ---- */

/* rewrite_commit（依赖 read_commit，不能和 read_composition 同时申请）：把上屏文本的可改写段换成 text，只在 before_commit 里。 */
MSIME_IMPORT(rewrite_commit) int32_t msime_rewrite_commit(const char *text, int32_t len);

/* rerank_candidates（依赖 read_candidates）：按 rows 里的行号重排，只在 candidates 里。 */
MSIME_IMPORT(candidates_order) int32_t msime_candidates_order(const uint32_t *rows, int32_t count);

/* filter_candidates（依赖 read_candidates）：把 rows 里的行降到末尾并隐藏，只在 candidates 里。 */
MSIME_IMPORT(candidates_hide) int32_t msime_candidates_hide(const uint32_t *rows, int32_t count);

/* annotate_candidates（依赖 read_candidates）：给第 index 行加注释，只在 candidates 里。 */
MSIME_IMPORT(candidates_annotate) int32_t msime_candidates_annotate(int32_t index, const char *text, int32_t len);

/* ---- provide_commands：在自己声明的 / 指令里给出候选（清单 [[script.commands]] 声明触发词，hooks 里要有 command）。command 只投递给触发词的主人，跑在 msime_init 之后的快照上，每次都看不到之前任何事件留下的状态 ---- */

/* msime_command_text 的字段。 */
/* 用户打的触发词，即清单里的某个 trigger。 */
#define MSIME_COMMAND_TRIGGER 0
/* 触发词后面用户打的小写字母，可以为空（/uuid），最多 64 字节。 */
#define MSIME_COMMAND_ARGUMENT 1

/* 复制触发词或参数，只在 command 里有值，别处返回 MSIME_ABSENT。 */
MSIME_IMPORT(command_text) int32_t msime_command_text(int32_t field, char *dst, int32_t cap);

/* 给出一行候选：text 是选中时上屏的文本（1 到 64 个 UTF-16 码元，首尾不能是空白，不能含控制、格式和不可见字符，也不能含 ://、www.、mailto:），note 是画在插件注释格里的说明（最多 32 个 UTF-16 码元，可以为空）。只在 command 里有效，别处返回 MSIME_DENIED；第 10 行或太长返回 MSIME_LIMIT，不合规返回 MSIME_INVALID；成功返回行号。钩子正常返回之后这些行才会显示。 */
MSIME_IMPORT(command_row) int32_t msime_command_row(const char *text, int32_t text_len, const char *note, int32_t note_len);

/* ---- 不需要能力：随机数 ---- */

/* 每次 command 事件有 64 字节随机数，按调用顺序取：把接下来的 len 个字节写进 dst，返回 len；剩下的不够时返回 MSIME_LIMIT，什么都不写。别的钩子里没有随机数。 */
MSIME_IMPORT(random_fill) int32_t msime_random_fill(uint8_t *dst, int32_t len);

#endif /* MSIME_SCRIPT_H */
