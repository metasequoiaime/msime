package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 「键盘调整」遮罩（高度、底部抬高、宽度、位置的预览与保存）、键区下的底部留白，以及底行与字母键同高的设备验收。 */
public final class KeyboardHeightDeviceSmoke extends DeviceSmoke {
    /** {@code AndroidLocalSettings.FILE_NAME} 与 {@code KEYBOARD_HEIGHT_ADJUSTMENT}；设备套件不编译宿主的设置类，这里写同样的字面值。 */
    private static final String LOCAL_SETTINGS_FILE = "android-settings.json";
    private static final String HEIGHT_SETTING = "platform.android.keyboard_height_adjustment";
    /** {@code AndroidLocalSettings.BOTTOM_BAR} 与 {@code BOTTOM_PADDING}，同样写字面值。 */
    private static final String BOTTOM_BAR_SETTING = "platform.android.bottom_bar";
    private static final String BOTTOM_PADDING_SETTING = "platform.android.bottom_padding";
    private static final String BOTTOM_PADDING_HEIGHT_SETTING = "platform.android.bottom_padding_height";
    /** {@code AndroidLocalSettings.KEYBOARD_WIDTH_PERCENT} 与 {@code KEYBOARD_POSITION_X}。 */
    private static final String WIDTH_SETTING = "platform.android.keyboard_width_percent";
    private static final String POSITION_SETTING = "platform.android.keyboard_position_x";
    /** 遮罩各部件的读屏描述（{@code KeyboardAdjustOverlay}）：整块、上下左右四个柄、三个按钮。 */
    private static final String ADJUST_OVERLAY = "键盘调整";
    private static final String TOP_HANDLE = "键盘高度";
    private static final String BOTTOM_HANDLE = "键盘底部高度";
    private static final String LEFT_HANDLE = "键盘宽度（左边）";
    private static final String RIGHT_HANDLE = "键盘宽度（右边）";
    private static final String MOVE_BUTTON = "移动";

    /** 功能面板和输入方式面板逐档检查的键盘高度（%），覆盖用户报告的 90% 和它两边各档。 */
    private static final int[] PANEL_FIT_PERCENTS = {75, 80, 85, 90, 95, 100, 110};
    /** {@code PagedTileGrid.PICKER_MIN_TILE_DP}：磁贴缩小的下限，再矮就退回两行。 */
    private static final float PANEL_MIN_TILE_DP = 44f;

    private boolean portraitOnly;
    private boolean panelFitOnly;
    private String panelShotPrefix;

    @Override public void onCreate(Bundle arguments) {
        portraitOnly = arguments != null && "true".equals(arguments.getString("portrait-split"));
        panelFitOnly = arguments != null && "true".equals(arguments.getString("panel-fit"));
        panelShotPrefix = arguments == null ? null : arguments.getString("panel-shots");
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (portraitOnly) return "portrait split live setting, native control and narrow cover window fallback";
        if (panelFitOnly) return "function panel and input-scheme picker fit without vertical scrolling at every keyboard height";
        return "keyboard adjust overlay: height, bottom lift, width, position, reset, confirm and save on hide; editor frame fixed while adjusting; persistence, restart, bottom padding, panels that fit every height and a bottom row as tall as the letter rows";
    }

