#include "JniCallbacks.h"

#include <atomic>
#include <limits>
#include <new>

namespace msime::android {
namespace {

std::atomic<JavaVM *> java_vm{nullptr};

// 一个注册给 Rust 的会话存储：Java 对象的全局引用和它的三个方法。方法按对象的实际类查，Java 一侧实现 NativeClient.SessionStore 即可。
struct SessionStore {
    jobject store = nullptr;
    jmethodID load = nullptr;
    jmethodID save = nullptr;
    jmethodID remove = nullptr;
};

// 这一次回调用的 JNIEnv。不能用调用 msime_client_* 的那条线程的 env：回调可能在 Rust 自己起的线程上发生。当前线程已经 attach 时直接取；没有 attach 时由这里 attach，析构时 detach，否则线程退出时 ART 会以「native thread exited while attached」中止进程。取不到 env 时 get() 为空，回调按失败返回。
class ThreadEnv {
public:
    ThreadEnv() noexcept {
        JavaVM *vm = java_vm.load(std::memory_order_acquire);
        if (!vm) return;
        void *current = nullptr;
        const jint status = vm->GetEnv(&current, JNI_VERSION_1_6);
        if (status == JNI_OK) {
            env_ = static_cast<JNIEnv *>(current);
            return;
        }
        if (status != JNI_EDETACHED) return;
        JavaVMAttachArgs arguments{JNI_VERSION_1_6, const_cast<char *>("msime-session-store"), nullptr};
        // NDK 的 jni.h 声明 AttachCurrentThread(JNIEnv **, void *)，JDK 的是 (void **, void *)；桌面上的回调测试用后者。
#ifdef __ANDROID__
        JNIEnv *attached = nullptr;
#else
        void *attached = nullptr;
#endif
        if (vm->AttachCurrentThread(&attached, &arguments) != JNI_OK) return;
        env_ = static_cast<JNIEnv *>(attached);
        attached_vm_ = vm;
    }
    ~ThreadEnv() {
        if (attached_vm_) attached_vm_->DetachCurrentThread();
    }
    ThreadEnv(const ThreadEnv &) = delete;
    ThreadEnv &operator=(const ThreadEnv &) = delete;

    JNIEnv *get() const noexcept { return env_; }

private:
    JNIEnv *env_ = nullptr;
    // 只有本对象 attach 的线程才记下 VM，析构时 detach；本来就 attach 着的线程（调用 msime_client_* 的 Java 线程）不能被这里 detach。
    JavaVM *attached_vm_ = nullptr;
};

// 每次回调开一个局部帧。在本来就 attach 着的线程上，局部引用要等外层那次 JNI 调用返回才释放，而一次 msime_client_* 调用里可能有很多次回调。
class LocalFrame {
public:
    explicit LocalFrame(JNIEnv *env) noexcept : env_(env), pushed_(env->PushLocalFrame(4) == JNI_OK) {
        if (!pushed_) env_->ExceptionClear();
    }
    ~LocalFrame() {
        if (pushed_) env_->PopLocalFrame(nullptr);
    }
    LocalFrame(const LocalFrame &) = delete;
    LocalFrame &operator=(const LocalFrame &) = delete;

