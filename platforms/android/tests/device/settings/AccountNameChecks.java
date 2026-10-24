package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.Intent;
import android.widget.EditText;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Predicate;
import org.json.JSONException;
import org.json.JSONObject;

/** 昵称规则经产品 JNI 与 Rust 验证；只使用合成文本，不登录、不联网。 */
final class AccountNameChecks {
    private AccountNameChecks() {}

    static void run(Instrumentation instrumentation) throws Exception {
        Context context = instrumentation.getTargetContext();
        ClassLoader loader = context.getClassLoader();
        Class<?> policy = loader.loadClass("app.msime.android.AccountNamePolicy");
        Method present = policy.getMethod("present", String.class);
        Object sample = present.invoke(null, "\uFEFF　合成🌲　\uFEFF");
        check(sample != null && valid(sample), "共享空白规范化后可以保存");
        check("合成🌲".equals(sample.getClass().getMethod("normalized").invoke(sample)), "UTF-8 与规范化结果");
        check(Integer.valueOf(64).equals(sample.getClass().getMethod("maximumChars").invoke(sample)), "共享码点上限");
        check(Integer.valueOf(128).equals(sample.getClass().getMethod("maximumUtf16Units").invoke(sample)), "共享控件上限");
        for (String name : new String[] {"林", "林".repeat(64), "🌲".repeat(64), "👩‍💻", "e\u0301".repeat(32)})
            check(valid(present.invoke(null, name)), "共享规则接受合成 Unicode 昵称");
        for (String name : new String[] {"", "　\uFEFF", "林".repeat(65), "🌲".repeat(65),
                "e\u0301".repeat(33), "a\nb", "a\u007fb", "a\u0085b", "\u0085合成", "合成\u009f",
                "\u0000合成", "合成\u001f", "\ud800", "\udc00", "林".repeat(16 * 1024), null}) {
            Object result = present.invoke(null, name);
            check(result == null || !valid(result), "共享规则或传输失败拒绝保存");
        }

        Class<?> client = loader.loadClass("app.msime.android.NativeClient");
        Method bridge = client.getMethod("accountName", String.class);
        JSONObject request = new JSONObject().put("name", "　合成🌲　").put("id", "synthetic");
        String response = (String) bridge.invoke(null, request.toString());
        JSONObject envelope = new JSONObject(response);
        check(Boolean.TRUE.equals(envelope.opt("ok")), "真实 JNI 成功信封");
        Method parse = policy.getDeclaredMethod("parse", String.class);
        parse.setAccessible(true);
        for (String field : new String[] {"normalized", "valid", "maximum_chars", "maximum_utf16_units"}) {
            JSONObject missing = new JSONObject(response);
            missing.getJSONObject("value").remove(field);
            reject(parse, missing.toString());
        }
        for (String field : new String[] {"ok", "valid", "maximum_chars", "maximum_utf16_units", "normalized"}) {
            JSONObject wrong = new JSONObject(response);
            if (field.equals("ok")) wrong.put(field, "true");
            else wrong.getJSONObject("value").put(field, field.equals("normalized") ? 12 : "true");
            reject(parse, wrong.toString());
        }
        for (Object bound : new Object[] {0, -1, 1.5, 1L << 32, "64"}) {
            JSONObject wrong = new JSONObject(response);
            wrong.getJSONObject("value").put("maximum_chars", bound);
            reject(parse, wrong.toString());
        }
        reject(parse, "{}");
        reject(parse, "");
        JSONObject changed = new JSONObject(response);
        changed.getJSONObject("value").put("maximum_chars", 12).put("maximum_utf16_units", 24);
        Object updated = parse.invoke(null, changed.toString());
        check(Integer.valueOf(12).equals(updated.getClass().getMethod("maximumChars").invoke(updated))
            && Integer.valueOf(24).equals(updated.getClass().getMethod("maximumUtf16Units").invoke(updated)),
            "Java 不重建共享上限");

        Method raw = client.getDeclaredMethod("accountNameRaw", byte[].class);
        raw.setAccessible(true);
        byte[] oversized = new byte[16 * 1024 + 1];
        JSONObject failure = new JSONObject(new String((byte[]) raw.invoke(null, (Object) oversized), StandardCharsets.UTF_8));
        check(Boolean.FALSE.equals(failure.opt("ok")), "JNI 拒绝超限请求");
        try {
            bridge.invoke(null, "林".repeat(6000));
            throw new AssertionError("UTF-8 超限请求不能过桥");
        } catch (InvocationTargetException error) {
            check(error.getCause() instanceof IllegalArgumentException, "传输上限拒绝多字节请求");
        }

        Class<?> api = loader.loadClass("app.msime.android.DeviceDataApi");
        Object instance = api.getConstructor(Context.class, String.class).newInstance(context, "synthetic-user");
        for (String invalid : new String[] {"a\u0085b", "\u0000合成", ""}) {
            try {
                api.getMethod("rename", String.class).invoke(instance, invalid);
                throw new AssertionError("无效昵称不能进入账号请求");
            } catch (InvocationTargetException error) {
                Throwable cause = error.getCause();
                check("account_invalid".equals(cause.getClass().getField("code").get(cause)), "保存入口复用共享校验");
            }
        }
        rawDialogValues(instrumentation, loader);
    }

    private static void rawDialogValues(Instrumentation instrumentation, ClassLoader loader) throws Exception {
        Context context = instrumentation.getTargetContext();
        Intent intent = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        check(intent != null, "原生宿主可启动");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK);
        Activity activity = instrumentation.startActivitySync(intent);
        AtomicReference<Throwable> failure = new AtomicReference<>();
        instrumentation.runOnMainSync(() -> {
            try {
                Class<?> dialogType = loader.loadClass("app.msime.android.home.InputDialog");
                Object dialog = dialogType.getConstructor(Context.class, CharSequence.class, CharSequence.class)
                    .newInstance(activity, "合成昵称", "共享昵称规则");
                dialogType.getMethod("preserveWhitespace").invoke(dialog);
                EditText input = (EditText) dialogType.getMethod("addField", CharSequence.class, CharSequence.class, int.class)
                    .invoke(dialog, "昵称", "", 0);
                AtomicReference<String> observed = new AtomicReference<>();
                Predicate<List<String>> validator = values -> { observed.set(values.get(0)); return false; };
                dialogType.getMethod("setValidator", Predicate.class).invoke(dialog, validator);
                input.setText("\u0000合成\u001f");
                check("\u0000合成\u001f".equals(observed.get()), "昵称控件把原文交给共享校验");
                dialogType.getMethod("dismiss").invoke(dialog);
            } catch (Throwable error) {
                failure.set(error);
            } finally {
                activity.finish();
            }
        });
        if (failure.get() != null) throw new AssertionError("原生昵称输入框验收失败", failure.get());
    }

    private static boolean valid(Object value) throws Exception {
        return value != null && Boolean.TRUE.equals(value.getClass().getMethod("valid").invoke(value));
    }

    private static void reject(Method parse, String response) throws Exception {
        try {
            check(parse.invoke(null, response) == null, "损坏响应拒绝保存");
        } catch (InvocationTargetException error) {
            check(error.getCause() instanceof JSONException, "解析失败关闭保存入口");
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