    @Override protected void runChecks() throws Exception {
        PortraitSplitDeviceChecks.run(this);
        if (portraitOnly) return;
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject snapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", new JSONObject(options.getJSONObject("preferences").toString())
                .put("touch_keyboard_height_adjustment", 0));
        // 设计范围的键盘高度（75%..170%）存在 Android 本地设置里，不在共享偏好里。
        File localSettings = new File(new File(new File(root, "bootstrap"), "state"), LOCAL_SETTINGS_FILE);
        byte[] originalLocal = localSettings.exists() ? Files.readAllBytes(localSettings.toPath()) : null;
        JSONObject localBaseline = new JSONObject().put("version", 1).put("settings", new JSONObject());
        try {
            stage = "baseline preferences";
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            if (!localSettings.getParentFile().isDirectory() && !localSettings.getParentFile().mkdirs())
                throw new AssertionError("Local settings directory unavailable");
            publish(localSettings, localBaseline.toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            stage = "baseline keyboard height";
            int standard = keyHeight("n");

            checkPanelsFit(localSettings);
            if (panelFitOnly) return;

            // 键盘调整 is a function-panel tile; it lays the adjust overlay over the whole keyboard, which previews while the handles are walked and saves on 确认 or any other way out. The panel is reachable only from an idle keyboard, so each visit starts from the brand key.
            stage = "open keyboard adjust";
            openHeightBar();
            stage = "increase keyboard height";
            setHeight(130);
            confirm();
            awaitHeightSetting(localSettings, 55);
            int tall = keyHeight("n");
            int minimumDelta = Math.round(8 * getTargetContext()
                .getResources().getDisplayMetrics().density);
            if (tall < standard + minimumDelta)
                throw new AssertionError("Taller keyboard did not enlarge key faces: "
                    + standard + " -> " + tall + ", expected delta " + minimumDelta);

            // 上沿柄只在窗口封顶以内可调：模拟器窗口够高时到 170%（129 dp），矮窗口停在封顶处，总比 130% 高。
            stage = "increase keyboard height to the window's maximum";
            openHeightBar();
            int top = Math.round(await(heightSlider()).getRangeInfo().getMax());
            setHeight(top);
            confirm();
            if (top >= 170) awaitHeightSetting(localSettings, 129);
            else awaitLocalSetting(localSettings, HEIGHT_SETTING, value -> value > 55, "a height above 130%");
            int maximum = keyHeight("n");
            if (top > 130 && maximum <= tall)
                throw new AssertionError(top + "% did not enlarge key faces beyond 130%: " + tall + " -> " + maximum);

            stage = "decrease keyboard height";
            openHeightBar();
            setHeight(75);
            confirm();
            awaitHeightSetting(localSettings, -46);
            int shortHeight = keyHeight("n");
            if (shortHeight >= standard)
                throw new AssertionError("Shorter keyboard did not shrink key faces: "
                    + standard + " -> " + shortHeight);

            // 没有「取消」：不点「确认」就按返回键收起键盘，照「确认」保存。110% 是 18 dp（KeyboardHeightPolicy.heightPercentToAdjustment）。
            stage = "hiding mid-adjustment keeps the height";
            openHeightBar();
            if (findInIme(described("取消")) != null)
                throw new AssertionError("The keyboard adjust overlay must not offer a cancel button");
            setHeight(110);
            shellDiscard("input keyevent KEYCODE_BACK");
            awaitHeightSetting(localSettings, 18);
            openEditor();
            int keptHeight = keyHeight("n");
            if (keptHeight <= shortHeight + 2)
                throw new AssertionError("Height adjusted before hiding was not kept: "
                    + shortHeight + " -> " + keptHeight);

            stage = "reset to 100 percent";
            openHeightBar();
            tap(described("重置").and(AccessibilityNodeInfo::isClickable));
            if (Math.round(await(heightSlider()).getRangeInfo().getCurrent()) != 100)
                throw new AssertionError("重置 did not return the height to 100%");
            confirm();
            awaitHeightSetting(localSettings, 0);
            int resetHeight = keyHeight("n");
            if (Math.abs(resetHeight - standard) > 2)
                throw new AssertionError("Reset keyboard height did not return to default: "
                    + standard + " -> " + resetHeight);

            checkLiftWidthAndMove(localSettings);
            checkDragKeepsEditorFrame(localSettings, resetHeight);

            stage = "height survives input method restart";
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            openEditor();
            int restarted = keyHeight("n");
            if (Math.abs(restarted - resetHeight) > 2)
                throw new AssertionError("Persisted height changed after restart");

            // 底部留白：旧开关仍默认 40 dp，自定义目标只抬高键区，不能改变键本身的高度。
            stage = "bottom padding lifts the bottom row";
            Rect unpadded = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
            Rect padded = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                .put(BOTTOM_PADDING_SETTING, true));
            int lift = unpadded.bottom - padded.bottom;
            int expectedLift = Math.round(40 * getTargetContext().getResources().getDisplayMetrics().density);
            if (Math.abs(lift - expectedLift) > 2)
                throw new AssertionError("Bottom padding lifted the keys by " + lift + " px, expected " + expectedLift);
            if (Math.abs(padded.height() - unpadded.height()) > 2)
                throw new AssertionError("Bottom padding changed the key height: "
                    + unpadded.height() + " -> " + padded.height());
            for (int heightDp : new int[] {0, 80, 160}) {
                stage = "custom bottom padding " + heightDp;
                Rect custom = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                    .put(BOTTOM_PADDING_SETTING, true).put(BOTTOM_PADDING_HEIGHT_SETTING, heightDp));
                int expected = Math.round(heightDp * getTargetContext().getResources().getDisplayMetrics().density);
                if (Math.abs(unpadded.bottom - custom.bottom - expected) > 2
                        || Math.abs(custom.height() - unpadded.height()) > 2)
                    throw new AssertionError("Independent bottom padding target was not preserved: " + heightDp);
            }

