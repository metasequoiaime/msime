package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

/** 不加载模型、不访问麦克风，验证真实 JNI 的语音句柄创建、拒绝和释放。 */
public final class LocalSpeechHandleDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_LOCAL_SPEECH_HANDLES_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_LOCAL_SPEECH_HANDLES_FAILED: " + error.getClass().getSimpleName()
                + ": " + error.getMessage() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void runCases() throws ReflectiveOperationException, java.io.IOException {
        Class<?> client = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        Method create = client.getMethod("localSpeechCreate");
        Method destroy = client.getMethod("localSpeechDestroy", long.class);
        Method cancel = client.getMethod("localSpeechCancel", long.class);
        Method start = client.getMethod("localSpeechStart", long.class, String.class,
            String.class, String.class, int.class);
        Method accept = client.getMethod("localSpeechAccept", long.class, short[].class, int.class);
        Method finish = client.getMethod("localSpeechFinish", long.class);
        Method speaking = client.getMethod("localSpeechSpeaking", long.class);
        long first = 0;
        long second = 0;
        try {
            first = (Long) create.invoke(null);
            check(first > 0, "Native pointer sign escaped into the speech handle");
            second = (Long) create.invoke(null);
            long lastIssued = second;
            check(second > first, "Speech handles must be unique and monotonic");
            cancel.invoke(null, first);
            destroy.invoke(null, first);
            long stale = first;
            first = 0;
            check("invalid local speech session".equals(start.invoke(null, stale, "", "zh-CN", "", 0)),
                "Released speech handle was accepted");
            check("invalid local speech session".equals(start.invoke(null, Long.MAX_VALUE, "", "zh-CN", "", 0)),
                "Unknown speech handle was accepted");
            refused(accept, IllegalStateException.class, stale, new short[0], 0);
            refused(finish, IllegalStateException.class, stale);
            check(!(Boolean) speaking.invoke(null, stale), "A released speech handle reported speech");
            check(!(Boolean) speaking.invoke(null, second), "A speech handle without a session reported speech");
            refused(speaking, IllegalArgumentException.class, -1L);
            refused(start, IllegalArgumentException.class, -1L, "", "zh-CN", "", 0);
            cancel.invoke(null, stale);
            destroy.invoke(null, stale);
            cancel.invoke(null, 0L);
            destroy.invoke(null, 0L);
            // 另一个句柄仍有效，缺模型时应由原生模型检查返回错误。
            String missing = getTargetContext().getCacheDir().toPath()
                .resolve("synthetic-missing-speech-model").toString();
            Object error = start.invoke(null, second, missing, "zh-CN", "", 0);
            check(error instanceof String && !((String) error).isEmpty()
                && !"invalid local speech session".equals(error), "Live speech handle was refused");
            // 应用私有目录经过系统别名时，原生层必须接受与共享安装器相同的路径。
            var model = java.nio.file.Files.createTempDirectory(getTargetContext().getCacheDir().toPath(),
                "synthetic-speech-path-");
            var manifest = model.resolve("msime-model.json");
            var linked = model.resolveSibling(model.getFileName() + "-link");
            try {
                java.nio.file.Files.write(manifest, "{}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
                Object modelError = start.invoke(null, second, model.toString(), "zh-CN", "", 0);
                check(modelError instanceof String && !((String) modelError).isEmpty()
                    && !"Not an installed local speech model".equals(modelError),
                    "Native model validation rejected the application's private directory");
                java.nio.file.Files.createSymbolicLink(linked, model);
                check("Not an installed local speech model".equals(
                    start.invoke(null, second, linked.toString(), "zh-CN", "", 0)),
                    "Native model validation accepted an application-created alias");
            } finally {
                java.nio.file.Files.deleteIfExists(linked);
                java.nio.file.Files.deleteIfExists(manifest);
                java.nio.file.Files.deleteIfExists(model);
            }
            destroy.invoke(null, second);
            second = 0;
            long replacement = (Long) create.invoke(null);
            try { check(replacement > lastIssued, "Released speech identity was reused"); }
            finally { destroy.invoke(null, replacement); }
        } finally {
            // 负指针只会出现在旧实现的失败复现中，随后由测试脚本强停进程。
            if (first > 0) destroy.invoke(null, first);
            if (second > 0) destroy.invoke(null, second);
        }
    }

    private static void refused(Method method, Class<? extends Throwable> type, Object... args)
            throws ReflectiveOperationException {
        try { method.invoke(null, args); }
        catch (InvocationTargetException error) {
            if (type.isInstance(error.getCause())) return;
            throw error;
        }
        throw new AssertionError("Invalid speech handle was accepted");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
