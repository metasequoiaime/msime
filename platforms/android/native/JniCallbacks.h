#pragma once

// 共享层回调进 Java 的那一半：注入的会话存储（msime_client_session_store 的 load、save、remove、release）和两种只在调用线程上触发的 listener（流式对话的增量、AI 皮肤的进度）。单独成一个编译单元、不依赖 msime_client_* 的导出，这样 check-host.sh 能在桌面 JVM 上直接驱动它（tests/native/JniCallbacksTest.cpp），不用模拟器。
#include <jni.h>
#include <cstddef>
#include <cstdint>

namespace msime::android {

// JNI_OnLoad 里记下的 JavaVM。会话存储的回调可能在 Rust 自己起的线程上发生，那里没有现成的 JNIEnv，只能经它取。
void remember_java_vm(JavaVM *vm) noexcept;

// 为一个有 byte[] load(int)、int save(int, byte[])、int remove(int) 三个方法的 Java 对象（NativeClient.SessionStore）建回调上下文，持有它的全局引用。失败时返回 nullptr，并留下挂起的 Java 异常（NoSuchMethodError 或 OutOfMemoryError）由调用方交还给 Java。成功后上下文归 session_store_release 释放：交给 msime_client_session_store_register 且注册成功以后由 Rust 调用它，注册失败时由调用方自己调用。
void *session_store_create(JNIEnv *env, jobject store) noexcept;

// 下面四个就是 msime_client_session_store 的 load、save、remove、release，契约见头文件。任意线程上都可以调用：当前线程没有 attach 时先 attach，返回前 detach；本来就 attach 着的线程不动。Java 方法抛出的异常在这里清掉、按失败返回，不会留给调用 msime_client_* 的那一方；进入时这条线程上已经挂着别人的异常则不碰 Java、直接按失败返回，也不清掉那个异常。
intptr_t session_store_load(void *context, uint32_t item, uint8_t *buffer, size_t capacity) noexcept;
int32_t session_store_save(void *context, uint32_t item, const uint8_t *bytes, size_t length) noexcept;
int32_t session_store_remove(void *context, uint32_t item) noexcept;
void session_store_release(void *context) noexcept;

// 在调用 msime_client_* 的同一线程上把回调转给 Java 的 listener。listener 抛出的异常先取下来存着、从线程上清掉：同一次调用里 Rust 还可能在这条线程上调会话存储的回调，异常挂着时那些 JNI 调用都不合法。之后不再调用这个 listener，调用结束后 listener_finish 把异常原样抛回 Java。
struct ListenerCall {
    JNIEnv *env = nullptr;
    jobject listener = nullptr;
    jmethodID method = nullptr;
    jthrowable failure = nullptr;
};

// listener 为 null 时什么也不查、返回 true（调用方随后不传回调）。找不到方法时返回 false，NoSuchMethodError 挂在线程上。
bool listener_begin(JNIEnv *env, jobject listener, const char *name, const char *signature, ListenerCall &call) noexcept;
// msime_client_account_chat_delta_callback：context 是 ListenerCall，listener 的方法是 void onDelta(byte[])。
void chat_delta(void *context, const uint8_t *text, size_t length) noexcept;
// msime_client_ai_skin_progress_callback：context 是 ListenerCall，listener 的方法是 void onProgress(int)。
void ai_skin_progress(void *context, uint32_t completed) noexcept;
// 调用结束后调用：listener 抛过异常时把它重新抛到线程上并返回 true，调用方随即丢掉这次的结果、返回 null。
bool listener_finish(ListenerCall &call) noexcept;

} // namespace msime::android
