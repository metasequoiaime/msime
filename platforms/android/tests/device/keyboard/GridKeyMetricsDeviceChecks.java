package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 14/18 键字母键面和候选行提示的尺寸，复用字母组入口（`-e grid_metrics true`），不增加清单项数。
 *
 * <ul>
 * <li>九键与 26 键一样高，14 键与 18 键（拼音条在读音行上）一样高并预留读音行，打字时都不变高。英文键盘不在其列（中文和其他语言键盘之间的高度另行处理）。高度按键盘面板量，不按输入法窗口。</li>
 * <li>没开释义时，纠错候选只在候选字后面画 *、候选行不加高；五笔剩余编码这类画在候选字下面的提示出现时候选行加高，不被裁掉。</li>
 * <li>14 键和 18 键的字母同一个字号，主字母在键帽里居中（按截图的墨迹量）。</li>
 * </ul>
 */
final class GridKeyMetricsDeviceChecks {
    private final DeviceSmoke t;
    private File preferences;
    private long revision;
    private JSONObject configured;

    private GridKeyMetricsDeviceChecks(DeviceSmoke test) { t = test; }

    static void run(DeviceSmoke test) throws Exception {
        new GridKeyMetricsDeviceChecks(test).run();
    }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = json(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("合成测试目录越界");
        preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        revision = original == null ? 0 : json(preferences).getLong("revision");
        configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("learning", false).put("cloud_candidates", false)
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        try {
            readingRows();

            t.stage = "grid metrics correction mark";
            configured.put("touch_keyboard_layout", "twenty_six_key");
            save();
            t.restartIme();
            editor();
            chinese();
            typeLetters("nihao");
            int plainChip = bounds(t.describedPrefix("候选 1：")).height();
            clear();
            typeLetters("zhognguo");
            // 纠错候选只在候选字后面画 *，原串不再另起一行：候选行保持紧凑。读屏描述照旧念原串。
            Rect corrected = bounds(t.describedPrefix("候选 1：").and(node -> node.getText() != null
                && node.getText().toString().endsWith("*")
                && node.getContentDescription().toString().contains("提示：zhognguo")));
            check(Math.abs(corrected.height() - plainChip) <= 1,
                "纠错候选不该加高候选行：" + plainChip + " → " + corrected.height());
            clear();

            t.stage = "grid metrics wubi code hint";
            configured.put("scheme", "wubi").put("wubi_profile", "wubi86").put("touch_wubi_layout", "twenty_six_key")
                .put("wubi_code_hint", true).put("wubi_auto_commit_unique", false)
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin").put("wubi"))
                    .put("selected", "wubi"));
            save();
            t.restartIme();
            editor();
            chinese();
            typeLetters("wq");
            // 五笔的剩余编码画在候选字下面另起一行：这一页带它时候选行加高，不沿用无提示的紧凑高度把它裁掉。
            Rect hinted = bounds(t.describedPrefix("候选 ").and(node -> node.getContentDescription() != null
                && node.getContentDescription().toString().contains("还需输入")));
            check(hinted.height() > plainChip, "五笔剩余编码出现时候选行没有加高：" + plainChip + " → " + hinted.height());
            clear();
        } catch (Exception | AssertionError failure) {
            t.shellDiscard("screencap -p /data/local/tmp/msime-grid-metrics-failure.png");
            throw failure;
        } finally {
            t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, original);
        }
    }

    /**
     * 截图量 14/18 键每个字母键的主字母墨迹：只认和键帽底色反差很大的像素（主字母是正文色，右上角角标是次要色，反差小得多），水平和垂直中心都要在键帽中心 1.5 dp 以内，墨迹高度记下来跨布局比较。带下伸部分的 Q 不比高度。
     */
    private void letterFaces(String layout, List<Integer> caps) throws Exception {
        SystemClock.sleep(600);
        List<AccessibilityNodeInfo> keys = new ArrayList<>();
        for (AccessibilityWindowInfo window : t.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD || window.getRoot() == null) continue;
            ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>();
            queue.add(window.getRoot());
            while (!queue.isEmpty()) {
                AccessibilityNodeInfo node = queue.poll();
                for (int index = 0; index < node.getChildCount(); index++)
                    if (node.getChild(index) != null) queue.add(node.getChild(index));
                CharSequence description = node.getContentDescription();
                if (description != null && (description.toString().startsWith("字母 ")
                        || description.toString().matches("按键 [A-Z] [A-Z]"))) keys.add(node);
            }
        }
        check(keys.size() >= 9, layout + " 没找到字母键：" + keys.size());
        Bitmap screen = t.automation.takeScreenshot();
        float density = t.getTargetContext().getResources().getDisplayMetrics().density;
        float tolerance = 1.5f * density;
        for (AccessibilityNodeInfo key : keys) {
            Rect bounds = new Rect();
            key.getBoundsInScreen(bounds);
            String face = String.valueOf(key.getText());
            Rect ink = ink(screen, bounds);
            check(!ink.isEmpty(), layout + " " + face + " 没有主字母墨迹");
            float dx = ink.exactCenterX() - bounds.exactCenterX();
            float dy = ink.exactCenterY() - bounds.exactCenterY();
            check(Math.abs(dx) <= tolerance && (face.contains("Q") || Math.abs(dy) <= tolerance),
                layout + " " + face + " 主字母不居中：dx=" + dx + " dy=" + dy);
            if (!face.contains("Q")) caps.add(ink.height());
        }
    }

    /**
     * 四副中文键盘的读音行分组：九键与 26 键一样高，14 键与 18 键一样高并高出读音行，四副都在打字时不变高；14/18 键的字母同字号且居中。
     */
    private void readingRows() throws Exception {
        int plain = -1;
        int grid = -1;
        List<Integer> caps = new ArrayList<>();
        for (String layout : new String[] {"twenty_six_key", "nine_key", "fourteen_key", "eighteen_key"}) {
            boolean readingRow = layout.equals("fourteen_key") || layout.equals("eighteen_key");
            t.stage = "grid metrics " + layout + " idle";
            configured.put("touch_keyboard_layout", layout);
            save();
            t.restartIme();
            editor();
            chinese();
            int idle = keyboardHeight();
            if (readingRow) {
                t.stage = "grid metrics " + layout + " letter faces";
                letterFaces(layout, caps);
            }
            t.stage = "grid metrics " + layout + " composing";
            typeLetters("ni");
            t.await(t.describedPrefix("候选 "));
            int composing = keyboardHeight();
            clear();
            check(Math.abs(composing - idle) <= 1, layout + " 打字时键盘高度变了：" + idle + " → " + composing);
            if (readingRow) {
                if (grid < 0) grid = idle;
                check(Math.abs(grid - idle) <= 1, "14 键和 18 键的键盘高度不同：" + grid + " / " + idle);
            } else {
                if (plain < 0) plain = idle;
                check(Math.abs(plain - idle) <= 1, "九键和 26 键的键盘高度不同：" + plain + " / " + idle);
            }
        }
        check(grid > plain, "14/18 键没有预留读音行：" + grid + " / " + plain);
        // 14 键和 18 键的主字母同一个字号：大写字母墨迹高度彼此相差不超过 2 px。
        Collections.sort(caps);
        check(!caps.isEmpty() && caps.get(caps.size() - 1) - caps.get(0) <= 2,
            "14/18 键主字母字号不一致，墨迹高度：" + caps);
    }

    /**
     * 键盘面板的高度：先把所有可点击的控件（工具栏、候选、按键）框在一起，再找框得住它们的最小节点，那就是面板，它的顶边算上空闲时预留、没有文字的读音行。不量输入法窗口：浮动和单手时窗口和外层容器比键盘大得多。
     */
    private int keyboardHeight() throws Exception {
        SystemClock.sleep(400);
        t.dropStaleNodes();
        List<Rect> nodes = new ArrayList<>();
        Rect controls = new Rect();
        for (AccessibilityWindowInfo window : t.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD || window.getRoot() == null) continue;
            ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>();
            queue.add(window.getRoot());
            while (!queue.isEmpty()) {
                AccessibilityNodeInfo node = queue.poll();
                for (int index = 0; index < node.getChildCount(); index++)
                    if (node.getChild(index) != null) queue.add(node.getChild(index));
                if (!node.isVisibleToUser()) continue;
                Rect bounds = new Rect();
                node.getBoundsInScreen(bounds);
                if (bounds.isEmpty()) continue;
                nodes.add(bounds);
                if (node.isClickable()) controls.union(bounds);
            }
        }
        check(!controls.isEmpty(), "量不到键盘的控件");
        Rect panel = null;
        for (Rect bounds : nodes)
            if (bounds.contains(controls) && (panel == null
                    || (long) bounds.width() * bounds.height() < (long) panel.width() * panel.height()))
                panel = bounds;
        check(panel != null, "找不到框住键盘控件的面板：" + controls);
        return controls.bottom - panel.top;
    }

    private static Rect ink(Bitmap screen, Rect key) {
        int inset = 4;
        int background = screen.getPixel(key.left + key.width() / 6, key.bottom - inset - 2);
        float base = luminance(background);
        Rect ink = new Rect();
        boolean any = false;
        for (int y = key.top + inset; y < key.bottom - inset; y++)
            for (int x = key.left + inset; x < key.right - inset; x++) {
                if (Math.abs(luminance(screen.getPixel(x, y)) - base) < 170) continue;
                if (!any) { ink.set(x, y, x + 1, y + 1); any = true; }
                else ink.union(x, y);
            }
        if (any) { ink.right++; ink.bottom++; }
        return ink;
    }

    private static float luminance(int color) {
        return 0.299f * Color.red(color) + 0.587f * Color.green(color) + 0.114f * Color.blue(color);
    }

    /**
     * 点出一串字母：按读屏描述找含有这个字母的键（26 键「字母 Q」，14/18 键「按键 W E」，九键「按键 6 MNO；…」）。不按键面文字找：九键左侧拼音栏的 ni、mi 也是字母，会被误认成键。
     */
    private void typeLetters(String letters) throws Exception {
        for (char letter : letters.toCharArray()) {
            String wanted = String.valueOf(Character.toUpperCase(letter));
            t.tap(node -> {
                if (!DeviceSmoke.equalsText("app.msime.android", node.getPackageName()) || !node.isClickable()
                        || node.getContentDescription() == null) return false;
                String description = node.getContentDescription().toString();
                int end = description.indexOf('；');
                String name = end < 0 ? description : description.substring(0, end);
                if (!name.startsWith("字母 ") && !name.startsWith("按键 ")) return false;
                String keyLetters = name.substring(3).replaceAll("[^A-Z]", "");
                return !keyLetters.isEmpty() && keyLetters.contains(wanted);
            });
            SystemClock.sleep(250);
        }
    }

    private void clear() throws Exception {
        for (int attempt = 0; attempt < 12; attempt++) {
            if (present(t.field("msime-test-plain").and(GridKeyMetricsDeviceChecks::empty))
                    && !present(t.describedPrefix("候选 "))) return;
            // 九键的删除键读作「按键 删除」，26/14/18 键读作「删除」。
            t.tap(t.described("删除").or(t.described("按键 删除")));
            SystemClock.sleep(200);
        }
        throw new AssertionError("清不掉组字");
    }

    private void chinese() throws Exception {
        t.await(language("中").or(language("英")));
        if (present(language("英"))) t.tap(language("英"));
        t.awaitStableBounds(language("中"));
    }

    private boolean present(Predicate<AccessibilityNodeInfo> match) {
        t.dropStaleNodes();
        for (AccessibilityWindowInfo window : t.automation.getWindows())
            if (t.find(window.getRoot(), match) != null) return true;
        return false;
    }

    private Predicate<AccessibilityNodeInfo> language(String face) {
        return t.key(face).and(node -> node.isClickable() && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("切换到"));
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect rect = new Rect();
        t.awaitStableBounds(match).getBoundsInScreen(rect);
        return rect;
    }

    private void editor() throws Exception {
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.dropStaleNodes();
    }

    private void save() throws Exception {
        if (preferences.exists()) revision = Math.max(revision, json(preferences).getLong("revision"));
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
    }

    private static JSONObject json(File path) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(path.toPath()), StandardCharsets.UTF_8));
    }

    private static boolean empty(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
