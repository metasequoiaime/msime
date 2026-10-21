package app.msime.android.test;

import android.content.ContextWrapper;
import android.view.InputDevice;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.inputmethod.InputConnection;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.ExecutorService;
import org.json.JSONArray;
import org.json.JSONObject;

/** 从安装后的服务调用硬件按键入口；合成连接不读取或修改正在使用的编辑器。 */
final class HardwareDigitChecks {
    private HardwareDigitChecks() { }

    static void run(DeviceSmoke smoke) throws Exception {
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
            smoke.getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        options.getJSONObject("preferences").put("scheme", "quanpin")
            .put("touch_keyboard_layout", "twenty_six_key").put("learning", false)
            .put("cloud_candidates", false).put("number_row_selection", true)
            .put("character_width", "halfwidth");
        options.getJSONObject("preferences").getJSONObject("local_modes")
            .put("emoji", true).put("shift_letter_entry", true);
        Throwable[] failure = new Throwable[1];
        smoke.runOnMainSync(() -> {
            Object service = null;
            try {
                ClassLoader loader = smoke.getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                Class<?> nativeType = loader.loadClass("app.msime.android.NativeClient");
                service = type.getConstructor().newInstance();
                Method attach = ContextWrapper.class.getDeclaredMethod("attachBaseContext", android.content.Context.class);
                attach.setAccessible(true);
                attach.invoke(service, smoke.getTargetContext());
                type.getMethod("onCreate").invoke(service);
                type.getMethod("onCreateInputView").invoke(service);
                long session = value(nativeType.getMethod("create", String.class)
                    .invoke(null, options.toString())).getLong("session");
                field(type, "session").setLong(service, session);
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                field(type, "view").set(service, value(nativeType.getMethod("setEnglishMode", long.class,
                    boolean.class).invoke(null, session, false)));
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
                Method character = method(type, "character", int.class, boolean.class);
                Method command = method(type, "command", int.class);
                Method onKey = type.getMethod("onKeyDown", int.class, KeyEvent.class);
                for (int grid : new int[] {1, 3, 0}) {
                    nativeType.getMethod("destroy", long.class).invoke(null, session);
                    field(type, "session").setLong(service, 0);
                    String layout = grid == 1 ? "nine_key" : grid == 3 ? "nine_key_shuangpin" : "twenty_six_key";
                    options.getJSONObject("preferences").put("touch_keyboard_layout", layout)
                        .put("nine_key_shuangpin_profile", "migala3")
                        .put("touch_keyboard_schemes", new JSONObject()
                            .put("enabled", new JSONArray().put("quanpin").put(layout))
                            .put("selected", grid == 0 ? "quanpin" : layout));
                    session = value(nativeType.getMethod("create", String.class)
                        .invoke(null, options.toString())).getLong("session");
                    field(type, "session").setLong(service, session);
                    value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                    field(type, "view").set(service, value(nativeType.getMethod("setEnglishMode", long.class,
                        boolean.class).invoke(null, session, false)));
                    for (boolean selection : new boolean[] {true, false}) {
                        smoke.stage = "idle hardware digits, grid " + grid + ", selection " + selection;
                        field(type, "numberRowSelection").setBoolean(service, selection);
                        committed.setLength(0);
                        for (int code : new int[] {KeyEvent.KEYCODE_2, KeyEvent.KEYCODE_3, KeyEvent.KEYCODE_4,
                                KeyEvent.KEYCODE_5, KeyEvent.KEYCODE_6, KeyEvent.KEYCODE_7,
                                KeyEvent.KEYCODE_8, KeyEvent.KEYCODE_9, KeyEvent.KEYCODE_0, KeyEvent.KEYCODE_1}) {
                            boolean handled = key(onKey, service, code);
                            // 普通 26 键可以把数字交回系统编辑器；九键必须吃掉并字面上屏，不能启动组码。
                            if (!handled && grid == 0) committed.append((char) ('0' + code - KeyEvent.KEYCODE_0));
                        }
                        check(committed.toString().equals("2345678901"), "digits must remain literal");
                        check(view(type, service).getString("editing_text").isEmpty(), "digits must not compose");
                        if (!selection && grid == 0) continue;
                        smoke.stage = "candidate selection, grid " + grid + ", selection " + selection;
                        committed.setLength(0);
                        for (char letter : "nihao".toCharArray()) character.invoke(service, (int) letter, false);
                        check(view(type, service).getJSONArray("candidates").length() > 1, "candidate fixture");
                        String second = view(type, service).getJSONArray("candidates").getJSONObject(1).getString("text");
                        key(onKey, service, KeyEvent.KEYCODE_2);
                        if (selection) check(committed.toString().equals(second), "digit still selects candidate");
                        else if (grid != 0) check(committed.toString().equals("你好2"), "unclaimed digit follows composition");
                        command.invoke(service, 3);
                    }
                    if (grid == 0) continue;
                    smoke.stage = "numpad and fullwidth hardware digits, grid " + grid;
                    committed.setLength(0);
                    key(onKey, service, KeyEvent.KEYCODE_NUMPAD_2);
                    check(committed.toString().equals("2"), "numpad digit stays literal");
                    field(type, "fullWidthInput").setBoolean(service, true);
                    key(onKey, service, KeyEvent.KEYCODE_3);
                    check(committed.toString().equals("2３"), "fullwidth digit follows width preference");
                    field(type, "fullWidthInput").setBoolean(service, false);
                    smoke.stage = "touch grid still composes, grid " + grid;
                    character.invoke(service, (int) '2', false);
                    character.invoke(service, (int) '3', false);
                    check(view(type, service).getString("editing_text").equals("23"), "touch digits stay grid codes");
                    command.invoke(service, 3);
                    smoke.stage = "Unicode digits remain spelling, grid " + grid;
                    character.invoke(service, (int) 'U', true);
                    key(onKey, service, KeyEvent.KEYCODE_4);
                    key(onKey, service, KeyEvent.KEYCODE_2);
                    check(view(type, service).getString("editing_text").contains("42"), "local-mode digit input");
                    command.invoke(service, 3);
                    smoke.stage = "emoji mode keeps raw fallback, grid " + grid;
                    field(type, "numberRowSelection").setBoolean(service, false);
                    committed.setLength(0);
                    character.invoke(service, (int) 'E', true);
                    character.invoke(service, (int) 'j', false);
                    character.invoke(service, (int) 'i', false);
                    check(view(type, service).getString("local_mode").equals("emoji"), "emoji mode fixture");
                    check(view(type, service).getJSONArray("candidates").length() > 0, "emoji candidate fixture");
                    // 模式不接数字时按原文结束；合成系统编辑器接住宿主交还的数字。
                    if (!key(onKey, service, KeyEvent.KEYCODE_2)) committed.append('2');
                    check(committed.toString().equals("Eji2"), "unclaimed local-mode digit must keep raw text");
                    check(view(type, service).getString("editing_text").isEmpty(), "emoji mode exits on digit");
                    command.invoke(service, 3);
                    smoke.stage = "emoji mode keeps number selection, grid " + grid;
                    field(type, "numberRowSelection").setBoolean(service, true);
                    committed.setLength(0);
                    character.invoke(service, (int) 'E', true);
                    character.invoke(service, (int) 'j', false);
                    character.invoke(service, (int) 'i', false);
                    String emoji = view(type, service).getJSONArray("candidates").getJSONObject(0).getString("text");
                    key(onKey, service, KeyEvent.KEYCODE_1);
                    check(committed.toString().equals(emoji), "local-mode number selection stays available");
                }
            } catch (Exception | AssertionError error) { failure[0] = error; }
            finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
                    for (Field member : service.getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                        try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                        catch (ReflectiveOperationException ignored) { /* 清理测试线程。 */ }
                    }
                }
            }
        });
        if (failure[0] != null) {
            Throwable cause = failure[0];
            while (cause.getCause() != null) cause = cause.getCause();
            throw new AssertionError("Native hardware digit routing failed (" + cause.getClass().getSimpleName()
                + (cause instanceof AssertionError ? ": " + cause.getMessage() : "") + ")");
        }
    }

    private static boolean key(Method onKey, Object service, int code) throws Exception {
        long now = android.os.SystemClock.uptimeMillis();
        int meta = code == KeyEvent.KEYCODE_NUMPAD_2 ? KeyEvent.META_NUM_LOCK_ON : 0;
        return (boolean) onKey.invoke(service, code, new KeyEvent(now, now, KeyEvent.ACTION_DOWN,
            code, 0, meta, KeyCharacterMap.VIRTUAL_KEYBOARD, 0, 0, InputDevice.SOURCE_KEYBOARD));
    }
    private static JSONObject view(Class<?> type, Object service) throws Exception {
        return (JSONObject) field(type, "view").get(service);
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
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
