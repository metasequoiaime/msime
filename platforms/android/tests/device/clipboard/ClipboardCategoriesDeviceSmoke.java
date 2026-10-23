package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 专用模拟器上验收分类设置、移动、筛选、持久化及固定顶栏和返回顶部。 */
public final class ClipboardCategoriesDeviceSmoke extends DeviceSmoke {
    private static final String PREFIX = "Synthetic category record ";
    private static final String NAME = "合成分类";
    private File history;
    private File catalog;

    @Override protected String successDescription() {
        return "clipboard category settings, move/filter, rename/delete, reload, sticky header and back to top";
    }

    @Override protected void runChecks() throws Exception {
        getTargetContext().getClassLoader().loadClass("app.msime.android.Bootstrap")
            .getMethod("prepare", android.content.Context.class).invoke(null, getTargetContext());
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        history = new File(root, "MSIME/clipboard_history.json");
        catalog = new File(root, "MSIME/clipboard_history.json.categories.json");
        Map<File, byte[]> backups = new LinkedHashMap<>();
        for (File file : new File[] {preferences, history, catalog})
            backups.put(file, file.isFile() ? Files.readAllBytes(file.toPath()) : null);
        android.content.SharedPreferences onboarding = getTargetContext().getSharedPreferences("msime_onboarding_v1", 0);
        boolean seenPresent = onboarding.contains("seen"), seen = onboarding.getBoolean("seen", false);
        try {
            check(onboarding.edit().putBoolean("seen", true).commit(), "Synthetic onboarding preparation failed");
            JSONObject prefs = read(preferences);
            prefs.put("revision", prefs.getLong("revision") + 1);
            prefs.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            write(preferences, prefs.toString());
            JSONArray entries = new JSONArray();
            for (int i = 0; i < 30; i++) entries.put(new JSONObject().put("text", PREFIX + i)
                .put("timestampMs", 1700000000030L - i).put("pinned", true));
            write(history, entries.toString());
            write(catalog, "[]");

            stage = "native category settings add";
            openSettings("CLIPBOARD_CATEGORIES");
            tap(hostText("添加"));
            editName(NAME);
            await(hostText(NAME));
            check(categories().length() == 1, "Category was not persisted");
            capture("settings");
            String id = categories().getJSONObject(0).getString("id");

            stage = "keyboard category selector and move";
            restartIme();
            openClipboard();
            tap(described("选择本机剪贴板分类"));
            tap(described("剪贴板目标分类 未分类"));
            AccessibilityNodeInfo first = await(card(0));
            check(first.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK), "Synthetic long press refused");
            // 全文预览外的原管理行保留移动入口；先显式关闭预览。
            tap(described("关闭剪贴板全文预览"));
            tap(hostText("移动分类"));
            tap(described("剪贴板目标分类 " + NAME));
            awaitHistoryCategory(id);
            tap(described("选择本机剪贴板分类"));
            tap(described("剪贴板目标分类 " + NAME));
            await(card(0));
            assertAbsent(card(1), "Category filter leaked another record");
            assertEditorEmpty();
            capture("filter");

            stage = "keyboard classification management shortcut";
            tap(described("管理剪贴板分类"));
            await(hostText(NAME));
            tap(hostText(NAME));
            editName("合成改名");
            check(id.equals(categories().getJSONObject(0).getString("id")), "Rename changed stable category ID");
            stage = "reload retains category membership and renamed label";
            restartIme();
            openClipboard();
            tap(described("选择本机剪贴板分类"));
            tap(described("剪贴板目标分类 合成改名"));
            await(card(0));
            assertAbsent(card(1), "Reload lost category filter");

            stage = "delete category retains body and returns unclassified";
            tap(described("管理剪贴板分类"));
            tap(hostText("删除"));
            await(hostText("添加"));
            check(categories().length() == 0, "Category delete did not persist");
            openClipboard();
            tap(described("选择本机剪贴板分类"));
            tap(described("剪贴板目标分类 未分类"));
            await(card(0));
            await(card(1));
            check(records().length() >= 30, "Category delete removed body records");

            stage = "history list scroll keeps fixed toolbar and shows back to top";
            assertAbsent(described("返回剪贴板顶部"), "Back to top was visible at top");
            Rect before = bounds(await(described("选择本机剪贴板分类")));
            AccessibilityNodeInfo list = await(card(0)).getParent();
            while (list != null && !"android.widget.ScrollView".contentEquals(list.getClassName())) list = list.getParent();
            check(list != null && list.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD), "Synthetic list did not scroll");
            await(described("返回剪贴板顶部"));
            capture("scrolled");
            check(before.equals(bounds(await(described("选择本机剪贴板分类")))), "Scrolling moved fixed toolbar");
            tap(described("返回剪贴板顶部"));
            await(card(0));
            assertAbsent(described("返回剪贴板顶部"), "Back to top remained visible after returning");
            assertEditorEmpty();
            stage = "filtered history card still inserts original text";
            tap(card(0));
            await(field("msime-test-plain").and(node -> equalsText(PREFIX + 0, node.getText())));
        } finally {
            // 停止测试键盘监听后恢复合成前的私有文件，不输出任何正文。
            shellDiscard("ime disable app.msime.android/.MSIMEInputService");
            for (var backup : backups.entrySet()) {
                if (backup.getValue() == null) Files.deleteIfExists(backup.getKey().toPath());
                else publish(backup.getKey(), backup.getValue());
            }
            android.content.SharedPreferences.Editor editor = onboarding.edit();
            if (seenPresent) editor.putBoolean("seen", seen); else editor.remove("seen");
            check(editor.commit(), "Synthetic onboarding restore failed");
        }
    }

    private void capture(String name) throws Exception {
        android.graphics.Bitmap bitmap = automation.takeScreenshot();
        check(bitmap != null, "Synthetic screenshot unavailable");
        try (var output = new java.io.FileOutputStream(new File(getTargetContext().getCacheDir(),
                "clipboard-categories-" + name + ".png"))) {
            check(bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output), "Synthetic screenshot write failed");
        } finally { bitmap.recycle(); }
    }

    private void editName(String name) throws Exception {
        AccessibilityNodeInfo input = await(node -> equalsText("app.msime.android", node.getPackageName()) && node.isEditable());
        Bundle text = new Bundle();
        text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, name);
        check(input.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text), "Category name input refused");
        tap(hostText("保存"));
        await(hostText(name));
    }

    private void openSettings(String page) throws Exception {
        shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page " + page);
        await(hostText("剪贴板分类"));
    }

    private void openClipboard() throws Exception {
        Intent intent = new Intent().setClassName("app.msime.android.test", "app.msime.android.test.EditorActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        getContext().startActivity(intent);
        tap(field("msime-test-plain"));
        await(key("n"));
        tap(described("剪贴板"));
        await(described("选择本机剪贴板分类"));
    }

    private void awaitHistoryCategory(String id) throws Exception {
        long until = SystemClock.uptimeMillis() + 15000;
        do {
            JSONArray entries = records();
            for (int i = 0; i < entries.length(); i++) {
                JSONObject entry = entries.getJSONObject(i);
                if ((PREFIX + 0).equals(entry.getString("text")) && id.equals(entry.optString("categoryId"))) return;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("Synthetic category membership was not persisted");
    }

    private void assertEditorEmpty() { await(field("msime-test-plain").and(node -> node.isShowingHintText()
        || node.getText() == null || node.getText().length() == 0)); }
    private void assertAbsent(Predicate<AccessibilityNodeInfo> match, String message) {
        SystemClock.sleep(250);
        dropStaleNodes();
        for (var window : automation.getWindows()) check(find(window.getRoot(), match.and(AccessibilityNodeInfo::isVisibleToUser)) == null, message);
    }
    private Predicate<AccessibilityNodeInfo> hostText(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && equalsText(text, node.getText());
    }
    private Predicate<AccessibilityNodeInfo> card(int index) {
        return node -> node.isLongClickable() && node.getText() != null && node.getText().toString().startsWith(PREFIX + index + "\n");
    }
    private static Rect bounds(AccessibilityNodeInfo node) { Rect rect = new Rect(); node.getBoundsInScreen(rect); return rect; }
    private JSONArray records() throws Exception { return array(history); }
    private JSONArray categories() throws Exception { return array(catalog); }
    private static JSONArray array(File file) throws Exception { return new JSONArray(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8)); }
    private static JSONObject read(File file) throws Exception { return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8)); }
    private void write(File file, String text) throws Exception { publish(file, text.getBytes(StandardCharsets.UTF_8)); }
    private static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