    bool pushed() const noexcept { return pushed_; }

private:
    JNIEnv *env_;
    bool pushed_;
};

// Java 方法抛出了异常时清掉它，返回 true。
bool cleared(JNIEnv *env) noexcept {
    if (!env->ExceptionCheck()) return false;
    env->ExceptionClear();
    return true;
}

void throw_out_of_memory(JNIEnv *env) noexcept {
    if (env->ExceptionCheck()) return;
    jclass type = env->FindClass("java/lang/OutOfMemoryError");
    if (type) env->ThrowNew(type, "session store context");
}

// listener 这次抛出的异常取下来存着，线程上不再挂着它。
void keep_failure(ListenerCall &call) noexcept {
    if (!call.env->ExceptionCheck()) return;
    call.failure = call.env->ExceptionOccurred();
    call.env->ExceptionClear();
}

} // namespace

void remember_java_vm(JavaVM *vm) noexcept {
    java_vm.store(vm, std::memory_order_release);
}

void *session_store_create(JNIEnv *env, jobject object) noexcept {
    if (!env || !object) return nullptr;
    jclass type = env->GetObjectClass(object);
    if (!type) return nullptr;
    jmethodID load = env->GetMethodID(type, "load", "(I)[B");
    jmethodID save = load ? env->GetMethodID(type, "save", "(I[B)I") : nullptr;
    jmethodID remove = save ? env->GetMethodID(type, "remove", "(I)I") : nullptr;
    env->DeleteLocalRef(type);
    if (!remove) return nullptr;
    auto *store = new (std::nothrow) SessionStore{};
    if (!store) {
        throw_out_of_memory(env);
        return nullptr;
    }
    store->store = env->NewGlobalRef(object);
    if (!store->store) {
        delete store;
        throw_out_of_memory(env);
        return nullptr;
    }
    store->load = load;
    store->save = save;
    store->remove = remove;
    return store;
}

intptr_t session_store_load(void *context, uint32_t item, uint8_t *buffer, size_t capacity) noexcept {
    auto *store = static_cast<SessionStore *>(context);
    if (!store || !buffer) return -1;
    ThreadEnv thread;
    JNIEnv *env = thread.get();
    if (!env || env->ExceptionCheck()) return -1;
    LocalFrame frame(env);
    if (!frame.pushed()) return -1;
    auto bytes = static_cast<jbyteArray>(env->CallObjectMethod(store->store, store->load, static_cast<jint>(item)));
    if (cleared(env)) return -1;
    // null 和空数组都是「不存在」：头文件里 0 就表示不存在，写入的会话从不为空。
    if (!bytes) return 0;
    const jsize length = env->GetArrayLength(bytes);
    if (length == 0) return 0;
    // 比 capacity 长的一项按失败处理，一个字节也不往 buffer 里写。
    if (static_cast<size_t>(length) > capacity) return -1;
    env->GetByteArrayRegion(bytes, 0, length, reinterpret_cast<jbyte *>(buffer));
    if (cleared(env)) return -1;
    return static_cast<intptr_t>(length);
}

int32_t session_store_save(void *context, uint32_t item, const uint8_t *bytes, size_t length) noexcept {
    auto *store = static_cast<SessionStore *>(context);
    if (!store || (!bytes && length > 0) || length > static_cast<size_t>(std::numeric_limits<jsize>::max())) return -1;
    ThreadEnv thread;
    JNIEnv *env = thread.get();
    if (!env || env->ExceptionCheck()) return -1;
    LocalFrame frame(env);
    if (!frame.pushed()) return -1;
    jbyteArray array = env->NewByteArray(static_cast<jsize>(length));
    if (!array) {
        env->ExceptionClear();
        return -1;
    }
    if (length > 0) {
        env->SetByteArrayRegion(array, 0, static_cast<jsize>(length), reinterpret_cast<const jbyte *>(bytes));
        if (cleared(env)) return -1;
    }
    const jint result = env->CallIntMethod(store->store, store->save, static_cast<jint>(item), array);
    if (cleared(env)) return -1;
    return static_cast<int32_t>(result);
}

int32_t session_store_remove(void *context, uint32_t item) noexcept {
    auto *store = static_cast<SessionStore *>(context);
    if (!store) return -1;
    ThreadEnv thread;
    JNIEnv *env = thread.get();
    if (!env || env->ExceptionCheck()) return -1;
    const jint result = env->CallIntMethod(store->store, store->remove, static_cast<jint>(item));
    if (cleared(env)) return -1;
    return static_cast<int32_t>(result);
}

void session_store_release(void *context) noexcept {
    auto *store = static_cast<SessionStore *>(context);
    if (!store) return;
    ThreadEnv thread;
    // 取不到 env 时（还没记下 JavaVM，或者 attach 失败）全局引用只能留着，那个 Java 对象跟着进程走；上下文本身照样释放。DeleteGlobalRef 在挂着异常时也可以调用。
    if (JNIEnv *env = thread.get()) env->DeleteGlobalRef(store->store);
    delete store;
}

bool listener_begin(JNIEnv *env, jobject listener, const char *name, const char *signature, ListenerCall &call) noexcept {
    call = ListenerCall{};
    call.env = env;
    call.listener = listener;
    if (!listener) return true;
    jclass type = env->GetObjectClass(listener);
    if (!type) return false;
    call.method = env->GetMethodID(type, name, signature);
    env->DeleteLocalRef(type);
    return call.method != nullptr;
}

void chat_delta(void *context, const uint8_t *text, size_t length) noexcept {
    auto *call = static_cast<ListenerCall *>(context);
    if (!call || !call->listener || !call->method || call->failure) return;
    if ((!text && length > 0) || length > static_cast<size_t>(std::numeric_limits<jsize>::max())) return;
    JNIEnv *env = call->env;
    if (env->ExceptionCheck()) {
        keep_failure(*call);
        return;
    }
    jbyteArray bytes = env->NewByteArray(static_cast<jsize>(length));
    if (bytes) {
        if (length > 0) env->SetByteArrayRegion(bytes, 0, static_cast<jsize>(length), reinterpret_cast<const jbyte *>(text));
        if (!env->ExceptionCheck()) env->CallVoidMethod(call->listener, call->method, bytes);
        env->DeleteLocalRef(bytes);
    }
    keep_failure(*call);
}

void ai_skin_progress(void *context, uint32_t completed) noexcept {
    auto *call = static_cast<ListenerCall *>(context);
    if (!call || !call->listener || !call->method || call->failure) return;
    JNIEnv *env = call->env;
    if (env->ExceptionCheck()) {
        keep_failure(*call);
        return;
    }
    const jint value = completed > static_cast<uint32_t>(std::numeric_limits<jint>::max())
        ? std::numeric_limits<jint>::max() : static_cast<jint>(completed);
    env->CallVoidMethod(call->listener, call->method, value);
    keep_failure(*call);
}

bool listener_finish(ListenerCall &call) noexcept {
    if (!call.failure) return false;
    call.env->Throw(call.failure);
    call.env->DeleteLocalRef(call.failure);
    call.failure = nullptr;
    return true;
}

} // namespace msime::android
