package app.msime.android.test;

import android.content.Context;
import android.content.ContextWrapper;
import android.content.SharedPreferences;
import android.system.Os;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import org.json.JSONObject;

/** 在真实应用沙箱中验证引导队列；失败由目录权限触发，不替换产品存储实现。 */
public final class OnboardingDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "原生引导选择在保存失败、离开后保留，恢复权限后写入共享偏好";
    }

    @Override protected void runChecks() throws Exception {
        Context target = getTargetContext();
        Path root = Files.createTempDirectory(target.getCacheDir().toPath(), "synthetic-oobe-");
        Path preferences = Files.createDirectory(root.resolve("preferences"));
        String prefix = "synthetic-oobe-" + root.getFileName() + "-";
        Context isolated = new ContextWrapper(target) {
            @Override public File getFilesDir() { return root.toFile(); }
            @Override public Context getApplicationContext() { return this; }
            @Override public SharedPreferences getSharedPreferences(String name, int mode) {
                return target.getSharedPreferences(prefix + name, mode);
            }
        };
        Class<?> choices = target.getClassLoader().loadClass("app.msime.android.home.OnboardingChoices");
        Class<?> store = target.getClassLoader().loadClass("app.msime.android.home.HostStore");
        try {
            stage = "系统服务不可查询时保留未知";
            Class<?> setup = target.getClassLoader().loadClass("app.msime.android.home.ImeSetup");
            Context unavailable = new ContextWrapper(target) {
                @Override public Object getSystemService(String name) {
                    return Context.INPUT_METHOD_SERVICE.equals(name) ? null : super.getSystemService(name);
                }
            };
            check(invoke(setup, "enabledState", new Class<?>[] {Context.class}, unavailable) == null,
                "缺少系统服务被误报为未启用");
            stage = "返回系统设置后重新读取启用和默认状态";
            shellDiscard("ime disable app.msime.android/.MSIMEInputService");
            check(Boolean.FALSE.equals(invoke(setup, "enabledState", new Class<?>[] {Context.class}, target)), "禁用状态没有刷新");
            restartIme();
            check(Boolean.TRUE.equals(invoke(setup, "enabledState", new Class<?>[] {Context.class}, target)), "启用状态没有刷新");
            check(Boolean.TRUE.equals(invoke(setup, "currentState", new Class<?>[] {Context.class}, target)), "默认状态没有刷新");
            stage = "资源尚未准备时保留 false 译文选择";
            invoke(choices, "rememberGloss", new Class<?>[] {Context.class, boolean.class}, isolated, false);
            Object waiting = invoke(choices, "syncNow", new Class<?>[] {Context.class, boolean.class}, isolated, true);
            check(outcome(waiting).equals("WAITING"), "未准备的存储没有报告等待");
            check(Boolean.FALSE.equals(invoke(choices, "pendingGloss", new Class<?>[] {Context.class}, isolated)), "false 选择丢失");
            Files.write(root.resolve("runtime-options.json"), new JSONObject()
                .put("preferences_directory", preferences.toString()).toString().getBytes(StandardCharsets.UTF_8));
            JSONObject snapshot = (JSONObject) store.getMethod("loadPreferences", Context.class).invoke(null, isolated);
            check(snapshot != null, "共享偏好没有就绪");
            snapshot.getJSONObject("preferences").put("candidate_english_gloss", true);
            check(store.getMethod("savePreferences", Context.class, JSONObject.class).invoke(null, isolated, snapshot) != null, "合成偏好初始化失败");
            stage = "目录不可写时有限重试并保留待保存选择";
            Os.chmod(preferences.toString(), 0500);
            Object failed = invoke(choices, "syncNow", new Class<?>[] {Context.class, boolean.class}, isolated, true);
            check(outcome(failed).equals("FAILED"), "可读但不可写没有报告保存失败");
            check((boolean) invoke(choices, "hasPending", new Class<?>[] {Context.class}, isolated), "保存失败清空了选择");
            stage = "离开标记不替代保存，恢复权限后重试";
            Class<?> activity = target.getClassLoader().loadClass("app.msime.android.home.OnboardingActivity");
            invoke(activity, "markSeen", new Class<?>[] {Context.class}, isolated);
            check((boolean) activity.getMethod("seen", Context.class).invoke(null, isolated), "离开标记没有保存");
            check((boolean) invoke(choices, "hasPending", new Class<?>[] {Context.class}, isolated), "离开引导清空了选择");
            Os.chmod(preferences.toString(), 0700);
            Object saved = invoke(choices, "syncNow", new Class<?>[] {Context.class, boolean.class}, isolated, true);
            check(outcome(saved).equals("SAVED"), "恢复权限后无法保存");
            check(!(boolean) invoke(choices, "hasPending", new Class<?>[] {Context.class}, isolated), "已保存的选择仍在队列");
            JSONObject actual = (JSONObject) store.getMethod("loadPreferences", Context.class).invoke(null, isolated);
            check(!actual.getJSONObject("preferences").getBoolean("candidate_english_gloss"), "共享偏好没有保存 false");
        } finally {
            Os.chmod(preferences.toString(), 0700);
            isolated.getSharedPreferences("msime_onboarding_v1", Context.MODE_PRIVATE).edit().clear().commit();
            try (var paths = Files.walk(root)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toArray(Path[]::new)) Files.delete(path);
            }
        }
    }

    private static Object invoke(Class<?> type, String name, Class<?>[] parameters, Object... arguments) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters);
        method.setAccessible(true);
        return method.invoke(null, arguments);
    }

    private static String outcome(Object result) throws Exception {
        Method method = result.getClass().getDeclaredMethod("outcome");
        method.setAccessible(true);
        return method.invoke(result).toString();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
