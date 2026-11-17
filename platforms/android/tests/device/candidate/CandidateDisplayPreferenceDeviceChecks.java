package app.msime.android.test;

import android.view.View;
import android.view.ViewGroup;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import org.json.JSONObject;

/** 在实际服务中重放实时设置、换框和旧副本，检查显示选择与候选区测量高度。 */
final class CandidateDisplayPreferenceDeviceChecks {
    private CandidateDisplayPreferenceDeviceChecks() {}

    static void run(Class<?> type, Object service, ClassLoader loader, float density) throws Exception {
        Class<?> source = loader.loadClass("app.msime.android.ClipboardHistoryRetentionPolicy$Source");
        Object live = source(source, "LIVE"), copy = source(source, "RUNTIME_OPTIONS_COPY");
        Class<?> display = loader.loadClass("app.msime.android.ImeCandidateDisplayPreferences");
        Method apply = method(display, "apply", type, JSONObject.class, source);
        Method restore = method(display, "restoreHint", type, JSONObject.class);
        Method runtime = method(type, "applyEditorPreferences", JSONObject.class, source);
        Method stop = method(type, "stopSessionState");
        Method update = method(type, "updateCandidateViewportHeight");
        String[] members = {"candidateEnglishGloss", "candidatePronunciationEnabled",
            "spellingPosition", "spellingEditorEnabled"};
        Object[] previous = new Object[members.length];
        for (int index = 0; index < members.length; index++)
            previous[index] = field(type, members[index]).get(service);
        View header = (View) field(type, "candidateHeader").get(service);
        View line = (View) field(type, "candidateLine").get(service);
        View toolbar = (View) field(type, "shortcutToolbar").get(service);
        int headerVisibility = header.getVisibility(), lineVisibility = line.getVisibility();
        int toolbarVisibility = toolbar.getVisibility();
        ViewGroup region = (ViewGroup) field(type, "candidateCanvasRegion").get(service);
        JSONObject factory = preferences(true, false, "editor", true);
        try {
            for (JSONObject chosen : new JSONObject[] {
                    preferences(false, false, "candidate_bar", false),
                    preferences(true, true, "editor", true)}) {
                apply.invoke(null, service, chosen, live);
                assertChoice(type, service, chosen);
                // 会话销毁清除词典和异步答案，用户的显示开关继续保留。
                stop.invoke(service);
                assertChoice(type, service, chosen);
                header.setVisibility(View.GONE);
                line.setVisibility(View.GONE);
                toolbar.setVisibility(View.VISIBLE);
                update.invoke(service);
                int before = height(region, density);
                // 走完整进框偏好入口，防止辅助类正确但宿主仍在别处覆盖字段。
                runtime.invoke(service, factory, copy);
                assertChoice(type, service, chosen);
                update.invoke(service);
                if (height(region, density) != before)
                    throw new AssertionError("旧启动副本改变了候选区高度");
                // 冷启动缓存只含显示字段；不需要引擎会话和翻译凭据。
                apply.invoke(null, service, factory, live);
                restore.invoke(null, service, chosen);
                assertChoice(type, service, chosen);
            }
            apply.invoke(null, service, factory, live);
            restore.invoke(null, service, new JSONObject());
            assertChoice(type, service, factory);
            // 真正的实时修改仍然生效，不能为了防闪烁忽略后续设置。
            JSONObject disabled = preferences(false, false, "candidate_bar", false);
            apply.invoke(null, service, disabled, live);
            assertChoice(type, service, disabled);
        } finally {
            for (int index = 0; index < members.length; index++)
                field(type, members[index]).set(service, previous[index]);
            header.setVisibility(headerVisibility);
            line.setVisibility(lineVisibility);
            toolbar.setVisibility(toolbarVisibility);
            update.invoke(service);
        }
    }

    private static JSONObject preferences(boolean gloss, boolean pronunciation, String position,
            boolean editor) throws Exception {
        return new JSONObject().put("candidate_english_gloss", gloss)
            .put("candidate_pronunciation", pronunciation)
            .put("touch_spelling_position", position).put("touch_spelling_editor", editor);
    }

    private static void assertChoice(Class<?> type, Object service, JSONObject expected) throws Exception {
        String[][] keys = {{"candidateEnglishGloss", "candidate_english_gloss"},
            {"candidatePronunciationEnabled", "candidate_pronunciation"},
            {"spellingPosition", "touch_spelling_position"}, {"spellingEditorEnabled", "touch_spelling_editor"}};
        for (String[] pair : keys)
            if (!expected.get(pair[1]).equals(field(type, pair[0]).get(service)))
                throw new AssertionError("候选显示选择被覆盖：" + pair[1]);
    }

    private static int height(ViewGroup view, float density) {
        view.measure(View.MeasureSpec.makeMeasureSpec(Math.round(400 * density), View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        return view.getMeasuredHeight();
    }

    private static Object source(Class<?> type, String name) {
        return java.util.Arrays.stream(type.getEnumConstants())
            .filter(value -> value.toString().equals(name)).findFirst().orElseThrow();
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field result = type.getDeclaredField(name); result.setAccessible(true); return result;
    }

    private static Method method(Class<?> type, String name, Class<?>... arguments) throws Exception {
        Method result = type.getDeclaredMethod(name, arguments); result.setAccessible(true); return result;
    }
}
