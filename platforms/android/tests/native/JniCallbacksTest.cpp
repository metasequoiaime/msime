// 在桌面 JVM（JDK 的 JNI 调用接口，-Xcheck:jni）上驱动 native/JniCallbacks.cpp：会话存储的回调在没有 attach 的线程上要自己 attach、返回前 detach，在本来就 attach 着的线程上不能 detach；Java 抛出的异常要清掉、按失败返回；release 要放掉全局引用；listener 抛出的异常先存着、调用结束后再抛。模拟器上的设备冒烟造不出没有 attach 的线程，所以这些在这里验。用法：jni-callbacks-test <JniCallbacksFixture 所在的 classpath>。
#include "../../native/JniCallbacks.h"

#include <jni.h>

#include <cstdio>
#include <cstring>
#include <string>
#include <thread>

namespace {

int failures = 0;

void check(bool condition, const char *message, int line) {
    if (condition) return;
    std::fprintf(stderr, "JniCallbacksTest:%d: %s\n", line, message);
    ++failures;
}
#define CHECK(condition, message) check((condition), (message), __LINE__)

JavaVM *vm = nullptr;

// 当前线程是否 attach 着。
bool attached() {
    void *env = nullptr;
    return vm->GetEnv(&env, JNI_VERSION_1_6) == JNI_OK;
}

struct Fixture {
    JNIEnv *env;
    jclass type;
    jobject object;

    int field(const char *name) {
        return env->GetIntField(object, env->GetFieldID(type, name, "I"));
    }
    void set_int(const char *name, int value) {
        env->SetIntField(object, env->GetFieldID(type, name, "I"), value);
    }
    void set_boolean(const char *name, bool value) {
        env->SetBooleanField(object, env->GetFieldID(type, name, "Z"), value ? JNI_TRUE : JNI_FALSE);
    }
    void set_failing(bool value) { set_boolean("failing", value); }
    std::string string_of(jstring value) {
        const char *chars = env->GetStringUTFChars(value, nullptr);
        std::string text = chars ? chars : "";
        if (chars) env->ReleaseStringUTFChars(value, chars);
        env->DeleteLocalRef(value);
        return text;
    }
    std::string last_thread() {
        return string_of(static_cast<jstring>(env->GetObjectField(object, env->GetFieldID(type, "lastThread", "Ljava/lang/String;"))));
    }
    std::string delta_text() {
        auto text = static_cast<jstring>(env->CallObjectMethod(object, env->GetMethodID(type, "deltaText", "()Ljava/lang/String;")));
        if (env->ExceptionCheck() || !text) return "";
        return string_of(text);
    }
    bool collected() {
        const jboolean result = env->CallStaticBooleanMethod(type, env->GetStaticMethodID(type, "collected", "()Z"));
        return !env->ExceptionCheck() && result == JNI_TRUE;
    }
};

void store_on_attached_thread(Fixture &fixture, void *store) {
    JNIEnv *env = fixture.env;
    uint8_t buffer[16];
    CHECK(msime::android::session_store_save(store, 1, reinterpret_cast<const uint8_t *>("abc"), 3) == 0, "save on the calling thread");
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == 3 && std::memcmp(buffer, "abc", 3) == 0, "load returns the saved bytes");
    CHECK(msime::android::session_store_load(store, 2, buffer, sizeof(buffer)) == 0, "a missing item loads as 0");
    CHECK(msime::android::session_store_save(store, 2, nullptr, 0) == 0, "save of an empty value");
    CHECK(msime::android::session_store_load(store, 2, buffer, sizeof(buffer)) == 0, "an empty value loads as missing");
    std::memset(buffer, 0x5a, sizeof(buffer));
    CHECK(msime::android::session_store_load(store, 1, buffer, 2) == -1, "a value longer than capacity is a failure");
    CHECK(buffer[0] == 0x5a && buffer[1] == 0x5a, "an oversized value is not copied");
    fixture.set_int("saveResult", 1);
    CHECK(msime::android::session_store_save(store, 4, reinterpret_cast<const uint8_t *>("x"), 1) == 1, "save passes the Java result through");
    fixture.set_int("saveResult", 0);
    CHECK(msime::android::session_store_remove(store, 1) == 0, "remove");
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == 0, "a removed item loads as missing");
    CHECK(fixture.last_thread() == "main", "callbacks on the calling thread run on that thread");
    CHECK(!env->ExceptionCheck(), "no exception is left on the calling thread");
    CHECK(attached(), "the calling thread stays attached");

    // 本来就 attach 着的线程上，局部引用要到外层那次 JNI 调用返回才释放，一次调用里的很多次回调会把它们攒起来（ART 的局部引用表满了直接中止进程）。回调返回以后，load 交出的数组和 save 收到的数组都不能再被这条线程的局部引用拉住。
    msime::android::session_store_save(store, 1, reinterpret_cast<const uint8_t *>("abc"), 3);
    fixture.set_boolean("trackArrays", true);
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == 3, "load of a tracked array");
    CHECK(fixture.collected(), "load leaves no local reference to the returned array");
    CHECK(msime::android::session_store_save(store, 3, buffer, 3) == 0, "save of a tracked array");
    CHECK(fixture.collected(), "save leaves no local reference to the array it passed");
    fixture.set_boolean("trackArrays", false);

    fixture.set_failing(true);
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == -1, "a throwing load is a failure");
    CHECK(!env->ExceptionCheck(), "a throwing load leaves no exception");
    CHECK(msime::android::session_store_save(store, 1, buffer, 3) == -1, "a throwing save is a failure");
    CHECK(!env->ExceptionCheck(), "a throwing save leaves no exception");
    CHECK(msime::android::session_store_remove(store, 1) == -1, "a throwing remove is a failure");
    CHECK(!env->ExceptionCheck(), "a throwing remove leaves no exception");
    fixture.set_failing(false);

    // 进入时线程上已经挂着别人的异常：不碰 Java、按失败返回，也不替别人清掉。
    const int before = fixture.field("calls");
    env->ThrowNew(env->FindClass("java/lang/RuntimeException"), "pending");
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == -1, "a load with a pending exception is a failure");
    CHECK(env->ExceptionCheck(), "the pending exception belongs to the caller and stays");
    env->ExceptionClear();
    CHECK(fixture.field("calls") == before, "Java is not called while an exception is pending");
}

