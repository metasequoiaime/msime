package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 实测九宫格宽高比例，并通过真实输入连接验收数字底行、右列空格和键面往返。 */
public final class NineKeyNumericLayoutDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key measured proportions, height allocation, numeric zero, symbol entry, space ownership and layer restoration; tall-return frame";
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
        File local = new File(new File(new File(root, "bootstrap"), "state"), "android-settings.json");
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key")
            .put("touch_key_spacing_tenths", 60).put("touch_row_spacing_tenths", 70)
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable synthetic nine-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            checkGeometry(local);
            publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject()).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            selectChineseKeyboard();
            await(described("打开符号面板").and(AccessibilityNodeInfo::isEnabled));
            String expected = "";
            for (int cycle = 0; cycle < 2; cycle++) {
                stage = "numeric geometry cycle " + cycle;
                Rect letterSymbol = bounds(key("符"));
                Rect digitsEntry = bounds(key("123"));
                tap(key("123"));
                Rect symbol = bounds(key("符"));
                Rect returnToLetters = bounds(key("九键"));
                Rect zero = bounds(key("0"));
                Rect eight = bounds(key("8"));
                Rect space = bounds(describedPrefix("空格"));
                Rect nine = bounds(key("9"));
                if (zero.top < eight.bottom
                        || !(zero.left < eight.centerX() && zero.right > eight.centerX())
                        || space.left < nine.right || space.centerY() != nine.centerY())
                    throw new AssertionError("0 must sit below 8, with the original space key beside 9");
                if (symbol.left != letterSymbol.left || symbol.right != letterSymbol.right
                        || returnToLetters.left != digitsEntry.left
                        || returnToLetters.right != digitsEntry.right)
                    throw new AssertionError("数字层的符号入口与返回入口应保持切层前的横向位置");
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
            if (originalLocal == null) Files.deleteIfExists(local.toPath());
            else publish(local, originalLocal);
        }
        // 九键「右侧大回车」样式的外框（清单已到 instrumentation 上限，不另加入口）。
        TallReturnDeviceChecks.run(this);
    }

    /** 量真实键帽及其间距，防止只改常量却漏接布局或底行的高度分摊。 */
    private void checkGeometry(File local) throws Exception {
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        int horizontalMargin = Math.round(3 * density);
        int verticalMargin = Math.round(3.5f * density);
        int tolerance = Math.round(2 * density) + 3;
        for (int adjustment : new int[] {0, -46, 55, 129}) {
            stage = "nine-key measured geometry at " + adjustment;
            publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.bottom_bar", false)
                .put("platform.android.keyboard_height_adjustment", adjustment)).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            if (adjustment == 0) {
                // 确定性前提对照：先真正进入英文，防止用空白设备的默认中文掩盖跨套件模式残留。
                AccessibilityNodeInfo language = await(key("中").or(key("英"))
                    .and(AccessibilityNodeInfo::isEnabled).and(AccessibilityNodeInfo::isClickable));
                if (equalsText("中", language.getText())) tap(key("中"));
                await(key("英"));
            }
            selectChineseKeyboard();
            for (boolean digits : new boolean[] {false, true}) {
                if (digits) tap(key("123"));
                Rect first = bounds(key(digits ? "1" : "@#"));
                Rect last = bounds(key(digits ? "3" : "DEF"));
                Rect bottom = bounds(key(digits ? "9" : "WXYZ"));
                Rect left = bounds(described("符号栏，可上下滑动"));
                Rect right = bounds(described("按键 删除"));
                Rect action = bounds(key(digits ? "0" : "符"));
                int outerWidth = right.right + horizontalMargin - left.left;
                int gridWidth = last.right - first.left + 2 * horizontalMargin;
                if (Math.abs(left.width() - outerWidth * 0.15f) > 3
                        || Math.abs(right.width() + 2 * horizontalMargin - outerWidth * 0.15f) > 3
                        || Math.abs(gridWidth - outerWidth * 0.70f) > 3)
                    throw new AssertionError("九宫格外框必须为 15% / 70% / 15%："
                        + left.width() + "/" + gridWidth + "/" + right.width() + "，总宽 " + outerWidth);
                int windowDp = getTargetContext().getResources().getConfiguration().screenHeightDp;
                int actual = adjustment <= 55 ? adjustment : Math.min(adjustment, Math.max(55, windowDp * 17 / 100));
                int bottomShare = actual > 0 ? 0 : Math.floorDiv(actual, 4);
                int expectedBottom = Math.round((52 + bottomShare) * density);
                int expectedGrid = Math.round((156 + actual - bottomShare) * density) + 4 * verticalMargin;
                if (Math.abs(action.height() - expectedBottom) > tolerance
                        || Math.abs(bottom.bottom - first.top - expectedGrid) > tolerance)
                    throw new AssertionError("九宫格增量应全部留给上方键区：底行 " + action.height()
                        + " / " + expectedBottom + "，网格 " + (bottom.bottom - first.top) + " / " + expectedGrid);
                int expectedSpan = Math.round((208 + actual) * density) + 6 * verticalMargin;
                if (Math.abs(action.bottom - first.top - expectedSpan) > tolerance)
                    throw new AssertionError("九宫格切层不能改变键区总高");
                System.out.println("geometry adjustment=" + adjustment + " digits=" + digits
                    + " grid_width=" + gridWidth + " outer_width=" + outerWidth
                    + " grid_height=" + (bottom.bottom - first.top) + " bottom_height=" + action.height());
                if (digits) tap(key("九键"));
            }
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
