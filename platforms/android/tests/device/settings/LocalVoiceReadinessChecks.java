package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.SystemClock;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Predicate;
import android.view.ViewGroup;
import android.widget.TextView;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/** 在真实原生设置宿主中呈现合成准备状态；不安装模型、不保存偏好、不录音或联网。 */
public abstract class LocalVoiceReadinessChecks extends DeviceSmoke {
    private Class<?> pageType;
    private Object page;
    private Object original;
    private Constructor<?> stateConstructor;
    private Method render;
    private Activity activity;

    @Override protected String successDescription() { return "local voice model selection and runtime readiness are distinct"; }

    @Override protected void runChecks() throws Exception {
        stage = "open native voice page";
        activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
            .putExtra("msime_open_page", "VOICE"));
        ClassLoader loader = getTargetContext().getClassLoader();
        pageType = loader.loadClass("app.msime.android.home.VoicePage");
        Class<?> stateType = loader.loadClass("app.msime.android.home.VoicePage$State");
        stateConstructor = stateType.getDeclaredConstructors()[0];
        stateConstructor.setAccessible(true);
        render = pageType.getDeclaredMethod("render", stateType);
        render.setAccessible(true);
        try {
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (page == null && SystemClock.uptimeMillis() < deadline) {
                onMain(() -> {
                    Object manager = activity.getClass().getMethod("getSupportFragmentManager").invoke(activity);
                    List<?> fragments = (List<?>) manager.getClass().getMethod("getFragments").invoke(manager);
                    for (Object fragment : fragments) if (pageType.isInstance(fragment)) page = fragment;
                });
                if (page == null) SystemClock.sleep(100);
            }
            check(page != null, "语音页由真实导航创建");
            await(text("识别语言"));
            // 等待初次异步读取完成后再注入内存状态，避免把尚未加载的空视图当成产品结果。
            waitForIdleSync();
            Method read = pageType.getDeclaredMethod("read", Context.class);
            read.setAccessible(true);
            original = read.invoke(null, activity);
            check(original != null, "原生页能读取共享状态");
            tap(text("服务"));

            stage = "selected model without runtime";
            show(true, false);
            await(text("从文件导入运行库"));
            onMain(() -> {
                View root = activity.getWindow().getDecorView();
                TextView preparation = find(root, "本地识别准备");
                // 识别服务列表里也有一项叫「本地模型」，排在准备入口之上；模型列表的分组标题是页面上最后一个同名文字。
                TextView models = findLast(root, "本地模型");
                check(preparation != null && models != null && preparation.isShown(), "准备状态在模型页可见");
                int[] before = new int[2];
                int[] after = new int[2];
                preparation.getLocationOnScreen(before);
                models.getLocationOnScreen(after);
                check(before[1] < after[1], "准备入口位于长模型列表之前");
                check(find(root, "已选用") != null && find(root, "使用中") == null, "模型选择不冒充可用状态");
            });
            tap(text("从文件导入运行库"));
            await(text("导入本地语音运行库"));
            await(text("选择文件"));
            await(text("下载地址"));
            tap(text("取消"));
            await(text("从文件导入运行库"));

            stage = "runtime ready but model not selected";
            show(false, true);
            onMain(() -> {
                View root = activity.getWindow().getDecorView();
                check(find(root, "未选用") != null && find(root, "已就绪") != null, "分别列出缺模型和已准备的运行库");
                check(find(root, "从文件导入运行库") == null, "运行库存在时不重复导入");
                check(find(root, "模型和运行库均已准备好，可以开始语音输入。") == null, "缺模型不能声称可用");
            });

            stage = "model and runtime ready";
            show(true, true);
            await(text("模型和运行库均已准备好，可以开始语音输入。"));
            onMain(() -> check(find(activity.getWindow().getDecorView(), "从文件导入运行库") == null,
                "准备完成后移除过期入口"));

            stage = "both missing";
            show(false, false);
            await(text("从文件导入运行库"));
            onMain(() -> check(find(activity.getWindow().getDecorView(), "未选用") != null,
                "两项都缺时仍列明模型缺项"));
        } finally {
            runOnMainSync(activity::finish);
        }
    }

    private void show(boolean selected, boolean runtimeReady) throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> modelType = loader.loadClass("app.msime.android.LocalVoiceModels$Model");
        Object model = modelType.getConstructors()[0].newInstance("synthetic-model", "合成已装模型", "合成描述",
            false, true, "/synthetic/model", 100L, 200L, "", false, List.of());
        JSONObject preferences = new JSONObject(((JSONObject) component("preferences")).toString());
        preferences.put("voice_input", new JSONObject().put("asr_provider", "local")
            .put("asr_model_path", selected ? "/synthetic/model" : ""));
        Object state = stateConstructor.newInstance(preferences, component("local"), false, runtimeReady,
            component("runtimePack"), "local", List.of(model), null, false, true, component("catalog"));
        onMain(() -> render.invoke(page, state));
        waitForIdleSync();
    }

    private Object component(String name) throws Exception {
        Method method = original.getClass().getDeclaredMethod(name);
        method.setAccessible(true);
        return method.invoke(original);
    }

    private static Predicate<AccessibilityNodeInfo> text(String value) {
        return node -> node.isVisibleToUser() && value.contentEquals(node.getText() == null ? "" : node.getText());
    }

    private static TextView find(View view, String value) {
        if (view instanceof TextView text && text.isShown() && value.contentEquals(text.getText())) return text;
        if (view instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++) {
                TextView found = find(group.getChildAt(index), value);
                if (found != null) return found;
            }
        }
        return null;
    }

    private static TextView findLast(View view, String value) {
        TextView last = view instanceof TextView text && text.isShown() && value.contentEquals(text.getText()) ? text : null;
        if (view instanceof ViewGroup group) {
            for (int index = 0; index < group.getChildCount(); index++) {
                TextView found = findLast(group.getChildAt(index), value);
                if (found != null) last = found;
            }
        }
        return last;
    }

    @FunctionalInterface private interface MainAction { void run() throws Exception; }

    private void onMain(MainAction action) throws Exception {
        List<Throwable> errors = new ArrayList<>();
        runOnMainSync(() -> { try { action.run(); } catch (Throwable error) { errors.add(error); } });
        if (!errors.isEmpty()) {
            if (errors.get(0) instanceof AssertionError failure) throw failure;
            throw new AssertionError("本地语音准备界面检查失败：" + errors.get(0).getClass().getSimpleName());
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
