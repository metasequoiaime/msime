package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 原生页面保存自定义表后，经独立键盘进程验证真实键位提示和候选；只使用合成状态。 */
final class ShuangpinCustomDeviceChecks {
    private final DeviceSmoke test;
    private Activity activity;

    ShuangpinCustomDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        File preferences = test.sharedPreferencesFile();
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        File imported = new File(test.getTargetContext().getCacheDir(), "synthetic-shuangpin.json");
        try {
            test.stage = "自定义双拼导入边界";
            JSONObject layout = layout("xiaohe", null);
            JSONObject table = layout.getJSONObject("table");
            JSONObject finals = table.getJSONObject("finals");
            for (var units = finals.keys(); units.hasNext();) {
                String unit = units.next();
                String key = finals.getString(unit);
                if (key.equals("j")) finals.put(unit, "k");
                else if (key.equals("k")) finals.put(unit, "j");
            }
            check(layout("custom", table).isNull("problem"), "合成码表未通过共享校验");
            Method read = test.getTargetContext().getClassLoader()
                .loadClass("app.msime.android.home.ShuangpinCustomPage")
                .getDeclaredMethod("readTable", Context.class, Uri.class);
            read.setAccessible(true);
            Files.write(imported.toPath(), table.toString().getBytes(StandardCharsets.UTF_8));
            JSONObject loaded = (JSONObject) read.invoke(null, test.getTargetContext(), Uri.fromFile(imported));
            check(loaded != null && loaded.getJSONObject("finals").getString("ing").equals("j"),
                "文件导入丢失自定义映射");
            Files.write(imported.toPath(), "{\"unknown\":{}}".getBytes(StandardCharsets.UTF_8));
            check(read.invoke(null, test.getTargetContext(), Uri.fromFile(imported)) == null, "未知分组被接受");
            Files.write(imported.toPath(), new byte[16_385]);
            check(read.invoke(null, test.getTargetContext(), Uri.fromFile(imported)) == null, "超限文件被接受");
            Method remove = test.getTargetContext().getClassLoader()
                .loadClass("app.msime.android.home.ShuangpinCustomSelection")
                .getMethod("remove", JSONObject.class);
            for (String profile : new String[] {"custom", "microsoft"}) {
                JSONObject snapshot = new JSONObject().put("preferences", new JSONObject()
                    .put("scheme", "quanpin").put("shuangpin_profile", profile)
                    .put("shuangpin_custom_profile", table).put("learning", false));
                JSONObject deleted = ((JSONObject) remove.invoke(null, snapshot)).getJSONObject("preferences");
                check(!deleted.has("shuangpin_custom_profile"), "删除后仍保存自定义表");
                check(deleted.getString("shuangpin_profile").equals(profile.equals("custom") ? "xiaohe" : profile),
                    "删除改变了其他内置方案");
                check(deleted.getString("scheme").equals("quanpin") && !deleted.getBoolean("learning"),
                    "删除改变了无关设置");
                check(snapshot.getJSONObject("preferences").has("shuangpin_custom_profile"), "删除修改了原始快照");
            }

            test.stage = "从全拼保存并使用自定义双拼";
            test.putSharedPreference("scheme", "quanpin");
            test.putSharedPreference("touch_keyboard_layout", "twenty_six_key");
            test.putSharedPreference("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
            test.putSharedPreference("shuangpin_profile", "xiaohe");
            test.putSharedPreference("shuangpin_custom_profile", table);
            test.putSharedPreference("learning", false);
            test.putSharedPreference("cloud_candidates", false);
            test.putSharedPreference("candidate_translations", false);
            test.putSharedPreference("translation_account", false);
            test.putSharedPreference("translation_ai", false);
            test.putSharedPreference("ai_assistant", new JSONObject().put("enabled", false));
            test.putSharedPreference("touch_shuangpin_key_hints", true);
            test.putSharedPreference("helpcode", new JSONObject().put("enabled", false));
            test.putSharedPreference("default_ime_mode", "chinese");
            activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
                "app.msime.android.home.HomeActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
                .putExtra("msime_open_page", "SHUANGPIN_CUSTOM"));
            AccessibilityNodeInfo ready = test.await(test.key("临时说明（首次运行）").or(test.key("双拼自定义")));
            if (DeviceSmoke.equalsText("临时说明（首次运行）", ready.getText())) test.tap(test.key("知道了"));
            test.tap(test.key("自定义").and(node -> "android.widget.RadioButton".contentEquals(node.getClassName())));
            test.tap(test.key("保存").and(AccessibilityNodeInfo::isClickable));
            long deadline = SystemClock.uptimeMillis() + 15000;
            JSONObject saved;
            do {
                saved = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8))
                    .getJSONObject("preferences");
                if (saved.optString("shuangpin_profile").equals("custom")) break;
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < deadline);
            check(saved.optString("shuangpin_profile").equals("custom"), "页面没有保存自定义方案");
            check(saved.optString("scheme").equals("shuangpin"), "保存并使用没有切换引擎方案");
            JSONObject touch = saved.getJSONObject("touch_keyboard_schemes");
            check(touch.getString("selected").equals("xiaohe"), "旧的触屏选择覆盖了自定义入口");
            check(saved.getJSONObject("shuangpin_custom_profile").getJSONObject("finals")
                .getString("ing").equals("j"), "保存覆盖了自定义码表");

            screenshot("settings");
            test.stage = "自定义双拼键位提示与实际输入";
            test.restartIme();
            test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            test.tap(test.field("msime-test-plain"));
            test.selectChineseKeyboard();
            SystemClock.sleep(1500);
            AccessibilityNodeInfo jKey = test.await(test.key("J").or(test.key("j")));
            screenshot("custom-keyboard");
            check(jKey.getContentDescription() != null && jKey.getContentDescription().toString().contains("ing uai"),
                "自定义 J 键提示不符：" + jKey.getContentDescription() + "；保存值=" +
                    new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8))
                        .getJSONObject("preferences").optString("shuangpin_profile") + "；提示=" +
                    new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8))
                        .getJSONObject("preferences").optBoolean("touch_shuangpin_key_hints"));
            test.tap(test.key("b").or(test.key("B")).and(AccessibilityNodeInfo::isClickable));
            test.tap(test.key("j").or(test.key("J")).and(AccessibilityNodeInfo::isClickable));
            test.tap(node -> node.isClickable() && node.getContentDescription() != null
                && node.getContentDescription().toString().matches("候选 [0-9]+：冰(?:；.*)?"));
            test.await(test.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("冰", node.getText())));

            test.stage = "自定义双拼分号键与实际组字路由";
            test.putSharedPreference("shuangpin_custom_profile", layout("microsoft", null).getJSONObject("table"));
            test.await(node -> node.getContentDescription() != null
                && node.getContentDescription().toString().startsWith("分号，自定义双拼"));
            test.tap(test.key("b").or(test.key("B")).and(AccessibilityNodeInfo::isClickable));
            screenshot("semicolon");
            test.tap(test.key(";").and(AccessibilityNodeInfo::isClickable));
            test.tap(node -> node.isClickable() && node.getContentDescription() != null
                && node.getContentDescription().toString().matches("候选 [0-9]+：冰(?:；.*)?"));
            test.await(test.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("冰冰", node.getText())));

            test.stage = "原生删除自定义方案与取消保护";
            test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page SHUANGPIN_CUSTOM");
            test.tap(test.key("自定义").and(node -> "android.widget.RadioButton".contentEquals(node.getClassName())));
            openDelete();
            test.await(test.key("删除自定义方案？"));
            test.tap(test.key("取消").and(AccessibilityNodeInfo::isClickable));
            check(new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8)).getJSONObject("preferences")
                .has("shuangpin_custom_profile"), "取消删除仍丢失码表");
            openDelete();
            test.await(test.key("删除自定义方案？"));
            screenshot("delete-confirmation");
            test.tap(test.key("删除").and(AccessibilityNodeInfo::isClickable));
            deadline = SystemClock.uptimeMillis() + 15000;
            do {
                saved = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8)).getJSONObject("preferences");
                if (!saved.has("shuangpin_custom_profile")) break;
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < deadline);
            check(!saved.has("shuangpin_custom_profile") && saved.optString("shuangpin_profile", "xiaohe").equals("xiaohe"),
                "原生删除没有移除码表并回退小鹤");
        } finally {
            try {
                if (activity != null) test.runOnMainSync(activity::finish);
            } finally {
                Files.deleteIfExists(imported.toPath());
                if (original == null) Files.deleteIfExists(preferences.toPath());
                else test.publish(preferences, original);
            }
        }
    }

    private void openDelete() throws Exception {
        test.dropStaleNodes();
        AccessibilityNodeInfo button = test.awaitAny(test.key("删除").and(AccessibilityNodeInfo::isClickable));
        if (!button.isVisibleToUser()) {
            check(button.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()),
                "删除入口无法滚动到屏幕内");
            test.dropStaleNodes();
        }
        test.await(test.key("删除").and(AccessibilityNodeInfo::isClickable));
        test.tap(test.key("删除").and(AccessibilityNodeInfo::isClickable));
    }

    private void screenshot(String name) throws Exception {
        android.graphics.Bitmap bitmap = test.automation.takeScreenshot();
        check(bitmap != null, "合成界面截图不可用");
        try (FileOutputStream output = new FileOutputStream(new File(
                test.getTargetContext().getExternalFilesDir(null), "shuangpin-custom-" + name + ".png"))) {
            check(bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output), "截图写入失败");
        } finally { bitmap.recycle(); }
    }

    private JSONObject layout(String profile, JSONObject table) throws Exception {
        JSONObject request = new JSONObject().put("profile", profile);
        if (table != null) request.put("table", table);
        Class<?> nativeClient = test.getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        String response = (String) nativeClient.getMethod("shuangpinCustomLayout", String.class)
            .invoke(null, request.toString());
        JSONObject envelope = new JSONObject(response);
        check(envelope.optBoolean("ok"), "共享码表接口失败");
        return envelope.getJSONObject("value");
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
