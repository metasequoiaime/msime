package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.ExecutorService;
import org.json.JSONObject;

/** 从安装产品验证完整候选展开后，释义任务和选词使用当前运行时代次。 */
public final class CandidateExpansionDeviceSmoke extends Instrumentation {
    private String stage = "prepare synthetic shoudao session";
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
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
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
                    getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
                options.getJSONObject("preferences").put("scheme", "shuangpin")
                    .put("shuangpin_profile", "shoudao").put("learning", false).put("cloud_candidates", false);
                options.put("phrase_preedit", true);
                JSONObject initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                session = initial.getLong("session");
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                field(type, "session").setLong(service, session);
                field(type, "touchLayoutHint").setInt(service, 0);
                field(type, "renderingSuppressed").setBoolean(service, true);
                field(type, "candidateEnglishGloss").setBoolean(service, true);
                field(type, "candidateGlossResources").set(service, options.getString("resources"));
                int[] commits = new int[1];
                InputConnection connection = (InputConnection) Proxy.newProxyInstance(
                    InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                    (proxy, call, arguments) -> {
                        if (call.getName().equals("commitText")) commits[0]++;
                        if (call.getReturnType() == boolean.class) return true;
                        if (call.getReturnType() == int.class) return 0;
                        return null;
                    });
                field(type, "connection").set(service, connection);
                Method character = nativeType.getMethod("character", long.class, int.class, boolean.class);
                Method command = nativeType.getMethod("command", long.class, int.class);
                Method apply = method(type, "apply", String.class);
                Method glosses = method(type, "scheduleCandidateGlosses");
                Method pick = method(type, "selectVisibleCandidate", Button.class, int.class);
                Method open = method(type, "openCandidatePanel");
                Method nativeView = nativeType.getMethod("view", long.class);
                // 固定种子的合成字母在首道双拼里包含暂扣候选，禁止替换为用户输入。
                String letters = "rmnjxverelbzfolycgieqwqccqhkyydpdbdvsksoxkbfmxpphjfdfceompilhys";
                int expansions = 0;
                for (int mode = 0; mode < 3; mode++) {
                    if (mode > 0) {
                        nativeType.getMethod("destroy", long.class).invoke(null, session);
                        initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                        session = initial.getLong("session");
                        field(type, "session").setLong(service, session);
                        value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                    }
                    apply.invoke(service, command.invoke(null, session, 3));
                    for (char letter : letters.toCharArray()) {
                        apply.invoke(service, character.invoke(null, session, (int) letter, false));
                        if (mode == 2) glosses.invoke(service);
                    }
                    JSONObject before = (JSONObject) field(type, "view").get(service);
                    if (mode == 0) {
                        stage = "offline gloss expansion synchronizes view";
                        glosses.invoke(service);
                    } else if (mode == 1) {
                        stage = "manual panel expansion synchronizes view";
                        open.invoke(service);
                        check(field(type, "candidatePanelOpen").getBoolean(service), "panel opened");
                    }
                    JSONObject current = (JSONObject) field(type, "view").get(service);
                    JSONObject actual = value(nativeView.invoke(null, session));
                    check(current.getLong("generation") == actual.getLong("generation"), "host generation matches runtime");
                    check(current.getJSONArray("candidates").toString().equals(actual.getJSONArray("candidates").toString()),
                        "candidate identities and order match runtime");
                    if (current.getLong("generation") > before.getLong("generation")) expansions++;
                    for (int choice = 0; choice < letters.length() + 1; choice++) {
                        current = (JSONObject) field(type, "view").get(service);
                        if (current.getJSONArray("candidates").length() == 0) break;
                        stage = "repeated candidate selection " + mode + ":" + choice;
                        pick.invoke(service, new Button(getTargetContext()), 0);
                        check(field(type, "session").getLong(service) == session, "selection preserves session");
                        glosses.invoke(service);
                    }
                    check(((JSONObject) field(type, "view").get(service)).getString("editing_text").isEmpty(),
                        "repeated selection finishes composition");
                }
                stage = "both full-list entry points release held candidates";
                check(expansions >= 2, "both full-list entry points expanded held candidates");
                stage = "all synthetic compositions committed";
                check(commits[0] >= 3, "all synthetic compositions committed");
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                if (nativeType != null && session != 0) {
                    try { nativeType.getMethod("destroy", long.class).invoke(null, session); }
                    catch (ReflectiveOperationException ignored) { /* 旧实现可能已停止合成会话。 */ }
                }
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务，不显示系统窗口。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 清理测试工作线程。 */ }
                    }
                }
            }
        });
        Bundle result = new Bundle();
        result.putString("stream", failure[0] == null
            ? "MSIME_DEVICE_SMOKE_PASSED: shoudao gloss and panel expansion, rapid typing and repeated candidate selection\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")\n");
        finish(failure[0] == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters); method.setAccessible(true); return method;
    }
    private static JSONObject value(Object response) throws Exception {
        JSONObject result = new JSONObject((String) response);
        check(result.getBoolean("ok"), "native operation accepted");
        return result.getJSONObject("value");
    }
    private static void check(boolean condition, String description) {
        if (!condition) throw new AssertionError(description);
    }
}
