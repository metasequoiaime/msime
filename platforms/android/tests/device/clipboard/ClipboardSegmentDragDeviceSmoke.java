package app.msime.android.test;

import android.graphics.Rect;
import android.graphics.Bitmap;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真实键盘上全文浮层、列表滚动和跨行拖选，只使用合成文本。 */
public final class ClipboardSegmentDragDeviceSmoke extends DeviceSmoke {
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private long downTime;
    private boolean fixedCardsOnly;
    private boolean previewActionsOnly;
    private boolean firstRecordOnly;

    @Override public void onCreate(android.os.Bundle arguments) {
        fixedCardsOnly = arguments != null && "true".equals(arguments.getString("fixed-cards-only"));
        previewActionsOnly = arguments != null && "true".equals(arguments.getString("preview-actions-only"));
        firstRecordOnly = arguments != null && "true".equals(arguments.getString("first-record-only"));
        super.onCreate(arguments);
    }
    private static String word(int i) { return "Synthetic" + (char) ('A' + i); }

    @Override protected String successDescription() {
        if (firstRecordOnly) return "first clipboard record exposes inline actions after clear and copy in single and double columns";
        if (fixedCardsOnly) return "fixed clipboard cards delegate vertical drags to history without insertion";
        if (previewActionsOnly) return "right-swipe detail exposes insert and edit actions without changing history";
        return "floating full-text preview above IME and lifecycle cleanup; fixed cards, list scroll, swipe, segment drag and preview action regression";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        for (File f : new File[] {preferences, history, local})
            backups.put(f, f.isFile() ? Files.readAllBytes(f.toPath()) : null);
        android.content.SharedPreferences onboarding = getTargetContext().getSharedPreferences("msime_onboarding_v1", 0);
        boolean seenPresent = onboarding.contains("seen"), seen = onboarding.getBoolean("seen", false);
        try {
            // 全新专用设备也能进入编辑页；首次引导不是本套件要测的操作。
            if (!onboarding.edit().putBoolean("seen", true).commit())
                throw new AssertionError("Synthetic onboarding preparation failed");
            JSONObject prefs = read(preferences);
            prefs.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            savePreferences(prefs);
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)).toString());
            if (fixedCardsOnly) { fixedCardsScrollList(history); return; }
            if (previewActionsOnly) { detailActions(history); return; }
            if (firstRecordOnly) { firstRecordActions(history, local); return; }
            StringBuilder source = new StringBuilder();
            for (int i = 0; i < 26; i++) { if (i > 0) source.append(' '); source.append(word(i)); }
            StringBuilder preview = new StringBuilder();
            for (int i = 0; i < 80; i++) preview.append("Synthetic preview line ").append(i).append('\n');
            write(history, new JSONArray().put(new JSONObject().put("text", preview.toString())
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
            openClipboardEditor();
            longPressManagementOnly(history, preview.toString());
            fixedCardsScrollList(history);
            write(history, new JSONArray().put(new JSONObject().put("text", source.toString())
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
            openClipboardEditor();
            AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
                && node.getText().toString().startsWith(word(0)));
            stage = "long clipboard card stays fixed without insertion";
            byte[] seededHistory = Files.readAllBytes(history.toPath());
            previewCard(card);
            stage = "card swipe and dismiss leave editor and history untouched";
            if (!java.util.Arrays.equals(seededHistory, Files.readAllBytes(history.toPath())))
                throw new AssertionError("Preview rewrote history");
            // 空编辑器的节点文字是提示语 Plain editor，不是空串；和其他套件一样按 isShowingHintText 判空。
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            card = await(node -> node.isLongClickable() && node.getText() != null
                && node.getText().toString().startsWith(word(0)));
            stage = "long press remains available after card gestures";
            if (!card.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK)) throw new AssertionError("Long press refused");
            tap(node -> equalsText("分词", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
            stage = "single tap keeps original behavior";
            tap(described("词片 " + word(0)));
            selected(0, true);
            tap(described("词片 " + word(0)));
            selected(0, false);
            stage = "drag spans multiple rows";
            Rect first = rect(0), fourth = rect(5);
            if (fourth.centerY() <= first.centerY()) throw new AssertionError("Fixture did not wrap");
            drag(first, fourth, false);
            for (int i = 0; i <= 5; i++) selected(i, true);
            selected(6, false);
            stage = "reverse drag deselects selected range";
            drag(rect(5), rect(0), false);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "cancel restores original selection";
            drag(rect(0), rect(5), true);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "range shrink restores words outside final range";
            Rect start = rect(0), far = rect(5), near = rect(1);
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            move(start, far);
            move(far, near);
            event(MotionEvent.ACTION_UP, near.centerX(), near.centerY());
            selected(0, true); selected(1, true); selected(2, false); selected(3, false);
            stage = "edge hold scrolls to hidden final word";
            tap(key("全选")); tap(key("全不选"));
            start = rect(0);
            Rect bottom = new Rect();
            await(described("分词滚动区域")).getBoundsInScreen(bottom);
            int edgeY = bottom.bottom - 8;
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            // 最后一行可能有多个词片，停在右下角才能覆盖末词。
            Rect edge = new Rect(bottom.right - 8, edgeY, bottom.right - 7, edgeY + 1);
            move(start, edge);
            SystemClock.sleep(8000);
            event(MotionEvent.ACTION_UP, edge.centerX(), edge.centerY());
            selected(25, true);
            tap(key("插入"));
            await(field("msime-test-plain").and(node -> equalsText(source.toString(), node.getText())));
            detailActions(history);
            firstRecordActions(history, local);
        } catch (Exception | AssertionError error) {
            if (firstRecordOnly) {
                Bitmap screenshot = automation.takeScreenshot();
                if (screenshot != null) {
                    try (var output = new java.io.FileOutputStream(new File(getTargetContext().getCacheDir(),
                            "clipboard-first-record-failure.png"))) {
                        screenshot.compress(Bitmap.CompressFormat.PNG, 100, output);
                    } finally { screenshot.recycle(); }
                }
            }
            throw error;
        } finally {
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else publish(entry.getKey(), entry.getValue());
            }
            android.content.SharedPreferences.Editor editor = onboarding.edit();
            if (seenPresent) editor.putBoolean("seen", seen); else editor.remove("seen");
            if (!editor.commit()) throw new AssertionError("Synthetic onboarding restore failed");
            restartIme();
        }
    }

    private void firstRecordActions(File history, File local) throws Exception {
        // 清空剪贴板历史要先通过系统认证：专用 AVD 默认没有锁屏凭据，先确认此时清空被拒绝且记录保留，再设置合成 PIN 走认证成功的路径。
        write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
            .put("platform.android.toolbar_hidden", false)).toString());
        write(history, new JSONArray().put(new JSONObject().put("text", "Synthetic prior record")
            .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
        stage = "clear clipboard without device credential is refused";
        if (DeviceCredentialFixture.secure(getTargetContext()))
            throw new AssertionError("Dedicated AVD starts with a device credential");
        openClipboardEditor();
        confirmClearClipboard();
        SystemClock.sleep(1500);
        if (DeviceCredentialFixture.promptVisible(automation))
            throw new AssertionError("System credential prompt opened without a device credential");
        awaitHistoryCount(history, 1);
        DeviceCredentialFixture.set(getTargetContext(), automation);
        try {
            firstRecordActionsAuthenticated(history, local);
        } finally {
            DeviceCredentialFixture.clear(getTargetContext(), automation);
        }
    }

    /**
     * 点「清空」，确认问句和「取消」「清空」真的画出来了，再点确认。
     *
     * <p>`tap` 对本应用节点走无障碍点击，0 高度的按钮照样点得中；顶行曾整行塌成 0 高，测试一直能过，真机上手指却什么都点不到（#10027）。所以点确认之前先按屏幕上的实际尺寸断言三者可见。
     */
    private void confirmClearClipboard() throws Exception {
        // 「清空」收进了顶行的「更多」菜单（#10037），点了菜单收起、顶行换成确认。
        tap(described("更多剪贴板操作"));
        assertDrawn(described("清空剪贴板历史"), "more menu clear");
        tap(described("清空剪贴板历史"));
        assertDrawn(node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("确认清空全部记录（含固定项）？", node.getText()), "clear question");
        assertDrawn(key("取消").and(AccessibilityNodeInfo::isClickable), "clear cancel");
        assertDrawn(described("确认清空剪贴板历史"), "clear confirm");
        tap(described("确认清空剪贴板历史"));
    }

    private void assertDrawn(java.util.function.Predicate<AccessibilityNodeInfo> match, String name) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        if (bounds.width() <= 0 || bounds.height() <= 0)
            throw new AssertionError("Clipboard " + name + " is not drawn: " + bounds.toShortString());
    }

    private void firstRecordActionsAuthenticated(File history, File local) throws Exception {
        for (String columns : new String[] {"one", "two"}) {
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)
                .put("platform.android.clipboard_columns", columns)).toString());
            for (boolean multiline : new boolean[] {false, true}) {
                String text = "Synthetic first clipboard " + columns + (multiline
                    ? "\nSecond synthetic line\nThird synthetic line\nSynthetic hidden tail" : " short");
                write(history, new JSONArray().put(new JSONObject().put("text", "Synthetic prior record")
                    .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
                openClipboardEditor();
                stage = "clear clipboard before first copy: " + columns + ", multiline=" + multiline;
                confirmClearClipboard();
                DeviceCredentialFixture.enter(automation);
                awaitHistoryCount(history, 0);
                // 认证界面的密码框也由水杉键盘输入，回到编辑器之前键盘仍按密码框不记剪贴板；先回到合成编辑器再复制，和用户认证完回来再复制一致。
                openClipboardEditor();
                copySynthetic(text);
                awaitHistoryCount(history, 1);
                // 与报告步骤一致：复制后重新进入面板，等设置重载与剪贴板补读完成再长按。
                openClipboardEditor();
                assertRecordActions(history, text, columns + ", multiline=" + multiline + ", count=1");
                if ("two".equals(columns)) {
                    copySynthetic("Synthetic second clipboard record");
                    awaitHistoryCount(history, 2);
                    openClipboardEditor();
                    assertRecordActions(history, text, columns + ", multiline=" + multiline + ", count=2");
                }
            }
        }
    }

    private void copySynthetic(String text) {
        runOnMainSync(() -> getTargetContext().getSystemService(android.content.ClipboardManager.class)
            .setPrimaryClip(android.content.ClipData.newPlainText("Synthetic fixture", text)));
    }

    private void assertRecordActions(File history, String text, String fixture) throws Exception {
        stage = "physical long press on clipboard record: " + fixture;
        AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(text));
        byte[] seeded = Files.readAllBytes(history.toPath());
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 200);
        event(MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        stage = "inline actions visible for clipboard record: " + fixture;
        for (String action : new String[] {"固定", "删除", "编辑", "分词", "分享", "移动分类", "添加到常用语", "收起"})
            await(node -> equalsText(action, node.getText()) && node.isVisibleToUser()
                && node.getParent() != null
                && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
        if (!java.util.Arrays.equals(seeded, Files.readAllBytes(history.toPath())))
            throw new AssertionError("First-record preview changed history");
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        // 保存的画面只含本套件生成的内容，方便区分未触发长按和操作行被挤出视口。
        Bitmap screenshot = automation.takeScreenshot();
        if (screenshot != null) {
            try (var output = new java.io.FileOutputStream(new File(getTargetContext().getCacheDir(),
                    "clipboard-first-record-" + fixture.replaceAll("[^a-z0-9=]", "-") + ".png"))) {
                screenshot.compress(Bitmap.CompressFormat.PNG, 100, output);
            } finally { screenshot.recycle(); }
        }
        touch(node -> equalsText("收起", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
        managementClosed();
    }

    private void awaitHistoryCount(File history, int count) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            if ((!history.isFile() && count == 0) || (history.isFile()
                    && new JSONArray(new String(Files.readAllBytes(history.toPath()), StandardCharsets.UTF_8)).length() == count))
                return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic history count did not settle: " + count);
    }

    private void detailActions(File history) throws Exception {
        StringBuilder source = new StringBuilder();
        for (int i = 0; i < 80; i++) source.append("Synthetic preview line ").append(i).append('\n');
        String original = source.toString();
        String replacement = "Synthetic modified preview text🙂";
        seedHistory(original, 1700000000000L, true);
        byte[] seeded = Files.readAllBytes(history.toPath());
        openClipboardEditor(true);
        stage = "right swipe opens clipboard detail with full record time and unavailable source (seeded entries "
            + sharedHistoryCount() + ")";
        // 长文本超过 Android 无障碍节点的文本传输上限时，节点仍可见但 `getText()` 可能为空；用卡片自己的操作说明定位，再按真实边界注入右滑。
        // API 34 以上的输入法窗口会把滚动容器里的卡片报告成不可见，但坐标仍是当前卡片的真实位置；这里按无障碍树定位后再用坐标注入手势。
        AccessibilityNodeInfo card = awaitAny(node -> node.isLongClickable()
            && node.getContentDescription() != null
            && node.getContentDescription().toString().contains("右滑查看详情"));
        Rect cardBounds = new Rect();
        card.getBoundsInScreen(cardBounds);
        drag(new Rect(cardBounds.left + 8, cardBounds.centerY(), cardBounds.left + 9, cardBounds.centerY() + 1),
            new Rect(cardBounds.right - 8, cardBounds.centerY(), cardBounds.right - 7, cardBounds.centerY() + 1), false);
        await(node -> equalsText("app.msime.android", node.getPackageName()) && equalsText("记录详情", node.getText()));
        await(described("返回剪贴板历史列表"));
        await(described("插入这条剪贴板记录"));
        await(described("编辑这条剪贴板记录"));
        // 详情页先放整段正文，这条合成记录有 80 行，时间和来源那段排在正文下面、剪贴板滚动区的视口之外，isVisibleToUser 为 false，只认看得见节点的 await 永远等不到它。先在输入法的树里找到这段，让滚动区把它滚进视口，再按看得见的节点核对文案。
        java.util.function.Predicate<AccessibilityNodeInfo> recordDetails = node ->
            equalsText("app.msime.android", node.getPackageName()) && node.getText() != null
            && node.getText().toString().startsWith("记录时间：")
            && !node.getText().toString().startsWith("记录时间：时间未知")
            && node.getText().toString().contains("来源 App：系统未提供");
        awaitAny(recordDetails).performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        await(recordDetails);
        if (!java.util.Arrays.equals(seeded, Files.readAllBytes(history.toPath())))
            throw new AssertionError("Opening detail changed history");
        // 详情页直接提供编辑入口；编辑前不应把原正文插入外部编辑器。
        openClipboardEditor(true);
        card = awaitAny(node -> node.isLongClickable()
            && node.getContentDescription() != null
            && node.getContentDescription().toString().contains("右滑查看详情"));
        cardBounds = new Rect();
        card.getBoundsInScreen(cardBounds);
        drag(new Rect(cardBounds.left + 8, cardBounds.centerY(), cardBounds.left + 9, cardBounds.centerY() + 1),
            new Rect(cardBounds.right - 8, cardBounds.centerY(), cardBounds.right - 7, cardBounds.centerY() + 1), false);
        await(node -> equalsText("app.msime.android", node.getPackageName()) && equalsText("记录详情", node.getText()));
        touch(described("编辑这条剪贴板记录"));
        await(described("剪贴板记录的文字").and(node -> equalsText(original, node.getText())));
        android.os.Bundle changed = new android.os.Bundle();
        changed.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, replacement);
        if (!await(described("剪贴板记录的文字")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, changed))
            throw new AssertionError("Synthetic clipboard replacement refused");
        tap(node -> equalsText("保存", node.getText()) && node.isEnabled());
        stage = "detail edit saves and returns to unchanged external editor";
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        tap(field("msime-test-plain"));
        await(key("n"));
        tap(clipboardEntry());
        card = awaitAny(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(replacement));
        cardBounds = new Rect();
        card.getBoundsInScreen(cardBounds);
        drag(new Rect(cardBounds.left + 8, cardBounds.centerY(), cardBounds.left + 9, cardBounds.centerY() + 1),
            new Rect(cardBounds.right - 8, cardBounds.centerY(), cardBounds.right - 7, cardBounds.centerY() + 1), false);
        await(node -> equalsText("app.msime.android", node.getPackageName()) && equalsText("记录详情", node.getText()));
        stage = "detail insertion preserves modified record identity";
        touch(described("插入这条剪贴板记录"));
        await(field("msime-test-plain").and(node -> equalsText(replacement, node.getText())));
        JSONArray saved = new JSONArray(new String(Files.readAllBytes(history.toPath()), StandardCharsets.UTF_8));
        if (saved.length() != 1 || !saved.getJSONObject(0).getBoolean("pinned")
                || saved.getJSONObject(0).getLong("timestampMs") != 1700000000000L
                || !replacement.equals(saved.getJSONObject(0).getString("text")))
            throw new AssertionError("Detail edit lost record identity");
    }

    /** 通过移动端剪贴板公开入口写入夹具，避免 API 31 以上复用进程时直写文件与宿主状态分叉。 */
    private void seedHistory(String text, long timestamp, boolean pinned) throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> modeType = loader.loadClass("app.msime.android.ClipboardModePolicy$Mode");
        Object main = null;
        for (Object candidate : modeType.getEnumConstants()) {
            if ("MAIN".equals(candidate.toString())) { main = candidate; break; }
        }
        if (main == null) throw new AssertionError("Main clipboard mode is unavailable");
        Class<?> storeType = loader.loadClass("app.msime.android.ClipboardHistoryStore");
        Object store = storeType.getMethod("forMode", android.content.Context.class, modeType)
            .invoke(null, getTargetContext(), main);
        storeType.getMethod("clear").invoke(store);
        Class<?> itemType = loader.loadClass("app.msime.android.ClipboardHistory$Item");
        Object item = itemType.getConstructor(String.class, long.class, boolean.class, String.class)
            .newInstance(text, timestamp, pinned, null);
        storeType.getMethod("restoreMany", List.class).invoke(store, List.of(item));
    }

    /** 通过设置宿主的 compare-and-swap 入口发布偏好，让 :ime 进程读到同一份快照。 */
    private void savePreferences(JSONObject preferences) throws Exception {
        Class<?> hostStore = getTargetContext().getClassLoader().loadClass("app.msime.android.home.HostStore");
        Object saved = hostStore.getMethod("savePreferences", android.content.Context.class, JSONObject.class)
            .invoke(null, getTargetContext(), preferences);
        if (saved == null) throw new AssertionError("Synthetic clipboard preferences could not be saved");
    }

    private int sharedHistoryCount() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> storeType = loader.loadClass("app.msime.android.ClipboardHistoryStore");
        Class<?> modeType = loader.loadClass("app.msime.android.ClipboardModePolicy$Mode");
        Object main = null;
        for (Object candidate : modeType.getEnumConstants()) {
            if ("MAIN".equals(candidate.toString())) { main = candidate; break; }
        }
        if (main == null) throw new AssertionError("Main clipboard mode is unavailable");
        Object store = storeType.getMethod("forMode", android.content.Context.class, modeType)
            .invoke(null, getTargetContext(), main);
        return ((List<?>) storeType.getMethod("load").invoke(store)).size();
    }

    private void openClipboardEditor() throws Exception {
        openClipboardEditor(false);
    }

    private void openClipboardEditor(boolean multiline) throws Exception {
        restartIme();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_editor_probe true"
            + (multiline ? " --ez msime_test_multiline true" : ""));
        tap(field("msime-test-plain"));
        await(key("n"));
        AccessibilityNodeInfo clipboard = await(clipboardEntry());
        if (clipboard.isSelected()) {
            tap(clipboardEntry());
            await(clipboardEntry().and(node -> !node.isSelected()));
        }
        tap(clipboardEntry());
        await(clipboardEntry().and(AccessibilityNodeInfo::isSelected));
    }

    private void longPressManagementOnly(File history, String text) throws Exception {
        stage = "physical long press opens management actions without full-text preview";
        byte[] before = Files.readAllBytes(history.toPath());
        AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(text.substring(0, Math.min(text.length(), 32))));
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(ViewConfiguration.getLongPressTimeout() + 200);
        event(MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        await(described("剪贴板记录操作"));
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), described("关闭剪贴板全文预览")) != null)
                throw new AssertionError("Long press opened full-text preview");
        }
        if (!java.util.Arrays.equals(before, Files.readAllBytes(history.toPath())))
            throw new AssertionError("Long press changed history");
        touch(node -> equalsText("收起", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
        managementClosed();
    }

    private void touch(java.util.function.Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        await(match).getBoundsInScreen(bounds);
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        event(MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(200);
    }

    private void managementClosed() {
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            boolean found = false;
            for (var window : automation.getWindows())
                found |= find(window.getRoot(), described("剪贴板记录操作")) != null;
            if (!found) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Clipboard management actions survived their close boundary");
    }

    private void fixedCardsScrollList(File history) throws Exception {
        JSONArray records = new JSONArray();
        for (int i = 0; i < 20; i++) records.put(new JSONObject()
            .put("text", "Synthetic fixed card " + i + "\nSecond synthetic line\nThird synthetic line\nHidden synthetic tail")
            .put("timestampMs", 1700000000000L - i).put("pinned", true));
        write(history, records.toString());
        byte[] before = Files.readAllBytes(history.toPath());
        openClipboardEditor();
        stage = "fixed card exposes no private scroll actions";
        AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith("Synthetic fixed card 0\n"));
        assertFixedCard(card);
        AccessibilityNodeInfo list = card.getParent();
        while (list != null && (!list.isScrollable() || !equalsText("android.widget.ScrollView", list.getClassName())))
            list = list.getParent();
        if (list == null) throw new AssertionError("Synthetic history did not overflow its list");
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        stage = "vertical drag starting inside long card moves the history list";
        drag(new Rect(bounds.centerX(), bounds.bottom - 12, bounds.centerX() + 1, bounds.bottom - 11),
            new Rect(bounds.centerX(), bounds.top + 12, bounds.centerX() + 1, bounds.top + 13), false);
        SystemClock.sleep(400);
        list.refresh();
        if (!list.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD))
            throw new AssertionError("Card swallowed vertical drag instead of scrolling history");
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        if (!java.util.Arrays.equals(before, Files.readAllBytes(history.toPath())))
            throw new AssertionError("List scroll rewrote history");
    }

    private void assertFixedCard(AccessibilityNodeInfo card) {
        if (card.isScrollable()
                || card.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)
                || card.performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD))
            throw new AssertionError("Clipboard card still scrolls independently of its list");
    }

    private void previewCard(AccessibilityNodeInfo card) throws Exception {
        assertFixedCard(card);
        Rect bounds = new Rect();
        card.getBoundsInScreen(bounds);
        Rect from = new Rect(bounds.centerX(), bounds.bottom - 8, bounds.centerX() + 1, bounds.bottom - 7);
        Rect to = new Rect(bounds.centerX(), bounds.top + 8, bounds.centerX() + 1, bounds.top + 9);
        drag(from, to, false);
        SystemClock.sleep(200);
        card.refresh();
        assertFixedCard(card);
        stage = "vertical drag inside a fixed card does not insert it";
        await(field("msime-test-plain").and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
        card = await(node -> node.isLongClickable() && node.isVisibleToUser()
            && node.getText() != null && node.getText().toString().startsWith(word(0)));
        card.getBoundsInScreen(bounds);
        stage = "horizontal delete still works after vertical drag";
        // 纵向手势会交给外层列表；横滑前重新定位卡片，不能复用滚动前的坐标。
        card.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
        card = awaitStableBounds(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(word(0)));
        card.getBoundsInScreen(bounds);
        stage = "horizontal delete still works after vertical drag: " + bounds.toShortString();
        Rect swipeFrom = new Rect(bounds.right - 8, bounds.centerY(), bounds.right - 7, bounds.centerY() + 1);
        Rect swipeTo = new Rect(bounds.left + 8, bounds.centerY(), bounds.left + 9, bounds.centerY() + 1);
        drag(swipeFrom, swipeTo, false);
        await(described("删除这条剪贴板记录").and(AccessibilityNodeInfo::isVisibleToUser));
        // 已打开的卡片第一次点按只收回删除区，下一步再检查长按管理。
        tap(node -> node.isLongClickable() && node.getText() != null
            && node.getText().toString().startsWith(word(0)));
    }

    private Rect rect(int i) {
        Rect r = new Rect();
        await(described("词片 " + word(i)).and(AccessibilityNodeInfo::isVisibleToUser)).getBoundsInScreen(r);
        return r;
    }
    private void selected(int i, boolean value) {
        await(described("词片 " + word(i)).and(node -> node.isSelected() == value));
    }
    private void drag(Rect from, Rect to, boolean cancel) {
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, from.centerX(), from.centerY());
        move(from, to);
        event(cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, to.centerX(), to.centerY());
    }
    private void move(Rect from, Rect to) {
        for (int i = 1; i <= 12; i++) {
            event(MotionEvent.ACTION_MOVE, from.centerX() + (to.centerX() - from.centerX()) * i / 12f,
                from.centerY() + (to.centerY() - from.centerY()) * i / 12f);
            SystemClock.sleep(20);
        }
    }
    private void event(int action, float x, float y) {
        MotionEvent e = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0);
        e.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try { if (!automation.injectInputEvent(e, true)) throw new AssertionError("Touch injection refused"); }
        finally { e.recycle(); }
    }
    private static JSONObject read(File f) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8));
    }
    private void write(File f, String text) throws Exception { publish(f, text.getBytes(StandardCharsets.UTF_8)); }
}
