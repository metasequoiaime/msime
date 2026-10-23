package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.os.SystemClock;
import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在真实键盘、系统剪贴板和系统分享面板中验证分词操作，只操作合成记录。 */
public final class ClipboardSegmentActionsDeviceSmoke extends DeviceSmoke {
    private static final String SOURCE = "SyntheticAlpha SyntheticBeta SyntheticGamma";
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private boolean phraseNotice;

    @Override public void onCreate(Bundle arguments) {
        phraseNotice = arguments != null && "true".equals(arguments.getString("phrase-notice"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        if (phraseNotice) return "clipboard phrase notice position, stable list, editor focus, dismissal and delayed-result cancellation";
        return "segmented sharing, common phrases, copying, recent preview and insertion";
    }

    @Override protected void runChecks() throws Exception {
        if (phraseNotice) {
            new ClipboardPhraseNoticeDeviceCheck(this).run();
            return;
        }
        Context context = getTargetContext();
        File root = context.getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        File phrases = new File(preferences.getParentFile(), "CommonPhrases.json");
        for (File file : new File[] {preferences, history, local, phrases,
                new File(preferences.getParentFile(), "CommonPhrases.seeded")})
            backups.put(file, file.isFile() ? Files.readAllBytes(file.toPath()) : null);
        ClipboardManager clipboard = context.getSystemService(ClipboardManager.class);
        ClipData previousClip = clipboard.getPrimaryClip();
        try {
            JSONObject document = read(preferences);
            document.put("revision", document.getLong("revision") + 1);
            document.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            publish(preferences, bytes(document.toString()));
            publish(local, bytes(new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)
                .put("platform.android.clipboard_suggestion", true)).toString()));
            publish(history, bytes(new JSONArray().put(new JSONObject().put("text", SOURCE)
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString()));
            clipboard.clearPrimaryClip();
            restartIme();
            openEditor();
            openSegments();
            stage = "empty selection disables actions";
            for (String action : new String[] {"分享选中的词", "添加选中的词到常用语",
                    "复制选中的词并返回键盘", "插入选中的词"})
                await(described(action).and(node -> !node.isEnabled()));
            await(key("全选").and(AccessibilityNodeInfo::isEnabled));
            tap(described("词片 SyntheticBeta"));
            tap(described("词片 SyntheticAlpha"));
            stage = "common phrase stores selection in original order";
            tap(described("添加选中的词到常用语"));
            awaitPhrase("SyntheticAlpha SyntheticBeta");
            stage = "system share receiver gets selected text";
            tap(described("分享选中的词"));
            Predicate<AccessibilityNodeInfo> sink = node -> equalsText("MSIME 合成分享", node.getText());
            tap(sink);
            await(field("msime-test-shared-text").and(node ->
                equalsText("SyntheticAlpha SyntheticBeta", node.getText())));
            openEditor();
            openSegments();
            tap(described("词片 SyntheticGamma"));
            tap(described("词片 SyntheticAlpha"));
            stage = "copy returns to keyboard without insertion";
            tap(described("复制选中的词并返回键盘"));
            await(key("n"));
            // 旧系统把空编辑器的提示文字也放进无障碍 text；提示可见时实际内容仍为空。
            await(field("msime-test-plain").and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            ClipData copied = clipboard.getPrimaryClip();
            if (copied == null || !"SyntheticAlphaSyntheticGamma".contentEquals(copied.getItemAt(0).getText()))
                throw new AssertionError("Copied selection did not preserve segment order");
            stage = "recent clipboard preview pastes the copied selection";
            tap(described("粘贴最近复制的内容：SyntheticAlphaSyntheticGamma"));
            await(field("msime-test-plain").and(node ->
                equalsText("SyntheticAlphaSyntheticGamma", node.getText())));
            openEditor();
            openSegments();
            stage = "select all, clear and insert keep existing behavior";
            tap(key("全选"));
            tap(key("全不选"));
            await(described("插入选中的词").and(node -> !node.isEnabled()));
            tap(key("全选"));
            tap(described("插入选中的词"));
            await(field("msime-test-plain").and(node -> equalsText(SOURCE, node.getText())));
        } finally {
            if (previousClip == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(previousClip);
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else publish(entry.getKey(), entry.getValue());
            }
            restartIme();
        }
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(key("n"));
    }

    private void openSegments() throws Exception {
        tap(described("剪贴板"));
        Predicate<AccessibilityNodeInfo> card = node -> equalsText("app.msime.android", node.getPackageName())
            && node.isLongClickable() && node.getText() != null && node.getText().toString().startsWith(SOURCE);
        if (!await(card).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Synthetic clipboard long press refused");
        tap(node -> equalsText("分词", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
        await(described("词片 SyntheticAlpha"));
    }

    private void awaitPhrase(String expected) throws Exception {
        Class<?> store = getTargetContext().getClassLoader().loadClass("app.msime.android.CommonPhrasesStore");
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            Object result = store.getMethod("load", Context.class).invoke(null, getTargetContext());
            Object document = result.getClass().getMethod("document").invoke(result);
            if (document != null) {
                for (Object phrase : (Iterable<?>) document.getClass().getMethod("phrases").invoke(document))
                    if (expected.equals(phrase.getClass().getMethod("text").invoke(phrase))) return;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Selected phrase was not stored");
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static byte[] bytes(String text) { return text.getBytes(StandardCharsets.UTF_8); }
}
