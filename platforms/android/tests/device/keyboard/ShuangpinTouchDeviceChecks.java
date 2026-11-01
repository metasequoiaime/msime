package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 26 键双拼的按下点（#10018）在安装产品里的验收：注入真实的触摸（不是无障碍点击，那条路不带按下点），按在键的边角和键距里时组字的是命中的那个键；按错的一键偏向想按的键时，纠错整句出现在候选里，而同样的按键经无障碍点击（没有按下点、按平面表计价）时它不在候选里。开着「显示调试信息」时宿主把注册的几何写到 `files/debug/key-geometry.json`，这里核对它和无障碍树里各键的位置一致，再经 instrumentation 状态（`msime_key_geometry_<名字>`）交给 smoke.sh 存下来，用来替换评测的参考布局。
 *
 * <p>经 `MicrosoftShuangpinDeviceSmoke -e shuangpin-touch true` 运行，不增加 instrumentation 入口（API 35 的清单最多 100 个）。偏好、本地设置和导出文件都在结束时还原。
 */
final class ShuangpinTouchDeviceChecks {
    private final DeviceSmoke t;
    private final File preferences;
    private final File local;
    private final File exported;
    private JSONObject configured;
    private long revision;
    /** 最近一次读到的导出文件原文，原样交给 smoke.sh，不经 org.json 重新序列化。 */
    private String exportedText;

    private ShuangpinTouchDeviceChecks(DeviceSmoke test) throws Exception {
        t = test;
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        preferences = new File(new File(options.getString("preferences_directory")).getCanonicalFile(), "preferences.json");
        local = new File(root, "bootstrap/state/android-settings.json");
        exported = new File(root, "debug/key-geometry.json");
        configured = new JSONObject(options.getJSONObject("preferences").toString());
    }

    static void run(DeviceSmoke test) throws Exception {
        new ShuangpinTouchDeviceChecks(test).run();
    }

