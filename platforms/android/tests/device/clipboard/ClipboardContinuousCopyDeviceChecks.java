package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/** 在专用模拟器的另一个应用里连续复制，验收真实输入法进程的系统监听与持久化。 */
final class ClipboardContinuousCopyDeviceChecks {
    private static final String PREFIX = "Synthetic continuous clipboard ";
    private static final List<String> TEXTS = List.of(PREFIX + "01", PREFIX + "02 中文", PREFIX + "03\nsecond line");
    private final DeviceSmoke owner;
    private final Context context;
    private final Class<?> storeType;
    private final Method load;
    private final File preferences;
    private final File history;

    private ClipboardContinuousCopyDeviceChecks(DeviceSmoke owner) throws Exception {
        this.owner = owner;
        context = owner.getTargetContext();
        ClassLoader loader = context.getClassLoader();
        loader.loadClass("app.msime.android.Bootstrap").getMethod("prepare", Context.class).invoke(null, context);
        JSONObject options = read(new File(context.getFilesDir(), "runtime-options.json"));
        preferences = new File(options.getString("preferences_directory"), "preferences.json");
        history = new File(context.getFilesDir(), "MSIME/clipboard_history.json");
        storeType = loader.loadClass("app.msime.android.ClipboardHistoryStore");
        load = storeType.getMethod("load");
    }

    static void run(DeviceSmoke owner) throws Exception {
        new ClipboardContinuousCopyDeviceChecks(owner).run();
    }

    private void run() throws Exception {
        byte[] savedPreferences = Files.readAllBytes(preferences.toPath());
        byte[] savedHistory = history.isFile() ? Files.readAllBytes(history.toPath()) : null;
        android.content.SharedPreferences mode = context.getSharedPreferences("android-clipboard-mode", 0);
        boolean hadMode = mode.contains("visitor"), savedMode = mode.getBoolean("visitor", false);
        ClipboardManager clipboard = context.getSystemService(ClipboardManager.class);
        ClipData savedClip = null;
        boolean readClip = false;
        try {
            JSONObject document = new JSONObject(new String(savedPreferences, StandardCharsets.UTF_8));
            document.put("revision", document.getLong("revision") + 1);
            document.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key");
            owner.publish(preferences, document.toString().getBytes(StandardCharsets.UTF_8));
            if (!mode.edit().putBoolean("visitor", false).commit()) throw new AssertionError("Test mode setup failed");
            owner.restartIme();
            openEditor();
            savedClip = clipboard.getPrimaryClip();
            readClip = true;
            owner.stage = "copy three distinct texts with clipboard panel closed";
            for (String text : TEXTS) copyInEditor(text);
            expect(List.of(TEXTS.get(2), TEXTS.get(1), TEXTS.get(0)));

            owner.stage = "copy an earlier text again without dropping the other two";
            copyInEditor(TEXTS.get(0));
            expect(List.of(TEXTS.get(0), TEXTS.get(2), TEXTS.get(1)));

            owner.stage = "rebind input method and keep all three persisted copies";
            owner.restartIme();
            openEditor();
            expect(List.of(TEXTS.get(0), TEXTS.get(2), TEXTS.get(1)));
        } finally {
            // 每一步恢复都尝试执行，单项失败不能阻止偏好与历史恢复。
            final ClipData originalClip = savedClip;
            final boolean restoreClip = readClip;
            cleanup(
                () -> {
                    if (!restoreClip) return;
                    if (originalClip == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(originalClip);
                },
                () -> owner.shellDiscard("ime disable app.msime.android/.MSIMEInputService"),
                () -> {
                    android.content.SharedPreferences.Editor restore = mode.edit();
                    if (hadMode) restore.putBoolean("visitor", savedMode); else restore.remove("visitor");
                    if (!restore.commit()) throw new AssertionError("Test mode restoration failed");
                },
                () -> owner.publish(preferences, savedPreferences),
                () -> {
                    if (savedHistory == null) Files.deleteIfExists(history.toPath()); else owner.publish(history, savedHistory);
                },
                () -> owner.restartIme());
        }
    }

    private void openEditor() throws Exception {
        owner.shellDiscard("am start -W -f 0x10008000 --ez msime_test_multiline true -n app.msime.android.test/app.msime.android.test.EditorActivity");
        owner.tap(owner.field("msime-test-plain"));
        owner.await(owner.key("n"));
    }

    private void copyInEditor(String text) throws Exception {
        AccessibilityNodeInfo editor = owner.await(owner.field("msime-test-plain"));
        Bundle value = new Bundle();
        value.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, value))
            throw new AssertionError("Synthetic editor text setup failed");
        editor = owner.await(owner.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(text, node.getText())));
        Bundle selection = new Bundle();
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 0);
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, text.length());
        if (!editor.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection))
            throw new AssertionError("Synthetic editor selection failed");
        editor = owner.await(owner.field("msime-test-plain").and(node -> node.getTextSelectionStart() == 0
            && node.getTextSelectionEnd() == text.length()));
        if (!editor.performAction(AccessibilityNodeInfo.ACTION_COPY))
            throw new AssertionError("Synthetic system copy failed");
        // 让真实系统分发复制事件；三次复制之间不读取历史或打开剪贴板面板。
        SystemClock.sleep(150);
    }

    private void expect(List<String> expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            Object store = storeType.getMethod("forContext", Context.class).invoke(null, context);
            List<?> entries = (List<?>) load.invoke(store);
            List<String> actual = new ArrayList<>();
            for (Object entry : entries) {
                String text = (String) entry.getClass().getMethod("text").invoke(entry);
                if (text.startsWith(PREFIX)) actual.add(text);
            }
            if (actual.equals(expected)) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Continuous copy history lost or reordered synthetic entries");
    }

    @FunctionalInterface private interface Cleanup { void run() throws Exception; }

    private static void cleanup(Cleanup... steps) throws Exception {
        Throwable first = null;
        for (Cleanup step : steps) {
            try { step.run(); }
            catch (Exception | AssertionError error) {
                if (first == null) first = error; else first.addSuppressed(error);
            }
        }
        if (first instanceof Exception error) throw error;
        if (first instanceof AssertionError error) throw error;
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
}
