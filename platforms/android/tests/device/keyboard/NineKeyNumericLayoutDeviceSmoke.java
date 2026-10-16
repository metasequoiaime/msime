package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 通过真实输入连接验收九键数字底行、右列空格和键面往返。 */
public final class NineKeyNumericLayoutDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key numeric zero, symbol entry, space ownership and layer restoration";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key")
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable synthetic nine-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            await(described("打开符号面板").and(AccessibilityNodeInfo::isEnabled));
            String expected = "";
            for (int cycle = 0; cycle < 2; cycle++) {
                stage = "numeric geometry cycle " + cycle;
                tap(key("123"));
                Rect zero = bounds(key("0"));
                Rect eight = bounds(key("8"));
                Rect space = bounds(describedPrefix("空格"));
                Rect nine = bounds(key("9"));
                if (zero.top < eight.bottom
                        || !(zero.left < eight.centerX() && zero.right > eight.centerX())
                        || space.left < nine.right || space.centerY() != nine.centerY())
                    throw new AssertionError("0 must sit below 8, with the original space key beside 9");
                stage = "literal numeric, space and decimal commit cycle " + cycle;
                for (String digit : new String[] {"1", "0"}) tap(key(digit));
                tap(describedPrefix("空格"));
                tap(key("."));
                expected += "10 .";
                final String committed = expected;
                await(field("msime-test-plain").and(node -> equalsText(committed, node.getText())));
                stage = "numeric symbol panel and ownership cycle " + cycle;
                tap(described("打开符号面板").and(AccessibilityNodeInfo::isEnabled));
                stage = "numeric symbol panel opens cycle " + cycle;
                await(described("符号面板"));
                stage = "numeric symbol panel closes cycle " + cycle;
                tap(described("关闭符号面板"));
                await(key("0"));
                if (bounds(describedPrefix("空格")).left < nine.right)
                    throw new AssertionError("Space ownership changed after closing the symbol panel");
                stage = "restore letter layer cycle " + cycle;
                tap(key("九键"));
                await(key("@#").and(AccessibilityNodeInfo::isLongClickable));
                if (bounds(describedPrefix("空格")).top < bounds(key("WXYZ")).bottom)
                    throw new AssertionError("Letter layer must restore space to the bottom row");
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    // 旧系统没有 UiAutomation.clearCache：换页后缓存可能仍给出原来的字母网格。
    // 每次遍历刷新节点，以产品当前层级断言；API 34 起复用基类的缓存清理。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node,
            Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node,
            Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
    }
}
