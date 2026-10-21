package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import org.json.JSONObject;

/** 从安装后的原生服务验证中英键帽与临时大写返回中文，不读取或修改实际编辑器内容。 */
public final class ChineseShiftReturnDeviceSmoke extends Instrumentation {
    private String stage = "prepare synthetic shoudao session";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                Class<?> nativeType = loader.loadClass("app.msime.android.NativeClient");
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
                JSONObject initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                long session = initial.getLong("session");
                field(type, "session").setLong(service, session);
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                field(type, "view").set(service, value(nativeType.getMethod("setEnglishMode", long.class,
                    boolean.class).invoke(null, session, false)));
                field(type, "touchLayoutHint").setInt(service, 0);
                field(type, "selectedScheme").set(service,
                    loader.loadClass("app.msime.android.KeyboardScheme").getField("SHOUDAO").get(null));
                field(type, "localSettings").set(service,
                    loader.loadClass("app.msime.android.AndroidLocalSettings").getMethod("defaults").invoke(null));
                StringBuilder committed = new StringBuilder();
                InputConnection connection = (InputConnection) Proxy.newProxyInstance(
                    InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                    (proxy, call, args) -> {
                        if (call.getName().equals("commitText")) committed.append((CharSequence) args[0]);
                        if (call.getReturnType() == boolean.class) return true;
                        if (call.getReturnType() == int.class) return 0;
                        return null;
                    });
                field(type, "connection").set(service, connection);
                int[] modeWrites = new int[1];
                Class<?> persistence = loader.loadClass("app.msime.android.InputModeStore$Persistence");
                Object storage = Proxy.newProxyInstance(loader, new Class<?>[] {persistence}, (proxy, call, args) -> {
                    if (call.getName().equals("putBoolean")) { modeWrites[0]++; return null; }
                    return false;
                });
                Object store = loader.loadClass("app.msime.android.InputModeStore")
                    .getConstructor(persistence).newInstance(storage);
                field(type, "inputModeStore").set(service, store);
                field(type, "imeModeScope").set(service, "global");
                Button shift = (Button) field(type, "shiftButton").get(service);
                Method typeLetter = method(type, "type", char.class);
                Method toggleLanguage = method(type, "toggleInputLanguage");
                stage = "Chinese letter faces before language switch";
                Object rows = field(type, "imeLetterRows").get(service);
                method(rows.getClass(), "rebuildKeyRows").invoke(rows);
                method(type, "render").invoke(service);
                List<?> originalKeys = new ArrayList<>((List<?>) field(type, "shuangpinKeyButtons").get(service));
                checkLetterFaces(type, service, true, originalKeys);
                for (int cycle = 0; cycle < 3; cycle++) {
                    stage = "English lowercase faces after language switch";
                    toggleLanguage.invoke(service);
                    checkLetterFaces(type, service, false, originalKeys);
                    Button letter = (Button) originalKeys.get(0);
                    letter.performClick();
                    check(committed.toString().equals("q".repeat(cycle + 1)), "英文键面与实际小写上屏一致");
                    stage = "Chinese uppercase faces after language switch";
                    toggleLanguage.invoke(service);
                    checkLetterFaces(type, service, true, originalKeys);
                }
                committed.setLength(0);
                modeWrites[0] = 0;
                stage = "one-shot uppercase returns to Chinese";
                shift.performClick();
                stage = "one-shot enters temporary English";
                check(english(type, service), "Shift enters English temporarily");
                typeLetter.invoke(service, 'a');
                stage = "one-shot commits uppercase exactly once";
                check(committed.toString().equals("A"), "uppercase letter committed once");
                stage = "one-shot restores host Chinese mode";
                check(!english(type, service), "one-shot returns to Chinese");
                stage = "one-shot restores native Chinese mode";
                check(!((JSONObject) field(type, "view").get(service)).getBoolean("dedicated_english"),
                    "native session returns to Chinese");
                stage = "one-shot does not persist temporary mode";
                check(modeWrites[0] == 0, "temporary shift does not persist an English mode");
                stage = "Chinese composition resumes after uppercase";
                typeLetter.invoke(service, 'i');
                typeLetter.invoke(service, 'i');
                JSONObject chinese = (JSONObject) field(type, "view").get(service);
                check(!chinese.getString("editing_text").isEmpty()
                    && chinese.getJSONArray("candidates").length() > 0, "Chinese candidates resume");
                method(type, "command", int.class).invoke(service, 3);
                stage = "slow second tap cancels temporary uppercase";
                shift.performClick();
                SystemClock.sleep(400);
                shift.performClick();
                check(!english(type, service), "turning Shift off returns to Chinese");
                stage = "double tap locks and unlocking returns to Chinese";
                shift.performClick();
                shift.performClick();
                check(english(type, service), "double tap keeps temporary English");
                typeLetter.invoke(service, 'b');
                typeLetter.invoke(service, 'c');
                check(committed.toString().equals("ABC"), "locked letters stay uppercase");
                check(english(type, service), "caps lock survives letters");
                shift.performClick();
                check(!english(type, service), "unlocking returns to Chinese");
                check(modeWrites[0] == 0, "neither lock nor unlock persists a mode");
                stage = "explicit language selection keeps English after one letter";
                toggleLanguage.invoke(service);
                shift.performClick();
                typeLetter.invoke(service, 'd');
                check(committed.toString().equals("ABCD"), "permanent English still types uppercase");
                check(english(type, service), "permanent English survives one-shot Shift");
                check(modeWrites[0] == 1, "explicit mode change is remembered");
                toggleLanguage.invoke(service);
                stage = "explicit language key cancels temporary Shift";
                shift.performClick();
                toggleLanguage.invoke(service);
                check(!english(type, service), "language key returns to Chinese");
                check(modeWrites[0] == 3, "explicit language changes retain normal persistence");
                stage = "hardware letter consumes temporary uppercase";
                shift.performClick();
                type.getMethod("onKeyDown", int.class, android.view.KeyEvent.class).invoke(service,
                    android.view.KeyEvent.KEYCODE_E,
                    new android.view.KeyEvent(android.view.KeyEvent.ACTION_DOWN, android.view.KeyEvent.KEYCODE_E));
                check(committed.toString().equals("ABCDE"), "hardware uppercase committed once");
                check(!english(type, service), "hardware letter returns to Chinese");
                stage = "session stop discards temporary English";
                shift.performClick();
                method(type, "stop", boolean.class).invoke(service, false);
                check(!english(type, service), "temporary English does not survive session stop");
                check(modeWrites[0] == 3, "session stop does not persist temporary mode");
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
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
            ? "MSIME_DEVICE_SMOKE_PASSED: temporary Chinese Shift, caps lock, explicit language and session stop\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")"
                + (failure[0] instanceof AssertionError ? " " + failure[0].getMessage() : "") + "\n");
        finish(failure[0] == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static boolean english(Class<?> type, Object service) throws Exception {
        return field(type, "dedicatedEnglish").getBoolean(service);
    }
    private static void checkLetterFaces(Class<?> type, Object service, boolean uppercase,
            List<?> originalKeys) throws Exception {
        List<?> keys = (List<?>) field(type, "shuangpinKeyButtons").get(service);
        List<?> inputs = (List<?>) field(type, "shuangpinKeyInputs").get(service);
        check(keys.equals(originalKeys), "中英 26 键切换应保留原来的按键对象");
        int letterCount = 0;
        for (int index = 0; index < keys.size(); index++) {
            Button key = (Button) keys.get(index);
            String input = (String) inputs.get(index);
            // 首道双拼还有分词键，只检查 a-z 的大小写。
            if (input.length() != 1 || input.charAt(0) < 'a' || input.charAt(0) > 'z') continue;
            letterCount++;
            String expected = uppercase ? input.toUpperCase(Locale.ROOT) : input.toLowerCase(Locale.ROOT);
            check(expected.contentEquals(key.getText()), "键帽大小写应与当前语言状态一致");
            check(key.getContentDescription().toString().startsWith("字母 " + input.toUpperCase(Locale.ROOT)),
                "普通中英字母键的无障碍描述不应残留大写状态");
        }
        check(letterCount == 26, "完整的 26 个字母键夹具");
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
