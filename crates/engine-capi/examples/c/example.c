/*
 * msime-engine 的 C 示例：准备运行时、建会话、打一句全拼、核对修订号后选词上屏、冲刷学习、释放会话，再从个人词典里读回学到的这句。
 *
 *   example RESOURCES USER_DATA CACHE
 *
 * RESOURCES 里要有 msime-pinyin.db 和 msime-english.db（tests/fixture/make_fixture.py 生成最小的一份，或者发布里的数据包 msime-engine-data-quanpin-*），三个目录都要是绝对路径。成功退出 0，任何一步不对都打印原因并退出 1。
 *
 * 选中的是整句联想解出的句子时，学习把它作为用户词写进个人词典（夹具词库就是这样）；词库里本来就有这个词时（真实词库里的「你好世界」），学习只调它的词频，个人词典里不会多出一条，这时不要求读回。
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
#include <msime_engine.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef MSIME_EXAMPLE_SYSTEM_SQLITE
#include <sqlite3.h>
#endif

#define EXPECTED_SENTENCE "\xe4\xbd\xa0\xe5\xa5\xbd\xe4\xb8\x96\xe7\x95\x8c" /* 你好世界 */

static int fail(const char *step, int32_t status) {
    fprintf(stderr, "%s failed with status %d: %s\n", step, (int)status, msime_engine_last_error());
    return 1;
}

#define CHECK(step, call)                      \
    do {                                       \
        int32_t status_ = (call);              \
        if (status_ != MSIME_ENGINE_OK) {      \
            return fail(step, status_);        \
        }                                      \
    } while (0)

static msime_engine_str borrow(const char *text) {
    msime_engine_str value;
    value.ptr = text;
    value.len = text == NULL ? 0 : strlen(text);
    return value;
}

static int same(msime_engine_str value, const char *expected) {
    return value.len == strlen(expected) && memcmp(value.ptr, expected, value.len) == 0;
}

/* JSON 里的路径要转义反斜杠和引号（Windows 路径带反斜杠）。 */
static void append_json_string(char *buffer, size_t size, const char *text) {
    size_t used = strlen(buffer);
    if (used + 1 < size) {
        buffer[used++] = '"';
    }
    for (; *text != '\0' && used + 3 < size; ++text) {
        if (*text == '\\' || *text == '"') {
            buffer[used++] = '\\';
        }
        buffer[used++] = *text;
    }
    if (used + 1 < size) {
        buffer[used++] = '"';
    }
    buffer[used] = '\0';
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "usage: %s RESOURCES USER_DATA CACHE\n", argv[0]);
        return 2;
    }
    const char *resources = argv[1];
    const char *user_data = argv[2];
    const char *cache = argv[3];

    uint32_t abi = msime_engine_abi_version();
    if ((abi >> 16) != MSIME_ENGINE_ABI_MAJOR || (abi & 0xffff) < MSIME_ENGINE_ABI_MINOR) {
        fprintf(stderr, "library ABI %u.%u does not match header ABI %u.%u\n", abi >> 16, abi & 0xffff,
                MSIME_ENGINE_ABI_MAJOR, MSIME_ENGINE_ABI_MINOR);
        return 1;
    }
    printf("msime-engine %s, ABI %u.%u\n", msime_engine_version(), abi >> 16, abi & 0xffff);
#ifdef MSIME_EXAMPLE_SYSTEM_SQLITE
    printf("linked next to SQLite %s\n", sqlite3_libversion());
