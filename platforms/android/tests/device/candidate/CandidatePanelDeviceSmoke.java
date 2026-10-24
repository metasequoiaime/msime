package app.msime.android.test;

import android.content.Context;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.ContextThemeWrapper;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用真实触摸检查候选条、完整网格与九键三栏选词后不必再按其他键就能刷新。 */
public final class CandidatePanelDeviceSmoke extends DeviceSmoke {
    private boolean stripPartialSelection;
    private boolean continuousStrip;
    private volatile int stripScrollX;

    @Override public void onCreate(Bundle arguments) {
        stripPartialSelection = arguments != null && "true".equals(arguments.getString("strip-partial-selection"));
        continuousStrip = arguments != null && "true".equals(arguments.getString("continuous-strip"));
        super.onCreate(arguments);
    }
    @Override protected String successDescription() {
        if (continuousStrip) return "横向候选条连续到达末项、反向返回并准确选词";
        return stripPartialSelection ? "小鹤双拼横向候选条部分选词后自动刷新，无需再次滑动"
            : "touch candidate release redraws the strip, complete grid and nine-key panel";
    }

    @Override protected void runChecks() throws Exception {
        if (continuousStrip) { checkContinuousStrip(); return; }
        if (stripPartialSelection) { checkStripPartialSelection(); return; }
        stage = "pressed release cancellation and repeated key guard";
        Throwable[] failure = {null};
        runOnMainSync(() -> {
            try { checkReleaseLifecycle(); }
            catch (Throwable error) { failure[0] = error; }
        });
        if (failure[0] != null) throw new AssertionError("松开状态回调不符合约定：" + failure[0], failure[0]);
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
            getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (boolean nineKey : new boolean[] {false, true}) {
                String layout = nineKey ? "nine_key" : "quanpin";
                JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("touch_keyboard_layout", nineKey ? "nine_key" : "twenty_six_key")
                    .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
                    .put("cloud_candidates", false).put("learning", false)
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject()
                        .put("enabled", new JSONArray().put("quanpin").put("nine_key"))
                        .put("selected", layout));
                stage = layout + " preferences";
                publish(preferences, new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
                if (!modes.edit().clear().commit()) throw new AssertionError("输入模式准备失败");
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                stage = layout + " strip composition";
                compose(nineKey);
                String first = candidateText(await(candidate(1)));
                stage = layout + " strip touch commit";
                touch(candidate(1));
                await(field("msime-test-plain").and(node -> equalsText(first, node.getText())));
                stage = layout + " strip released candidate redraw";
                awaitGone(describedPrefix("候选 "));

                stage = layout + " complete panel composition";
                compose(nineKey);
                tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
                await(completeCount());
                if (nineKey) await(described("九键候选面板"));
                AccessibilityNodeInfo later = await(candidate(10));
                if (later.getText() == null || later.getText().toString().startsWith("10. ")
                        || !later.isLongClickable())
                    throw new AssertionError("展开候选应隐藏序号并保留长按菜单");
                String expected = first + candidateText(later);
                stage = layout + " complete panel touch commit";
                touch(candidate(10));
                await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
                stage = layout + " complete panel released candidate redraw";
                awaitGone(describedPrefix("完整候选列表；"));
                awaitGone(described("九键候选面板"));
                awaitGone(describedPrefix("候选 "));
                stage = layout + " keyboard continues after panel selection";
                touch(nineKey ? key("MNO") : described("字母 N"));
                if (nineKey) await(candidate(1));
                else await(field("msime-test-plain").and(node -> equalsText(expected + "n", node.getText())));
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("输入模式恢复失败");
            restartIme();
        }
    }

