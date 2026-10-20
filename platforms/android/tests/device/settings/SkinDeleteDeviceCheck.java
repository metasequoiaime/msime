package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.function.BooleanSupplier;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 复用设置 instrumentation 的显式专项，不增加已达系统上限的清单入口。 */
final class SkinDeleteDeviceCheck {
    private final DeviceSmoke test;

    SkinDeleteDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        File root = test.getTargetContext().getFilesDir();
        JSONObject options = object(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        check(directory.toPath().startsWith(root.getCanonicalFile().toPath()), "Fixture directory escaped product files");
        File preferences = new File(directory, "preferences.json");
        File library = new File(directory, "CustomSkins/library.json");
        byte[] originalPreferences = Files.readAllBytes(preferences.toPath());
        byte[] originalLibrary = library.exists() ? Files.readAllBytes(library.toPath()) : null;
        Activity activity = null;
        try {
            test.stage = "seed synthetic saved skins through installed library";
            Files.createDirectories(library.getParentFile().toPath());
            test.publish(library, "[]".getBytes(StandardCharsets.UTF_8));
            Class<?> type = test.getTargetContext().getClassLoader().loadClass("app.msime.android.CustomSkinLibrary");
            Method add = type.getMethod("add", Path.class, String.class, String.class, JSONObject.class, long.class);
            JSONObject design = new JSONObject().put("background", 0xedf3ff).put("accent", 0x336699)
                .put("cornerRadius", 12).put("keyShape", "pebble");
            Class<?> skinType = test.getTargetContext().getClassLoader().loadClass("app.msime.android.CustomKeyboardSkin");
            Object skin = skinType.getMethod("from", JSONObject.class).invoke(null, design);
            design = (JSONObject) skinType.getMethod("toJson", boolean.class).invoke(skin, true);
            check(Boolean.TRUE.equals(add.invoke(null, directory.toPath(), "fixture-a", "合成晨雾", design, 100L)), "Seed first skin failed");
            JSONObject otherDesign = new JSONObject(design.toString()).put("accent", 0x886633);
            check(Boolean.TRUE.equals(add.invoke(null, directory.toPath(), "fixture-b", "合成夜航", otherDesign, 200L)), "Seed second skin failed");
            JSONObject snapshot = new JSONObject(new String(originalPreferences, StandardCharsets.UTF_8));
            snapshot.put("revision", snapshot.getLong("revision") + 1);
            JSONObject values = snapshot.getJSONObject("preferences");
            JSONObject custom = values.optJSONObject("custom_theme");
            if (custom == null) custom = new JSONObject();
            values.put("global_theme", "custom").put("custom_theme", custom.put("keyboard", design));
            test.publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            byte[] appliedPreferences = Files.readAllBytes(preferences.toPath());
            byte[] bothSkins = Files.readAllBytes(library.toPath());

            test.stage = "open native saved skins";
            activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
                "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
                .putExtra("msime_open_page", "SKINS"));
            test.tap(text("我的").and(node -> equals("android.widget.RadioButton", node.getClassName())));
            test.await(card("合成晨雾"));

            test.stage = "cancel native delete confirmation";
            openDelete("合成晨雾");
            test.tap(text("取消").and(AccessibilityNodeInfo::isClickable));
            test.await(card("合成晨雾"));
            check(Arrays.equals(bothSkins, Files.readAllBytes(library.toPath())), "Cancel rewrote saved skins");

            test.stage = "delete active saved skin and refresh list";
            openDelete("合成晨雾");
            test.tap(text("删除").and(AccessibilityNodeInfo::isClickable));
            await(() -> rows(library).length() == 1, "Saved skin was not removed");
            await(() -> !visible(card("合成晨雾")), "Removed skin remained in the list");
            test.await(card("合成夜航"));
            JSONObject remaining = rows(library).getJSONObject(0);
            check("fixture-b".equals(remaining.getString("id")) && remaining.getLong("updated_at") == 200,
                "Other saved skin changed");
            check(Arrays.equals(appliedPreferences, Files.readAllBytes(preferences.toPath())),
                "Deleting saved skin changed active design or feedback");

            test.stage = "delete last skin and ignore repeated removal";
            openDelete("合成夜航");
            test.tap(text("删除").and(AccessibilityNodeInfo::isClickable));
            await(() -> rows(library).length() == 0, "Last saved skin was not removed");
            await(() -> !visible(card("合成夜航")), "Last skin remained in the list");
            byte[] emptyLibrary = Files.readAllBytes(library.toPath());
            check(Boolean.FALSE.equals(type.getMethod("remove", Path.class, String.class)
                .invoke(null, directory.toPath(), "fixture-b")), "Repeated removal reported success");
            check(Arrays.equals(emptyLibrary, Files.readAllBytes(library.toPath())), "Repeated removal rewrote library");
            check(Arrays.equals(appliedPreferences, Files.readAllBytes(preferences.toPath())), "Last removal changed active design");
        } finally {
            if (activity != null) {
                Activity owner = activity;
                test.runOnMainSync(owner::finish);
                test.waitForIdleSync();
            }
            test.publish(preferences, originalPreferences);
            if (originalLibrary == null) Files.deleteIfExists(library.toPath());
            else test.publish(library, originalLibrary);
        }
    }

    private void openDelete(String name) {
        check(test.await(card(name)).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK), "Skin long press failed");
        test.await(text("删除「" + name + "」？"));
    }

    private boolean visible(Predicate<AccessibilityNodeInfo> match) {
        test.dropStaleNodes();
        for (var window : test.automation.getWindows()) {
            if (test.find(window.getRoot(), match) != null) return true;
        }
        return false;
    }

    private static Predicate<AccessibilityNodeInfo> card(String name) {
        return node -> equals("app.msime.android", node.getPackageName())
            && equals("皮肤 " + name + "，点击使用，长按删除", node.getContentDescription());
    }

    private static Predicate<AccessibilityNodeInfo> text(String value) {
        return node -> equals("app.msime.android", node.getPackageName()) && equals(value, node.getText());
    }

    private static boolean equals(String expected, CharSequence actual) {
        return actual != null && expected.contentEquals(actual);
    }

    private static JSONObject object(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static JSONArray rows(File library) {
        try { return new JSONArray(new String(Files.readAllBytes(library.toPath()), StandardCharsets.UTF_8)); }
        catch (Exception error) { throw new AssertionError("Synthetic skin library unreadable"); }
    }

    private static void await(BooleanSupplier condition, String failure) {
        long deadline = SystemClock.uptimeMillis() + 15_000;
        do {
            if (condition.getAsBoolean()) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError(failure);
    }

    private static void check(boolean condition, String failure) {
        if (!condition) throw new AssertionError(failure);
    }
}
