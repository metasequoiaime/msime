package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * 「语音输入 → 启动方式」选「工具栏按钮」后，键盘工具栏上要真的有语音按钮（#10142）。
 *
 * <p>这个按钮在 #3782 改版时被拆下了工具栏，却一直被设成可见：设置页能选「工具栏按钮」，键盘上没有，「按钮顺序」里也没有，「更多工具」面板还因为它「可见」把「语音输入」磁贴藏掉，用户一个语音入口都找不到。工具栏是 View，JVM 冒烟画不出来，这里按源码（先压空白）钉住几处接线：按钮回到中间区、参与排序、整行显隐算上它、设置页的「语音」开关和语音页写的是同一个共享偏好 `touch_voice_shortcut`。面板磁贴的规则由 {@link SpaceKeyMicSmoke} 钉住。
 */
public final class ToolbarVoiceButtonSmoke {
    public static void main(String[] args) throws Exception {
        String key = ToolbarOrderPolicy.Item.VOICE.preferenceKey;
        check("touch_voice_shortcut".equals(key), "the voice tool is switched by touch_voice_shortcut");

        String toolbar = read("core/ImeToolbar.java");
        String install = body(toolbar, "void installShortcutBar(Button dismissButton) {");
        check(install.contains("s.undoShortcutButton, s.redoShortcutButton, s.voiceShortcutButton};"),
            "the voice button is one of the middle tools");
        check(install.contains("for (Button retired : new Button[] {s.scriptShortcutButton, s.aiPolishShortcutButton, s.replyShortcutButton, s.layoutSettingsButton}) {"),
            "the voice button is no longer retired from the toolbar");
        check(body(toolbar, "private void applyShortcutOrder() {").contains("case VOICE -> s.voiceShortcutButton;"),
            "the voice button follows the saved order");

        String service = read("core/MSIMEInputService.java");
        check(body(service, "boolean toolbarRowHidden() {").contains("|| touchVoiceShortcutEnabled);"),
            "a toolbar with only the voice button still shows its row");
        check(service.contains("ViewPolicy.setVisible(voiceShortcutButton, touchVoiceShortcutEnabled);"),
            "the voice button shows exactly when its switch is on");

        String options = read("home/KeyboardOptionsPage.java");
        check(body(options, "private static boolean toolbarButton(")
                .contains("if (member.equals(ToolbarOrderPolicy.VOICE_SHORTCUT_PREFERENCE)) return preferences.optBoolean(member, false);"),
            "the toolbar settings read the voice switch from the top-level preference, off by default");
        check(body(options, "private void saveToolbar(String member, boolean value) {")
                .contains("if (member.equals(ToolbarOrderPolicy.VOICE_SHORTCUT_PREFERENCE)) { KeyboardSheets.save(this, preferences -> preferences.put(member, value),"),
            "the toolbar settings write the voice switch at the top level, not into touch_toolbar");

        String voicePage = read("home/VoicePage.java");
        check(voicePage.contains("preferences.put(\"" + key + "\", trigger == TRIGGER_TOOLBAR)"),
            "the voice page's 工具栏按钮 writes the same switch the toolbar uses");
        System.out.println("ToolbarVoiceButtonSmoke passed");
    }

    /** `signature` 起到它的第一个方法体结束为止的一段源码（已压空白），只认 `{` `}` 配对，不处理字符串里的括号。 */
    private static String body(String source, String signature) {
        int start = source.indexOf(signature);
        check(start >= 0, "missing " + signature);
        int depth = 0;
        for (int index = start; index < source.length(); index++) {
            char c = source.charAt(index);
            if (c == '{') depth++;
            else if (c == '}' && --depth == 0) return source.substring(start, index + 1);
        }
        throw new AssertionError("unterminated " + signature);
    }

    /** 读源码并把连续空白压成一个空格。 */
    private static String read(String relative) throws Exception {
        return Files.readString(repoRoot().resolve("platforms/android/java/app/msime/android/" + relative),
            StandardCharsets.UTF_8).replaceAll("\\s+", " ");
    }

    private static Path repoRoot() {
        Path path = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (path != null) {
            if (Files.isRegularFile(path.resolve("platforms/android/java/app/msime/android/core/ImeToolbar.java"))) return path;
            path = path.getParent();
        }
        throw new IllegalStateException("repository root not found");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
