package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 在真实原生分类和卡片上检查本地库变动，合成设计从下载完成后的安装边界进入。 */
final class SkinTabRefreshDeviceCheck {
    private static final String NAME = "合成晨雾";
    private static final String ID = "fixture-tab-refresh";
    private final DeviceSmoke test;
    private final ClassLoader loader;
    private Activity activity;

    SkinTabRefreshDeviceCheck(DeviceSmoke test) {
        this.test = test;
        loader = test.getTargetContext().getClassLoader();
    }

    void run() throws Exception {
        File root = test.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = json(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Fixture escaped product files");
        File library = new File(directory, "CustomSkins/library.json");
        byte[] original = library.exists() ? Files.readAllBytes(library.toPath()) : null;
        File preferences = new File(directory, "preferences.json");
        byte[] originalPreferences = Files.readAllBytes(preferences.toPath());
        try {
            test.publish(library, "[]".getBytes(StandardCharsets.UTF_8));
            activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
                "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK)
                .putExtra("msime_open_page", "SKINS"));
            test.tap(tab("我的"));
            test.await(node -> same("AI 设计皮肤，描述一句话生成", node.getContentDescription()));
            test.tap(tab("社区"));
            Object page = fragment(activity.getClass().getMethod("getSupportFragmentManager").invoke(activity),
                "app.msime.android.home.SkinsPage");
            Object manager = page.getClass().getMethod("getChildFragmentManager").invoke(page);
            Object community = fragment(manager, "app.msime.android.home.CommunityFragment");
            Class<?> communityType = community.getClass();
            Object adapter = field(communityType, "adapter").get(community);
            Class<?> itemType = loader.loadClass("app.msime.android.CommunityCatalog$Item");
            Class<?> kind = loader.loadClass("app.msime.android.CommunityRequest$Kind");
            Class<?> category = loader.loadClass("app.msime.android.CommunityRequest$Category");
            Class<?> action = loader.loadClass("app.msime.android.home.CommunityAdapter$Action");
            JSONObject design = new JSONObject().put("background", 0xedf3ff).put("accent", 0x336699)
                .put("cornerRadius", 12).put("keyShape", "pebble");
            Class<?> skin = loader.loadClass("app.msime.android.CustomKeyboardSkin");
            design = (JSONObject) skin.getMethod("toJson", boolean.class).invoke(
                skin.getMethod("from", JSONObject.class).invoke(null, design), true);
            Object item = itemType.getConstructors()[0].newInstance(ID, kind.getField("SKIN").get(null),
                NAME, "", "合成作者", 0L, 0L, 0.0, design, null, false, 0L, new JSONObject());
            // 丢弃尚在路上的公开目录结果；产品卡片和本地库仍使用真实实现。
            test.runOnMainSync(() -> {
                try {
                    Field generation = field(communityType, "requestGeneration");
                    generation.setLong(community, generation.getLong(community) + 1);
                    field(communityType, "loading").setBoolean(community, false);
                    field(communityType, "hasMore").setBoolean(community, false);
                    java.lang.reflect.Method state = communityType.getDeclaredMethod("state", String.class, boolean.class);
                    state.setAccessible(true);
                    state.invoke(community, "", false);
                    adapter.getClass().getMethod("set", List.class).invoke(adapter, List.of(item));
                } catch (Exception error) { throw new AssertionError(error); }
            });
            test.await(described("获取，" + NAME));
            mark("synthetic community available");
            String failure = (String) loader.loadClass("app.msime.android.CommunityCatalog")
                .getMethod("install", Path.class, itemType, JSONObject.class)
                .invoke(null, directory.toPath(), item, design);
            if (!failure.isEmpty()) throw new AssertionError("Synthetic design install failed");
            test.runOnMainSync(() -> {
                try { adapter.getClass().getMethod("setAction", String.class, action)
                    .invoke(adapter, ID, action.getField("DONE").get(null)); }
                catch (Exception error) { throw new AssertionError(error); }
            });
            test.await(described("使用，" + NAME));
            mark("synthetic community saved");
            test.stage = "newly installed skin appears after selecting My tab";
            test.tap(tab("我的"));
            test.await(described("删除皮肤 " + NAME));
            mark("newly installed skin visible in My tab");
            test.stage = "delete saved skin through native confirmation";
            test.tap(described("删除皮肤 " + NAME));
            test.tap(node -> same("删除", node.getText()) && node.isClickable()
                && !same("删除皮肤 " + NAME, node.getContentDescription()));
            long deadline = SystemClock.uptimeMillis() + 10000;
            while (Files.readString(library.toPath()).contains(ID) && SystemClock.uptimeMillis() < deadline)
                SystemClock.sleep(100);
            if (Files.readString(library.toPath()).contains(ID)) throw new AssertionError("Synthetic skin not deleted");
            test.stage = "deleted skin becomes available after selecting Community tab";
            test.tap(tab("社区"));
            test.await(described("获取，" + NAME));
            mark("deleted skin available again in Community tab");
            if (!java.util.Arrays.equals(originalPreferences, Files.readAllBytes(preferences.toPath())))
                throw new AssertionError("Saving or deleting a design changed active preferences");
        } finally {
            if (activity != null) test.runOnMainSync(activity::finish);
            test.waitForIdleSync();
            test.publish(preferences, originalPreferences);
            if (original == null) Files.deleteIfExists(library.toPath());
            else test.publish(library, original);
        }
    }

    private void mark(String message) {
        Bundle status = new Bundle();
        status.putString("stream", "SKIN_TAB_CHECK: " + message + "\n");
        test.sendStatus(0, status);
        SystemClock.sleep(1200);
    }

    private Object fragment(Object manager, String name) throws Exception {
        for (Object fragment : (List<?>) manager.getClass().getMethod("getFragments").invoke(manager))
            if (fragment.getClass().getName().equals(name)) return fragment;
        throw new AssertionError("Native skin fragment unavailable");
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field value = type.getDeclaredField(name);
        value.setAccessible(true);
        return value;
    }

    private static JSONObject json(File file) throws Exception {
        return new JSONObject(Files.readString(file.toPath()));
    }

    private static Predicate<AccessibilityNodeInfo> tab(String text) {
        return node -> same(text, node.getText()) && same("android.widget.RadioButton", node.getClassName());
    }

    private static Predicate<AccessibilityNodeInfo> described(String value) {
        return node -> same("app.msime.android", node.getPackageName()) && same(value, node.getContentDescription());
    }

    private static boolean same(String expected, CharSequence actual) {
        return actual != null && expected.contentEquals(actual);
    }
}
