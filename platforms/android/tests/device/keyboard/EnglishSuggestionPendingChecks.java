package app.msime.android.test;

import android.content.Context;
import android.text.InputType;
import android.view.View;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;

/** 阻住真实服务的英文查询线程，确定性验收等待结果时的原文候选与工具栏。 */
final class EnglishSuggestionPendingChecks {
    private EnglishSuggestionPendingChecks() {}

    static void run(DeviceSmoke test) throws Exception {
        Throwable[] failure = new Throwable[1];
        test.runOnMainSync(() -> {
            Object service = null;
            CountDownLatch release = new CountDownLatch(1);
            try {
                ClassLoader loader = test.getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                Class<?> nativeType = loader.loadClass("app.msime.android.NativeClient");
                service = type.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                attach.setAccessible(true);
                attach.invoke(service, test.getTargetContext());
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
                    test.getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
                options.getJSONObject("preferences").put("scheme", "quanpin")
                    .put("learning", false).put("cloud_candidates", false);
                long session = value(nativeType.getMethod("create", String.class)
                    .invoke(null, options.toString())).getLong("session");
                field(type, "session").setLong(service, session);
                nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true);
                field(type, "view").set(service, value(nativeType.getMethod("setEnglishMode", long.class,
                    boolean.class).invoke(null, session, true)));
                field(type, "dedicatedEnglish").setBoolean(service, true);
                field(type, "englishSuggestionsEnabled").setBoolean(service, true);
                field(type, "editorInputType").setInt(service, InputType.TYPE_CLASS_TEXT);
                field(type, "candidateGlossResources").set(service, options.getString("resources"));
                StringBuilder typed = new StringBuilder();
                InputConnection connection = (InputConnection) Proxy.newProxyInstance(
                    InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                    (proxy, call, args) -> {
                        if (call.getName().equals("getTextBeforeCursor")) return typed.toString();
                        if (call.getReturnType() == boolean.class) return true;
                        if (call.getReturnType() == int.class) return 0;
                        return null;
                    });
                field(type, "connection").set(service, connection);
                ExecutorService worker = (ExecutorService) field(type, "englishSuggestionWorker").get(service);
                CountDownLatch started = new CountDownLatch(1);
                worker.execute(() -> {
                    started.countDown();
                    try { release.await(); }
                    catch (InterruptedException stopped) { Thread.currentThread().interrupt(); }
                });
                if (!started.await(5, TimeUnit.SECONDS)) throw new AssertionError("Synthetic query barrier unavailable");
                Method refresh = method(type, "refreshEnglishSuggestions");
                Method render = method(type, "render");
                long firstEpoch = -1;
                // 只使用合成前缀；覆盖第一次输入、继续输入和删除，查询结果在这些断言后才可返回。
                for (String prefix : new String[] {"m", "my", "m"}) {
                    test.stage = "English query pending keeps the current original row";
                    typed.setLength(0);
                    typed.append(prefix);
                    refresh.invoke(service);
                    if (firstEpoch < 0) firstEpoch = field(type, "englishSuggestionEpoch").getLong(service);
                    render.invoke(service);
                    View toolbar = (View) field(type, "shortcutToolbar").get(service);
                    if (toolbar.getVisibility() == View.VISIBLE)
                        throw new AssertionError("Pending English query flashed the shortcut toolbar");
                    List<?> buttons = (List<?>) field(type, "englishSuggestionButtons").get(service);
                    if (buttons.isEmpty() || !prefix.contentEquals(((Button) buttons.get(0)).getText()))
                        throw new AssertionError("Pending English query did not show the current original word");
                }
                test.stage = "old English answer is rejected after typing and deleting back to the same word";
                Class<?> model = loader.loadClass("app.msime.android.candidate.EnglishSuggestionModel");
                Object stale = model.getMethod("decode", String.class).invoke(null,
                    "{\"ok\":true,\"value\":{\"prefix\":\"m\",\"items\":[\"m\",\"synthetic\"]}}");
                Method apply = type.getDeclaredMethod("applyEnglishSuggestions", long.class, String.class, stale.getClass());
                apply.setAccessible(true);
                apply.invoke(service, firstEpoch, "m", stale);
                if (((List<?>) field(type, "englishSuggestions").get(service)).size() != 1)
                    throw new AssertionError("Old English answer replaced the pending original row");
                test.stage = "empty English word returns to toolbar";
                typed.setLength(0);
                refresh.invoke(service);
                render.invoke(service);
                if (((View) field(type, "shortcutToolbar").get(service)).getVisibility() != View.VISIBLE)
                    throw new AssertionError("Empty English word did not restore the shortcut toolbar");
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                // 先销毁隔离服务，使排队的查询失效；解除屏障后不能把结果应用到已结束的测试。
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 清理测试工作线程。 */ }
                    }
                }
                release.countDown();
            }
        });
        if (failure[0] instanceof AssertionError assertion) throw assertion;
        if (failure[0] != null) throw new AssertionError(test.stage, failure[0]);
    }

    private static JSONObject value(Object response) throws Exception {
        JSONObject result = new JSONObject((String) response);
        if (!result.optBoolean("ok")) throw new AssertionError("Synthetic English session unavailable");
        return result.getJSONObject("value");
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field result = type.getDeclaredField(name);
        result.setAccessible(true);
        return result;
    }

    private static Method method(Class<?> type, String name) throws Exception {
        Method result = type.getDeclaredMethod(name);
        result.setAccessible(true);
        return result;
    }
}