            // 底行与字母键同高（#6354、#6472）：默认、75% 和 130% 时空格键都和「n」一样高——底行和三行键一起均分高度调整，两者最多差 1 dp。默认高度时从「q」的上沿到空格的下沿是四行 52 dp 加三份行距（229 dp），与原来三行 56 dp 加 39 dp 的底栏键帽（228 dp）基本相同。同样关掉键盘底栏，排除手势导航 AVD 上的差异。
            stage = "bottom row matches the letter rows";
            float density = getTargetContext().getResources().getDisplayMetrics().density;
            int tolerance = Math.round(density) + 2;
            for (int adjustment : new int[] {0, -46, 55}) {
                Rect letter = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                    .put(HEIGHT_SETTING, adjustment));
                Rect space = keyBounds("空格");
                if (Math.abs(space.height() - letter.height()) > tolerance)
                    throw new AssertionError("Bottom row is not as tall as the letter rows at " + adjustment
                        + " dp: space " + space.height() + " vs n " + letter.height());
                if (adjustment == 0) {
                    int span = space.bottom - keyBounds("q").top;
                    int expected = Math.round(229 * density);
                    if (Math.abs(span - expected) > Math.round(2 * density) + 2)
                        throw new AssertionError("Default key area changed: " + span + " px from q to space, expected "
                            + expected);
                }
            }

            // 全拼 14 键：三行两字母键加 26 键的底行，键区总高与 26 键相同，底行也和字母键同高。
            stage = "fourteen-key keyboard is as tall as the 26 keys";
            settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
            int twentySixSpan = keyBounds("空格").bottom - keyBounds("q").top;
            JSONObject fourteen = new JSONObject(snapshot.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "fourteen_key")
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new org.json.JSONArray().put("quanpin").put("fourteen_key"))
                    .put("selected", "fourteen_key"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 2)
                .put("preferences", fourteen).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            Rect pair = new Rect();
            awaitSettledKeyboardKey(description("按键 Q W")).getBoundsInScreen(pair);
            Rect fourteenSpace = keyBounds("空格");
            int fourteenSpan = fourteenSpace.bottom - pair.top;
            if (Math.abs(fourteenSpan - twentySixSpan) > tolerance)
                throw new AssertionError("Fourteen-key area is " + fourteenSpan + " px, the 26 keys " + twentySixSpan);
            if (Math.abs(fourteenSpace.height() - pair.height()) > tolerance)
                throw new AssertionError("Fourteen-key bottom row is not as tall as its keys: space "
                    + fourteenSpace.height() + " vs QW " + pair.height());
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(localSettings.toPath());
            else publish(localSettings, originalLocal);
        }
    }

    /**
     * 功能面板和输入方式面板在每一档键盘高度下都放进面板里：外层 ScrollView 没有可滚动的余量，竖着拖动面板磁贴不动，磁贴不矮于 44 dp。用户在 90% 高度下录到功能面板能上下拖动几个像素：行数和行距原先按预测的键区高度挑，比面板实际的高度高出几个像素，三行网格溢出后由 ScrollView 变成了竖向滚动。
     *
     * <p>高度直接写进本地设置再重启输入法，换算与高度条拖到这一档时相同（{@link app.msime.android.KeyboardHeightPolicy#heightPercentToAdjustment}）。每一档都查完再一起报告，每档的磁贴高度和拖动位移写进 logcat（`MSIMEPanelFit`）；传入 {@code panel-shots} 时每一档各截一张图，供人工比对。
     */
    private void checkPanelsFit(File localSettings) throws Exception {
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        int minimumTile = (int) Math.floor(PANEL_MIN_TILE_DP * density) - 1;
        java.util.List<String> problems = new java.util.ArrayList<>();
        try {
            for (int percent : PANEL_FIT_PERCENTS) {
                stage = "function panel fits at " + percent + "%";
                publish(localSettings, new JSONObject().put("version", 1).put("settings",
                    new JSONObject().put(HEIGHT_SETTING, app.msime.android.KeyboardHeightPolicy.heightPercentToAdjustment(percent))).toString()
                    .getBytes(StandardCharsets.UTF_8));
                restartIme();
                openEditor();
                awaitSettledKeyboardKey(key("n"));
                tap(key("更多").and(AccessibilityNodeInfo::isEnabled));
                await(toolPanel());
                showTool("全角");
                checkPanelFits(toolPanel(), tool("全角"), percent, minimumTile, "function", problems);
                tap(key("更多").and(AccessibilityNodeInfo::isEnabled));
                await(key("n"));

                stage = "input-scheme picker fits at " + percent + "%";
                tap(schemeButton());
                checkPanelFits(described("输入方案卡片区域"), describedPrefix("输入方案卡片 "), percent, minimumTile,
                    "scheme", problems);
                tap(schemeButton());
                await(key("n"));
            }
            if (!problems.isEmpty()) throw new AssertionError(String.join("; ", problems));
        } finally {
            publish(localSettings, new JSONObject().put("version", 1).put("settings", new JSONObject()).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
        }
    }

    /** 面板停稳后：外层 ScrollView 不可滚动，竖拖一下第一张磁贴不移位，磁贴不矮于下限；不满足的记进 {@code problems}，回到面板顶部后接着查下一档。 */
    private void checkPanelFits(java.util.function.Predicate<AccessibilityNodeInfo> panelMatch,
            java.util.function.Predicate<AccessibilityNodeInfo> tileMatch, int percent, int minimumTile, String name,
            java.util.List<String> problems) throws Exception {
        AccessibilityNodeInfo tile = awaitStableBounds(tileMatch);
        AccessibilityNodeInfo panel = await(panelMatch);
        for (AccessibilityNodeInfo node = panel.getParent(); node != null; node = node.getParent()) {
            if (equalsText("android.widget.ScrollView", node.getClassName()) && node.isScrollable())
                problems.add(name + " panel scrolls vertically at " + percent + "%");
        }
        Rect before = new Rect();
        tile.getBoundsInScreen(before);
        Rect panelBounds = new Rect();
        panel.getBoundsInScreen(panelBounds);
        if (panelShotPrefix != null)
            shellDiscard("screencap -p " + panelShotPrefix + "-" + name + "-" + percent + ".png");
        // 贴着网格左边、在 4 dp 的侧边距里竖着拖：起点不落在磁贴上，免得松手点中卡片；分页网格只接手横拖，竖拖若能移动磁贴，就只能是外层 ScrollView 在滚。
        int x = panelBounds.left + Math.round(2 * getTargetContext().getResources().getDisplayMetrics().density);
        int from = panelBounds.top + panelBounds.height() * 3 / 4;
        int to = panelBounds.top + panelBounds.height() / 4;
        shellDiscard("input swipe " + x + " " + from + " " + x + " " + to + " 300");
        SystemClock.sleep(600);
        AccessibilityNodeInfo moved = awaitStableBounds(tileMatch);
        Rect after = new Rect();
        moved.getBoundsInScreen(after);
        android.util.Log.i("MSIMEPanelFit", name + " " + percent + "%: panel " + panelBounds.toShortString() + " tile "
            + before.toShortString() + " height " + before.height() + " px, drag moved " + (before.top - after.top) + " px");
        if (after.top != before.top) {
            problems.add("dragging the " + name + " panel at " + percent + "% moved its tiles by "
                + (before.top - after.top) + " px");
            // 拖回去，下一档从面板顶部开始。
            shellDiscard("input swipe " + x + " " + to + " " + x + " " + from + " 300");
            SystemClock.sleep(600);
        }
        if (before.height() < minimumTile)
            problems.add(name + " panel tiles are " + before.height() + " px tall at " + percent + "%, below "
                + PANEL_MIN_TILE_DP + " dp");
    }

    /** 工具栏的输入方式键：描述是「输入方案」或带当前方案名的「输入方案：全拼」，不能按前缀匹配，否则会点到面板里的「输入方案卡片 …」。 */
    private java.util.function.Predicate<AccessibilityNodeInfo> schemeButton() {
        return described("输入方案").or(describedPrefix("输入方案：")).and(AccessibilityNodeInfo::isEnabled);
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }

    /**
     * 「键盘调整」期间输入法窗口不跟着改高（#10025）：每改一次窗口高度应用都按新 insets 重排，新尺寸的窗口在键区画好前先露出白底，录屏里拖动时整块闪白、应用上下跳。这里经合成编辑器的探针读应用窗口的可见区下沿：调整期间键盘变高、变矮、抬高、收窄都不能动它，编辑器也不能丢焦点；点「确认」后才按新高度让出空间。下沿柄把高度挪给抬高、上沿不动，所以确认后让出的空间只由上沿柄定。结束时四项回到默认。
     */
    private void checkDragKeepsEditorFrame(File localSettings, int standardKey) throws Exception {
        stage = "drag keeps the editor frame: open probed editor";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_editor_probe true");
        tap(field("msime-test-plain"));
        await(key("n"));
        // 打开遮罩那一刻应用也不能跳：铺满窗口后固定的边界必须就是停靠键盘原来让出的那条线。
        int docked = awaitSettledVisibleBottom();
        stage = "drag keeps the editor frame: open keyboard adjust";
        openHeightBar();
        int frame = awaitSettledVisibleBottom();
        if (frame != docked)
            throw new AssertionError("Opening keyboard adjust moved the editor frame: " + docked + " -> " + frame);
        int openedHeight = overlayBounds().height();
        for (int target : new int[] {130, 80, 120}) {
            stage = "drag keeps the editor frame: preview " + target + "%";
            setHeight(target);
            automation.waitForIdle(200, 3000);
            int during = awaitFocusedVisibleBottom();
            if (during != frame)
                throw new AssertionError("Previewing " + target + "% moved the editor frame: " + frame + " -> " + during);
        }
        stage = "drag keeps the editor frame: preview resized the keyboard";
        Rect tallPreview = overlayBounds();
        if (tallPreview.height() <= openedHeight + 2)
            throw new AssertionError("120% preview did not enlarge the keyboard inside the fixed frame: "
                + openedHeight + " -> " + tallPreview.height());
        stage = "drag keeps the editor frame: lifting keeps the top edge";
        stepHandle(BOTTOM_HANDLE, AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, 2);
        automation.waitForIdle(200, 3000);
        Rect lifted = overlayBounds();
        if (Math.abs(lifted.top - tallPreview.top) > 2)
            throw new AssertionError("The bottom handle moved the keyboard's top edge: " + tallPreview + " -> " + lifted);
        if (lifted.bottom >= tallPreview.bottom)
            throw new AssertionError("The bottom handle did not lift the keyboard: " + tallPreview + " -> " + lifted);
        if (awaitFocusedVisibleBottom() != frame)
            throw new AssertionError("Lifting the keyboard moved the editor frame");
        stage = "drag keeps the editor frame: narrowing keeps the frame";
        stepHandle(RIGHT_HANDLE, AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD, 2);
        automation.waitForIdle(200, 3000);
        if (awaitFocusedVisibleBottom() != frame)
            throw new AssertionError("Narrowing the keyboard moved the editor frame");

        stage = "drag keeps the editor frame: 确认 yields to the new height";
        confirm();
        // 120% 是 37 dp，下沿柄两步把 8 dp 挪给了抬高。
        awaitHeightSetting(localSettings, app.msime.android.KeyboardHeightPolicy.heightPercentToAdjustment(120) - 8);
        int done = awaitVisibleBottomChange(frame);
        if (done >= frame)
            throw new AssertionError("Taller keyboard did not push the editor frame up after 确认: " + frame + " -> " + done);
        // 遮罩下面的按键在调整期间不进读屏，关上后才量得到。
        if (keyHeight("n") <= standardKey + 2)
            throw new AssertionError("The adjusted keyboard did not keep taller key faces after 确认");

        stage = "drag keeps the editor frame: 重置 and 确认 restore the frame";
        openHeightBar();
        int adjusting = awaitSettledVisibleBottom();
        if (Math.abs(adjusting - done) > 2)
            throw new AssertionError("Reopening keyboard adjust moved the editor frame: " + done + " -> " + adjusting);
        tap(described("重置").and(AccessibilityNodeInfo::isClickable));
        automation.waitForIdle(200, 3000);
        if (awaitFocusedVisibleBottom() != adjusting)
            throw new AssertionError("Previewing the reset moved the editor frame");
        confirm();
        awaitHeightSetting(localSettings, 0);
        awaitLocalSetting(localSettings, WIDTH_SETTING, value -> value == 100, "the default width");
        int restored = awaitVisibleBottomChange(done);
        if (Math.abs(restored - frame) > 2)
            throw new AssertionError("Default height did not return the editor frame: " + frame + " -> " + restored);
    }

    /**
     * 下沿柄、左右柄和「移动」在真机上的效果与落盘：抬高只抬键区、上沿不动并按目标总距离存进「键盘底部高度」的两项；右边收窄时左边不动、左边收窄时右边不动；「移动」左右移位置；收起键盘照「确认」保存；「重置」加「确认」四项都回默认。关掉键盘底栏，排除手势导航 AVD 上底栏的 40 dp。
     */
    private void checkLiftWidthAndMove(File localSettings) throws Exception {
        float density = getTargetContext().getResources().getDisplayMetrics().density;
        int tolerance = Math.round(density) + 2;
        // 键宽随键盘宽度伸缩，键距是固定的 dp：贴边那个键的外沿离键盘边只差半份键距，比较外沿时多留 2 dp。
        int edgeTolerance = tolerance + Math.round(2 * density);
        // settledKeyBounds 量的是「n」（第三行），这里要的是第一行左边的「q」：上沿不动、左边不动都拿它比。
        settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
        Rect q = keyBounds("q");
        Rect space = keyBounds("空格");
        Rect p = keyBounds("p");

        stage = "bottom handle lifts the keys and keeps the top edge";
        openHeightBar();
        stepHandle(BOTTOM_HANDLE, AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, 6);
        AccessibilityNodeInfo bottom = await(handle(BOTTOM_HANDLE));
        bottom.refresh();
        if (Math.round(bottom.getRangeInfo().getCurrent()) != 24)
            throw new AssertionError("Six lift steps did not reach 24 dp: " + bottom.getRangeInfo().getCurrent());
        confirm();
        awaitLocalSetting(localSettings, BOTTOM_PADDING_HEIGHT_SETTING, value -> value == 24, "a 24 dp bottom height");
        awaitLocalBoolean(localSettings, BOTTOM_PADDING_SETTING, true);
        // 下沿柄每抬 4 dp 就从高度里拿走 4 dp。
        awaitHeightSetting(localSettings, -24);
        Rect liftedSpace = awaitMovedKey("空格", space);
        Rect liftedQ = keyBounds("q");
        int expectedLift = Math.round(24 * density);
        if (Math.abs(liftedQ.top - q.top) > tolerance)
            throw new AssertionError("Lifting moved the top key row: " + q + " -> " + liftedQ);
        if (Math.abs(space.bottom - liftedSpace.bottom - expectedLift) > tolerance)
            throw new AssertionError("Lifting raised the bottom row by " + (space.bottom - liftedSpace.bottom)
                + " px, expected " + expectedLift);

        stage = "right handle narrows from the right";
        openHeightBar();
        stepHandle(RIGHT_HANDLE, AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD, 4);
        confirm();
        awaitLocalSetting(localSettings, WIDTH_SETTING, value -> value == 80, "an 80% keyboard");
        awaitLocalSetting(localSettings, POSITION_SETTING, value -> value == 0, "the keyboard against the left");
        Rect narrowP = awaitMovedKey("p", p);
        Rect narrowQ = keyBounds("q");
        if (narrowP.right >= p.right - edgeTolerance)
            throw new AssertionError("Narrowing from the right did not move the right edge: " + p + " -> " + narrowP);
        if (Math.abs(narrowQ.left - q.left) > edgeTolerance)
            throw new AssertionError("Narrowing from the right moved the left edge: " + q + " -> " + narrowQ);

        stage = "move slides the keyboard to the right";
        openHeightBar();
        stepHandle(MOVE_BUTTON, AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, 20);
        confirm();
        awaitLocalSetting(localSettings, POSITION_SETTING, value -> value == 1000, "the keyboard against the right");
        Rect movedP = awaitMovedKey("p", narrowP);
        if (Math.abs(movedP.right - p.right) > edgeTolerance)
            throw new AssertionError("Moving to the right did not bring the right edge back: " + p + " -> " + movedP);

        stage = "left handle keeps the right edge, saved on hide";
        Rect movedQ = keyBounds("q");
        openHeightBar();
        stepHandle(LEFT_HANDLE, AccessibilityNodeInfo.ACTION_SCROLL_FORWARD, 1);
        shellDiscard("input keyevent KEYCODE_BACK");
        awaitLocalSetting(localSettings, WIDTH_SETTING, value -> value == 85, "an 85% keyboard");
        awaitLocalSetting(localSettings, POSITION_SETTING, value -> value == 1000, "the right edge kept");
        openEditor();
        Rect widerQ = awaitMovedKey("q", movedQ);
        Rect widerP = keyBounds("p");
        if (Math.abs(widerP.right - movedP.right) > edgeTolerance)
            throw new AssertionError("Widening from the left moved the right edge: " + movedP + " -> " + widerP);

        stage = "reset returns all four values to their defaults";
        openHeightBar();
        tap(described("重置").and(AccessibilityNodeInfo::isClickable));
        confirm();
        awaitLocalSetting(localSettings, WIDTH_SETTING, value -> value == 100, "the full width");
        awaitLocalSetting(localSettings, POSITION_SETTING, value -> value == 500, "the centred position");
        awaitLocalBoolean(localSettings, BOTTOM_PADDING_SETTING, false);
        awaitHeightSetting(localSettings, 0);
        Rect resetQ = awaitMovedKey("q", widerQ);
        if (Math.abs(resetQ.left - q.left) > edgeTolerance || Math.abs(keyBounds("空格").bottom - space.bottom) > tolerance)
            throw new AssertionError("Reset did not return the keyboard to its default place: " + q + " -> " + resetQ);
    }

    /** 等某个键离开 {@code before} 的位置并停稳：确认后外框重新布局、窗口恢复停靠，有一两帧的过渡。 */
    private Rect awaitMovedKey(String label, Rect before) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            Rect bounds = new Rect();
            awaitSettledKeyboardKey(key(label)).getBoundsInScreen(bounds);
            if (!bounds.equals(before)) return bounds;
            SystemClock.sleep(100);
        }
        throw new AssertionError("Key " + label + " stayed at " + before);
    }

    private android.os.Bundle editorProbe() {
        android.os.Bundle snapshot = getTargetContext().getContentResolver()
            .call(EditorProbeProvider.URI, "snapshot", "msime-test-plain", null);
        if (snapshot == null) throw new AssertionError("Synthetic editor probe returned no state");
        return snapshot;
    }

    /** 编辑器仍持有输入焦点时应用窗口可见区的下沿。 */
    private int awaitFocusedVisibleBottom() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        android.os.Bundle snapshot;
        do {
            snapshot = editorProbe();
            if (snapshot.getBoolean("focused") && snapshot.getBoolean("window_focused"))
                return snapshot.getInt("visible_bottom");
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic editor lost input focus (focus loss="
            + snapshot.getString("focus_loss") + ")");
    }

    /** 键盘弹出、收起都有动画；连续四次读到同一个下沿才算停稳。 */
    private int awaitSettledVisibleBottom() {
        int previous = Integer.MIN_VALUE;
        int stable = 0;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            int current = awaitFocusedVisibleBottom();
            stable = current == previous ? stable + 1 : 1;
            previous = current;
            if (stable >= 4) return current;
            SystemClock.sleep(250);
        }
        throw new AssertionError("Editor frame never settled; last bottom " + previous);
    }

    private int awaitVisibleBottomChange(int from) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            if (awaitFocusedVisibleBottom() != from) return awaitSettledVisibleBottom();
            SystemClock.sleep(100);
        }
        throw new AssertionError("Editor frame stayed at " + from);
    }


    /** 写入这份本地设置、重新绑定输入法后，等键盘停稳再量「n」键在屏幕上的位置：键盘弹出有动画，动画中量到的位置是过渡值。 */
    private Rect settledKeyBounds(File localSettings, JSONObject settings) throws Exception {
        publish(localSettings, new JSONObject().put("version", 1).put("settings", settings).toString()
            .getBytes(StandardCharsets.UTF_8));
        restartIme();
        openEditor();
        Rect bounds = new Rect();
        awaitSettledKeyboardKey(key("n")).getBoundsInScreen(bounds);
        return bounds;
    }

    /**
     * 输入法重启后，旧窗口可能在无障碍树里短暂保留；只等键位本身两次相同会把旧窗口的稳定坐标当成新窗口。这里同时比较输入法窗口的边界，并连续等四次，覆盖 API 35 上的窗口切换动画。
     */
    private AccessibilityNodeInfo awaitSettledKeyboardKey(
            java.util.function.Predicate<AccessibilityNodeInfo> selector) {
        Rect previousKey = null;
        Rect previousWindow = null;
        int stable = 0;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD)
                    continue;
                AccessibilityNodeInfo node = find(window.getRoot(), selector);
                if (node == null) continue;
                Rect keyBounds = new Rect();
                Rect windowBounds = new Rect();
                node.getBoundsInScreen(keyBounds);
                window.getBoundsInScreen(windowBounds);
                if (keyBounds.isEmpty() || !windowBounds.contains(keyBounds)) {
                    stable = 0;
                    break;
                }
                if (keyBounds.equals(previousKey) && windowBounds.equals(previousWindow)) {
                    stable++;
                } else {
                    previousKey = keyBounds;
                    previousWindow = windowBounds;
                    stable = 1;
                }
                if (stable >= 4) return node;
                break;
            }
            SystemClock.sleep(250);
        }
        throw new AssertionError("Keyboard window bounds never settled; IME showed " + imeTexts());
    }

    /** 键盘已停稳时量一个键在屏幕上的位置。 */
    private Rect keyBounds(String label) {
        Rect bounds = new Rect();
        await(key(label)).getBoundsInScreen(bounds);
        return bounds;
    }

    private int keyHeight(String label) {
        // A letter key is described 字母 N, not 按键 n: 按键 is the symbol-key form, and the letter
        // is drawn in caps in Chinese mode. Match the key itself and leave both to their policies.
        AccessibilityNodeInfo node = await(key(label));
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds.height();
    }

    /** 「键盘调整」的上沿柄：读屏描述「键盘高度」，范围是 75% 到窗口封顶处，每次滚动调整 5%。 */
    private java.util.function.Predicate<AccessibilityNodeInfo> heightSlider() {
        return handle(TOP_HANDLE);
    }

    /** 遮罩上的一个柄或「移动」：按读屏描述找，带 RangeInfo 的才是。 */
    private java.util.function.Predicate<AccessibilityNodeInfo> handle(String value) {
        return description(value).and(node -> node.getRangeInfo() != null);
    }

    private void openHeightBar() throws Exception {
        tap(key("更多").and(AccessibilityNodeInfo::isEnabled));
        await(toolPanel());
        tap(tool("键盘调整").and(AccessibilityNodeInfo::isEnabled));
        await(heightSlider());
    }

    /** 点「确认」，等遮罩收起、键盘回到可按的状态。 */
    private void confirm() throws Exception {
        tap(described("确认").and(AccessibilityNodeInfo::isClickable));
        await(key("n").and(AccessibilityNodeInfo::isClickable));
    }

    /** 遮罩（键盘外框）此刻在屏幕上的位置。 */
    private Rect overlayBounds() {
        AccessibilityNodeInfo overlay = await(description(ADJUST_OVERLAY));
        overlay.refresh();
        Rect bounds = new Rect();
        overlay.getBoundsInScreen(bounds);
        return bounds;
    }

    /** 对一个柄连做 {@code times} 次读屏增减动作，每次都确认 RangeInfo 跟着动了。 */
    private void stepHandle(String value, int action, int times) {
        for (int step = 0; step < times; step++) {
            AccessibilityNodeInfo node = await(handle(value));
            node.refresh();
            float before = node.getRangeInfo().getCurrent();
            if (!node.performAction(action))
                throw new AssertionError(value + " accessibility action failed at " + before);
            long deadline = SystemClock.uptimeMillis() + 5000;
            while (true) {
                node.refresh();
                if (node.getRangeInfo() != null && node.getRangeInfo().getCurrent() != before) break;
                if (SystemClock.uptimeMillis() > deadline)
                    throw new AssertionError(value + " stayed at " + before + " after an accessibility step");
                SystemClock.sleep(50);
            }
        }
    }

    /** 输入法窗口里此刻有没有这样的节点；不等待。 */
    private AccessibilityNodeInfo findInIme(java.util.function.Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    private java.util.function.Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    /**
     * Walk the height to `target`.
     *
     * <p>The bar exposes only the scroll actions, five percent apiece, so there is no progress to set; the range it reports is what says where the walk has got to.
     */
    private void setHeight(int target) {
        for (int guard = 0; guard < 64; guard++) {
            AccessibilityNodeInfo slider = await(heightSlider());
            // The framework hands back cached node info; without this the range never moves.
            slider.refresh();
            int current = Math.round(slider.getRangeInfo().getCurrent());
            if (current == target) return;
            int action = current < target
                ? AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                : AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD;
            if (!slider.performAction(action))
                throw new AssertionError("Height accessibility action failed at " + current);
        }
        throw new AssertionError("Height adjustment never reached " + target);
    }

    private void awaitHeightSetting(File file, int expected) throws Exception {
        awaitLocalSetting(file, HEIGHT_SETTING, value -> value == expected, expected + " dp");
    }

    /** 等本地设置里这个整数项满足条件；没写过的项不算。 */
    private void awaitLocalSetting(File file, String key, java.util.function.IntPredicate expected, String what)
            throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        int last = Integer.MIN_VALUE;
        do {
            try {
                byte[] bytes = Files.readAllBytes(file.toPath());
                JSONObject current = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
                last = current.getJSONObject("settings").optInt(key, Integer.MIN_VALUE);
                if (last != Integer.MIN_VALUE && expected.test(last)) return;
            } catch (java.io.IOException | RuntimeException | org.json.JSONException ignored) {
                // Atomic replacement can briefly expose no complete file to this polling read.
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError(key + " was not saved as " + what + "; last read " + last);
    }

    private void awaitLocalBoolean(File file, String key, boolean expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            try {
                JSONObject current = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
                JSONObject settings = current.getJSONObject("settings");
                if (settings.has(key) && settings.getBoolean(key) == expected) return;
            } catch (java.io.IOException | RuntimeException | org.json.JSONException ignored) {
                // Atomic replacement can briefly expose no complete file to this polling read.
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError(key + " was not saved as " + expected);
    }


}