void store_on_detached_thread(Fixture &fixture, void *store) {
    intptr_t loaded = -2;
    int32_t saved = -2;
    int32_t removed = -2;
    bool detached_before = false;
    bool detached_after = true;
    uint8_t buffer[16];
    std::thread worker([&] {
        detached_before = !attached();
        saved = msime::android::session_store_save(store, 3, reinterpret_cast<const uint8_t *>("anon"), 4);
        detached_after = detached_after && !attached();
        loaded = msime::android::session_store_load(store, 3, buffer, sizeof(buffer));
        detached_after = detached_after && !attached();
        removed = msime::android::session_store_remove(store, 3);
        detached_after = detached_after && !attached();
    });
    worker.join();
    CHECK(detached_before, "a fresh native thread starts detached");
    CHECK(saved == 0 && loaded == 4 && std::memcmp(buffer, "anon", 4) == 0 && removed == 0, "callbacks work on a thread that was never attached");
    CHECK(detached_after, "every callback detaches the thread it attached");
    CHECK(fixture.last_thread() == "msime-session-store", "the attached thread carries its name");
}

void release_drops_the_global_reference(Fixture &fixture) {
    JNIEnv *env = fixture.env;
    jobject other = env->NewObject(fixture.type, env->GetMethodID(fixture.type, "<init>", "()V"));
    env->CallStaticVoidMethod(fixture.type, env->GetStaticMethodID(fixture.type, "track", "(Ljava/lang/Object;)V"), other);
    CHECK(!env->ExceptionCheck(), "track the second store");
    void *store = msime::android::session_store_create(env, other);
    CHECK(store != nullptr, "a second store context");
    env->DeleteLocalRef(other);
    CHECK(!fixture.collected(), "the context holds the Java store");
    bool detached_after = false;
    std::thread worker([&] {
        msime::android::session_store_release(store);
        detached_after = !attached();
    });
    worker.join();
    CHECK(detached_after, "release detaches the thread it attached");
    CHECK(fixture.collected(), "release drops the global reference");
}

