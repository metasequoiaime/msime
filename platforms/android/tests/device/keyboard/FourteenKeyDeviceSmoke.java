package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 全拼 14 键的设备验收：组码打出「你好」；键区左侧纵向选择 shi / shu，锁定后退格撤销；窄屏侧栏；展开候选、分词、长按字母、英文、123 和密码回落维持原有行为。
 */
public final class FourteenKeyDeviceSmoke extends DeviceSmoke {
    private boolean shuangpinOnly;

    @Override public void onCreate(android.os.Bundle arguments) {
        shuangpinOnly = arguments != null && "true".equals(arguments.getString("shuangpin_only"));
        super.onCreate(arguments);
    }

    private static final String STRIP = "14 键拼音选择";
    private static final String PANEL = "九键候选面板";

    @Override protected String successDescription() {
        return shuangpinOnly ? "14 键双拼设置、组码、消歧、分号、自定义表与布局回落"
            : "14 键全拼与双拼的完整键面、侧栏、候选、分词、长按和布局回落";
    }

    @Override protected void runChecks() throws Exception {
        if (shuangpinOnly) {
            FourteenKeyShuangpinDeviceChecks.run(this);
            return;
        }
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "fourteen_key")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin").put("fourteen_key"))
                .put("selected", "fourteen_key"));
        try {
            stage = "enable shared fourteen-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            // 键面可以先于会话准备好；等待语言键可点击，再确认上一轮没有留下英文模式。
            Predicate<AccessibilityNodeInfo> language = node -> node.isClickable()
                && node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("切换到");
            await(key("中").or(key("英")).and(language));
            if (findKey(key("英").and(language))) tap(key("英").and(language));
            awaitStableBounds(key("中").and(language));

            stage = "fourteen-key face";
            await(described("按键 Q W"));
            await(described("字母 L"));
            await(described("字母 M"));
            await(described("按键 符号"));
            await(described("符号栏，可上下滑动"));
            Rect idleLetter = new Rect();
            awaitStableBounds(described("按键 Q W")).getBoundsInScreen(idleLetter);
            Rect idleSpace = new Rect();
            awaitStableBounds(key("空格")).getBoundsInScreen(idleSpace);
            int heightTolerance = Math.round(getTargetContext().getResources()
                .getDisplayMetrics().density) + 2;
            if (Math.abs(idleLetter.height() - idleSpace.height()) > heightTolerance)
                throw new AssertionError("Fourteen-key bottom row height differs from its letter rows");
            // 中文 14 键没有 Shift，也没有 26 键的单字母键。
            awaitGone(key("q"));

            // 「你好」：BN UI GH AS OP，组码 b u g a o。
            stage = "nihao through the group keys";
            for (String pair : new String[] {"B N", "U I", "G H", "A S", "O P"}) tap(described("按键 " + pair));
            // 组字是组码字母，不写进输入框；读音行显示引擎首选的读音（BN UI 也是 bu，首选可能是「不好」），不是组码 bugao。
            // 读音文字与编辑拼写动作一并校验，不混用描述和文字。
            await(reading("ni'hao").or(reading("bu'hao")));
            awaitGone(reading("bugao"));
            stage = "group composition leaves editor empty";
            await(field("msime-test-plain").and(FourteenKeyDeviceSmoke::emptyField));
            stage = "nihao commit";
            tap(candidate("你好"));
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            // AS GH UI：左侧栏列出 shi 和 shu，读音与候选行保持独立；选 shu 锁定，退格先撤销锁定。
            stage = "left-side pinyin column";
            for (String pair : new String[] {"A S", "G H", "U I"}) tap(described("按键 " + pair));
            await(described(STRIP));
            awaitGone(described("符号栏，可上下滑动"));
            await(described("选择拼音 shu"));
            // 组字一开始，候选条和左侧栏还在布局；每个尺寸都等停稳再量，否则会读到过渡中的字母键顶边，误报区域高度变了（Release Android run 38669833730，API 35）。
            Rect composingSpace = new Rect();
            awaitStableBounds(key("空格")).getBoundsInScreen(composingSpace);
            Rect letter = new Rect();
            awaitStableBounds(described("按键 Q W")).getBoundsInScreen(letter);
            Rect strip = new Rect();
            awaitStableBounds(described("选择拼音 shi")).getBoundsInScreen(strip);
            Rect sidebar = new Rect();
            awaitStableBounds(described(STRIP)).getBoundsInScreen(sidebar);
            String measured = "idle letter " + idleLetter + ", idle space " + idleSpace + ", letter " + letter
                + ", space " + composingSpace + ", tolerance " + heightTolerance;
            if (letter.left != idleLetter.left || letter.width() != idleLetter.width())
                throw new AssertionError("Starting composition moved the fourteen-key letter grid: " + measured);
            if (Math.abs((composingSpace.bottom - letter.top)
                    - (idleSpace.bottom - idleLetter.top)) > heightTolerance)
                throw new AssertionError("Pinyin sidebar changed the fourteen-key area height: " + measured);
            Rect candidate = new Rect();
            await(describedPrefix("候选 1：")).getBoundsInScreen(candidate);
            if (sidebar.right > letter.left || sidebar.top < candidate.bottom - 1)
                throw new AssertionError("Pinyin column is not beside the letter keys: sidebar " + sidebar
                    + ", letter " + letter + ", candidate " + candidate);
            float density = getTargetContext().getResources().getDisplayMetrics().density;
            int minimumHeight = Math.round(36 * density);
            if (strip.height() < minimumHeight)
                throw new AssertionError("Pinyin row is smaller than the nine-key row: " + strip);
            stage = "lock shu";
            tap(described("选择拼音 shu"));
            await(reading("shu"));
            stage = "backspace undoes the lock";
            tap(key("⌫"));
            await(reading("shi"));
            await(described("选择拼音 shu"));

            // 展开候选用九键的三栏面板：左栏是拼音，单字筛选照常可用；返回收起面板，组字还在。
            stage = "open the three-column panel";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            await(inPanel(described("选择拼音 shu")));
            await(inPanel(described("只显示单字")));
            stage = "close the three-column panel";
            tap(inPanel(described("收起候选面板")));
            awaitGone(described(PANEL));
            await(described("选择拼音 shu"));
            stage = "cancel the composition";
            tap(key("⌫"));
            tap(key("⌫"));
            tap(key("⌫"));
            awaitGone(described(STRIP));
            await(described("按键 符号"));
            await(described("符号栏，可上下滑动"));

            // 分词键：ZX UI 分词 AS BN 是 z u ' a b，得到「西安」而不是「先」。
            stage = "separator";
            tap(described("按键 Z X"));
            tap(described("按键 U I"));
            tap(described("按键 分词，在这里断开音节"));
            tap(described("按键 A S"));
            tap(described("按键 B N"));
            await(reading("xi'an"));
            tap(candidate("西安"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("你好西安")));

            // 长按 QW 弹出 q 和 w（没有数字），选 w 上屏字面字母。
            stage = "long-press letters";
            AccessibilityNodeInfo qw = await(described("按键 Q W"));
            if (!qw.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("Long press on QW failed");
            await(described("输入 q"));
            tap(described("输入 w"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("西安w")));
            AccessibilityNodeInfo l = await(described("字母 L"));
            if (l.isLongClickable()) throw new AssertionError("L must not offer a long press");

            // 英文画 26 键 QWERTY，切回中文仍是 14 键，`touch_keyboard_layout` 不变。
            stage = "English draws 26 keys";
            tap(key("中"));
            await(key("q").and(AccessibilityNodeInfo::isClickable));
            awaitGone(described("按键 Q W"));
            awaitGone(described(STRIP));
            stage = "Chinese returns to 14 keys";
            tap(key("英"));
            await(described("按键 Q W"));
            awaitGone(key("q"));
            JSONObject stored = new JSONObject(new String(Files.readAllBytes(preferences.toPath()),
                StandardCharsets.UTF_8)).getJSONObject("preferences");
            if (!"fourteen_key".equals(stored.optString("touch_keyboard_layout")))
                throw new AssertionError("English rewrote the stored layout");

            // 123 默认数字九宫格；扩展符号页保留标点开关，返回仍回到 14 键。
            stage = "123 layer";
            tap(key("123"));
            await(key("1").and(AccessibilityNodeInfo::isClickable));
            await(key("#+="));
            tap(key("#+="));
            stage = "number layer punctuation toggle";
            await(key("中标"));
            tap(key("中标"));
            await(key("英标"));
            tap(key("英标"));
            await(key("中标"));
            tap(described("切换到字母键盘"));
            await(described("按键 Q W"));

            // 密码框画 26 键。
            stage = "password field falls back to 26 keys";
            tap(field("msime-test-password"));
            await(field("msime-test-password").and(AccessibilityNodeInfo::isFocused));
            await(key("q").and(AccessibilityNodeInfo::isClickable));
            awaitGone(described("按键 Q W"));
            stage = "plain field returns to 14 keys";
            tap(field("msime-test-plain"));
            await(described("按键 Q W"));
            // 专用 AVD 临时缩窄为约 183dp，验证拼音仍在左侧纵向排列，不改用户设备的偏好。
            stage = "pinyin column on a narrow display";
            shellDiscard("wm size 480x1600");
            try {
                // 改显示尺寸会重建编辑器并收起输入法，先在新配置下重新获得焦点。
                openEditor();
                dropStaleNodes();
                stage = "narrow display editor focus";
                tap(field("msime-test-plain"));
                await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
                stage = "narrow display fourteen-key face";
                awaitStableBounds(described("按键 A S"));
                stage = "narrow display pinyin column";
                tap(described("按键 A S"));
                tap(described("按键 U I"));
                Rect narrowLetter = new Rect();
                awaitStableBounds(described("按键 Q W")).getBoundsInScreen(narrowLetter);
                int columnLeft = -1;
                int firstTop = Integer.MAX_VALUE;
                int lastTop = Integer.MIN_VALUE;
                for (String spelling : new String[] {"ai", "si", "su"}) {
                    Rect bounds = new Rect();
                    awaitStableBounds(described("选择拼音 " + spelling)).getBoundsInScreen(bounds);
                    if (bounds.height() < minimumHeight || bounds.width() <= 0
                            || bounds.right > narrowLetter.left)
                        throw new AssertionError("Narrow pinyin column changed: " + bounds);
                    if (columnLeft >= 0 && columnLeft != bounds.left)
                        throw new AssertionError("Pinyin options are not in one column");
                    columnLeft = bounds.left;
                    firstTop = Math.min(firstTop, bounds.top);
                    lastTop = Math.max(lastTop, bounds.top);
                }
                if (lastTop <= firstTop)
                    throw new AssertionError("Pinyin options are not stacked vertically");
                tap(described("选择拼音 su"));
                await(reading("su"));
            } finally {
                shellDiscard("wm size reset");
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
        FourteenKeyShuangpinDeviceChecks.run(this);
    }

    // API 28 没有 clearCache；逐节点刷新，避免组字重建后仍点击已经拆掉的键面。
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && android.os.Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private boolean findKey(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows())
            if (find(window.getRoot(), match) != null) return true;
        return false;
    }

    private static boolean emptyField(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }

    /** 读音文字与「编辑拼写」动作描述分别校验，避免误匹配侧栏拼音。 */
    private Predicate<AccessibilityNodeInfo> reading(String text) {
        return described("编辑拼写").and(node -> equalsText(text, node.getText()));
    }

    /** 候选条上写着 `text` 的候选（描述是「候选 N：text；…」）。 */
    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    /** 三栏面板里的节点。 */
    private Predicate<AccessibilityNodeInfo> inPanel(Predicate<AccessibilityNodeInfo> match) {
        return match.and(node -> {
            for (AccessibilityNodeInfo parent = node.getParent(); parent != null;
                    parent = parent.getParent()) {
                if (equalsText(PANEL, parent.getContentDescription())) return true;
            }
            return false;
        });
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (find(window.getRoot(), match) != null) {
                    present = true;
                    break;
                }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic UI state did not go away; IME showed " + imeTexts());
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }


}
