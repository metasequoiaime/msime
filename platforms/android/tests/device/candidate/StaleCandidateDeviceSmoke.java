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
    private boolean nextWord;
    @Override public void onCreate(Bundle arguments) {
        nextWord = arguments != null && "true".equals(arguments.getString("next-word"));
        super.onCreate(arguments);
        start();
    }

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
                if (nextWord) preparePrediction(options);
                JSONObject initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                session = initial.getLong("session");
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                if (nextWord) {
                    checkPrediction(type, nativeType, service, session);
                    return;
                }
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
            ? nextWord ? "MSIME_DEVICE_SMOKE_PASSED: post-commit prediction selection, editor echo, external cursor and privacy\n"
                : "MSIME_DEVICE_SMOKE_PASSED: stale menu, expanded button and spelling preserve session without editor writes; fresh candidate commits\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")\n");
        finish(failure[0] == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private void preparePrediction(JSONObject options) throws Exception {
        // 每次使用独立词序，重复运行不能让前一次的合成学习改变候选排序。
        File directory = Files.createTempDirectory(
            getTargetContext().getCacheDir().toPath(), "next-word-device-").toFile();
        File dictionary = new File(directory, "msime-pinyin.db");
        try (android.database.sqlite.SQLiteDatabase database = android.database.sqlite.SQLiteDatabase.openOrCreateDatabase(dictionary, null)) {
            database.execSQL("DROP TABLE IF EXISTS tbl_1_c");
            database.execSQL("DROP TABLE IF EXISTS tbl_2_c");
            database.execSQL("CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER)");
            database.execSQL("INSERT INTO tbl_1_c VALUES('chi','c','吃',1000)");
            database.execSQL("CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER)");
            database.execSQL("INSERT INTO tbl_2_c VALUES('chi''fan','cf','吃饭',100)");
            database.execSQL("INSERT INTO tbl_2_c VALUES('chi''hao','ch','吃好',90)");
        }
        options.put("dictionaries", directory.getAbsolutePath()).put("user_data", directory.getAbsolutePath());
        options.remove("preferences_directory");
        options.getJSONObject("preferences").put("next_word_prediction", true).put("learning", true)
            .put("traditional_chinese_output", false);
    }

    private JSONObject prediction(Class<?> nativeType, long session) throws Exception {
        Method character = nativeType.getMethod("character", long.class, int.class, boolean.class);
        JSONObject current = null;
        for (char letter : "chi".toCharArray())
            current = value(character.invoke(null, session, (int) letter, false)).getJSONObject("view");
        JSONObject id = current.getJSONArray("candidates").getJSONObject(0).getJSONObject("id");
        JSONObject selected = value(nativeType.getMethod("select", long.class, long.class, long.class)
            .invoke(null, session, id.getLong("generation"), id.getLong("index")));
        check("吃".equals(selected.getString("commit")), "合成首字未上屏");
        JSONObject suggestions = selected.getJSONObject("view");
        check(suggestions.optBoolean("prediction", false), "缺少空闲联想");
        check(suggestions.getString("editing_text").isEmpty(), "联想进入组字");
        check(suggestions.getJSONArray("candidates").length() > 0, "未产生后续词");
        return suggestions;
    }

    private void checkPrediction(Class<?> type, Class<?> nativeType, Object service, long session) throws Exception {
        stage = "synthetic next word appears";
        JSONObject suggestions = prediction(nativeType, session);
        field(type, "session").setLong(service, session);
        field(type, "view").set(service, suggestions);
        field(type, "touchLayoutHint").setInt(service, 0);
        field(type, "traditionalChineseOutput").setBoolean(service, false);
        Method confirms = type.getDeclaredMethod("returnKeyConfirms"); confirms.setAccessible(true);
        Method management = type.getDeclaredMethod("candidateManagementEnabled"); management.setAccessible(true);
        check(!(Boolean) confirms.invoke(service), "联想改变回车动作");
        check(!(Boolean) management.invoke(service), "联想开放普通词库管理");
        stage = "own selection echo retains prediction";
        Object echo = field(type, "selectionEcho").get(service);
        echo.getClass().getMethod("reset", int.class, int.class).invoke(echo, 0, 0);
        echo.getClass().getMethod("commit", int.class).invoke(echo, 1);
        echo.getClass().getMethod("expect").invoke(echo);
        Method update = type.getDeclaredMethod("updateEditorSelection", int.class, int.class,
            int.class, int.class, int.class, int.class); update.setAccessible(true);
        update.invoke(service, 0, 0, 1, 1, -1, -1);
        check(((JSONObject) field(type, "view").get(service)).optBoolean("prediction", false), "自己的上屏回声清掉联想");
        stage = "external cursor clears prediction";
        update.invoke(service, 1, 1, 0, 0, -1, -1);
        check(!((JSONObject) field(type, "view").get(service)).optBoolean("prediction", false), "外部光标移动保留联想");
        stage = "explicit next word writes editor";
        suggestions = prediction(nativeType, session);
        field(type, "view").set(service, suggestions);
        int[] writes = {0};
        InputConnection connection = (InputConnection) Proxy.newProxyInstance(
            InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
            (proxy, call, arguments) -> {
                if (call.getName().equals("commitText")) {
                    stage = "explicit next word commits suffix only";
                    check("饭".contentEquals((CharSequence) arguments[0]), "联想上屏包含前缀"); writes[0]++;
                }
                if (call.getReturnType() == boolean.class) return true;
                if (call.getReturnType() == int.class) return 0;
                return null;
            });
        field(type, "connection").set(service, connection);
        Object candidates = field(type, "imeCandidates").get(service);
        Method button = candidates.getClass().getDeclaredMethod("expandedCandidateButton", JSONObject.class);
        button.setAccessible(true);
        ((Button) button.invoke(candidates, suggestions.getJSONArray("candidates").getJSONObject(0))).performClick();
        stage = "explicit next word calls editor once";
        check(writes[0] == 1, "点选联想没有上屏");
        stage = "private mode clears native and Java prediction";
        field(type, "view").set(service, prediction(nativeType, session));
        field(type, "incognitoEnabled").setBoolean(service, true);
        Method mark = type.getDeclaredMethod("markPrivateSession"); mark.setAccessible(true); mark.invoke(service);
        check(!((JSONObject) field(type, "view").get(service)).optBoolean("prediction", false), "隐私模式留下旧 Java 候选");
        JSONObject current = value(nativeType.getMethod("view", long.class).invoke(null, session));
        check(!current.optBoolean("prediction", false), "隐私模式留下原生候选");
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