void listeners(Fixture &fixture) {
    JNIEnv *env = fixture.env;
    msime::android::ListenerCall call;
    CHECK(msime::android::listener_begin(env, nullptr, "onDelta", "([B)V", call), "a null listener is accepted");
    msime::android::chat_delta(&call, reinterpret_cast<const uint8_t *>("x"), 1);
    CHECK(!msime::android::listener_finish(call) && !env->ExceptionCheck(), "a null listener is a no-op");

    jobject plain = env->NewObject(env->FindClass("java/lang/Object"), env->GetMethodID(env->FindClass("java/lang/Object"), "<init>", "()V"));
    CHECK(!msime::android::listener_begin(env, plain, "onDelta", "([B)V", call), "a listener without the method is refused");
    CHECK(env->ExceptionCheck(), "the missing method is reported to Java");
    env->ExceptionClear();
    CHECK(msime::android::session_store_create(env, plain) == nullptr, "a store without the three methods is refused");
    CHECK(env->ExceptionCheck(), "the missing store method is reported to Java");
    env->ExceptionClear();
    env->DeleteLocalRef(plain);

    void *store = msime::android::session_store_create(env, fixture.object);
    fixture.set_int("throwOnDelta", 3);
    CHECK(msime::android::listener_begin(env, fixture.object, "onDelta", "([B)V", call), "the delta listener");
    const char *hello = "你好";
    msime::android::chat_delta(&call, reinterpret_cast<const uint8_t *>(hello), std::strlen(hello));
    msime::android::chat_delta(&call, reinterpret_cast<const uint8_t *>("!"), 1);
    CHECK(fixture.delta_text() == "你好!", "deltas arrive in order as UTF-8");
    msime::android::chat_delta(&call, reinterpret_cast<const uint8_t *>("?"), 1);
    CHECK(call.failure != nullptr && !env->ExceptionCheck(), "a throwing listener's exception is kept off the thread");
    uint8_t buffer[16];
    CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) >= 0, "store callbacks still work after the listener threw");
    msime::android::chat_delta(&call, reinterpret_cast<const uint8_t *>("#"), 1);
    CHECK(fixture.field("deltaCalls") == 3, "a listener that threw is not called again");
    CHECK(msime::android::listener_finish(call), "the kept exception is rethrown at the end");
    jthrowable thrown = env->ExceptionOccurred();
    env->ExceptionClear();
    CHECK(thrown && env->IsInstanceOf(thrown, env->FindClass("java/lang/IllegalStateException")), "the listener's own exception is rethrown");

    CHECK(msime::android::listener_begin(env, fixture.object, "onProgress", "(I)V", call), "the progress listener");
    msime::android::ai_skin_progress(&call, 2);
    CHECK(fixture.field("progress") == 2, "progress reaches Java");
    CHECK(!msime::android::listener_finish(call) && !env->ExceptionCheck(), "a progress listener that did not throw leaves nothing behind");
    msime::android::session_store_release(store);
}

} // namespace

int main(int argc, char **argv) {
    if (argc != 2) {
        std::fprintf(stderr, "usage: jni-callbacks-test <classpath>\n");
        return 2;
    }
    std::string classpath = std::string("-Djava.class.path=") + argv[1];
    JavaVMOption options[2];
    options[0].optionString = const_cast<char *>(classpath.c_str());
    options[1].optionString = const_cast<char *>("-Xcheck:jni");
    JavaVMInitArgs arguments{};
    arguments.version = JNI_VERSION_1_6;
    arguments.nOptions = 2;
    arguments.options = options;
    arguments.ignoreUnrecognized = JNI_FALSE;
    JNIEnv *env = nullptr;
    if (JNI_CreateJavaVM(&vm, reinterpret_cast<void **>(&env), &arguments) != JNI_OK) {
        std::fprintf(stderr, "JniCallbacksTest: the JVM did not start\n");
        return 1;
    }
    Fixture fixture{env, env->FindClass("app/msime/android/nativetest/JniCallbacksFixture"), nullptr};
    if (!fixture.type) {
        std::fprintf(stderr, "JniCallbacksTest: JniCallbacksFixture is not on the classpath\n");
        return 1;
    }
    fixture.object = env->NewObject(fixture.type, env->GetMethodID(fixture.type, "<init>", "()V"));
    void *store = msime::android::session_store_create(env, fixture.object);
    CHECK(store != nullptr && !env->ExceptionCheck(), "a store context for the fixture");
    if (store) {
        // 记下 JavaVM 之前回调取不到 env，按失败返回，不碰 Java。
        uint8_t buffer[4];
        CHECK(msime::android::session_store_load(store, 1, buffer, sizeof(buffer)) == -1, "a callback before JNI_OnLoad is a failure");
        CHECK(fixture.field("calls") == 0, "Java is not called before JNI_OnLoad");
    }
    msime::android::remember_java_vm(vm);
    if (store) {
        store_on_attached_thread(fixture, store);
        store_on_detached_thread(fixture, store);
        msime::android::session_store_release(store);
    }
    release_drops_the_global_reference(fixture);
    listeners(fixture);
    CHECK(attached() && !env->ExceptionCheck(), "the main thread ends attached and clean");
    if (failures) {
        std::fprintf(stderr, "JniCallbacksTest: %d checks failed\n", failures);
        return 1;
    }
    std::printf("JniCallbacks.cpp: attach/detach, exception clearing, release and listener checks passed on a desktop JVM\n");
    return 0;
}
