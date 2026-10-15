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
            assertCharacters(reveal(symbol("@protonmail.com")), "@protonmail.com");
            stage = "first recent panel after input method restart";
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            openPanel();
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
        await(described("符号分类 英文，已选中"));
        assertEmptyEditor();
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
        Bundle arguments = new Bundle();
        arguments.putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_START_INDEX, 0);
        arguments.putInt(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_ARG_LENGTH, expected.length());
        if (!node.refreshWithExtraData(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY, arguments))
            throw new AssertionError("Character locations unavailable");
        android.os.Parcelable[] characters = node.getExtras().getParcelableArray(AccessibilityNodeInfo.EXTRA_DATA_TEXT_CHARACTER_LOCATION_KEY);
        if (characters == null || characters.length != expected.length()) throw new AssertionError("Character locations missing");
        Rect key = new Rect();
        node.getBoundsInScreen(key);
        float previousRight = key.left;
        for (android.os.Parcelable character : characters) {
            if (!(character instanceof RectF bounds) || bounds.width() <= 0 || bounds.height() <= 0
                    || bounds.left < previousRight - 1.5f || bounds.right > key.right + 1.5f)
                throw new AssertionError("Suffix contains clipped or collapsed characters");
            previousRight = bounds.right;
        }
    }
}
