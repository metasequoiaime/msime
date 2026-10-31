package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** 工具栏语音按钮保留排序接线，设置写顶层共享偏好；组合入口的状态与保存由设备专项验证。 */
public final class ToolbarVoiceButtonSmoke {
    public static void main(String[] args) throws Exception {
        String key = ToolbarOrderPolicy.Item.VOICE.preferenceKey;
        check("touch_voice_shortcut".equals(key), "the voice tool is switched by touch_voice_shortcut");

        String toolbar = read("core/ImeToolbar.java");
        String install = body(toolbar, "void installShortcutBar(Button dismissButton) {");
        // 只看按钮在不在两张表里，表里其他按钮怎么排、换不换行都不影响。
        check(body(install, "shortcutButtons = new Button[] {").contains("s.voiceShortcutButton"),
            "the voice button is one of the middle tools");
        String retired = body(install, "for (Button retired : new Button[] {");
        check(retired.contains("s.replyShortcutButton") && !retired.contains("voiceShortcutButton"),
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
        String saveToolbar = body(options, "private void saveToolbar(String member, boolean value) {");
        check(saveToolbar.contains("preferences -> preferences.put(member, value)"),
            "the toolbar settings write the voice switch at the top level, not into touch_toolbar");
        check(!saveToolbar.contains("AndroidLocalSettings.SPACE_VOICE"),
            "the toolbar switch does not change long-press space voice");

        String voicePage = read("home/VoicePage.java");
        check(voicePage.contains("工具栏/长按空格"), "the voice page combines both entry labels");
        check(voicePage.contains("local.bool(AndroidLocalSettings.TOOLBAR_HIDDEN)"),
            "the voice page explains an enabled button on a hidden toolbar");
        Matcher hiddenHint = Pattern.compile("static final String TRIGGER_TOOLBAR_HIDDEN_HINT = \"([^\"]*)\";").matcher(voicePage);
        check(hiddenHint.find(), "missing TRIGGER_TOOLBAR_HIDDEN_HINT");
        String voiceTile = FunctionPanelModel.item(FunctionPanelModel.Id.VOICE_INPUT).label();
        check(hiddenHint.group(1).contains("工具栏已隐藏") && hiddenHint.group(1).contains("「" + voiceTile + "」"),
            "the hidden-toolbar hint names the voice tile that stays in 更多工具");
        check(!read("core/ImeFunctionPanel.java").contains("hiddenItems.add(FunctionPanelModel.Id.VOICE_INPUT)"),
            "more tools keeps the voice entry independently of the toolbar");
        System.out.println("ToolbarVoiceButtonSmoke passed");
    }

    /** `signature` 起到它之后第一对 `{` `}`（方法体、数组字面量或 if 块）结束为止的一段源码（已压空白），只认 `{` `}` 配对，不处理字符串里的括号。 */
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