    private void run() throws Exception {
        byte[] originalPreferences = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        revision = originalPreferences == null ? 0
            : new JSONObject(new String(originalPreferences, StandardCharsets.UTF_8)).getLong("revision");
        try {
            t.stage = "shuangpin touch: xiaohe 26 keys with the geometry export on";
            JSONObject settings = originalLocal == null
                ? new JSONObject().put("version", 1).put("settings", new JSONObject())
                : new JSONObject(new String(originalLocal, StandardCharsets.UTF_8));
            settings.getJSONObject("settings").put("platform.android.developer.logging_enabled", true)
                .put("platform.android.developer.debug_overlay", true)
                .put("platform.android.split_keyboard", false).put("platform.android.one_handed", "off")
                .put("platform.android.floating_keyboard", false).put("platform.android.glide_typing", false);
            t.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            scheme("xiaohe");
            Files.deleteIfExists(exported.toPath());
            t.restartIme();
            editor();

            t.stage = "shuangpin touch: portrait geometry matches the keys";
            geometry(false);
            emit("android-portrait");

            t.stage = "shuangpin touch: an off-centre press composes the key it lands on";
            Rect f = bounds("f");
            touch(f.left + f.width() * 0.12f, f.top + f.height() * 0.15f);
            awaitField("f");
            clear(1);
            touch(f.right - f.width() * 0.12f, f.bottom - f.height() * 0.15f);
            awaitField("f");
            clear(1);

            t.stage = "shuangpin touch: a press in the gap composes the key that owns it";
            Rect g = bounds("g");
            if (g.left - f.right < 4) throw new AssertionError("键距太窄，测不到空隙：" + f + " " + g);
            touch(f.right + (g.left - f.right) / 4f, f.centerY());
            awaitField("f");
            clear(1);
            touch(g.left - (g.left - f.right) / 4f, g.centerY());
            awaitField("g");
            clear(1);

            // 小鹤 mw gr xi 是「没关系」；按成 gf（f 在 r 的右下方）就是一处邻键误触。
            t.stage = "shuangpin touch: without touch-downs the correction stays off the strip";
            for (String key : new String[] {"m", "w", "g", "f", "x", "i"}) t.tap(t.key(key));
            awaitField("mwgfxi");
            // 先确认候选条已经画出来、谓词认得出候选行，下面的「不在」才有意义。
            t.await(node -> node.isClickable() && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("候选 1："));
            boolean plainShows = visible(candidate("没关系"));
            boolean plainFirstMarked = visible(marked());
            clear(6);
            if (plainShows) throw new AssertionError("没有按下点时纠错整句不该出现在候选条上（平面表把它排在表尾）");
            if (plainFirstMarked) throw new AssertionError("没有按下点时不该有候选带「已纠正」标记");

            t.stage = "shuangpin touch: a press leaning towards r keeps the correction on the strip";
            for (String key : new String[] {"m", "w", "g"}) center(key);
            touch(f.left + f.width() * 0.1f, f.top + f.height() * 0.1f);
            for (String key : new String[] {"x", "i"}) center(key);
            awaitField("mwgfxi");
            // 纠错行的注释是按下的原串（corrected_from），读屏描述里是「提示：mwgfxi」。
            t.await(candidate("没关系").and(node -> node.getContentDescription().toString().contains("，提示：mwgfxi")));
            // 默认参数下首选规则不起作用（#10018 PR3 没达到门槛，lead_margin 取无穷大）：纠错整句只排在后面，不坐第一位，候选条上也没有哪一行带「已纠正」。门槛达到、默认边距改成有限值时，这两条要改成它坐第一位并且读屏念「已纠正」。
            boolean leads = visible(candidate("没关系").and(node -> node.getContentDescription().toString().startsWith("候选 1：")));
            boolean touchedMarked = visible(marked());
            clear(6);
            if (leads) throw new AssertionError("默认参数下纠错整句不该坐首选");
            if (touchedMarked) throw new AssertionError("默认参数下不该有候选带「已纠正」标记");

            t.stage = "shuangpin touch: plain clicks after the touched pass still leave it off the strip";
            for (String key : new String[] {"m", "w", "g", "f", "x", "i"}) t.tap(t.key(key));
            awaitField("mwgfxi");
            t.await(node -> node.isClickable() && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("候选 1："));
            boolean plainAfterShows = visible(candidate("没关系"));
            boolean plainMarked = visible(marked());
            clear(6);
            if (plainAfterShows) throw new AssertionError("按过带落点的那一遍之后，没有按下点的同一串不该留下纠错整句");
            if (plainMarked) throw new AssertionError("没有按下点时不该有候选带「已纠正」标记");

            // 冻结词级评测中的误触：小鹤 ai vg（癌症）按成 ai vv，最后一键落在 v 的上缘。
            t.stage = "shuangpin touch: a short correction consumes all four pressed keys";
            for (String key : new String[] {"a", "i", "v", "v"}) t.tap(t.key(key));
            awaitField("aivv");
            Predicate<AccessibilityNodeInfo> first = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("候选 1：");
            // 产品可能没有评测用的小模型，保留这次设备自己的字面首选。
            String plainFirst = t.await(first).getContentDescription().toString();
            boolean plainShortShows = visible(candidate("癌症"));
            clear(4);
            if (plainShortShows) throw new AssertionError("没有按下点时不该补入短词纠错");
            String[] shortKeys = {"a", "i", "v", "v"};
            int[][] offsets = {{-3, 0}, {1, -4}, {7, -1}, {0, -9}};
            for (int index = 0; index < shortKeys.length; index++) {
                Rect key = bounds(shortKeys[index]);
                touch(key.exactCenterX() + offsets[index][0] * key.width() / 20f,
                    key.exactCenterY() + offsets[index][1] * key.height() / 20f);
            }
            awaitField("aivv");
            t.await(first.and(node -> plainFirst.equals(node.getContentDescription().toString())));
            t.tap(candidate("癌症").and(node -> node.getContentDescription().toString().contains("，提示：aivv")));
            awaitField("癌症");
            clear(2);

            t.stage = "shuangpin touch: Microsoft double-pinyin registers its ; key";
            // 服务重建后几何代次可以从 1 开始；删除旧导出，按新文件确认这次注册。
            Files.deleteIfExists(exported.toPath());
            scheme("microsoft");
            t.restartIme();
            editor();
            t.await(t.described("微软双拼 ing").and(AccessibilityNodeInfo::isVisibleToUser));
            geometry(true);
            emit("android-portrait-microsoft");
            t.stage = "shuangpin touch: the ; key carries its touch-down";
            center("b");
            Rect semicolon = boundsOf(t.described("微软双拼 ing"));
            touch(semicolon.left + semicolon.width() * 0.2f, semicolon.centerY());
            awaitField("b;");
            clear(2);
        } finally {
            Files.deleteIfExists(exported.toPath());
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else t.publish(local, originalLocal);
            if (originalPreferences == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, originalPreferences);
            t.restartIme();
        }
    }