#endif

    /* 1. 准备运行时：在 USER_DATA/dictionaries/demo 准备代次。 */
    msime_engine_paths paths;
    memset(&paths, 0, sizeof paths);
    paths.struct_size = sizeof paths;
    paths.resources = borrow(resources);
    paths.user_data = borrow(user_data);
    paths.cache = borrow(cache);
    char *dictionaries = NULL;
    const char *content_id = "demo";
    CHECK("prepare_runtime", msime_engine_prepare_runtime(&paths, content_id, strlen(content_id), &dictionaries));
    paths.dictionaries = borrow(dictionaries);

    /* 2. 建会话。 */
    char options[4096] = "{\"version\":1,\"scheme\":\"quanpin\",\"paths\":{\"resources\":";
    append_json_string(options, sizeof options, resources);
    strncat(options, ",\"user_data\":", sizeof options - strlen(options) - 1);
    append_json_string(options, sizeof options, user_data);
    strncat(options, ",\"cache\":", sizeof options - strlen(options) - 1);
    append_json_string(options, sizeof options, cache);
    strncat(options, ",\"dictionaries\":", sizeof options - strlen(options) - 1);
    append_json_string(options, sizeof options, dictionaries);
    strncat(options, "}}", sizeof options - strlen(options) - 1);
    msime_engine_session *session = NULL;
    CHECK("session_new", msime_engine_session_new(options, strlen(options), &session));

    /* 3. 打一句全拼。字母一律走 key_char。 */
    const char *keys = "nihaoshijie";
    msime_engine_result result;
    for (const char *key = keys; *key != '\0'; ++key) {
        memset(&result, 0, sizeof result);
        result.struct_size = sizeof result;
        CHECK("key_char", msime_engine_key_char(session, (uint8_t)*key, 0, &result));
        if (!result.handled) {
            fprintf(stderr, "key %c was not handled\n", *key);
            return 1;
        }
    }

    /* 4. 读状态和第一页候选。 */
    msime_engine_state state;
    memset(&state, 0, sizeof state);
    state.struct_size = sizeof state;
    CHECK("state_get", msime_engine_state_get(session, &state));
    printf("preedit: %.*s (revision %llu, %u candidates)\n", (int)state.preedit.len, state.preedit.ptr,
           (unsigned long long)state.revision, state.candidate_count);
    uint32_t page = state.candidate_count < 5 ? state.candidate_count : 5;
    int first_is_sentence = 0;
    for (uint32_t index = 0; index < page; ++index) {
        msime_engine_candidate candidate;
        memset(&candidate, 0, sizeof candidate);
        candidate.struct_size = sizeof candidate;
        CHECK("candidate_get", msime_engine_candidate_get(session, state.revision, index, &candidate));
        printf("  %u. %.*s [%.*s] source %u\n", index + 1, (int)candidate.word.len, candidate.word.ptr,
               (int)candidate.code.len, candidate.code.ptr, candidate.source);
        if (index == 0) {
            first_is_sentence = (candidate.flags & MSIME_ENGINE_CANDIDATE_SENTENCE) != 0;
        }
    }

    /* 5. 修订号对不上的选择被拒绝，什么也不做。 */
    memset(&result, 0, sizeof result);
    result.struct_size = sizeof result;
    if (msime_engine_select(session, state.revision + 1, 0, &result) != MSIME_ENGINE_STALE) {
        fprintf(stderr, "a stale revision was accepted\n");
        return 1;
    }

    /* 6. 选第一行上屏。 */
    CHECK("select", msime_engine_select(session, state.revision, 0, &result));
    printf("commit: %.*s\n", (int)result.commit.len, result.commit.ptr);
    if (!result.has_commit || !same(result.commit, EXPECTED_SENTENCE)) {
        fprintf(stderr, "expected the sentence to be committed\n");
        return 1;
    }

    /* 7. 冲刷学习，释放会话（释放时也会写下最近的学习、交还词库租约）。 */
    CHECK("flush_learning", msime_engine_flush_learning(0));
    CHECK("session_free", msime_engine_session_free(session));

    /* 8. 选的是整句时，个人词典里有刚学到的这句。会话都释放了，维护类的调用才拿得到租约；列表不需要。 */
    char *listed = NULL;
    CHECK("user_dict_list", msime_engine_user_dict_list(&paths, 0, 0, 0, &listed));
    printf("personal dictionary: %s\n", listed);
    int learned = strstr(listed, EXPECTED_SENTENCE) != NULL;
    msime_engine_string_free(listed);
    msime_engine_string_free(dictionaries);
    CHECK("flush_learning(stop)", msime_engine_flush_learning(MSIME_ENGINE_FLUSH_STOP_THREAD));
    if (first_is_sentence && !learned) {
        fprintf(stderr, "the sentence was not learned\n");
        return 1;
    }
    if (!first_is_sentence) {
        printf("the dictionary already has this word: learning adjusted its frequency instead of adding a personal entry\n");
    }
    printf("ok\n");
    return 0;
}
