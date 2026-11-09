package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 14/18 键字母键的角标符号（#11063）：用真实触摸验收空闲时沿设定方向滑出符号（中文标点与英文标点、上滑与下滑）、反方向打开选项、关掉「滑动输入符号」后不出符号、组字中纵滑和长按都是点按、上屏后的联想照常出符号。偏好和本机设置都在测试沙盒里改，结束时还原。
 */
final class GridLetterSymbolDeviceChecks {
    private static final String PLAIN = "msime-test-plain";
    private static final float SWIPE_DP = 24f;
    private final DeviceSmoke t;
    private File preferences;
    private File local;
    private JSONObject base;
    private JSONObject settings;
    private long revision;

    private GridLetterSymbolDeviceChecks(DeviceSmoke test) { t = test; }

    static void run(DeviceSmoke test) throws Exception { new GridLetterSymbolDeviceChecks(test).run(); }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("偏好路径越出测试沙盒");
        local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        settings = originalLocal == null ? new JSONObject()
            : new JSONObject(new String(originalLocal, StandardCharsets.UTF_8)).getJSONObject("settings");
        base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("cloud_candidates", false).put("learning", false).put("character_width", "halfwidth")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("punctuation_lock", "follow").put("paired_punctuation", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false));
        try {
            fourteen();
            eighteen();
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else t.publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else t.publish(local, originalLocal);
            t.restartIme();
        }
    }

    private void fourteen() throws Exception {
        base.put("touch_keyboard_layout", "fourteen_key").put("touch_keyboard_schemes", new JSONObject()
            .put("enabled", new JSONArray().put("quanpin").put("fourteen_key")).put("selected", "fourteen_key"));

        // 未保存方向时缺省上滑；表里 ( ) " ? ! : ; 走中文标点，、 照字面。
        configure(true, null, true, "按键 Q W");
        t.stage = "fourteen idle up swipe";
        swipe("按键 C V", -SWIPE_DP);
        expect("？");
        swipe("按键 J K", -SWIPE_DP);
        swipe("按键 G H", -SWIPE_DP);
        swipe("按键 Q W", -SWIPE_DP);
        swipe("按键 Z X", -SWIPE_DP);
        expect("？：、；~");
        // 一次触发就锁定：上滑出符号后回到起点仍输入符号。
        t.stage = "fourteen swipe back to the start keeps the symbol";
        swipe("字母 M", -SWIPE_DP, 0);
        expect("？：、；~/");

        t.stage = "fourteen opposite swipe opens the options";
        swipe("按键 D F", SWIPE_DP);
        t.await(t.described("输入 d"));
        t.await(t.described("输入 F"));
        t.await(t.described("输入 -"));
        t.tap(t.described("输入 F"));
        expect("？：、；~/F");

        // 组字中纵滑和长按都是点按：组码多一组，不出符号，不弹选项，不结束组字。
        t.stage = "fourteen composing swipe is a tap";
        t.tap(t.described("按键 B N"));
        t.await(t.describedPrefix("候选 1："));
        swipe("按键 U I", -SWIPE_DP);
        t.await(t.described("14 键拼音选择"));
        assertNoOptions();
        t.stage = "fourteen composing hold is a tap";
        hold("按键 G H");
        assertNoOptions();
        expect("？：、；~/F");
        // BN UI GH AS OP 是 b u g a o，滑动和长按各算一组，所以能选「你好」。
        t.stage = "fourteen composing taps commit as a word";
        t.tap(t.described("按键 A S"));
        t.tap(t.described("按键 O P"));
        t.tap(candidate("你好"));
        awaitGone(t.described("14 键拼音选择"));
        String typed = "？：、；~/F你好";
        expect(typed);

        // 上屏后的联想不算组字：滑动照常出符号。
        t.stage = "fourteen swipe right after a commit";
        swipe("按键 B N", -SWIPE_DP);
        expect(typed + "！");

        t.stage = "fourteen idle hold opens the options";
        hold("字母 L");
        t.await(t.described("输入 l"));
        t.await(t.described("输入 “"));
        t.tap(t.described("输入 L"));
        expect(typed + "！L");

        // 英文标点：同一个键输入 ASCII。
        configure(true, null, false, "按键 Q W");
        t.stage = "fourteen ASCII symbols";
        swipe("按键 C V", -SWIPE_DP);
        swipe("按键 J K", -SWIPE_DP);
        expect("?:");

        // 下滑：方向对调，上滑改为打开选项。
        configure(true, "down", true, "按键 Q W");
        t.stage = "fourteen down swipe";
        swipe("按键 C V", SWIPE_DP);
        expect("？");
        swipe("按键 A S", -SWIPE_DP);
        t.await(t.described("输入 a"));
        t.await(t.described("输入 @"));
        t.tap(t.described("输入 @"));
        expect("？@");

        // 关掉「滑动输入符号」：纵滑是普通点按，长按照旧打开选项。
        configure(false, "up", true, "按键 Q W");
        t.stage = "fourteen swipes off";
        swipe("按键 C V", -SWIPE_DP);
        assertNoOptions();
        t.await(t.describedPrefix("候选 1："));
        expect("");
        t.tap(t.key("⌫"));
        awaitGone(t.describedPrefix("候选 1："));
        hold("按键 C V");
        t.await(t.described("输入 ？"));
        t.tap(t.described("输入 ？"));
        expect("？");
    }

    private void eighteen() throws Exception {
        base.put("touch_keyboard_layout", "eighteen_key").put("touch_keyboard_schemes", new JSONObject()
            .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        configure(true, null, true, "按键 W E");
        t.stage = "eighteen idle up swipe";
        swipe("字母 Y", -SWIPE_DP);
        swipe("按键 F G", -SWIPE_DP);
        swipe("字母 H", -SWIPE_DP);
        swipe("按键 W E", -SWIPE_DP);
        swipe("按键 X C", -SWIPE_DP);
        swipe("按键 I O", -SWIPE_DP);
        expect("¥%*…、（");
        t.stage = "eighteen opposite swipe on a one-letter key";
        swipe("字母 Q", SWIPE_DP);
        t.await(t.described("输入 q"));
        t.await(t.described("输入 Q"));
        t.tap(t.described("输入 ；"));
        expect("¥%*…、（；");
        t.stage = "eighteen composing swipe is a tap";
        t.tap(t.described("按键 B N"));
        t.await(t.describedPrefix("候选 1："));
        swipe("按键 I O", -SWIPE_DP);
        t.await(t.described("18 键拼音选择"));
        assertNoOptions();
        expect("¥%*…、（；");
        t.tap(t.key("⌫"));
        t.tap(t.key("⌫"));
        awaitGone(t.describedPrefix("候选 1："));
    }

    /** 写入偏好和本机滑动设置，重启输入法，等引擎就绪后清空输入框。 */
    private void configure(boolean swipes, String direction, boolean chinese, String anyKey) throws Exception {
        t.stage = "configure " + base.optString("touch_keyboard_layout") + " " + direction
            + " swipes " + swipes + " Chinese " + chinese;
        base.put("chinese_punctuation", chinese);
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
        settings.put("platform.android.swipe_down_symbols", swipes);
        if (direction == null) settings.remove("platform.android.swipe_symbols_direction");
        else settings.put("platform.android.swipe_symbols_direction", direction);
        t.publish(local, new JSONObject().put("version", 1).put("settings", settings)
            .toString().getBytes(StandardCharsets.UTF_8));
        t.restartIme();
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field(PLAIN));
        t.await(t.field(PLAIN).and(AccessibilityNodeInfo::isFocused));
        t.selectChineseKeyboard();
        t.await(t.described(anyKey));
        // 键面会先于引擎会话出现：打一组、等候选，再删掉。
        t.tap(t.described("按键 B N"));
        t.await(t.describedPrefix("候选 1："));
        t.tap(t.key("⌫"));
        awaitGone(t.describedPrefix("候选 1："));
        expect("");
    }

    /**
     * 从键中心按下，纵向移动 `offsetsDp`（向下为正）后抬起。事件一起排进输入队列：逐个同步注入时慢的模拟器上按下到抬起可能超过长按时长（见 LetterHintPunctuationDeviceSmoke.simpleSwipe）。
     */
    private void swipe(String description, float... offsetsDp) throws Exception {
        Rect bounds = bounds(t.awaitStableBounds(t.described(description)));
        float density = t.getTargetContext().getResources().getDisplayMetrics().density;
        long started = SystemClock.uptimeMillis();
        if (offsetsDp.length > 1) {
            // 多段移动逐个同步注入：一次排进四个事件时 API 28 上这次按压什么都没输入（2026-11-07）。每段只隔 30 ms，整次远短于长按时长。
            inject(started, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
            float y = bounds.centerY();
            for (float offset : offsetsDp) {
                SystemClock.sleep(30);
                y = bounds.centerY() + offset * density;
                inject(started, MotionEvent.ACTION_MOVE, bounds.centerX(), y);
            }
            SystemClock.sleep(30);
            inject(started, MotionEvent.ACTION_UP, bounds.centerX(), y);
            t.automation.waitForIdle(300, 3000);
            t.dropStaleNodes();
            return;
        }
        MotionEvent[] events = new MotionEvent[offsetsDp.length + 2];
        events[0] = MotionEvent.obtain(started, started, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY(), 0);
        float y = bounds.centerY();
        for (int index = 0; index < offsetsDp.length; index++) {
            y = bounds.centerY() + offsetsDp[index] * density;
            events[index + 1] = MotionEvent.obtain(started, started + 20L * (index + 1), MotionEvent.ACTION_MOVE,
                bounds.centerX(), y, 0);
        }
        events[events.length - 1] = MotionEvent.obtain(started, started + 20L * (offsetsDp.length + 1),
            MotionEvent.ACTION_UP, bounds.centerX(), y, 0);
        try {
            for (MotionEvent event : events) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!t.automation.injectInputEvent(event, false)) throw new AssertionError("触摸事件未送达");
            }
        } finally {
            for (MotionEvent event : events) event.recycle();
        }
        t.automation.waitForIdle(300, 3000);
        t.dropStaleNodes();
    }

    /** 真实触摸的长按：按住超过长按时长再原地抬起。 */
    private void hold(String description) throws Exception {
        Rect bounds = bounds(t.awaitStableBounds(t.described(description)));
        long started = SystemClock.uptimeMillis();
        inject(started, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 200);
        inject(started, MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        t.automation.waitForIdle(300, 3000);
        t.dropStaleNodes();
    }

    private void inject(long started, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(started, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(event, true)) throw new AssertionError("触摸事件未送达");
        } finally { event.recycle(); }
    }

    private void expect(String value) {
        Predicate<AccessibilityNodeInfo> match = t.field(PLAIN).and(node -> DeviceSmoke.equalsText(value, node.getText())
            || value.isEmpty() && (node.isShowingHintText() || node.getText() == null));
        try {
            t.await(match);
        } catch (AssertionError error) {
            // 失败时带上输入框里实际的文字（只有本测试打进合成编辑器的符号和字母）。
            AccessibilityNodeInfo field = null;
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                field = t.find(window.getRoot(), t.field(PLAIN));
                if (field != null) break;
            }
            throw new AssertionError("输入框应为「" + value + "」，实际是「"
                + (field == null ? "（找不到输入框）" : field.getText()) + "」；" + error.getMessage());
        }
    }

    private void assertNoOptions() {
        t.dropStaleNodes();
        // 拼音读音选择条也有「输入 …」的读屏说明；字母弹窗用独立的容器说明，避免把主键盘里的读音条误判成选项。
        for (AccessibilityWindowInfo window : t.automation.getWindows())
            if (t.find(window.getRoot(), t.described("字母和符号选项")) != null)
                throw new AssertionError("组字中或关闭滑动时弹出了字母选项");
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            t.dropStaleNodes();
            boolean present = false;
            for (AccessibilityWindowInfo window : t.automation.getWindows())
                if (t.find(window.getRoot(), match) != null) { present = true; break; }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("界面状态没有消失：" + t.stage);
    }

    /** 候选条上写着 `text` 的候选（描述是「候选 N：text；…」），同 FourteenKeyDeviceSmoke。 */
    private static Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!DeviceSmoke.equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect rect = new Rect();
        node.getBoundsInScreen(rect);
        return rect;
    }
}
