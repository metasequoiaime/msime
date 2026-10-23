package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.os.SystemClock;
import android.view.View;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** 专用模拟器的合成电话本：真实导入、刷新、计数、候选选择与清空，不读取用户设备。 */
public final class ContactsDeviceSmoke extends DeviceSmoke {
    private static final String ACCOUNT = "app.msime.synthetic.9000";
    private static final String NAME = "拟好";
    private static final String PHONE = "202-555-0101";
    private static final String UPDATED = "202-555-0102";

    private String productPackage() { return getTargetContext().getPackageName(); }

    @Override protected String successDescription() {
        return "contacts import/count, refresh, name/combined editor commits, corner badge and clear";
    }

    @Override protected void runChecks() throws Exception {
        getTargetContext().getClassLoader().loadClass("app.msime.android.Bootstrap")
            .getMethod("prepare", android.content.Context.class).invoke(null, getTargetContext());
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File state = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!state.toPath().startsWith(root.toPath())) throw new AssertionError("合成状态路径越出沙盒");
        File preferences = new File(state, "preferences.json");
        File mentions = new File(state, "plugins/mentions.json");
        byte[] previousPreferences = bytes(preferences), previousMentions = bytes(mentions);
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> previousModes = modes.getAll();
        android.content.SharedPreferences onboarding = getTargetContext().getSharedPreferences("msime_onboarding_v1", 0);
        boolean onboardingPresent = onboarding.contains("seen"), onboardingSeen = onboarding.getBoolean("seen", false);
        try {
            if (!onboarding.edit().putBoolean("seen", true).commit()) throw new AssertionError("合成引导状态准备失败");
            stage = "prepare synthetic preferences";
            JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
                .put("candidate_page_size", 6).put("single_character_only", false)
                .put("cloud_candidates", false).put("learning", true)
                .put("character_width", "halfwidth").put("traditional_chinese_output", false)
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
            JSONObject localModes = configured.optJSONObject("local_modes");
            if (localModes == null) localModes = new JSONObject();
            configured.put("local_modes", localModes.put("mention", false));
            options.put("preferences", configured);
            long revision = previousPreferences == null ? 1
                : new JSONObject(new String(previousPreferences, StandardCharsets.UTF_8)).getLong("revision") + 1;
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision)
                .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
            if (!modes.edit().clear().commit()) throw new AssertionError("合成输入模式准备失败");
            shellDiscard("pm grant " + productPackage() + " android.permission.READ_CONTACTS");
            removeSyntheticContacts();
            long refreshedId = seedContact(900001, NAME, PHONE);
            seedContact(900002, "合成乙", "202-555-0103");

            stage = "import and saved count";
            openContacts();
            tap(imeText("导入"));
            Class<?> page = getTargetContext().getClassLoader().loadClass("app.msime.android.home.ContactsPage");
            Method load = page.getDeclaredMethod("load", android.content.Context.class);
            load.setAccessible(true);
            long deadline = SystemClock.uptimeMillis() + 15000;
            List<?> imported;
            do {
                imported = (List<?>) load.invoke(null, getTargetContext());
                if (imported != null && imported.size() >= 2) break;
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < deadline);
            if (imported == null || imported.size() < 2) throw new AssertionError("导入没有保存合成联系人");
            final int count = imported.size();
            await(imeTextContains("已保存 " + count + " 个联系人"));

            checkpoint("native imported contact candidates");
            checkNativeCandidates(options);

            stage = "first name candidate commits complete text";
            openEditor();
            typeName();
            checkpoint("first name source and priority");
            await(describedPrefix("候选 1：来自联系人：" + NAME));
            checkpoint("second combined source and priority");
            await(describedPrefix("候选 2：来自联系人：" + NAME + " " + PHONE));
            checkpoint("click first name");
            tap(describedPrefix("候选 1：来自联系人：" + NAME));
            checkpoint("first name editor commit");
            await(field("msime-test-plain").and(node -> equalsText(NAME, node.getText())));

            stage = "combined candidate commits complete text";
            openEditor();
            typeName();
            tap(describedPrefix("候选 2：来自联系人：" + NAME + " " + PHONE));
            await(field("msime-test-plain").and(node -> equalsText(NAME + " " + PHONE, node.getText())));

            stage = "refresh keeps count and updates phone";
            long phoneId;
            try (android.database.Cursor cursor = getTargetContext().getContentResolver().query(
                    android.provider.ContactsContract.Data.CONTENT_URI, new String[] {"_id"},
                    "raw_contact_id=? AND mimetype=?", new String[] {Long.toString(refreshedId),
                    "vnd.android.cursor.item/phone_v2"}, null)) {
                if (cursor == null || !cursor.moveToFirst()) throw new AssertionError("合成电话记录缺失");
                phoneId = cursor.getLong(0);
            }
            shellDiscard("content update --uri content://com.android.contacts/data --where _id=" + phoneId
                + " --bind data1:s:" + UPDATED);
            openContacts();
            tap(imeText("刷新"));
            await(imeTextContains("已保存 " + count + " 个联系人"));
            SystemClock.sleep(500);
            openEditor();
            typeName();
            await(describedPrefix("候选 2：来自联系人：" + NAME + " " + UPDATED));
            stage = "expanded combined selection";
            tap(described("展开候选"));
            tap(describedPrefix("候选 2：来自联系人：" + NAME + " " + UPDATED));
            await(field("msime-test-plain").and(node -> equalsText(NAME + " " + UPDATED, node.getText())));

