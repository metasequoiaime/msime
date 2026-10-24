package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.ExecutorService;
import org.json.JSONObject;

/** 在安装产品中创建合成原生会话，检查旧菜单操作不会把可继续输入的会话停止。 */
public final class StaleCandidateDeviceSmoke extends Instrumentation {
    private String stage = "prepare synthetic session";
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            long session = 0;
            Class<?> nativeType = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                nativeType = loader.loadClass("app.msime.android.NativeClient");
                service = type.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod(
                    "attachBaseContext", android.content.Context.class);
                attach.setAccessible(true);
                attach.invoke(service, getTargetContext());
                stage = "initialize installed service";
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
                    getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
                options.getJSONObject("preferences").put("scheme", "quanpin")
                    .put("learning", false).put("cloud_candidates", false);
                JSONObject initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                session = initial.getLong("session");
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                Method character = nativeType.getMethod("character", long.class, int.class, boolean.class);
                JSONObject old = value(character.invoke(null, session, (int) 'n', false)).getJSONObject("view");
                old = value(character.invoke(null, session, (int) 'i', false)).getJSONObject("view");
                JSONObject id = old.getJSONArray("candidates").getJSONObject(0).getJSONObject("id");
                field(type, "view").set(service, old);
                Object candidates = field(type, "imeCandidates").get(service);
                Method expanded = candidates.getClass().getDeclaredMethod("expandedCandidateButton", JSONObject.class);
                expanded.setAccessible(true);
                Button staleButton = (Button) expanded.invoke(candidates,
                    old.getJSONArray("candidates").getJSONObject(0));
                Method strip = candidates.getClass().getDeclaredMethod("stripCandidateButton",
                    JSONObject.class, JSONObject.class, int.class);
                strip.setAccessible(true);
                Button staleStrip = (Button) strip.invoke(candidates, old,
                    old.getJSONArray("candidates").getJSONObject(0), 0);
                JSONObject current = value(character.invoke(null, session, (int) 'h', false)).getJSONObject("view");
                check(current.getLong("generation") != id.getLong("generation"), "generation advanced");
                field(type, "session").setLong(service, session);
                field(type, "view").set(service, current);
                field(type, "touchLayoutHint").setInt(service, 0);
                int[] writes = new int[2];
                InputConnection connection = (InputConnection) Proxy.newProxyInstance(
                    InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                    (proxy, call, arguments) -> {
                        if (call.getName().equals("commitText")) writes[0]++;
                        if (call.getName().equals("setComposingText")) writes[1]++;
                        if (call.getReturnType() == boolean.class) return true;
                        if (call.getReturnType() == int.class) return 0;
                        return null;
                    });
                field(type, "connection").set(service, connection);
                Class<?> action = loader.loadClass("app.msime.android.CandidateManagementAction");
                Method edit = type.getDeclaredMethod("editCandidate", JSONObject.class, action);
                edit.setAccessible(true);
                stage = "stale menu preserves session";
                for (Object item : action.getEnumConstants()) {
                    Throwable callbackError = null;
                    try { edit.invoke(service, id, item); }
                    catch (InvocationTargetException error) { callbackError = error.getCause(); }
                    stage = "stale menu session retained";
                    check(field(type, "session").getLong(service) == session, "stale menu stopped session");
                    stage = "stale menu callback ignored";
                    check(callbackError == null, "stale menu entered failure path");
                    check(field(type, "view").get(service) == current, "stale menu changed view");
                }
                stage = "stale expanded button preserves session";
                staleButton.performClick();
                check(field(type, "session").getLong(service) == session, "stale expanded button stopped session");
                check(field(type, "view").get(service) == current, "stale expanded button changed view");
                stage = "stale continuous strip preserves session";
                staleStrip.performClick();
                Method stripGloss = type.getDeclaredMethod("insertStripCandidateGloss", JSONObject.class,
                    JSONObject.class, JSONObject.class, String.class, String.class);
                stripGloss.setAccessible(true);
                stripGloss.invoke(service, old, old.getJSONArray("candidates").getJSONObject(0),
                    id, old.getJSONArray("candidates").getJSONObject(0).getString("text"), "synthetic gloss");
                check(field(type, "session").getLong(service) == session, "stale strip stopped session");
                check(field(type, "view").get(service) == current, "stale strip changed view");
                stage = "stale spelling preserves session";
                Object rows = field(type, "imeLayoutRows").get(service);
                Method spelling = rows.getClass().getDeclaredMethod("chooseNineKeySpelling", long.class, long.class, int.class);
                spelling.setAccessible(true);
                spelling.invoke(rows, session, id.getLong("generation"), 0);
                spelling.invoke(rows, session + 1, current.getLong("generation"), 0);
                check(field(type, "session").getLong(service) == session, "stale spelling stopped session");
                check(field(type, "view").get(service) == current, "stale spelling changed view");
                check(writes[0] == 0 && writes[1] == 0, "stale callbacks wrote editor");
                stage = "fresh input remains usable";
                JSONObject next = value(character.invoke(null, session, (int) 'a', false)).getJSONObject("view");
                check(next.getLong("generation") != current.getLong("generation"), "fresh input advanced");
                field(type, "view").set(service, next);
                stage = "fresh expanded candidate writes editor";
                Button freshButton = (Button) expanded.invoke(candidates,
                    next.getJSONArray("candidates").getJSONObject(0));
                freshButton.performClick();
                check(writes[0] == 1, "fresh candidate committed");
                check(field(type, "session").getLong(service) == session, "fresh candidate stopped session");
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                if (nativeType != null && session != 0) {
                    try { nativeType.getMethod("destroy", long.class).invoke(null, session); }
                    catch (ReflectiveOperationException ignored) { /* 旧实现可能已停止合成会话。 */ }
                }
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理测试服务，不显示窗口。 */ }
                }
                if (service != null) for (Field member : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                    try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离对象的工作线程。 */ }
                }
            }
        });
        result.putString("stream", failure[0] == null
            ? "MSIME_DEVICE_SMOKE_PASSED: stale menu, expanded button and spelling preserve session without editor writes; fresh candidate commits\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")\n");
        finish(failure[0] == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static JSONObject value(Object response) throws Exception {
        JSONObject result = new JSONObject((String) response);
        check(result.getBoolean("ok"), "native operation rejected");
        return result.getJSONObject("value");
    }
    private static void check(boolean condition, String description) {
        if (!condition) throw new AssertionError(description);
    }
}
