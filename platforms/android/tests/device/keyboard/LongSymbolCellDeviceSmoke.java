package app.msime.android.test;

import android.graphics.Rect;
import android.graphics.RectF;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在系统键盘检查底栏切类、连续输入和删除，并逐字检查长后缀的绘制与上屏。 */
public final class LongSymbolCellDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "bottom category navigation, swipe isolation, continuous insertion, deletion and complete long suffixes";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        android.content.SharedPreferences recents = getTargetContext().getSharedPreferences("android-symbol-recents", android.content.Context.MODE_PRIVATE);
        String originalRecents = recents.getString("items", null);
        try {
            if (!recents.edit().putString("items", "[]").commit()) throw new AssertionError("Cannot prepare synthetic recents");
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            openPanel();
            checkReturnBar();
            checkNavigation();
            reveal(describedPrefix("符号分类 网络"));
            tap(describedPrefix("符号分类 网络"));
            Rect shortKey = new Rect();
            awaitStableBounds(symbol("@")).getBoundsInScreen(shortKey);
            for (String suffix : new String[] {"@proton.me", "@protonmail.com", "@yahoo.com", "@yahoo.co.jp"}) {
                stage = "network full suffix " + suffix;
                AccessibilityNodeInfo key = reveal(symbol(suffix));
                Rect wideKey = new Rect();
                key.getBoundsInScreen(wideKey);
                if (wideKey.width() < shortKey.width() * 1.7f) throw new AssertionError("Suffix still uses a narrow symbol cell");
                assertCharacters(key, suffix);
            }
            stage = "full suffix commit";
            reveal(symbol("@protonmail.com"));
            tap(symbol("@protonmail.com"));
            await(field("msime-test-plain").and(node -> equalsText("@protonmail.com", node.getText())));
            await(described("符号面板"));
            tap(described("关闭符号面板"));
            stage = "recent suffix width";
            openPanel();
            tap(describedPrefix("符号分类 常用"));
            stage = "recent suffix character locations";
            assertCharacters(reveal(symbol("@protonmail.com")), "@protonmail.com");
            stage = "first recent panel after input method restart";
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            openPanel();
            stage = "restarted recent suffix character locations";
            assertCharacters(reveal(symbol("@protonmail.com")), "@protonmail.com");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            android.content.SharedPreferences.Editor restore = recents.edit();
            if (originalRecents == null) restore.remove("items"); else restore.putString("items", originalRecents);
            if (!restore.commit()) throw new AssertionError("Cannot restore recents");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            restartIme();
        }
    }

    private Predicate<AccessibilityNodeInfo> symbol(String value) { return described("按键 符号 " + value); }

    /** 经系统触摸输入检查返回条两侧装饰和中间文字都能关闭面板，不会顺带输入底层字母。 */
    private void checkReturnBar() throws Exception {
        for (float position : new float[] {0.03f, 0.5f, 0.97f}) {
            stage = "symbol return bar at " + position;
            Rect header = bounds(described("关闭符号面板"));
            AccessibilityNodeInfo button = await(described("关闭符号面板"));
            if (!equalsText("返回上一界面", button.getText()))
                throw new AssertionError("返回条缺少明确文案");
            long now = SystemClock.uptimeMillis();
            for (int action : new int[] {android.view.MotionEvent.ACTION_DOWN, android.view.MotionEvent.ACTION_UP}) {
                android.view.MotionEvent event = android.view.MotionEvent.obtain(now, now + 30, action,
                    header.left + header.width() * position, header.centerY(), 0);
                event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
                try {
                    if (!automation.injectInputEvent(event, true)) throw new AssertionError("返回栏触摸未送达");
                } finally { event.recycle(); }
            }
            awaitGone(described("符号面板"));
            await(key("q"));
            assertEmptyEditor();
            openPanel();
            await(described("符号面板"));
            assertEmptyEditor();
        }
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            boolean visible = false;
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
                visible |= find(window.getRoot(), match) != null;
            if (!visible) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("点击返回条后符号面板没有关闭");
    }

    private void checkNavigation() throws Exception {
        stage = "symbol panel bottom geometry";
        tap(describedPrefix("符号分类 中文"));
        Rect panel = bounds(described("符号面板"));
        Rect grid = bounds(described("符号网格；长条目加宽"));
        Rect category = bounds(describedPrefix("符号分类 中文"));
        Rect delete = bounds(described("删除").and(this::inSymbolPanel));
        Rect back = bounds(described("关闭符号面板"));
        if (grid.width() < panel.width() * 0.95f || category.top < grid.bottom
                || delete.top < grid.bottom || back.bottom > grid.top)
            throw new AssertionError("Symbol layout bounds: panel " + panel + ", grid " + grid
                + ", category " + category + ", delete " + delete + ", back " + back);

        stage = "real horizontal grid swipe";
        shellDiscard("input swipe " + (grid.left + grid.width() * 4 / 5) + " " + grid.centerY()
            + " " + (grid.left + grid.width() / 5) + " " + grid.centerY() + " 250");
        await(described("符号分类 偏旁，已选中"));
        checkRadicalNavigation();
        grid = bounds(described("偏旁部首网格；按笔画连续滚动"));
        shellDiscard("input swipe " + (grid.left + grid.width() * 4 / 5) + " " + grid.centerY()
            + " " + (grid.left + grid.width() / 5) + " " + grid.centerY() + " 250");
        await(described("符号分类 英文，已选中"));
        assertEmptyEditor();
        grid = bounds(described("符号网格；长条目加宽"));
        stage = "real vertical grid swipe";
        shellDiscard("input swipe " + grid.centerX() + " " + (grid.top + grid.height() * 4 / 5)
            + " " + grid.centerX() + " " + (grid.top + grid.height() / 5) + " 250");
        await(described("符号分类 英文，已选中"));
        assertEmptyEditor();

        stage = "bottom category scrolling reaches the last category";
        Predicate<AccessibilityNodeInfo> bar = described("符号分类栏，左右滑动查看更多分类");
        for (int index = 0; index < 5; index++) {
            await(bar).performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(200);
        }
        await(describedPrefix("符号分类 更多"));
        tap(describedPrefix("符号分类 更多"));
        await(described("符号分类 更多，已选中"));
        Rect deleteAfter = bounds(described("删除").and(this::inSymbolPanel));
        if (!delete.equals(deleteAfter)) throw new AssertionError("Delete moved with the category strip");
        assertEmptyEditor();

        stage = "continuous symbol insertion and fixed delete";
        reveal(describedPrefix("符号分类 数字"));
        tap(describedPrefix("符号分类 数字"));
        tap(symbol("1"));
        tap(symbol("2"));
        await(field("msime-test-plain").and(node -> equalsText("12", node.getText())));
        await(described("符号面板"));
        tap(described("删除").and(this::inSymbolPanel));
        await(field("msime-test-plain").and(node -> equalsText("1", node.getText())));
        tap(described("删除").and(this::inSymbolPanel));
        assertEmptyEditor();
    }


    private void checkRadicalNavigation() throws Exception {
        stage = "radical continuous scroll and linked stroke rail";
        Rect grid = bounds(described("偏旁部首网格；按笔画连续滚动"));
        Rect rail = bounds(described("偏旁笔画分类"));
        if (grid.height() <= 0 || rail.right > grid.left || rail.height() != grid.height())
            throw new AssertionError("偏旁笔画栏与网格尺寸错误");
        stage = "radical initial one-stroke selection";
        await(described("偏旁笔画 1画，已选中"));
        captureRadicals("first");
        // 首组可能高于一次短划的距离，连续划动跨过分组后再核对联动。
        for (int swipe = 0; swipe < 3; swipe++) {
            swipeRadicals(grid);
        }
        captureRadicals("scrolled");
        stage = "radical scroll changes the selected stroke";
        await(describedPrefix("偏旁笔画 ").and(node -> node.getContentDescription().toString().endsWith("画，已选中")
            && !equalsText("偏旁笔画 1画，已选中", node.getContentDescription())));
        await(described("符号分类 偏旁，已选中"));
        assertEmptyEditor();
        stage = "radical last stroke and commit";
        reveal(describedPrefix("偏旁笔画 17画"));
        tap(describedPrefix("偏旁笔画 17画"));
        await(described("偏旁笔画 17画，已选中"));
        tap(symbol("龠"));
        await(field("msime-test-plain").and(node -> equalsText("龠", node.getText())));
        await(described("符号面板"));
        tap(described("删除").and(this::inSymbolPanel));
        assertEmptyEditor();
        stage = "radical first stroke and primary category swipe";
        reveal(describedPrefix("偏旁笔画 1画"));
        tap(describedPrefix("偏旁笔画 1画"));
        await(described("偏旁笔画 1画，已选中"));
        await(symbol("一"));
    }

    /** 和表情面板设备用例一样，经系统输入分发注入完整触摸轨迹。 */
    private void swipeRadicals(Rect grid) {
        long downTime = SystemClock.uptimeMillis();
        for (int step = 0; step <= 12; step++) {
            int action = step == 0 ? android.view.MotionEvent.ACTION_DOWN
                : step == 12 ? android.view.MotionEvent.ACTION_UP : android.view.MotionEvent.ACTION_MOVE;
            float y = grid.top + grid.height() * (.8f - .6f * step / 12);
            android.view.MotionEvent event = android.view.MotionEvent.obtain(
                downTime, downTime + step * 20, action, grid.centerX(), y, 0);
            event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
            try {
                if (!automation.injectInputEvent(event, true)) throw new AssertionError("偏旁触摸轨迹未送达");
            } finally {
                event.recycle();
            }
        }
        SystemClock.sleep(300);
    }

    /** 只保存专用合成编辑器的验收画面，方便检查实际皮肤和笔画栏。 */
    private void captureRadicals(String position) throws java.io.IOException {
        android.graphics.Bitmap image = automation.takeScreenshot();
        if (image == null) throw new AssertionError("没有取得偏旁验收画面");
        try (var output = new java.io.FileOutputStream(new File(getTargetContext().getExternalFilesDir(null),
                "radicals-api" + android.os.Build.VERSION.SDK_INT + "-" + position + ".png"))) {
            if (!image.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("偏旁验收画面写入失败");
        } finally {
            image.recycle();
        }
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
    }

    private boolean inSymbolPanel(AccessibilityNodeInfo node) {
        for (AccessibilityNodeInfo parent = node.getParent(); parent != null; parent = parent.getParent()) {
            if (equalsText("符号面板", parent.getContentDescription())) return true;
        }
        return false;
    }

    private void assertEmptyEditor() {
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
    }

    private void openPanel() {
        stage = "open symbol panel";
        AccessibilityNodeInfo layer = await(described("切换到数字和符号"));
        if (!layer.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK)) throw new AssertionError("Symbol panel hold failed");
        await(described("符号面板"));
    }

    private AccessibilityNodeInfo reveal(Predicate<AccessibilityNodeInfo> match) {
        AccessibilityNodeInfo node = awaitAny(match);
        node.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        SystemClock.sleep(400);
        return awaitStableBounds(match);
    }

    @SuppressWarnings("deprecation")
    private void assertCharacters(AccessibilityNodeInfo node, String expected) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        String failure = "Character locations unavailable";
        while (SystemClock.uptimeMillis() < deadline) {
            Bundle arguments = new Bundle();
            arguments.putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_START_INDEX, 0);
            arguments.putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_LENGTH, expected.length());
            if (node.refreshWithExtraData(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY, arguments)) {
                android.os.Parcelable[] characters = node.getExtras().getParcelableArray(
                    AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY);
                if (characters == null || characters.length != expected.length()) {
                    failure = "Character locations missing";
                } else {
                    Rect key = new Rect();
                    node.getBoundsInScreen(key);
                    float previousRight = key.left;
                    boolean valid = true;
                    for (android.os.Parcelable character : characters) {
                        if (!(character instanceof RectF bounds) || bounds.width() <= 0 || bounds.height() <= 0
                                || bounds.left < previousRight - 1.5f || bounds.right > key.right + 1.5f) {
                            valid = false;
                            break;
                        }
                        previousRight = bounds.right;
                    }
                    if (valid) return;
                    failure = "Suffix contains clipped or collapsed characters";
                }
            }
            SystemClock.sleep(100);
        }
        throw new AssertionError(failure);
    }
}