            stage = "corner badge raster and reuse";
            checkBadge();
            stage = "clear count and suggestions";
            openContacts();
            tap(imeText("清空"));
            await(imeTextContains("已保存 0 个联系人"));
            openEditor();
            typeName();
            await(describedPrefix("候选 1："));
            if (findAnyVisibleImeNode(describedPrefix("候选 1：来自联系人：")) != null)
                throw new AssertionError("清空后仍显示联系人来源");
        } finally {
            removeSyntheticContacts();
            restore(preferences, previousPreferences);
            restore(mentions, previousMentions);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : previousModes.entrySet()) {
                Object value = entry.getValue();
                if (value instanceof String text) editor.putString(entry.getKey(), text);
                else if (value instanceof Boolean flag) editor.putBoolean(entry.getKey(), flag);
                else if (value instanceof Integer number) editor.putInt(entry.getKey(), number);
                else if (value instanceof Long number) editor.putLong(entry.getKey(), number);
            }
            if (!editor.commit()) throw new AssertionError("输入模式恢复失败");
            android.content.SharedPreferences.Editor onboardingEditor = onboarding.edit();
            if (onboardingPresent) onboardingEditor.putBoolean("seen", onboardingSeen); else onboardingEditor.remove("seen");
            if (!onboardingEditor.commit()) throw new AssertionError("引导状态恢复失败");
        }
    }

    private long seedContact(int fixtureId, String name, String phone) throws Exception {
        String account = "synthetic-" + fixtureId;
        shellDiscard("content insert --uri content://com.android.contacts/raw_contacts"
            + " --bind account_type:s:" + ACCOUNT + " --bind account_name:s:" + account);
        long id;
        try (android.database.Cursor cursor = getTargetContext().getContentResolver().query(
                android.provider.ContactsContract.RawContacts.CONTENT_URI, new String[] {"_id"},
                "account_type=? AND account_name=? AND deleted=0", new String[] {ACCOUNT, account}, "_id DESC")) {
            if (cursor == null || !cursor.moveToFirst()) throw new AssertionError("合成电话本创建失败");
            id = cursor.getLong(0);
        }
        shellDiscard("content insert --uri content://com.android.contacts/data --bind raw_contact_id:i:" + id
            + " --bind mimetype:s:vnd.android.cursor.item/name --bind data1:s:" + name);
        shellDiscard("content insert --uri content://com.android.contacts/data --bind raw_contact_id:i:" + id
            + " --bind mimetype:s:vnd.android.cursor.item/phone_v2 --bind data1:s:" + phone);
        return id;
    }

    private void removeSyntheticContacts() throws Exception {
        // UiAutomation 直接拆分参数，不经过 shell；SQL 字符串保留自己的引号，不再套 shell 引号。
        shellDiscard("content delete --uri content://com.android.contacts/raw_contacts --where "
            + "account_type='" + ACCOUNT + "'");
    }

    private void openContacts() throws Exception {
        shellDiscard("am start -n " + productPackage()
            + "/app.msime.android.home.HomeActivity --es msime_open_page CONTACTS");
        await(imeText("导入"));
    }

    private void openEditor() throws Exception {
        restartIme(productPackage().equals("app.msime.android") ? "app.msime.android/.MSIMEInputService"
            : productPackage() + "/app.msime.android.MSIMEInputService");
        shellDiscard("am start -f 0x10008000 -n app.msime.android.test/.EditorActivity");
        tap(field("msime-test-plain"));
        // 键面会先于后台输入会话出现，先等语言键报告会话可用，再输入合成拼音。
        await(described("切换到英文输入").or(described("切换到所选输入方案")));
        if (findAnyVisibleImeNode(described("切换到所选输入方案")) != null) {
            tap(described("切换到所选输入方案"));
            await(described("切换到英文输入"));
        }
        await(key("n"));
    }

    private void checkpoint(String name) {
        stage = name;
        android.os.Bundle status = new android.os.Bundle();
        status.putString("stream", "CONTACTS_STAGE: " + name + "\n");
        sendStatus(0, status);
    }

    private void typeName() throws Exception {
        for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
    }

    private void checkNativeCandidates(JSONObject options) throws Exception {
        File stored = new File(options.getString("preferences_directory"), "plugins/mentions.json");
        JSONArray imported = new JSONObject(new String(Files.readAllBytes(stored.toPath()), StandardCharsets.UTF_8))
            .getJSONArray("entries");
        boolean found = false;
        for (int index = 0; index < imported.length(); index++) {
            JSONObject entry = imported.getJSONObject(index);
            if (NAME.equals(entry.optString("text")) && PHONE.equals(entry.optString("phone"))) found = true;
        }
        if (!found) throw new AssertionError("导入快照缺少合成姓名或号码");
        Class<?> nativeClient = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        JSONObject created = new JSONObject((String) nativeClient.getMethod("create", String.class)
            .invoke(null, options.toString()));
        if (!created.optBoolean("ok")) throw new AssertionError("合成原生会话创建失败");
        long session = created.getJSONObject("value").getLong("session");
        try {
            nativeClient.getMethod("focus", long.class, boolean.class).invoke(null, session, true);
            JSONObject typed = null;
            for (char letter : "nihao".toCharArray()) {
                typed = new JSONObject((String) nativeClient.getMethod("character", long.class, int.class, boolean.class)
                    .invoke(null, session, (int) letter, false));
            }
            JSONArray candidates = typed.getJSONObject("value").getJSONObject("view").getJSONArray("candidates");
            if (candidates.length() < 2 || !NAME.equals(candidates.getJSONObject(0).optString("text"))
                    || !(NAME + " " + PHONE).equals(candidates.getJSONObject(1).optString("text"))
                    || candidates.getJSONObject(0).optInt("source") != 16
                    || candidates.getJSONObject(1).optInt("source") != 16)
                throw new AssertionError("合成原生会话未按顺序生成联系人来源的姓名和组合候选");
        } finally {
            nativeClient.getMethod("destroy", long.class).invoke(null, session);
        }
    }

    private void checkBadge() throws Exception {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            try {
                Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.ContactCandidateButton");
                Button button = (Button) type.getConstructor(android.content.Context.class).newInstance(getTargetContext());
                button.setText(NAME);
                button.setBackgroundColor(android.graphics.Color.WHITE);
                button.setTextColor(android.graphics.Color.BLACK);
                Method setSource = type.getMethod("setContactSource", int.class);
                int width = 240, height = 100;
                button.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
                    View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
                button.layout(0, 0, width, height);
                int padding = button.getCompoundPaddingRight();
                setSource.invoke(button, 16);
                if (button.getCompoundPaddingRight() <= padding || !NAME.contentEquals(button.getText()))
                    throw new AssertionError("角标未预留空间或修改了文本");
                Bitmap image = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
                button.draw(new Canvas(image));
                float density = getTargetContext().getResources().getDisplayMetrics().density;
                int x = width - Math.round(7.5f * density), y = Math.round(5f * density);
                if (image.getPixel(x, y) == android.graphics.Color.WHITE)
                    throw new AssertionError("小人没有画在右上角");
                setSource.invoke(button, 0);
                if (button.getCompoundPaddingRight() != padding) throw new AssertionError("按钮复用后角标空间未清除");
                image.recycle();
            } catch (Exception | AssertionError error) { failure[0] = error; }
        });
        if (failure[0] != null) throw new AssertionError("联系人角标回归", failure[0]);
    }

    private java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> imeText(String text) {
        return node -> equalsText(productPackage(), node.getPackageName()) && equalsText(text, node.getText());
    }

    @Override protected java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> imeTextContains(String text) {
        return node -> equalsText(productPackage(), node.getPackageName()) && node.getText() != null
            && node.getText().toString().contains(text);
    }

    @Override protected java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> described(String text) {
        return node -> equalsText(productPackage(), node.getPackageName()) && equalsText(text, node.getContentDescription());
    }

    @Override protected java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> describedPrefix(String text) {
        return node -> equalsText(productPackage(), node.getPackageName()) && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith(text);
    }

    @Override protected java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> key(String text) {
        return node -> equalsText(productPackage(), node.getPackageName()) && node.getText() != null
            && text.equalsIgnoreCase(node.getText().toString()) && equalsText("android.widget.Button", node.getClassName());
    }

    @Override protected void tap(java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> match)
            throws java.util.concurrent.TimeoutException {
        var target = await(match);
        // 拼音版使用独立包名，也通过产品控件的真实点击路径，避免 API 35 拒绝坐标注入。
        if (equalsText(productPackage(), target.getPackageName()) && target.isClickable()) {
            automation.waitForIdle(500, 5000);
            target = await(match);
            if (!target.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("合成联系人控件点击失败");
            SystemClock.sleep(150);
        } else super.tap(match);
    }
    private android.view.accessibility.AccessibilityNodeInfo findAnyVisibleImeNode(
            java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> match) {
        dropStaleNodes();
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
            var node = find(window.getRoot(), match);
            if (node != null) return node;
        }
        return null;
    }

    private static byte[] bytes(File file) throws Exception {
        return file.isFile() ? Files.readAllBytes(file.toPath()) : null;
    }
    private void restore(File file, byte[] contents) throws Exception {
        if (contents == null) Files.deleteIfExists(file.toPath()); else publish(file, contents);
    }
}
