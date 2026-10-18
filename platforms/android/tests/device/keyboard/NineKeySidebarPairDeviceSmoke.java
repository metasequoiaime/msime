package app.msime.android.test;

import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 从真实九键侧栏输入完整括号，检查普通编辑器与忽略光标参数的编辑器（#8625）。 */
public final class NineKeySidebarPairDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key sidebar pairs keep the caret inside on both layers and with automatic completion disabled";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()))
            throw new AssertionError("Preferences escaped the test sandbox");
        File preferences = new File(directory, "preferences.json");
        File local = new File(new File(new File(root, "bootstrap"), "state"), "android-settings.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "nine_key")
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key")).put("selected", "nine_key"));
        try {
            JSONObject settings = new JSONObject()
                .put("platform.android.nine_key_symbols", "() （） :-) (")
                .put("platform.android.nine_key_digit_symbols", "() （） :-) (");
            publish(local, new JSONObject().put("version", 1).put("settings", settings).toString()
                .getBytes(StandardCharsets.UTF_8));
            for (boolean automatic : new boolean[] {true, false}) {
                stage = "prepare nine-key sidebar, automatic " + automatic;
                base.put("paired_punctuation", automatic).put("character_width", "halfwidth");
                publish(preferences, new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", base).toString()
                    .getBytes(StandardCharsets.UTF_8));
                restartIme();
                openEditor();
                for (String editor : new String[] {"msime-test-plain", "msime-test-cursor-at-end"}) {
                    tap(field(editor));
                    await(sidebar("()"));
                    for (boolean digits : new boolean[] {false, true}) {
                        if (digits) tap(key("123"));
                        for (String pair : new String[] {"()", "（）"}) {
                            stage = editor + " pair " + pair + ", digits " + digits + ", automatic " + automatic;
                            clearEditor(editor);
                            tap(sidebar(pair));
                            assertEditorState(editor, pair, 1);
                            // 继续输入应落在括号内，不能只检查上屏后的瞬时光标。
                            if (!digits) tap(key("123"));
                            tap(key("5"));
                            assertEditorState(editor, pair.substring(0, 1) + "5" + pair.substring(1), 2);
                            if (!digits) tap(key("九键"));
                        }
                        stage = editor + " nested pair, digits " + digits;
                        clearEditor(editor);
                        tap(sidebar("（）"));
                        tap(sidebar("()"));
                        assertEditorState(editor, "（()）", 2);
                        stage = editor + " nested identical pair, digits " + digits;
                        clearEditor(editor);
                        tap(sidebar("()"));
                        tap(sidebar("()"));
                        assertEditorState(editor, "(())", 2);
                        for (String literal : new String[] {":-)", "("}) {
                            stage = editor + " unchanged literal " + literal + ", digits " + digits;
                            clearEditor(editor);
                            tap(sidebar(literal));
                            assertEditorState(editor, literal, literal.length());
                        }
                        if (digits) tap(key("九键"));
                    }
                }
            }
            stage = "fullwidth sidebar pair";
            base.put("character_width", "fullwidth");
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", ++revision).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            tap(sidebar("()"));
            assertEditorState("msime-test-plain", "（）", 1);
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath());
            else publish(local, originalLocal);
            restartIme();
        }
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private Predicate<AccessibilityNodeInfo> sidebar(String text) {
        return described("按键 符号 " + text);
    }

    private Predicate<AccessibilityNodeInfo> editorState(String editor, String text, int caret) {
        return field(editor).and(node -> equalsText(text, node.getText())
            && node.getTextSelectionStart() == caret && node.getTextSelectionEnd() == caret);
    }

    private void assertEditorState(String editor, String text, int caret) {
        try {
            await(editorState(editor, text, caret));
        } catch (AssertionError error) {
            AccessibilityNodeInfo actual = await(field(editor));
            throw new AssertionError("Synthetic text matches: " + equalsText(text, actual.getText())
                + "; expected caret " + caret + ", actual " + actual.getTextSelectionStart()
                + ":" + actual.getTextSelectionEnd());
        }
    }

    /** 清掉本用例的合成文字，保持当前输入框与键盘层不变。 */
    private void clearEditor(String editor) {
        Bundle args = new Bundle();
        args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
        if (!await(field(editor)).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args))
            throw new AssertionError("Cannot clear the synthetic editor");
        await(field(editor).and(node -> node.isShowingHintText()
            || node.getText() == null || node.getText().length() == 0));
    }
}