    private void scheme(String profile) throws Exception {
        configured.put("scheme", "shuangpin").put("shuangpin_profile", profile)
            .put("touch_keyboard_layout", "twenty_six_key").put("touch_shuangpin_layout", "twenty_six_key")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                .put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put(profile)).put("selected", profile));
        if (preferences.exists()) revision = Math.max(revision,
            new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8)).getLong("revision"));
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
    }

    private void editor() throws Exception {
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.await(t.key("f").and(AccessibilityNodeInfo::isVisibleToUser));
        t.dropStaleNodes();
    }

    /**
     * 等宿主在旧文件被删除后写出有效代次的几何，并核对它就是屏幕上的键：每个字母键的中心减去几何里的中心是同一个偏移（按键区的原点），宽高与无障碍树里的键一致（各差不到 1 像素）。`semicolon` 是不是该有 `;` 键。代次只在同一服务实例内递增，不跨重启比较。
     */
    private JSONObject geometry(boolean semicolon) throws Exception {
        JSONObject json = null;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            if (exported.isFile()) {
                String text = new String(Files.readAllBytes(exported.toPath()), StandardCharsets.UTF_8);
                JSONObject candidate = new JSONObject(text);
                if (candidate.getInt("id") > 0 && candidate.isNull("semicolon") != semicolon) {
                    json = candidate;
                    exportedText = text;
                    break;
                }
            }
            SystemClock.sleep(100);
        }
        if (json == null) throw new AssertionError("宿主没有写出" + (semicolon ? "带 ; 的" : "") + "键位几何");
        JSONArray keys = json.getJSONArray("keys");
        if (keys.length() != 26) throw new AssertionError("几何不是 26 个字母键");
        if (json.getJSONArray("duplicates").length() != 0) throw new AssertionError("不分离的键盘不该有重复键");
        float offsetX = Float.NaN;
        float offsetY = Float.NaN;
        for (int letter = 0; letter < 26; letter++) {
            Rect key = bounds(String.valueOf((char) ('a' + letter)));
            JSONArray entry = keys.getJSONArray(letter);
            check(key, entry, letter == 0, String.valueOf((char) ('a' + letter)), new float[] {offsetX, offsetY});
            if (letter == 0) {
                offsetX = key.exactCenterX() - (float) entry.getDouble(0);
                offsetY = key.exactCenterY() - (float) entry.getDouble(1);
            }
        }
        if (semicolon) check(boundsOf(t.described("微软双拼 ing")), json.getJSONArray("semicolon"), false, ";",
            new float[] {offsetX, offsetY});
        return json;
    }

    private static void check(Rect key, JSONArray entry, boolean first, String name, float[] offset) throws Exception {
        if (Math.abs(key.width() - entry.getDouble(2)) > 1 || Math.abs(key.height() - entry.getDouble(3)) > 1)
            throw new AssertionError("几何里 " + name + " 的宽高与屏幕上的键不一致：" + key + " " + entry);
        if (first) return;
        if (Math.abs(key.exactCenterX() - entry.getDouble(0) - offset[0]) > 1
                || Math.abs(key.exactCenterY() - entry.getDouble(1) - offset[1]) > 1)
            throw new AssertionError("几何里 " + name + " 的位置与屏幕上的键不一致：" + key + " " + entry);
    }

    /** 把导出的几何作为 instrumentation 状态交出去，smoke.sh 存成 `target/android/device-test/key-geometry/<名字>.json`。 */
    private void emit(String name) {
        Bundle status = new Bundle();
        status.putString("msime_key_geometry_" + name, exportedText);
        t.sendStatus(1, status);
    }

    private Rect bounds(String letter) throws Exception {
        return boundsOf(t.key(letter).and(AccessibilityNodeInfo::isVisibleToUser));
    }

    private Rect boundsOf(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo node = t.awaitStableBounds(match);
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        if (bounds.isEmpty()) throw new AssertionError("键没有位置");
        return bounds;
    }

    private void center(String letter) throws Exception {
        Rect key = bounds(letter);
        touch(key.exactCenterX(), key.exactCenterY());
    }

    /** 一次真实的点按：按下和抬起在同一点，经系统输入分发到输入法窗口。 */
    private void touch(float x, float y) {
        long down = SystemClock.uptimeMillis();
        inject(down, MotionEvent.ACTION_DOWN, x, y);
        SystemClock.sleep(40);
        inject(down, MotionEvent.ACTION_UP, x, y);
        SystemClock.sleep(150);
        t.dropStaleNodes();
    }

    private void inject(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(event, true)) throw new AssertionError("触摸注入被拒绝");
        } finally {
            event.recycle();
        }
    }

    private void awaitField(String text) {
        t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(text, node.getText())));
    }

    private void clear(int letters) throws Exception {
        for (int index = 0; index < letters; index++) t.tap(t.key("⌫"));
        t.await(t.field("msime-test-plain").and(node -> node.getText() == null || node.getText().length() == 0
            || node.isShowingHintText()));
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) throws Exception {
        t.automation.waitForIdle(500, 5000);
        t.dropStaleNodes();
        for (AccessibilityWindowInfo window : t.automation.getWindows()) {
            if (t.find(window.getRoot(), match) != null) return true;
        }
        return false;
    }

    /** 读屏描述带「已纠正」后缀的候选行（`CandidateCorrectionMarkPolicy.ACCESSIBILITY_SUFFIX`），候选条和展开面板都算。 */
    private static Predicate<AccessibilityNodeInfo> marked() {
        return node -> node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 ")
            && node.getContentDescription().toString().contains("，已纠正");
    }

    /** 候选条上的一行：读屏描述是「候选 N：文字」，后面可能跟「，提示：…」（注释）或「；…」。 */
    private static Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            int end = value.length();
            for (char mark : new char[] {'；', '，'}) {
                int at = value.indexOf(mark);
                if (at >= 0) end = Math.min(end, at);
            }
            return value.substring(0, end).equals(text);
        };
    }
}