    /** 不展开面板，用真实左右滑动越过原分页边界并到达完整列表末项。 */
    private void checkContinuousStrip() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false).put("learning", false)
            .put("cloud_candidates", false).put("candidate_english_gloss", false)
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                .put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        long revision = original == null ? 1
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision") + 1;
        automation.setOnAccessibilityEventListener(event -> {
            if (event.getEventType() == android.view.accessibility.AccessibilityEvent.TYPE_VIEW_SCROLLED
                    && equalsText("app.msime.android", event.getPackageName())
                    && equalsText("android.widget.HorizontalScrollView", event.getClassName()))
                stripScrollX = event.getScrollX();
        });
        try {
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision)
                .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
            if (!modes.edit().clear().commit()) throw new AssertionError("输入模式准备失败");
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            continuousStage("continuous strip Chinese mode");
            Predicate<AccessibilityNodeInfo> language = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            AccessibilityNodeInfo mode = await(key("中").or(key("英")).and(language));
            if (equalsText("英", mode.getText())) tap(key("英").and(language));
            Class<?> nativeType = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
            options.put("preferences", configured).put("phrase_preedit", true);
            for (String letters : new String[] {"wo", "m"}) {
                continuousStage("prepare independent synthetic candidates " + letters);
                JSONArray expected = syntheticCandidates(nativeType, options, letters);
                int target = letters.equals("wo") ? expected.length() : 70;
                if (target <= 9 || target > expected.length())
                    throw new AssertionError("合成候选未覆盖分页或追加边界");
                continuousStage("compose horizontal candidates " + letters);
                for (char letter : letters.toCharArray()) tap(described("字母 " + Character.toUpperCase(letter)));
                AccessibilityNodeInfo first = await(candidate(1));
                AccessibilityNodeInfo scroller = first.getParent();
                while (scroller != null && !equalsText("android.widget.HorizontalScrollView", scroller.getClassName()))
                    scroller = scroller.getParent();
                if (scroller == null) throw new AssertionError("横向候选条不可用");
                Rect viewport = new Rect();
                scroller.getBoundsInScreen(viewport);
                continuousStage("swipe beyond page and batch to " + target);
                swipeTo(target, scroller, viewport, true);
                String expectedText = expected.getJSONObject(target - 1).getString("text");
                if (!expectedText.equals(candidateText(await(candidate(target)))))
                    throw new AssertionError("完整候选顺序变化");
                continuousStage("return to first candidate without expansion");
                swipeTo(1, scroller, viewport, false);
                continuousStage("return to later candidate and touch commit");
                swipeTo(target, scroller, viewport, true);
                touch(candidate(target));
                await(field("msime-test-plain").and(node -> equalsText(expectedText, node.getText())));
                awaitGone(describedPrefix("候选 "));
                // 下一轮输入使用新的编辑器，不把上一轮提交当组字前缀。
                openEditor();
                tap(field("msime-test-plain"));
                shellDiscard("input keyevent 123");
                for (int index = 0; index < expectedText.length(); index++) shellDiscard("input keyevent 67");
            }
        } catch (Exception | AssertionError error) {
            shellDiscard("screencap -p /sdcard/msime-candidate-strip-failure.png");
            throw error;
        } finally {
            automation.setOnAccessibilityEventListener(null);
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("输入模式恢复失败");
            restartIme();
        }
    }

    private void continuousStage(String value) {
        stage = value;
        Bundle progress = new Bundle();
        progress.putString("stream", "候选条专项阶段：" + value + "\n");
        sendStatus(0, progress);
    }

    private JSONArray syntheticCandidates(Class<?> nativeType, JSONObject options, String letters) throws Exception {
        JSONObject created = nativeValue(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
        long session = created.getLong("session");
        try {
            nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true);
            for (char letter : letters.toCharArray())
                nativeType.getMethod("character", long.class, int.class, boolean.class)
                    .invoke(null, session, (int) letter, false);
            return nativeValue(nativeType.getMethod("allCandidates", long.class).invoke(null, session))
                .getJSONArray("candidates");
        } finally { nativeType.getMethod("destroy", long.class).invoke(null, session); }
    }

    private JSONObject nativeValue(Object raw) throws Exception {
        JSONObject result = new JSONObject((String) raw);
        if (!result.getBoolean("ok")) throw new AssertionError("合成会话失败");
        return result.getJSONObject("value");
    }

    private void swipeTo(int ordinal, AccessibilityNodeInfo scroller,
                         Rect viewport, boolean forward) throws Exception {
        int previousX = Integer.MIN_VALUE;
        int stationary = 0;
        for (int attempt = 0; attempt < 220; attempt++) {
            dropStaleNodes();
            if (!scroller.refresh()) {
                scroller = awaitAny(node -> {
                    if (!equalsText("android.widget.HorizontalScrollView", node.getClassName())) return false;
                    AccessibilityNodeInfo content = node.getChild(0);
                    AccessibilityNodeInfo first = content == null ? null : content.getChild(0);
                    return first != null && first.getContentDescription() != null
                        && first.getContentDescription().toString().startsWith("候选 ");
                });
                scroller.getBoundsInScreen(viewport);
            }
            if (viewport.width() < 100 || viewport.height() < 20)
                throw new AssertionError("候选条触摸范围无效");
            AccessibilityNodeInfo row = scroller.getChild(0);
            if (row == null || !row.refresh()) throw new AssertionError("候选条内容消失");
            // 此专项关闭云候选与混合输入，节点按完整候选的序号排列。只刷新目标，
            // 通过真实滚动事件判定进展，避免 API 28 每帧刷新全部候选的 Binder 开销。
            AccessibilityNodeInfo node = ordinal <= row.getChildCount() ? row.getChild(ordinal - 1) : null;
            if (node != null && node.refresh() && candidate(ordinal).test(node)) {
                Rect bounds = new Rect();
                node.getBoundsInScreen(bounds);
                if (node.isVisibleToUser() && bounds.width() > 10
                        && bounds.centerX() > viewport.left + 8
                        && bounds.centerX() < viewport.right - 8) return;
            }
            int currentX = stripScrollX;
            stationary = currentX == previousX ? stationary + 1 : 0;
            if (stationary >= 4) {
                shellDiscard("screencap -p /sdcard/msime-candidate-strip-failure.png");
                throw new AssertionError("横向候选已停止滚动，但目标仍不可见；已建 " + row.getChildCount()
                    + " 项，目标 " + ordinal + "，偏移 " + currentX);
            }
            previousX = currentX;
            int left = viewport.left + 15, right = viewport.right - 15;
            dragStrip(forward ? right : left, forward ? left : right, viewport.centerY());
        }
        throw new AssertionError("横向滑动未到达目标候选");
    }

    /** 显式送达多个 MOVE，避免旧系统负载高时 shell 滑动只留下 DOWN/UP 而退化成点击。 */
    private void dragStrip(int startX, int endX, int y) {
        long downTime = SystemClock.uptimeMillis();
        injectStripEvent(downTime, MotionEvent.ACTION_DOWN, startX, y);
        for (int step = 1; step <= 10; step++) {
            SystemClock.sleep(30);
            injectStripEvent(downTime, MotionEvent.ACTION_MOVE,
                startX + (endX - startX) * step / 10f, y);
        }
        SystemClock.sleep(100);
        injectStripEvent(downTime, MotionEvent.ACTION_UP, endX, y);
        SystemClock.sleep(100);
    }

    private void injectStripEvent(long downTime, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(event, true)) throw new AssertionError("候选滑动事件未送达");
        } finally { event.recycle(); }
    }

    /** 迟到的松开不能提前通知；滑出和取消必须照常解除按下且不产生点击。 */
    private void checkReleaseLifecycle() throws ReflectiveOperationException {
        Context context = new ContextThemeWrapper(getTargetContext(), android.R.style.Theme_Material_Light);
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.KeyboardPressButton");
        View button = (View) type.getConstructor(Context.class).newInstance(context);
        button.layout(0, 0, 100, 100);
        int[] releases = {0}, clicks = {0};
        button.setOnClickListener(ignored -> clicks[0]++);
        type.getMethod("setPressReleaseListener", Runnable.class).invoke(button, (Runnable) () -> {
            if (button.isPressed()) throw new AssertionError("仍按下时收到松开通知");
            releases[0]++;
        });
        button.setPressed(true);
        dispatch(button, MotionEvent.ACTION_DOWN, 10);
        button.setPressed(false);
        if (!button.isPressed() || releases[0] != 0)
            throw new AssertionError("连按期间迟到的松开必须继续忽略");
        dispatch(button, MotionEvent.ACTION_CANCEL, 10);
        button.setPressed(false);
        if (button.isPressed() || releases[0] != 1 || clicks[0] != 0)
            throw new AssertionError("取消必须只通知一次且不点击");
        button.setPressed(true);
        dispatch(button, MotionEvent.ACTION_DOWN, 10);
        dispatch(button, MotionEvent.ACTION_MOVE, -100);
        dispatch(button, MotionEvent.ACTION_UP, -100);
        if (button.isPressed() || releases[0] != 2 || clicks[0] != 0)
            throw new AssertionError("滑出必须清除按下且不点击");
    }

    private static void dispatch(View button, int action, float x) {
        long now = SystemClock.uptimeMillis();
        MotionEvent event = MotionEvent.obtain(now, now, action, x, 10, 0);
        try { button.dispatchTouchEvent(event); }
        finally { event.recycle(); }
    }

    private void compose(boolean nineKey) throws Exception {
        if (nineKey) { tap(key("MNO")); tap(key("GHI")); }
        else { tap(described("字母 N")); tap(described("字母 I")); }
        await(candidate(1));
    }

    /** 不能用无障碍点击替代：它不进入 View 的按下、点击、延迟松开队列。 */
    private void touch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = awaitStableBounds(match);
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 100, MotionEvent.ACTION_UP,
            bounds.centerX(), bounds.centerY(), 0);
        down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(down, true)) throw new AssertionError("触摸按下未送达");
            SystemClock.sleep(100);
            if (!automation.injectInputEvent(up, true)) throw new AssertionError("触摸松开未送达");
        } finally { down.recycle(); up.recycle(); }
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            dropStaleNodes();
            boolean found = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), match) != null) { found = true; break; }
            if (!found) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("触摸选词后，候选界面仍未刷新");
    }

    private static String candidateText(AccessibilityNodeInfo node) {
        String description = node.getContentDescription().toString();
        String text = description.substring(description.indexOf('：') + 1);
        int annotation = text.indexOf('；');
        return annotation < 0 ? text : text.substring(0, annotation);
    }

    private Predicate<AccessibilityNodeInfo> completeCount() {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || node.getContentDescription() == null) return false;
            String value = node.getContentDescription().toString();
            if (!value.startsWith("完整候选列表；") || !value.endsWith(" 个候选")) return false;
            int start = value.lastIndexOf('；') + 1;
            try { return Integer.parseInt(value.substring(start, value.length() - 4)) > 9; }
            catch (NumberFormatException ignored) { return false; }
        };
    }

    private Predicate<AccessibilityNodeInfo> candidate(int index) {
        return describedPrefix("候选 " + index + "：").and(AccessibilityNodeInfo::isClickable);
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
    }

    /** 只在选词前滑动；选词后直接观察剩余候选，不用下一次手势触发补绘。 */
    private void checkStripPartialSelection() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "shuangpin").put("shuangpin_profile", "xiaohe")
            .put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false).put("learning", false)
            .put("cloud_candidates", false).put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("xiaohe")).put("selected", "xiaohe"));
        try {
            stage = "configure synthetic Xiaohe strip composition";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", configured).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            Predicate<AccessibilityNodeInfo> language = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            AccessibilityNodeInfo mode = await(key("中").or(key("英")).and(language));
            if (equalsText("英", mode.getText())) tap(key("英").and(language));
            // 小鹤 ni/hao/shi/yao，独立合成输入不使用报告人的原文。
            int typed = 0;
            for (char letter : "nihcuiyc".toCharArray()) {
                stage = "compose synthetic Xiaohe prefix " + (++typed);
                tap(key(String.valueOf(letter)));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && node.getText().length() > 0 && !node.isShowingHintText()));
            }
            stage = "synthetic long phrase and partial candidate are present";
            AccessibilityNodeInfo first = await(stripCandidate(1, "你好是要"));
            AccessibilityNodeInfo scroller = first.getParent();
            while (scroller != null && !equalsText("android.widget.HorizontalScrollView", scroller.getClassName()))
                scroller = scroller.getParent();
            if (scroller == null) throw new AssertionError("Synthetic candidate strip is not horizontal");
            Rect viewport = new Rect();
            scroller.getBoundsInScreen(viewport);
            if (viewport.width() < 100 || viewport.height() < 20)
                throw new AssertionError("Synthetic candidate viewport bounds are unavailable");
            stage = "swipe the horizontal candidate strip before selection";
            int start = viewport.right - 12;
            shellDiscard("input swipe " + start + " " + viewport.centerY() + " "
                + (start - viewport.width() / 8) + " " + viewport.centerY() + " 500");
            stage = "touch the synthetic partial phrase " + viewport.toShortString();
            touch(stripCandidate(0, "你好"));
            stage = "partial selection preserves the remaining composition";
            await(field("msime-test-plain").and(node -> equalsText("你好uiyc", node.getText())));
            stage = "remaining strip candidates refresh without another gesture";
            await(stripCandidate(1, "是要"));
            dropStaleNodes();
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), stripCandidate(0, "你好是要")) != null)
                    throw new AssertionError("Synthetic old phrase remained after partial selection");
            stage = "finish the refreshed synthetic composition";
            touch(stripCandidate(1, "是要"));
            await(field("msime-test-plain").and(node -> equalsText("你好是要", node.getText())));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }

    /** 只匹配候选描述，避免将仍在旧列表中的候选当成刷新证据。 */
    private Predicate<AccessibilityNodeInfo> stripCandidate(int index, String value) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName()) || !node.isClickable()
                    || !node.isVisibleToUser() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            String prefix = index == 0 ? "候选 " : "候选 " + index + "：";
            int delimiter = description.indexOf('：');
            if (!description.startsWith(prefix) || delimiter < 0) return false;
            String text = description.substring(delimiter + 1);
            return text.equals(value) || text.startsWith(value + "，") || text.startsWith(value + "；");
        };
    }



}
