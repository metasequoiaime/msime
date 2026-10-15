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

/** 在真实输入法面板逐字检查后缀的绘制位置，并验证宽键仍上屏完整值。 */
public final class LongSymbolCellDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "network and recent suffix keys show distinct complete domains and commit the complete suffix";
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

    // 旧 Android 的布局重建不总是清掉无障碍子树缓存，查询之前刷新节点。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }
}
